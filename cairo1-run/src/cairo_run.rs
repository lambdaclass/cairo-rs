use crate::error::Error;
use cairo_lang_casm::{
    builder::{CasmBuilder, Var},
    casm, casm_build_extend,
    cell_expression::CellExpression,
    deref, deref_or_immediate,
    hints::{Hint, StarknetHint},
    inline::CasmContext,
    instructions::{Instruction, InstructionBody},
};
use cairo_lang_sierra::{
    extensions::{
        bitwise::BitwiseType,
        core::{CoreLibfunc, CoreType},
        ec::EcOpType,
        gas::GasBuiltinType,
        pedersen::PedersenType,
        poseidon::PoseidonType,
        range_check::RangeCheckType,
        segment_arena::SegmentArenaType,
        starknet::syscalls::SystemType,
        ConcreteType, NamedType,
    },
    ids::{ConcreteTypeId, GenericTypeId},
    program::{Function, GenericArg, Program as SierraProgram},
    program_registry::ProgramRegistry,
};
use cairo_lang_sierra_gas::objects::CostInfoProvider;
use cairo_lang_sierra_to_casm::{
    compiler::{CairoProgram, SierraToCasmConfig},
    metadata::calc_metadata_ap_change_only,
};
use cairo_lang_sierra_type_size::get_type_size_map;
use cairo_lang_utils::{
    bigint::BigIntAsHex, casts::IntoOrPanic, unordered_hash_map::UnorderedHashMap,
};
use cairo_vm::{
    hint_processor::cairo_1_hint_processor::hint_processor::Cairo1HintProcessor,
    math_utils::signed_felt,
    serde::deserialize_program::{ApTracking, FlowTrackingData, HintParams, ReferenceManager},
    types::{
        builtin_name::BuiltinName, layout_name::LayoutName, program::Program,
        relocatable::MaybeRelocatable,
    },
    vm::{
        errors::{runner_errors::RunnerError, vm_errors::VirtualMachineError},
        runners::cairo_runner::{CairoRunner, RunResources, RunnerMode},
        vm_core::VirtualMachine,
    },
    Felt252,
};
use itertools::{chain, Itertools};
use num_bigint::{BigInt, Sign};
use num_traits::{cast::ToPrimitive, Zero};
use std::{collections::HashMap, iter::Peekable};

/// Representation of a cairo argument
/// Can consist of a single Felt or an array of Felts
#[derive(Debug, Clone)]
pub enum FuncArg {
    Array(Vec<Felt252>),
    Single(Felt252),
}

impl From<Felt252> for FuncArg {
    fn from(value: Felt252) -> Self {
        Self::Single(value)
    }
}

impl From<Vec<Felt252>> for FuncArg {
    fn from(value: Vec<Felt252>) -> Self {
        Self::Array(value)
    }
}

/// Configuration parameters for a cairo run
#[derive(Debug)]
pub struct Cairo1RunConfig<'a> {
    /// Input arguments for the `main` function in the cairo progran
    pub args: &'a [FuncArg],
    /// Serialize program output into a user-friendly format
    pub serialize_output: bool,
    /// Compute cairo trace during execution
    pub trace_enabled: bool,
    /// Relocate cairo memory at the end of the run
    pub relocate_mem: bool,
    /// Cairo layout chosen for the run
    pub layout: LayoutName,
    /// Run in proof_mode
    pub proof_mode: bool,
    /// Should be true if either air_public_input or cairo_pie_output are needed
    /// Sets builtins stop_ptr by calling `final_stack` on each builtin
    pub finalize_builtins: bool,
    /// Appends the return and input values to the output segment. This is performed by default when running in proof_mode
    pub append_return_values: bool,
}

impl Default for Cairo1RunConfig<'_> {
    fn default() -> Self {
        Self {
            args: Default::default(),
            serialize_output: false,
            trace_enabled: false,
            relocate_mem: false,
            layout: LayoutName::plain,
            proof_mode: false,
            finalize_builtins: false,
            append_return_values: false,
        }
    }
}

impl Cairo1RunConfig<'_> {
    // Returns true if the flags in the config enable adding the output builtin and
    // copying input and output values into it's segment
    fn copy_to_output(&self) -> bool {
        self.append_return_values || self.proof_mode
    }
}

/// Runs a Cairo 1 program
/// Returns the runner after execution + the return values + the serialized return values (if serialize_output is enabled)
/// The return values will contain the memory values just as they appear in the VM, after removing the PanicResult enum (if present).
/// Except if either the flag append_return_values or proof_mode are enabled, in which case the return values will consist of its serialized form: [array_len, array[0], array[1], ..., array[array_len -1]]
pub fn cairo_run_program(
    sierra_program: &SierraProgram,
    cairo_run_config: Cairo1RunConfig,
) -> Result<(CairoRunner, Vec<MaybeRelocatable>, Option<String>), Error> {
    let metadata = calc_metadata_ap_change_only(sierra_program)
        .map_err(|_| VirtualMachineError::Unexpected)?;
    let sierra_program_registry = ProgramRegistry::<CoreType, CoreLibfunc>::new(sierra_program)?;
    let type_sizes =
        get_type_size_map(sierra_program, &sierra_program_registry).unwrap_or_default();
    let config = SierraToCasmConfig {
        gas_usage_check: false,
        max_bytecode_size: usize::MAX,
    };
    let casm_program =
        cairo_lang_sierra_to_casm::compiler::compile(sierra_program, &metadata, config)?;

    let main_func = find_function(sierra_program, "::main")?;

    let initial_gas = 9999999999999_u64;

    // Fetch return type data
    let return_type_id = match main_func.signature.ret_types.last() {
        // We need to check if the last return type is indeed the function's return value and not an implicit return value
        return_type @ Some(concrete_ty)
            if get_info(&sierra_program_registry, concrete_ty)
                .is_some_and(|info| !is_implicit_generic_id(&info.long_id.generic_id)) =>
        {
            return_type
        }
        _ => None,
    };

    if cairo_run_config.copy_to_output()
        && !check_only_array_felt_input_type(
            &main_func.signature.param_types,
            &sierra_program_registry,
        )
    {
        return Err(Error::IlegalInputValue);
    };
    if cairo_run_config.copy_to_output()
        && !check_only_array_felt_return_type(return_type_id, &sierra_program_registry)
    {
        return Err(Error::IlegalReturnValue);
    };

    // Modified entry code to be compatible with custom cairo1 Proof Mode.
    // This adds code that's needed for dictionaries, adjusts ap for builtin pointers, adds initial gas for the gas builtin if needed, and sets up other necessary code for cairo1
    let (entry_code, builtins) = create_entry_code(
        &sierra_program_registry,
        &casm_program,
        &type_sizes,
        main_func,
        &cairo_run_config,
    )?;

    let return_type_size = return_type_id
        .and_then(|id| type_sizes.get(id).cloned())
        .unwrap_or_default();

    // This footer is used by lib funcs
    let libfunc_footer = create_code_footer();
    let builtin_count: i16 = builtins.len().into_or_panic();

    // This is the program we are actually running/proving
    // With (embedded proof mode), cairo1 header and the libfunc footer
    let instructions = chain!(
        entry_code.instructions.iter(),
        casm_program.instructions.iter(),
        libfunc_footer.iter(),
    );

    let (processor_hints, program_hints) = build_hints_vec(instructions.clone());

    let mut hint_processor = Cairo1HintProcessor::new(
        &processor_hints,
        RunResources::default(),
        cairo_run_config.copy_to_output(),
    );

    let data: Vec<MaybeRelocatable> = instructions
        .flat_map(|inst| inst.assemble().encode())
        .map(|x| Felt252::from(&x))
        .map(MaybeRelocatable::from)
        .collect();

    let program = if cairo_run_config.proof_mode {
        Program::new_for_proof(
            builtins.clone(),
            data,
            0,
            // Proof mode is on top
            // `jmp rel 0` is the last line of the entry code.
            entry_code.current_code_offset - 2,
            program_hints,
            ReferenceManager {
                references: Vec::new(),
            },
            HashMap::new(),
            vec![],
            None,
        )?
    } else {
        Program::new(
            builtins.clone(),
            data,
            Some(0),
            program_hints,
            ReferenceManager {
                references: Vec::new(),
            },
            HashMap::new(),
            vec![],
            None,
        )?
    };

    let runner_mode = if cairo_run_config.proof_mode {
        RunnerMode::ProofModeCairo1
    } else {
        RunnerMode::ExecutionMode
    };

    let mut runner = CairoRunner::new_v2(
        &program,
        cairo_run_config.layout,
        runner_mode,
        cairo_run_config.trace_enabled,
    )?;
    let end = runner.initialize(cairo_run_config.proof_mode)?;
    load_arguments(&mut runner, &cairo_run_config, main_func, initial_gas)?;

    // Run it until the end / infinite loop in proof_mode
    runner.run_until_pc(end, &mut hint_processor)?;
    if cairo_run_config.proof_mode {
        runner.run_for_steps(1, &mut hint_processor)?;
    }

    runner.end_run(false, false, &mut hint_processor)?;

    let result_inner_type_size =
        result_inner_type_size(return_type_id, &sierra_program_registry, &type_sizes);
    // Fetch return values
    let return_values = fetch_return_values(
        return_type_size,
        result_inner_type_size,
        &runner.vm,
        builtin_count,
        cairo_run_config.copy_to_output(),
    )?;

    let serialized_output = if cairo_run_config.serialize_output {
        if cairo_run_config.copy_to_output() {
            // The return value is already serialized, so we can just print the array values
            let mut output_string = String::from("[");
            // Skip array_len
            for elem in return_values[1..].iter() {
                maybe_add_whitespace(&mut output_string);
                output_string.push_str(&elem.to_string());
            }
            output_string.push(']');
            Some(output_string)
        } else {
            Some(serialize_output(
                &return_values,
                &mut runner.vm,
                return_type_id,
                &sierra_program_registry,
                &type_sizes,
            ))
        }
    } else {
        None
    };

    // Set stop pointers for builtins so we can obtain the air public input
    if cairo_run_config.finalize_builtins {
        if cairo_run_config.copy_to_output() {
            // Set stop pointer for each builtin
            runner.vm.builtins_final_stack_from_stack_pointer_dict(
                &builtins
                    .iter()
                    .enumerate()
                    .map(|(i, builtin)| {
                        (
                            *builtin,
                            (runner.vm.get_ap() - (builtins.len() - 1 - i)).unwrap(),
                        )
                    })
                    .collect(),
                false,
            )?;
        } else {
            finalize_builtins(
                &main_func.signature.ret_types,
                &type_sizes,
                &mut runner.vm,
                builtin_count,
            )?;
        }

        // Build execution public memory
        if cairo_run_config.proof_mode {
            runner.finalize_segments()?;
        }
    }

    runner.relocate(true)?;

    Ok((runner, return_values, serialized_output))
}

#[allow(clippy::type_complexity)]
fn build_hints_vec<'b>(
    instructions: impl Iterator<Item = &'b Instruction>,
) -> (Vec<(usize, Vec<Hint>)>, HashMap<usize, Vec<HintParams>>) {
    let mut hints: Vec<(usize, Vec<Hint>)> = Vec::new();
    let mut program_hints: HashMap<usize, Vec<HintParams>> = HashMap::new();

    let mut hint_offset = 0;

    for instruction in instructions {
        if !instruction.hints.is_empty() {
            hints.push((hint_offset, instruction.hints.clone()));
            program_hints.insert(
                hint_offset,
                vec![HintParams {
                    code: hint_offset.to_string(),
                    accessible_scopes: Vec::new(),
                    flow_tracking_data: FlowTrackingData {
                        ap_tracking: ApTracking::default(),
                        reference_ids: HashMap::new(),
                    },
                }],
            );
        }
        hint_offset += instruction.body.op_size();
    }
    (hints, program_hints)
}

/// Finds first function ending with `name_suffix`.
fn find_function<'a>(
    sierra_program: &'a SierraProgram,
    name_suffix: &'a str,
) -> Result<&'a Function, RunnerError> {
    sierra_program
        .funcs
        .iter()
        .find(|f| {
            if let Some(name) = &f.id.debug_name {
                name.ends_with(name_suffix)
            } else {
                false
            }
        })
        .ok_or_else(|| RunnerError::MissingMain)
}

/// Creates a list of instructions that will be appended to the program's bytecode.
fn create_code_footer() -> Vec<Instruction> {
    casm! {
        // Add a `ret` instruction used in libfuncs that retrieve the current value of the `fp`
        // and `pc` registers.
        ret;
    }
    .instructions
}

// Loads the input arguments into the execution segment, leaving the necessary gaps for the values that will be written by
// the instructions in the entry_code (produced by `create_entry_code`). Also loads the initial gas if the GasBuiltin is present

/* Example of execution segment before running the main function:
Before calling this function (after runner.initialize):
[
    (*1) output_builtin_base
    builtin_base_0
    builtin_base_1
    return_fp
    return_pc
]
After calling this function (before running the VM):
[
    (*1) output_builtin_base
    builtin_base_0
    builtin_base_1
    return_fp
    return_pc
    (*1+2+3) gap
    (*1+2) gap
    (*1+2) gap
    (*2) gap
    (*2) gap
    (*2) gap
    gap
    gap
    (*2) gap
    (*3) arg_0
    (*3) arg_1
]

After the entry_code (up until calling main) has been ran by the VM:
[
    (*1) output_builtin_base
    builtin_base_0
    builtin_base_1
    return_fp
    return_pc
    (*1) gap (for output_builtin final ptr)
    (*1) gap (for builtin_0 final ptr)
    (*1) gap (for builtin_1 final ptr)
    (*2) segment_arena_ptr
    (*2) infos_ptr
    (*2) 0
    builtin_base_0
    builtin_base_1
    (*2) segment_arena_ptr + 3 (segment_arena base)
    (*4) initial_gas
    (*3) arg_0
    (*3) arg_1
]
(*1) if output builtin is added (if either proof_mode or append_return_values is enabled)
(*2) if segment arena is present
(*3) if args are used
(*4) if gas builtin is present
*/
fn load_arguments(
    runner: &mut CairoRunner,
    cairo_run_config: &Cairo1RunConfig,
    main_func: &Function,
    initial_gas: u64,
) -> Result<(), Error> {
    let got_gas_builtin = main_func
        .signature
        .param_types
        .iter()
        .any(|ty| ty.debug_name.as_ref().is_some_and(|n| n == "GasBuiltin"));
    if cairo_run_config.args.is_empty() && !got_gas_builtin {
        // Nothing to be done
        return Ok(());
    }
    let got_segment_arena = main_func
        .signature
        .param_types
        .iter()
        .any(|ty| ty.debug_name.as_ref().is_some_and(|n| n == "SegmentArena"));
    // This AP correction represents the memory slots taken up by the values created by `create_entry_code`:
    // These include:
    // * The builtin bases (not including output)
    // * (Only if the output builtin is added) A gap for each builtin's final pointer
    // * The segment arena values (if present), including:
    //  * segment_arena_ptr
    //  * info_segment_ptr
    //  * 0
    //  * segment_arena_ptr + 3
    let mut ap_offset = runner.get_program().builtins_len();
    if cairo_run_config.copy_to_output() {
        ap_offset += runner.get_program().builtins_len() - 1;
    }
    if got_segment_arena {
        ap_offset += 4;
    }
    // Load initial gas if GasBuiltin is present
    if got_gas_builtin {
        runner.vm.insert_value(
            (runner.vm.get_ap() + ap_offset).map_err(VirtualMachineError::Math)?,
            Felt252::from(initial_gas),
        )?;
        ap_offset += 1;
    }
    for arg in cairo_run_config.args {
        match arg {
            FuncArg::Array(args) => {
                let array_start = runner.vm.add_memory_segment();
                let array_end = runner.vm.load_data(
                    array_start,
                    &args.iter().map(|f| f.into()).collect::<Vec<_>>(),
                )?;
                runner.vm.insert_value(
                    (runner.vm.get_ap() + ap_offset).map_err(VirtualMachineError::Math)?,
                    array_start,
                )?;
                ap_offset += 1;
                runner.vm.insert_value(
                    (runner.vm.get_ap() + ap_offset).map_err(VirtualMachineError::Math)?,
                    array_end,
                )?;
                ap_offset += 1;
            }
            FuncArg::Single(arg) => {
                runner.vm.insert_value(
                    (runner.vm.get_ap() + ap_offset).map_err(VirtualMachineError::Math)?,
                    arg,
                )?;
                ap_offset += 1;
            }
        }
    }

    Ok(())
}

/// Returns the instructions to add to the beginning of the code to successfully call the main
/// function, as well as the builtins required to execute the program.
fn create_entry_code(
    sierra_program_registry: &ProgramRegistry<CoreType, CoreLibfunc>,
    casm_program: &CairoProgram,
    type_sizes: &UnorderedHashMap<ConcreteTypeId, i16>,
    func: &Function,
    config: &Cairo1RunConfig,
) -> Result<(CasmContext, Vec<BuiltinName>), Error> {
    let copy_to_output_builtin = config.copy_to_output();
    let signature = &func.signature;
    let got_segment_arena = signature.param_types.iter().any(|ty| {
        get_info(sierra_program_registry, ty)
            .map(|x| x.long_id.generic_id == SegmentArenaType::ID)
            .unwrap_or_default()
    });
    let got_gas_builtin = signature.param_types.iter().any(|ty| {
        get_info(sierra_program_registry, ty)
            .map(|x| x.long_id.generic_id == GasBuiltinType::ID)
            .unwrap_or_default()
    });
    // The builtins in the formatting expected by the runner.
    let (builtins, builtin_offset) =
        get_function_builtins(&signature.param_types, copy_to_output_builtin);
    let mut ctx = CasmBuilder::default();
    // Getting a variable pointing to the location of each builtin.
    let mut builtin_vars =
        HashMap::<GenericTypeId, Var>::from_iter(builtin_offset.iter().map(|(id, offset)| {
            (
                id.clone(),
                ctx.add_var(CellExpression::Deref(deref!([fp - offset]))),
            )
        }));
    // Getting a variable for the location output builtin if required.
    let output_ptr = copy_to_output_builtin.then(|| {
        let offset: i16 = 2 + builtins.len().into_or_panic::<i16>();
        ctx.add_var(CellExpression::Deref(deref!([fp - offset])))
    });
    if copy_to_output_builtin {
        // Leave a gap to write the builtin final pointers
        // We write them on a fixed cells relative to the starting FP pointer so we don't lose them after serializing outputs
        for _ in 0..builtins.len() {
            casm_build_extend!(ctx, tempvar _local;);
        }
        casm_build_extend!(ctx, ap += builtins.len(););
    }
    let mut expected_arguments_size = 0;
    if got_segment_arena {
        // Allocating the segment arena and initializing it.
        casm_build_extend! {ctx,
            tempvar segment_arena;
            tempvar infos;
            hint AllocSegment {} into {dst: segment_arena};
            hint AllocSegment {} into {dst: infos};
            const czero = 0;
            tempvar zero = czero;
            // Write Infos segment, n_constructed (0), and n_destructed (0) to the segment.
            assert infos = *(segment_arena++);
            assert zero = *(segment_arena++);
            assert zero = *(segment_arena++);
        }
        // Adding the segment arena to the builtins var map.
        builtin_vars.insert(SegmentArenaType::ID, segment_arena);
    };

    for ty in &signature.param_types {
        let info = get_info(sierra_program_registry, ty)
            .ok_or_else(|| Error::NoInfoForType(ty.clone()))?;
        let generic_ty = &info.long_id.generic_id;
        if let Some(var) = builtin_vars.get(generic_ty).cloned() {
            casm_build_extend!(ctx, tempvar _builtin = var;);
        } else if generic_ty == &SystemType::ID {
            casm_build_extend! {ctx,
                tempvar system;
                hint AllocSegment {} into {dst: system};
                ap += 1;
            };
        } else if generic_ty == &GasBuiltinType::ID {
            // We already loaded the inital gas so we just advance AP
            casm_build_extend! {ctx,
                ap += 1;
            };
        } else {
            let ty_size = type_sizes[ty];
            // We already loaded these arguments, so we just advance AP
            casm_build_extend!(ctx,
                ap+=ty_size as usize;
            );
            expected_arguments_size += ty_size;
        }
    }
    let actual_args_size = config
        .args
        .iter()
        .map(|arg| match arg {
            FuncArg::Single(_) => 1,
            FuncArg::Array(_) => 2,
        })
        .sum::<i16>();
    if expected_arguments_size != actual_args_size {
        return Err(Error::ArgumentsSizeMismatch {
            expected: expected_arguments_size,
            actual: actual_args_size,
        });
    }

    casm_build_extend!(ctx, let () = call FUNCTION;);

    let return_type_id = signature.ret_types.last();
    let return_type_size = return_type_id
        .and_then(|id| type_sizes.get(id).cloned())
        .unwrap_or_default();
    let mut offset: i16 = 0;
    for ty in signature.ret_types.iter().rev() {
        let info = get_info(sierra_program_registry, ty)
            .ok_or_else(|| Error::NoInfoForType(ty.clone()))?;
        offset += type_sizes[ty];
        let generic_ty = &info.long_id.generic_id;
        let Some(var) = builtin_vars.get_mut(generic_ty) else {
            continue;
        };
        *var = ctx.add_var(CellExpression::Deref(deref!([ap - offset])));
    }

    // Helper to get a variable for a given builtin.
    // Fails for builtins that will never be present.
    let get_var = |name: &BuiltinName| match name {
        BuiltinName::range_check => builtin_vars[&RangeCheckType::ID],
        BuiltinName::pedersen => builtin_vars[&PedersenType::ID],
        BuiltinName::bitwise => builtin_vars[&BitwiseType::ID],
        BuiltinName::ec_op => builtin_vars[&EcOpType::ID],
        BuiltinName::poseidon => builtin_vars[&PoseidonType::ID],
        BuiltinName::segment_arena => builtin_vars[&SegmentArenaType::ID],
        _ => unreachable!(),
    };
    if copy_to_output_builtin {
        // Copying the final builtins into a local variables.
        for (i, builtin) in builtins.iter().enumerate() {
            // Skip output_ptr as we still haven't written into it and this will lead to the wrong size being written
            if matches!(builtin, BuiltinName::output) {
                continue;
            }
            let var = get_var(builtin);
            let local = ctx.add_var(CellExpression::Deref(deref!([fp + i.to_i16().unwrap()])));
            casm_build_extend!(ctx, assert local = var;);
        }
        // Serialize return values into output segment
        let output_ptr = output_ptr.unwrap();
        let outputs = (1..(return_type_size + 1))
            .rev()
            .map(|i| ctx.add_var(CellExpression::Deref(deref!([ap - i]))))
            .collect_vec();
        let (array_start_ptr, array_end_ptr) = if is_panic_result(signature.ret_types.last()) {
            // Write panic flag value
            let panic_flag = outputs[0];
            casm_build_extend! {ctx,
                assert panic_flag = *(output_ptr++);
            };
            // If the run did panic, these will point to the panic data
            (outputs[1], outputs[2])
        } else {
            (outputs[0], outputs[1])
        };
        casm_build_extend! {ctx,
            // Calculate size of array and write it into the output segment
            tempvar array_size = array_end_ptr - array_start_ptr;
            assert array_size = *(output_ptr++);
            // Create loop variables
            tempvar remaining_elements = array_size;
            tempvar array_ptr = array_start_ptr;
            tempvar write_ptr = output_ptr;
            // Enter copying loop
            rescope{remaining_elements = remaining_elements, array_ptr = array_ptr, write_ptr = write_ptr};
            jump CopyOutputArray if remaining_elements != 0;
            jump EndOutputCopy;

            // Main Loop
            CopyOutputArray:
            #{steps = 0;}
            // Write array value into output segment
            tempvar val = *(array_ptr++);
            assert val = *(write_ptr++);
            const one = 1;
            // Create loop variables
            tempvar new_remaining_elements = remaining_elements - one;
            tempvar new_array_ptr = array_ptr;
            tempvar new_write_ptr = write_ptr;
            // Continue the loop
            rescope{remaining_elements = new_remaining_elements, array_ptr = new_array_ptr, write_ptr = new_write_ptr};
            jump CopyOutputArray if remaining_elements != 0;

            EndOutputCopy:
        };
        if !actual_args_size.is_zero() {
            // Serialize the input values into the output segment
            // We lost the output_ptr var after re-scoping, so we need to create it again
            // The last instruction will write the last output ptr so we can find it in [ap - 1]
            let output_ptr = ctx.add_var(CellExpression::Deref(deref!([ap - 1])));
            // len(builtins - output) + len(builtins) + if segment_arena: segment_arena_ptr + info_ptr + 0 + (segment_arena_ptr + 3) + (gas_builtin)
            let offset = (2 * builtins.len() - 1
                + 4 * got_segment_arena as usize
                + got_gas_builtin as usize) as i16;
            let array_start_ptr = ctx.add_var(CellExpression::Deref(deref!([fp + offset])));
            let array_end_ptr = ctx.add_var(CellExpression::Deref(deref!([fp + offset + 1])));
            casm_build_extend! {ctx,
                // Calculate size of array and write it into the output segment
                tempvar array_size = array_end_ptr - array_start_ptr;
                assert array_size = *(output_ptr++);
                // Create loop variables
                tempvar remaining_elements = array_size;
                tempvar array_ptr = array_start_ptr;
                tempvar write_ptr = output_ptr;
                // Enter copying loop
                rescope{remaining_elements = remaining_elements, array_ptr = array_ptr, write_ptr = write_ptr};
                jump CopyInputArray if remaining_elements != 0;
                jump EndInputCopy;

                // Main Loop
                CopyInputArray:
                #{steps = 0;}
                // Write array value into output segment
                tempvar val = *(array_ptr++);
                assert val = *(write_ptr++);
                const one = 1;
                // Create loop variables
                tempvar new_remaining_elements = remaining_elements - one;
                tempvar new_array_ptr = array_ptr;
                tempvar new_write_ptr = write_ptr;
                // Continue the loop
                rescope{remaining_elements = new_remaining_elements, array_ptr = new_array_ptr, write_ptr = new_write_ptr};
                jump CopyInputArray if remaining_elements != 0;

                EndInputCopy:
            };
        }
        // After we are done writing into the output segment, we can write the final output_ptr into locals:
        // The last instruction will write the final output ptr so we can find it in [ap - 1]
        let output_ptr = ctx.add_var(CellExpression::Deref(deref!([ap - 1])));
        let local = ctx.add_var(CellExpression::Deref(deref!([fp])));
        casm_build_extend!(ctx, assert local = output_ptr;);

        if got_segment_arena {
            // We re-scoped when serializing the output so we have to create a var for the segment arena
            // len(builtins) + len(builtins - output) + segment_arena_ptr + info_segment + 0
            let off = 2 * builtins.len() + 2;
            let segment_arena_ptr = ctx.add_var(CellExpression::Deref(deref!([fp + off as i16])));
            // Call the hint that will relocate all dictionaries
            ctx.add_hint(
                |[ignored_in], [ignored_out]| StarknetHint::Cheatcode {
                    selector: BigIntAsHex {
                        value: BigInt::from_bytes_be(
                            Sign::Plus,
                            "RelocateAllDictionaries".as_bytes(),
                        ),
                    },
                    input_start: ignored_in.clone(),
                    input_end: ignored_in,
                    output_start: ignored_out,
                    output_end: ignored_out,
                },
                [segment_arena_ptr],
                [segment_arena_ptr],
            );
            // Validating the segment arena's segments are one after the other.
            casm_build_extend! {ctx,
                tempvar n_segments = segment_arena_ptr[-2];
                tempvar n_finalized = segment_arena_ptr[-1];
                assert n_segments = n_finalized;
                jump STILL_LEFT_PRE if n_segments != 0;
                rescope{};
                jump DONE_VALIDATION;
                STILL_LEFT_PRE:
                const one = 1;
                tempvar infos = segment_arena_ptr[-3];
                tempvar remaining_segments = n_segments - one;
                rescope{infos = infos, remaining_segments = remaining_segments};
                LOOP_START:
                jump STILL_LEFT_LOOP if remaining_segments != 0;
                rescope{};
                jump DONE_VALIDATION;
                STILL_LEFT_LOOP:
                const one = 1;
                const three = 3;
                tempvar prev_end = infos[1];
                tempvar curr_start = infos[3];
                assert curr_start = prev_end + one;
                tempvar next_infos = infos + three;
                tempvar next_remaining_segments = remaining_segments - one;
                rescope{infos = next_infos, remaining_segments = next_remaining_segments};
                #{ steps = 0; }
                jump LOOP_START;
                DONE_VALIDATION:
            };
        }
        // Copying the final builtins from locals into the top of the stack.
        for i in 0..builtins.len().to_i16().unwrap() {
            let local = ctx.add_var(CellExpression::Deref(deref!([fp + i])));
            casm_build_extend!(ctx, tempvar _r = local;);
        }
    } else {
        // Writing the final builtins into the top of the stack.
        for builtin in &builtins {
            let var = get_var(builtin);
            casm_build_extend!(ctx, tempvar _r = var;);
        }
    }

    if config.proof_mode {
        casm_build_extend! {ctx,
            INFINITE_LOOP:
            // To enable the merge of the branches.
            #{ steps = 0; }
            jump INFINITE_LOOP;
        };
    } else {
        casm_build_extend!(ctx, ret;);
    }
    let result = ctx.build(["FUNCTION"]);
    let [call_inst] = result.branches[0].1.as_slice() else {
        panic!("Expected a single relocation");
    };
    let mut instructions = result.instructions;
    let instruction_sizes = instructions.iter().map(|inst| inst.body.op_size());
    let prev_call_size: usize = instruction_sizes.clone().take(*call_inst).sum();
    let post_call_size: usize = instruction_sizes.skip(*call_inst).sum();
    let InstructionBody::Call(inst) = &mut instructions[*call_inst].body else {
        panic!("Expected call instruction");
    };
    inst.target = deref_or_immediate!(
        post_call_size
            + casm_program.debug_info.sierra_statement_info[func.entry_point.0].code_offset
    );
    Ok((
        CasmContext {
            instructions,
            current_code_offset: prev_call_size + post_call_size,
            current_hints: vec![],
        },
        builtins,
    ))
}

fn get_info<'a>(
    sierra_program_registry: &'a ProgramRegistry<CoreType, CoreLibfunc>,
    ty: &'a cairo_lang_sierra::ids::ConcreteTypeId,
) -> Option<&'a cairo_lang_sierra::extensions::types::TypeInfo> {
    sierra_program_registry
        .get_type(ty)
        .ok()
        .map(|ctc| ctc.info())
}

fn get_function_builtins(
    params: &[cairo_lang_sierra::ids::ConcreteTypeId],
    append_output: bool,
) -> (
    Vec<BuiltinName>,
    HashMap<cairo_lang_sierra::ids::GenericTypeId, i16>,
) {
    let mut builtins = Vec::new();
    let mut builtin_offset: HashMap<cairo_lang_sierra::ids::GenericTypeId, i16> = HashMap::new();
    let mut current_offset = 3;
    for (debug_name, builtin_name, sierra_id) in [
        ("Poseidon", BuiltinName::poseidon, PoseidonType::ID),
        ("EcOp", BuiltinName::ec_op, EcOpType::ID),
        ("Bitwise", BuiltinName::bitwise, BitwiseType::ID),
        ("RangeCheck", BuiltinName::range_check, RangeCheckType::ID),
        ("Pedersen", BuiltinName::pedersen, PedersenType::ID),
    ] {
        if params
            .iter()
            .any(|id| id.debug_name.as_deref() == Some(debug_name))
        {
            builtins.push(builtin_name);
            builtin_offset.insert(sierra_id, current_offset);
            current_offset += 1;
        }
    }
    // Force an output builtin so that we can write the program output into it's segment
    if append_output {
        builtins.push(BuiltinName::output);
    }
    builtins.reverse();
    (builtins, builtin_offset)
}

// Checks that the program input (if present) is of type Array<Felt252>
fn check_only_array_felt_input_type(
    params: &[ConcreteTypeId],
    sierra_program_registry: &ProgramRegistry<CoreType, CoreLibfunc>,
) -> bool {
    // Filter implicit arguments (builtins, gas)
    let arg_types = params
        .iter()
        .filter(|ty| {
            let info = get_info(sierra_program_registry, ty).unwrap();
            let generic_ty = &info.long_id.generic_id;
            !is_implicit_generic_id(generic_ty)
        })
        .collect_vec();
    if arg_types.is_empty() {
        // No inputs
        true
    } else if arg_types.len() == 1 {
        arg_types[0]
            .debug_name
            .as_ref()
            .is_some_and(|name| name == "Array<felt252>")
    } else {
        false
    }
}

// Returns true if the generic id corresponds to an implicit argument (aka a builtin, gas, or system type)
fn is_implicit_generic_id(generic_ty: &GenericTypeId) -> bool {
    [
        SegmentArenaType::ID,
        GasBuiltinType::ID,
        BitwiseType::ID,
        EcOpType::ID,
        PedersenType::ID,
        PoseidonType::ID,
        RangeCheckType::ID,
        SegmentArenaType::ID,
        SystemType::ID,
    ]
    .contains(generic_ty)
}
// Checks that the return type is either an Array<Felt252> or a PanicResult<Array<Felt252>> type
fn check_only_array_felt_return_type(
    return_type_id: Option<&ConcreteTypeId>,
    sierra_program_registry: &ProgramRegistry<CoreType, CoreLibfunc>,
) -> bool {
    if return_type_id.is_none() {
        return false;
    };
    // Unwrap PanicResult (if appicable)
    let return_type =
        if let Some(return_type) = result_inner_type(return_type_id, sierra_program_registry) {
            return_type
        } else {
            return_type_id.unwrap()
        };
    let return_type = sierra_program_registry.get_type(return_type).unwrap();
    // Check that the resulting type is an Array<Felt252>
    match return_type {
        cairo_lang_sierra::extensions::core::CoreTypeConcrete::Array(info) => {
            let inner_ty = sierra_program_registry.get_type(&info.ty).unwrap();
            matches!(
                inner_ty,
                cairo_lang_sierra::extensions::core::CoreTypeConcrete::Felt252(_)
            )
        }
        _ => false,
    }
}

fn is_panic_result(return_type_id: Option<&ConcreteTypeId>) -> bool {
    return_type_id
        .map(|id| {
            id.debug_name
                .as_ref()
                .is_some_and(|name| name.starts_with("core::panics::PanicResult::"))
        })
        .unwrap_or_default()
}

// Returns the T type in PanicResult::Ok(T) if applicable
// Returns None if the return_type_id is not a PanicResult
fn result_inner_type<'a>(
    return_type_id: Option<&'a ConcreteTypeId>,
    sierra_program_registry: &'a ProgramRegistry<CoreType, CoreLibfunc>,
) -> Option<&'a ConcreteTypeId> {
    if is_panic_result(return_type_id) {
        let return_type_info =
            get_info(sierra_program_registry, return_type_id.as_ref().unwrap()).unwrap();
        // We already know info.long_id.generic_args[0] contains the Panic variant
        let inner_args = &return_type_info.long_id.generic_args[1];
        let inner_type = {
            let inner_type = match inner_args {
                GenericArg::Type(type_id) => type_id,
                _ => unreachable!(),
            };
            // The inner type contains a single-element tuple so we need to get rid of it too
            let inner_type_info = get_info(sierra_program_registry, inner_type).unwrap();
            match &inner_type_info.long_id.generic_args[1] {
                GenericArg::Type(type_id) => type_id,
                _ => unreachable!(),
            }
        };

        Some(inner_type)
    } else {
        None
    }
}

// Returns the size of the T type in PanicResult::Ok(T) if applicable
// Returns None if the return_type_id is not a PanicResult
fn result_inner_type_size(
    return_type_id: Option<&ConcreteTypeId>,
    sierra_program_registry: &ProgramRegistry<CoreType, CoreLibfunc>,
    type_sizes: &UnorderedHashMap<ConcreteTypeId, i16>,
) -> Option<i16> {
    result_inner_type(return_type_id, sierra_program_registry)
        .and_then(|ty| type_sizes.get(ty).copied())
}

fn fetch_return_values(
    return_type_size: i16,
    result_inner_type_size: Option<i16>,
    vm: &VirtualMachine,
    builtin_count: i16,
    fetch_from_output: bool,
) -> Result<Vec<MaybeRelocatable>, Error> {
    if fetch_from_output {
        // In this case we will find the serialized return value in the format:
        // [*panic_flag, array_len, array[0], array[1],..., array[array_len-1]]
        // *: If the return value is a PanicResult

        // Output Builtin will always be on segment 2
        let return_values =
            vm.get_continuous_range((2, 0).into(), vm.get_segment_size(2).unwrap())?;
        // Remove panic wrapper
        let (return_values, panic_flag) = if result_inner_type_size.is_none() {
            // return value is not a PanicResult
            (&return_values[..], false)
        } else {
            // return value is a PanicResult
            (
                &return_values[1..],
                return_values[0] != MaybeRelocatable::from(0),
            )
        };
        // Take only the output (as the output segment will also contain the input)
        let output_len = return_values[0].get_int().unwrap().to_usize().unwrap() + 1;
        let return_values = &return_values[0..output_len];
        // Return Ok or Err based on panic_flag
        if panic_flag {
            return Err(Error::RunPanic(
                return_values
                    .iter()
                    .map(|mr| mr.get_int().unwrap_or_default())
                    .collect_vec(),
            ));
        } else {
            return Ok(return_values.to_vec());
        }
    }

    let mut return_values = vm.get_continuous_range(
        (vm.get_ap() - (return_type_size + builtin_count) as usize).unwrap(),
        return_type_size as usize,
    )?;
    // Handle PanicResult (we already checked if the type is a PanicResult when fetching the inner type size)
    if let Some(inner_type_size) = result_inner_type_size {
        // Check the failure flag (aka first return value)
        if return_values.first() != Some(&MaybeRelocatable::from(0)) {
            // In case of failure, extract the error from the return values (aka last two values)
            let panic_data_end = return_values
                .last()
                .ok_or(Error::FailedToExtractReturnValues)?
                .get_relocatable()
                .ok_or(Error::FailedToExtractReturnValues)?;
            let panic_data_start = return_values
                .get(return_values.len() - 2)
                .ok_or(Error::FailedToExtractReturnValues)?
                .get_relocatable()
                .ok_or(Error::FailedToExtractReturnValues)?;
            let panic_data = vm.get_integer_range(
                panic_data_start,
                (panic_data_end - panic_data_start).map_err(VirtualMachineError::Math)?,
            )?;
            return Err(Error::RunPanic(
                panic_data.iter().map(|c| *c.as_ref()).collect(),
            ));
        } else {
            if return_values.len() < inner_type_size as usize {
                return Err(Error::FailedToExtractReturnValues);
            }
            return_values =
                return_values[((return_type_size - inner_type_size).into_or_panic())..].to_vec()
        }
    }
    Ok(return_values)
}

// Calculates builtins' final_stack setting each stop_ptr
// Calling this function is a must if either air_public_input or cairo_pie are needed
fn finalize_builtins(
    main_ret_types: &[ConcreteTypeId],
    type_sizes: &UnorderedHashMap<ConcreteTypeId, i16>,
    vm: &mut VirtualMachine,
    builtin_count: i16,
) -> Result<(), Error> {
    // Set stop pointers for builtins so we can obtain the air public input
    // Cairo 1 programs have other return values aside from the used builtin's final pointers, so we need to hand-pick them
    let ret_types_sizes = main_ret_types
        .iter()
        .map(|id| type_sizes.get(id).cloned().unwrap_or_default());
    let ret_types_and_sizes = main_ret_types.iter().zip(ret_types_sizes.clone());

    let full_ret_types_size: i16 = ret_types_sizes.sum();
    let mut stack_pointer = (vm.get_ap()
        - (full_ret_types_size as usize + builtin_count as usize).saturating_sub(1))
    .map_err(VirtualMachineError::Math)?;

    // Calculate the stack_ptr for each return builtin in the return values
    let mut builtin_name_to_stack_pointer = HashMap::new();
    for (id, size) in ret_types_and_sizes {
        if let Some(ref name) = id.debug_name {
            let builtin_name = match name.as_str() {
                "RangeCheck" => BuiltinName::range_check,
                "Poseidon" => BuiltinName::poseidon,
                "EcOp" => BuiltinName::ec_op,
                "Bitwise" => BuiltinName::bitwise,
                "Pedersen" => BuiltinName::pedersen,
                "Output" => BuiltinName::output,
                "Ecdsa" => BuiltinName::ecdsa,
                _ => {
                    stack_pointer.offset += size as usize;
                    continue;
                }
            };
            builtin_name_to_stack_pointer.insert(builtin_name, stack_pointer);
        }
        stack_pointer.offset += size as usize;
    }

    // Set stop pointer for each builtin
    vm.builtins_final_stack_from_stack_pointer_dict(&builtin_name_to_stack_pointer, false)?;
    Ok(())
}

fn serialize_output(
    return_values: &[MaybeRelocatable],
    vm: &mut VirtualMachine,
    return_type_id: Option<&ConcreteTypeId>,
    sierra_program_registry: &ProgramRegistry<CoreType, CoreLibfunc>,
    type_sizes: &UnorderedHashMap<ConcreteTypeId, i16>,
) -> String {
    let mut output_string = String::new();
    let return_type_id = if let Some(id) = return_type_id {
        id
    } else {
        return output_string;
    };
    let mut return_values_iter = return_values.iter().peekable();
    serialize_output_inner(
        &mut return_values_iter,
        &mut output_string,
        vm,
        return_type_id,
        sierra_program_registry,
        type_sizes,
    );
    output_string
}

fn serialize_output_inner<'a>(
    return_values_iter: &mut Peekable<impl Iterator<Item = &'a MaybeRelocatable>>,
    output_string: &mut String,
    vm: &mut VirtualMachine,
    return_type_id: &ConcreteTypeId,
    sierra_program_registry: &ProgramRegistry<CoreType, CoreLibfunc>,
    type_sizes: &UnorderedHashMap<ConcreteTypeId, i16>,
) {
    match sierra_program_registry.get_type(return_type_id).unwrap() {
        cairo_lang_sierra::extensions::core::CoreTypeConcrete::Array(info) => {
            // Fetch array from memory
            let array_start = return_values_iter
                .next()
                .expect("Missing return value")
                .get_relocatable()
                .expect("Array start_ptr not Relocatable");
            let array_end = return_values_iter
                .next()
                .expect("Missing return value")
                .get_relocatable()
                .expect("Array end_ptr not Relocatable");
            let array_size = (array_end - array_start).unwrap();

            let array_data = vm.get_continuous_range(array_start, array_size).unwrap();
            let mut array_data_iter = array_data.iter().peekable();
            let array_elem_id = &info.ty;
            // Serialize array data
            maybe_add_whitespace(output_string);
            output_string.push('[');
            while array_data_iter.peek().is_some() {
                serialize_output_inner(
                    &mut array_data_iter,
                    output_string,
                    vm,
                    array_elem_id,
                    sierra_program_registry,
                    type_sizes,
                )
            }
            output_string.push(']');
        }
        cairo_lang_sierra::extensions::core::CoreTypeConcrete::Box(info) => {
            // As this represents a pointer, we need to extract it's values
            let ptr = return_values_iter
                .next()
                .expect("Missing return value")
                .get_relocatable()
                .expect("Box Pointer is not Relocatable");
            let type_size = type_sizes.type_size(&info.ty);
            let data = vm
                .get_continuous_range(ptr, type_size)
                .expect("Failed to extract value from nullable ptr");
            let mut data_iter = data.iter().peekable();
            serialize_output_inner(
                &mut data_iter,
                output_string,
                vm,
                &info.ty,
                sierra_program_registry,
                type_sizes,
            )
        }
        cairo_lang_sierra::extensions::core::CoreTypeConcrete::Const(_) => {
            unimplemented!("Not supported in the current version")
        },
        cairo_lang_sierra::extensions::core::CoreTypeConcrete::Felt252(_)
        | cairo_lang_sierra::extensions::core::CoreTypeConcrete::BoundedInt(_)
        // Only unsigned integer values implement Into<Bytes31>
        | cairo_lang_sierra::extensions::core::CoreTypeConcrete::Bytes31(_)
        | cairo_lang_sierra::extensions::core::CoreTypeConcrete::Uint8(_)
        | cairo_lang_sierra::extensions::core::CoreTypeConcrete::Uint16(_)
        | cairo_lang_sierra::extensions::core::CoreTypeConcrete::Uint32(_)
        | cairo_lang_sierra::extensions::core::CoreTypeConcrete::Uint64(_)
        | cairo_lang_sierra::extensions::core::CoreTypeConcrete::Uint128(_) => {
            maybe_add_whitespace(output_string);
            let val = return_values_iter
                .next()
                .expect("Missing return value")
                .get_int()
                .expect("Value is not an integer");
            output_string.push_str(&val.to_string());
        }
        cairo_lang_sierra::extensions::core::CoreTypeConcrete::Sint8(_)
        | cairo_lang_sierra::extensions::core::CoreTypeConcrete::Sint16(_)
        | cairo_lang_sierra::extensions::core::CoreTypeConcrete::Sint32(_)
        | cairo_lang_sierra::extensions::core::CoreTypeConcrete::Sint64(_)
        | cairo_lang_sierra::extensions::core::CoreTypeConcrete::Sint128(_) => {
            maybe_add_whitespace(output_string);
            let val = return_values_iter
                .next()
                .expect("Missing return value")
                .get_int()
                .expect("Value is not an integer");
            output_string.push_str(&signed_felt(val).to_string());
        }
        cairo_lang_sierra::extensions::core::CoreTypeConcrete::NonZero(info) => {
            serialize_output_inner(
                return_values_iter,
                output_string,
                vm,
                &info.ty,
                sierra_program_registry,
                type_sizes,
            )
        }
        cairo_lang_sierra::extensions::core::CoreTypeConcrete::Nullable(info) => {
            // As this represents a pointer, we need to extract it's values
            let ptr = match return_values_iter.next().expect("Missing return value") {
                MaybeRelocatable::RelocatableValue(ptr) => *ptr,
                MaybeRelocatable::Int(felt) if felt.is_zero() => {
                    // Nullable is Null
                    maybe_add_whitespace(output_string);
                    output_string.push_str("null");
                    return;
                }
                _ => panic!("Invalid Nullable"),
            };
            let type_size = type_sizes.type_size(&info.ty);
            let data = vm
                .get_continuous_range(ptr, type_size)
                .expect("Failed to extract value from nullable ptr");
            let mut data_iter = data.iter().peekable();
            serialize_output_inner(
                &mut data_iter,
                output_string,
                vm,
                &info.ty,
                sierra_program_registry,
                type_sizes,
            )
        }
        cairo_lang_sierra::extensions::core::CoreTypeConcrete::Enum(info) => {
            // First we check if it is a Panic enum, as we already handled panics when fetching return values,
            // we can ignore them and move on to the non-panic variant
            if let GenericArg::UserType(user_type) = &info.info.long_id.generic_args[0] {
                if user_type
                    .debug_name
                    .as_ref()
                    .is_some_and(|n| n.starts_with("core::panics::PanicResult"))
                {
                    return serialize_output_inner(
                        return_values_iter,
                        output_string,
                        vm,
                        &info.variants[0],
                        sierra_program_registry,
                        type_sizes,
                    );
                }
            }
            let num_variants = &info.variants.len();
            let casm_variant_idx: usize = return_values_iter
                .next()
                .expect("Missing return value")
                .get_int()
                .expect("Enum tag is not integer")
                .to_usize()
                .expect("Invalid enum tag");
            // Convert casm variant idx to sierra variant idx
            let variant_idx = if *num_variants > 2 {
                num_variants - 1 - (casm_variant_idx >> 1)
            } else {
                casm_variant_idx
            };
            let variant_type_id = &info.variants[variant_idx];

            // Handle core::bool separately
            if let GenericArg::UserType(user_type) = &info.info.long_id.generic_args[0] {
                if user_type
                    .debug_name
                    .as_ref()
                    .is_some_and(|n| n == "core::bool")
                {
                    // Sanity checks
                    assert!(
                        *num_variants == 2
                            && variant_idx < 2
                            && type_sizes
                                .get(&info.variants[0])
                                .is_some_and(|size| size.is_zero())
                            && type_sizes
                                .get(&info.variants[1])
                                .is_some_and(|size| size.is_zero()),
                        "Malformed bool enum"
                    );

                    let boolean_string = match variant_idx {
                        0 => "false",
                        _ => "true",
                    };
                    maybe_add_whitespace(output_string);
                    output_string.push_str(boolean_string);
                    return;
                }
            }
            // TODO: Something similar to the bool handling could be done for unit enum variants if we could get the type info with the variant names

            // Space is always allocated for the largest enum member, padding with zeros in front for the smaller variants
            let mut max_variant_size = 0;
            for variant in &info.variants {
                let variant_size = type_sizes.get(variant).unwrap();
                max_variant_size = std::cmp::max(max_variant_size, *variant_size)
            }
            for _ in 0..max_variant_size - type_sizes.get(variant_type_id).unwrap() {
                // Remove padding
                assert_eq!(
                    return_values_iter.next(),
                    Some(&MaybeRelocatable::from(0)),
                    "Malformed enum"
                );
            }
            serialize_output_inner(
                return_values_iter,
                output_string,
                vm,
                variant_type_id,
                sierra_program_registry,
                type_sizes,
            )
        }
        cairo_lang_sierra::extensions::core::CoreTypeConcrete::Struct(info) => {
            for member_type_id in &info.members {
                serialize_output_inner(
                    return_values_iter,
                    output_string,
                    vm,
                    member_type_id,
                    sierra_program_registry,
                    type_sizes,
                )
            }
        }
        cairo_lang_sierra::extensions::core::CoreTypeConcrete::Felt252Dict(info) => {
            // Process Dictionary
            let dict_ptr = return_values_iter
                .next()
                .expect("Missing return val")
                .get_relocatable()
                .expect("Dict Ptr not Relocatable");
            if !(dict_ptr.offset
                == vm
                    .get_segment_size(dict_ptr.segment_index as usize)
                    .unwrap_or_default()
                && dict_ptr.offset % 3 == 0)
            {
                panic!("Return value is not a valid Felt252Dict")
            }
            // Fetch dictionary values type id
            let value_type_id = &info.ty;
            // Fetch the dictionary's memory
            let dict_mem = vm
                .get_continuous_range((dict_ptr.segment_index, 0).into(), dict_ptr.offset)
                .expect("Malformed dictionary memory");
            // Serialize the dictionary
            output_string.push('{');
            // The dictionary's memory is made up of (key, prev_value, next_value) tuples
            // The prev value is not relevant to the user so we can skip over it for calrity
            for (key, _, value) in dict_mem.iter().tuples() {
                maybe_add_whitespace(output_string);
                // Serialize the key wich should always be a Felt value
                output_string.push_str(&key.to_string());
                output_string.push(':');
                // Serialize the value
                // We create a peekable array here in order to use the serialize_output_inner as the value could be a span
                let value_vec = vec![value.clone()];
                let mut value_iter = value_vec.iter().peekable();
                serialize_output_inner(
                    &mut value_iter,
                    output_string,
                    vm,
                    value_type_id,
                    sierra_program_registry,
                    type_sizes,
                );
            }
            output_string.push('}');
        }
        cairo_lang_sierra::extensions::core::CoreTypeConcrete::SquashedFelt252Dict(info) => {
            // Process Dictionary
            let dict_start = return_values_iter
                .next()
                .expect("Missing return val")
                .get_relocatable()
                .expect("Squashed dict_start ptr not Relocatable");
            let dict_end = return_values_iter
                .next()
                .expect("Missing return val")
                .get_relocatable()
                .expect("Squashed dict_end ptr not Relocatable");
            let dict_size = (dict_end - dict_start).unwrap();
            if dict_size % 3 != 0 {
                panic!("Return value is not a valid SquashedFelt252Dict")
            }
            // Fetch dictionary values type id
            let value_type_id = &info.ty;
            // Fetch the dictionary's memory
            let dict_mem = vm
                .get_continuous_range(dict_start, dict_size)
                .expect("Malformed squashed dictionary memory");
            // Serialize the dictionary
            output_string.push('{');
            // The dictionary's memory is made up of (key, prev_value, next_value) tuples
            // The prev value is not relevant to the user so we can skip over it for calrity
            for (key, _, value) in dict_mem.iter().tuples() {
                maybe_add_whitespace(output_string);
                // Serialize the key wich should always be a Felt value
                output_string.push_str(&key.to_string());
                output_string.push(':');
                // Serialize the value
                // We create a peekable array here in order to use the serialize_output_inner as the value could be a span
                let value_vec = vec![value.clone()];
                let mut value_iter = value_vec.iter().peekable();
                serialize_output_inner(
                    &mut value_iter,
                    output_string,
                    vm,
                    value_type_id,
                    sierra_program_registry,
                    type_sizes,
                );
            }
            output_string.push('}');
        }
        cairo_lang_sierra::extensions::core::CoreTypeConcrete::Span(_) => unimplemented!("Span types get resolved to Array in the current version"),
        cairo_lang_sierra::extensions::core::CoreTypeConcrete::Snapshot(info) => {
            serialize_output_inner(
                return_values_iter,
                output_string,
                vm,
                &info.ty,
                sierra_program_registry,
                type_sizes,
            )
        }
        cairo_lang_sierra::extensions::core::CoreTypeConcrete::GasBuiltin(_info) => {
            // Ignore it
            let _ = return_values_iter.next();
        },
        _ => panic!("Unexpected return type")
    }
}

fn maybe_add_whitespace(string: &mut String) {
    if !string.is_empty() && !string.ends_with('[') && !string.ends_with('{') {
        string.push(' ');
    }
}

#[cfg(test)]
mod tests {
    use std::path::Path;

    use super::*;
    use cairo_lang_compiler::{
        compile_prepared_db, db::RootDatabase, project::setup_project, CompilerConfig,
    };
    use cairo_vm::{program_hash::compute_program_hash_chain, types::relocatable::Relocatable};
    use rstest::rstest;

    fn compile_to_sierra(filename: &str) -> SierraProgram {
        let compiler_config = CompilerConfig {
            replace_ids: true,
            ..CompilerConfig::default()
        };
        let mut db = RootDatabase::builder()
            .detect_corelib()
            .skip_auto_withdraw_gas()
            .build()
            .unwrap();
        let main_crate_ids = setup_project(&mut db, Path::new(filename)).unwrap();
        compile_prepared_db(&mut db, main_crate_ids, compiler_config).unwrap()
    }

    fn main_hash_panic_result(sierra_program: &SierraProgram) -> bool {
        let main_func = find_function(sierra_program, "::main").unwrap();
        main_func
            .signature
            .ret_types
            .last()
            .and_then(|rt| {
                rt.debug_name
                    .as_ref()
                    .map(|n| n.as_ref().starts_with("core::panics::PanicResult::"))
            })
            .unwrap_or_default()
    }

    #[rstest]
    #[case("../cairo_programs/cairo-1-programs/serialized_output/array_append.cairo")]
    #[case("../cairo_programs/cairo-1-programs/serialized_output/array_get.cairo")]
    #[case("../cairo_programs/cairo-1-programs/serialized_output/dictionaries.cairo")]
    #[case("../cairo_programs/cairo-1-programs/serialized_output/enum_flow.cairo")]
    #[case("../cairo_programs/cairo-1-programs/serialized_output/enum_match.cairo")]
    #[case("../cairo_programs/cairo-1-programs/serialized_output/factorial.cairo")]
    #[case("../cairo_programs/cairo-1-programs/serialized_output/fibonacci.cairo")]
    #[case("../cairo_programs/cairo-1-programs/serialized_output/hello.cairo")]
    #[case("../cairo_programs/cairo-1-programs/serialized_output/pedersen_example.cairo")]
    #[case("../cairo_programs/cairo-1-programs/serialized_output/poseidon.cairo")]
    #[case("../cairo_programs/cairo-1-programs/serialized_output/print.cairo")]
    #[case("../cairo_programs/cairo-1-programs/serialized_output/array_append.cairo")]
    #[case("../cairo_programs/cairo-1-programs/serialized_output/recursion.cairo")]
    #[case("../cairo_programs/cairo-1-programs/serialized_output/sample.cairo")]
    #[case("../cairo_programs/cairo-1-programs/serialized_output/simple_struct.cairo")]
    #[case("../cairo_programs/cairo-1-programs/serialized_output/simple.cairo")]
    #[case("../cairo_programs/cairo-1-programs/serialized_output/struct_span_return.cairo")]
    fn check_append_ret_values_to_output_segment(
        #[case] filename: &str,
        #[values(true, false)] proof_mode: bool,
    ) {
        // Compile to sierra
        let sierra_program = compile_to_sierra(filename);
        // Set proof_mode
        let cairo_run_config = Cairo1RunConfig {
            proof_mode,
            layout: LayoutName::all_cairo,
            append_return_values: !proof_mode, // This is so we can test appending return values when not running in proof_mode
            finalize_builtins: true,
            ..Default::default()
        };
        // Run program
        let (runner, return_values, _) =
            cairo_run_program(&sierra_program, cairo_run_config).unwrap();
        // When the return type is a PanicResult, we remove the panic wrapper when returning the ret values
        // And handle the panics returning an error, so we need to add it here
        let return_values = if main_hash_panic_result(&sierra_program) {
            let mut rv = vec![Felt252::ZERO.into()];
            rv.extend_from_slice(&return_values);
            rv
        } else {
            return_values
        };
        // Check that the output segment contains the return values
        // The output builtin will always be the first builtin, so we know it's segment is 2
        let output_builtin_segment = runner
            .vm
            .get_continuous_range((2, 0).into(), return_values.len())
            .unwrap();
        // While this test can make sure that the return values are the same as the output segment values, as the code that fetches return values
        // takes them from the output segment we can't be sure that these return values are correct, for this we use the integration tests in the main.rs file
        assert_eq!(output_builtin_segment, return_values, "{}", filename);
        // Just for consistency, we will check that there are no values in the output segment after the return values
        assert!(runner
            .vm
            .get_maybe(&Relocatable::from((2_isize, return_values.len())))
            .is_none());
    }
    #[test]
    fn check_program_hash_doesnt_change_based_on_arguments() {
        let sierra_program = compile_to_sierra(
            "../cairo_programs/cairo-1-programs/with_input/array_input_sum.cairo",
        );
        let config_a = Cairo1RunConfig {
            layout: LayoutName::all_cairo,
            args: &[
                FuncArg::Single(Felt252::ONE),
                FuncArg::Array(vec![Felt252::ONE, Felt252::TWO, Felt252::THREE]),
                FuncArg::Single(Felt252::TWO),
                FuncArg::Array(vec![Felt252::ONE, Felt252::TWO, Felt252::THREE]),
            ],
            ..Default::default()
        };
        let config_b = Cairo1RunConfig {
            layout: LayoutName::all_cairo,
            args: &[
                FuncArg::Single(Felt252::ZERO),
                FuncArg::Array(vec![Felt252::THREE]),
                FuncArg::Single(Felt252::ZERO),
                FuncArg::Array(vec![Felt252::TWO]),
            ],
            ..Default::default()
        };
        let runner_a = cairo_run_program(&sierra_program, config_a).unwrap().0;
        let runner_b = cairo_run_program(&sierra_program, config_b).unwrap().0;
        let hash_a =
            compute_program_hash_chain(&runner_a.get_program().get_stripped_program().unwrap(), 0)
                .unwrap();
        let hash_b =
            compute_program_hash_chain(&runner_b.get_program().get_stripped_program().unwrap(), 0)
                .unwrap();
        assert_eq!(hash_a, hash_b)
    }

    #[rstest]
    fn check_output_segment_contains_program_ouput_and_input(
        #[values(true, false)] proof_mode: bool,
    ) {
        // tensor.cairo
        // inputs: [2 2 2 4 1 2 3 4]
        // outputs: [1]
        // Compile to sierra
        let sierra_program = compile_to_sierra(
            "../cairo_programs/cairo-1-programs/serialized_output/with_input/tensor.cairo",
        );
        // Set proof_mode
        let cairo_run_config = Cairo1RunConfig {
            proof_mode,
            layout: LayoutName::all_cairo,
            append_return_values: !proof_mode, // This is so we can test appending return values when not running in proof_mode
            finalize_builtins: true,
            args: &[FuncArg::Array(vec![
                2.into(),
                2.into(),
                2.into(),
                4.into(),
                1.into(),
                2.into(),
                3.into(),
                4.into(),
            ])],
            ..Default::default()
        };
        // Run program
        let (runner, _, _) = cairo_run_program(&sierra_program, cairo_run_config).unwrap();
        // Check output segment
        let expected_output_segment: Vec<Felt252> = vec![
            // panic_flag
            0.into(),
            // output len
            1.into(),
            // output
            1.into(),
            // input len
            8.into(),
            // input
            2.into(),
            2.into(),
            2.into(),
            4.into(),
            1.into(),
            2.into(),
            3.into(),
            4.into(),
        ];
        let output_segment_size = runner.vm.get_segment_size(2).unwrap_or_default();
        let output_segment = runner
            .vm
            .get_integer_range((2, 0).into(), output_segment_size)
            .unwrap()
            .iter()
            .map(|f| f.clone().into_owned())
            .collect_vec();
        assert_eq!(expected_output_segment, output_segment);
    }
}
