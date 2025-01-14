use crate::{
    air_private_input::AirPrivateInput,
    air_public_input::{PublicInput, PublicInputError},
    math_utils::safe_div_usize,
    stdlib::{
        any::Any,
        collections::{HashMap, HashSet},
        ops::{Add, AddAssign, Mul, MulAssign, Sub, SubAssign},
        prelude::*,
    },
    types::{builtin_name::BuiltinName, layout::CairoLayoutParams, layout_name::LayoutName},
    vm::{
        runners::builtin_runner::SegmentArenaBuiltinRunner,
        trace::trace_entry::{relocate_trace_register, RelocatedTraceEntry},
    },
    Felt252,
};

use crate::{
    hint_processor::hint_processor_definition::{HintProcessor, HintReference},
    types::{
        errors::{math_errors::MathError, program_errors::ProgramError},
        exec_scope::ExecutionScopes,
        layout::CairoLayout,
        program::Program,
        relocatable::{relocate_address, relocate_value, MaybeRelocatable, Relocatable},
    },
    utils::is_subsequence,
    vm::{
        errors::{
            cairo_run_errors::CairoRunError,
            memory_errors::{InsufficientAllocatedCellsError, MemoryError},
            runner_errors::RunnerError,
            trace_errors::TraceError,
            vm_errors::VirtualMachineError,
            vm_exception::VmException,
        },
        security::verify_secure_runner,
        {
            runners::builtin_runner::{
                BitwiseBuiltinRunner, BuiltinRunner, EcOpBuiltinRunner, HashBuiltinRunner,
                OutputBuiltinRunner, RangeCheckBuiltinRunner, SignatureBuiltinRunner,
            },
            vm_core::VirtualMachine,
        },
    },
};
use num_integer::div_rem;
use num_traits::{ToPrimitive, Zero};
use serde::{Deserialize, Serialize};

use super::{builtin_runner::ModBuiltinRunner, cairo_pie::CairoPieAdditionalData};
use super::{
    builtin_runner::{
        KeccakBuiltinRunner, PoseidonBuiltinRunner, RC_N_PARTS_96, RC_N_PARTS_STANDARD,
    },
    cairo_pie::{self, CairoPie, CairoPieMetadata, CairoPieVersion},
};
use crate::types::instance_definitions::mod_instance_def::ModInstanceDef;

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum CairoArg {
    Single(MaybeRelocatable),
    Array(Vec<MaybeRelocatable>),
    Composed(Vec<CairoArg>),
}

impl From<MaybeRelocatable> for CairoArg {
    fn from(other: MaybeRelocatable) -> Self {
        CairoArg::Single(other)
    }
}

impl From<Vec<MaybeRelocatable>> for CairoArg {
    fn from(other: Vec<MaybeRelocatable>) -> Self {
        CairoArg::Array(other)
    }
}

// ================
//   RunResources
// ================

/// Maintains the resources of a cairo run. Can be used across multiple runners.
#[derive(Clone, Default, Debug, PartialEq)]
pub struct RunResources {
    n_steps: Option<usize>,
}

/// This trait is in charge of overseeing the VM's step usage in contexts where a limited amount of steps are available
/// for a single execution (which may or not involve other executions taking place in the duration of it ).
/// This is mostly used in the context of starknet, where contracts can call other contracts while sharing the same step limit.
/// For the general use case, the default implementation can be used, which ignores resource tracking altogether
/// For an example on how to implement this trait for its intended purpose check out [BuiltinHintProcessor](cairo_vm::hint_processor::builtin_hint_processor::builtin_hint_processor_definition::BuiltinHintProcessor)
pub trait ResourceTracker {
    /// Returns true if there are no more steps left to run
    fn consumed(&self) -> bool {
        false
    }
    /// Subtracts 1 step from the available steps
    fn consume_step(&mut self) {}
    /// Returns the available steps for the run
    fn get_n_steps(&self) -> Option<usize> {
        None
    }
    /// Returns a reference to the available resources
    fn run_resources(&self) -> &RunResources {
        &RunResources { n_steps: None }
    }
}

impl RunResources {
    pub fn new(n_steps: usize) -> Self {
        Self {
            n_steps: Some(n_steps),
        }
    }
}

impl ResourceTracker for RunResources {
    fn consumed(&self) -> bool {
        if self.n_steps == Some(0) {
            return true;
        }
        false
    }

    fn consume_step(&mut self) {
        if let Some(n_steps) = self.n_steps {
            self.n_steps = Some(n_steps.saturating_sub(1));
        }
    }

    fn get_n_steps(&self) -> Option<usize> {
        self.n_steps
    }

    fn run_resources(&self) -> &RunResources {
        self
    }
}

pub struct CairoRunner {
    pub vm: VirtualMachine,
    pub(crate) program: Program,
    layout: CairoLayout,
    final_pc: Option<Relocatable>,
    pub program_base: Option<Relocatable>,
    execution_base: Option<Relocatable>,
    entrypoint: Option<usize>,
    initial_ap: Option<Relocatable>,
    initial_fp: Option<Relocatable>,
    initial_pc: Option<Relocatable>,
    run_ended: bool,
    segments_finalized: bool,
    execution_public_memory: Option<Vec<usize>>,
    runner_mode: RunnerMode,
    pub relocated_memory: Vec<Option<Felt252>>,
    pub exec_scopes: ExecutionScopes,
    pub relocated_trace: Option<Vec<RelocatedTraceEntry>>,
}

#[derive(Clone, Debug, PartialEq)]
pub enum RunnerMode {
    ExecutionMode,
    ProofModeCanonical,
    ProofModeCairo1,
}

impl CairoRunner {
    /// The `dynamic_layout_params` argument should only be used with dynamic layout.
    /// It is ignored otherwise.
    pub fn new_v2(
        program: &Program,
        layout: LayoutName,
        dynamic_layout_params: Option<CairoLayoutParams>,
        mode: RunnerMode,
        trace_enabled: bool,
        disable_trace_padding: bool,
    ) -> Result<CairoRunner, RunnerError> {
        let cairo_layout = match layout {
            LayoutName::plain => CairoLayout::plain_instance(),
            LayoutName::small => CairoLayout::small_instance(),
            LayoutName::dex => CairoLayout::dex_instance(),
            LayoutName::recursive => CairoLayout::recursive_instance(),
            LayoutName::starknet => CairoLayout::starknet_instance(),
            LayoutName::starknet_with_keccak => CairoLayout::starknet_with_keccak_instance(),
            LayoutName::recursive_large_output => CairoLayout::recursive_large_output_instance(),
            LayoutName::recursive_with_poseidon => CairoLayout::recursive_with_poseidon(),
            LayoutName::all_cairo => CairoLayout::all_cairo_instance(),
            LayoutName::all_solidity => CairoLayout::all_solidity_instance(),
            LayoutName::dynamic => {
                let params =
                    dynamic_layout_params.ok_or(RunnerError::MissingDynamicLayoutParams)?;

                CairoLayout::dynamic_instance(params)
            }
        };
        Ok(CairoRunner {
            program: program.clone(),
            vm: VirtualMachine::new(trace_enabled, disable_trace_padding),
            layout: cairo_layout,
            final_pc: None,
            program_base: None,
            execution_base: None,
            entrypoint: program.shared_program_data.main,
            initial_ap: None,
            initial_fp: None,
            initial_pc: None,
            run_ended: false,
            segments_finalized: false,
            runner_mode: mode.clone(),
            relocated_memory: Vec::new(),
            exec_scopes: ExecutionScopes::new(),
            execution_public_memory: if mode != RunnerMode::ExecutionMode {
                Some(Vec::new())
            } else {
                None
            },
            relocated_trace: None,
        })
    }

    pub fn new(
        program: &Program,
        layout: LayoutName,
        dynamic_layout_params: Option<CairoLayoutParams>,
        proof_mode: bool,
        trace_enabled: bool,
        disable_trace_padding: bool,
    ) -> Result<CairoRunner, RunnerError> {
        if proof_mode {
            Self::new_v2(
                program,
                layout,
                dynamic_layout_params,
                RunnerMode::ProofModeCanonical,
                trace_enabled,
                disable_trace_padding,
            )
        } else {
            Self::new_v2(
                program,
                layout,
                dynamic_layout_params,
                RunnerMode::ExecutionMode,
                trace_enabled,
                disable_trace_padding,
            )
        }
    }

    pub fn initialize(&mut self, allow_missing_builtins: bool) -> Result<Relocatable, RunnerError> {
        self.initialize_builtins(allow_missing_builtins)?;
        self.initialize_segments(None);
        let end = self.initialize_main_entrypoint()?;
        for builtin_runner in self.vm.builtin_runners.iter_mut() {
            if let BuiltinRunner::Mod(runner) = builtin_runner {
                runner.initialize_zero_segment(&mut self.vm.segments);
            }
        }
        self.initialize_vm()?;
        Ok(end)
    }

    /// Creates the builtin runners according to the builtins used by the program and the selected layout
    /// When running in proof_mode, all builtins in the layout will be created, and only those in the program will be included
    /// When not running in proof_mode, only program builtins will be created and included
    /// Unless `allow_missing_builtins` is set to true, an error will be returned if a builtin is included in the program but not on the layout
    pub fn initialize_builtins(&mut self, allow_missing_builtins: bool) -> Result<(), RunnerError> {
        let builtin_ordered_list = vec![
            BuiltinName::output,
            BuiltinName::pedersen,
            BuiltinName::range_check,
            BuiltinName::ecdsa,
            BuiltinName::bitwise,
            BuiltinName::ec_op,
            BuiltinName::keccak,
            BuiltinName::poseidon,
            BuiltinName::range_check96,
            BuiltinName::add_mod,
            BuiltinName::mul_mod,
        ];
        if !is_subsequence(&self.program.builtins, &builtin_ordered_list) {
            return Err(RunnerError::DisorderedBuiltins);
        };
        let mut program_builtins: HashSet<&BuiltinName> = self.program.builtins.iter().collect();

        if self.layout.builtins.output {
            let included = program_builtins.remove(&BuiltinName::output);
            if included || self.is_proof_mode() {
                self.vm
                    .builtin_runners
                    .push(OutputBuiltinRunner::new(included).into());
            }
        }

        if let Some(instance_def) = self.layout.builtins.pedersen.as_ref() {
            let included = program_builtins.remove(&BuiltinName::pedersen);
            if included || self.is_proof_mode() {
                self.vm
                    .builtin_runners
                    .push(HashBuiltinRunner::new(instance_def.ratio, included).into());
            }
        }

        if let Some(instance_def) = self.layout.builtins.range_check.as_ref() {
            let included = program_builtins.remove(&BuiltinName::range_check);
            if included || self.is_proof_mode() {
                self.vm.builtin_runners.push(
                    RangeCheckBuiltinRunner::<RC_N_PARTS_STANDARD>::new_with_low_ratio(
                        instance_def.ratio,
                        included,
                    )
                    .into(),
                );
            }
        }

        if let Some(instance_def) = self.layout.builtins.ecdsa.as_ref() {
            let included = program_builtins.remove(&BuiltinName::ecdsa);
            if included || self.is_proof_mode() {
                self.vm
                    .builtin_runners
                    .push(SignatureBuiltinRunner::new(instance_def.ratio, included).into());
            }
        }

        if let Some(instance_def) = self.layout.builtins.bitwise.as_ref() {
            let included = program_builtins.remove(&BuiltinName::bitwise);
            if included || self.is_proof_mode() {
                self.vm
                    .builtin_runners
                    .push(BitwiseBuiltinRunner::new(instance_def.ratio, included).into());
            }
        }

        if let Some(instance_def) = self.layout.builtins.ec_op.as_ref() {
            let included = program_builtins.remove(&BuiltinName::ec_op);
            if included || self.is_proof_mode() {
                self.vm
                    .builtin_runners
                    .push(EcOpBuiltinRunner::new(instance_def.ratio, included).into());
            }
        }

        if let Some(instance_def) = self.layout.builtins.keccak.as_ref() {
            let included = program_builtins.remove(&BuiltinName::keccak);
            if included || self.is_proof_mode() {
                self.vm
                    .builtin_runners
                    .push(KeccakBuiltinRunner::new(instance_def.ratio, included).into());
            }
        }

        if let Some(instance_def) = self.layout.builtins.poseidon.as_ref() {
            let included = program_builtins.remove(&BuiltinName::poseidon);
            if included || self.is_proof_mode() {
                self.vm
                    .builtin_runners
                    .push(PoseidonBuiltinRunner::new(instance_def.ratio, included).into());
            }
        }

        if let Some(instance_def) = self.layout.builtins.range_check96.as_ref() {
            let included = program_builtins.remove(&BuiltinName::range_check96);
            if included || self.is_proof_mode() {
                self.vm.builtin_runners.push(
                    RangeCheckBuiltinRunner::<RC_N_PARTS_96>::new_with_low_ratio(
                        instance_def.ratio,
                        included,
                    )
                    .into(),
                );
            }
        }
        if let Some(instance_def) = self.layout.builtins.add_mod.as_ref() {
            let included = program_builtins.remove(&BuiltinName::add_mod);
            if included || self.is_proof_mode() {
                self.vm
                    .builtin_runners
                    .push(ModBuiltinRunner::new_add_mod(instance_def, included).into());
            }
        }
        if let Some(instance_def) = self.layout.builtins.mul_mod.as_ref() {
            let included = program_builtins.remove(&BuiltinName::mul_mod);
            if included || self.is_proof_mode() {
                self.vm
                    .builtin_runners
                    .push(ModBuiltinRunner::new_mul_mod(instance_def, included).into());
            }
        }
        if !program_builtins.is_empty() && !allow_missing_builtins {
            return Err(RunnerError::NoBuiltinForInstance(Box::new((
                program_builtins.iter().map(|n| **n).collect(),
                self.layout.name,
            ))));
        }

        Ok(())
    }

    fn is_proof_mode(&self) -> bool {
        self.runner_mode == RunnerMode::ProofModeCanonical
            || self.runner_mode == RunnerMode::ProofModeCairo1
    }

    // Initialize all program builtins. Values used are the original one from the CairoFunctionRunner
    // Values extracted from here: https://github.com/starkware-libs/cairo-lang/blob/4fb83010ab77aa7ead0c9df4b0c05e030bc70b87/src/starkware/cairo/common/cairo_function_runner.py#L28
    pub fn initialize_program_builtins(&mut self) -> Result<(), RunnerError> {
        fn initialize_builtin(name: BuiltinName, vm: &mut VirtualMachine) {
            match name {
                BuiltinName::pedersen => vm
                    .builtin_runners
                    .push(HashBuiltinRunner::new(Some(32), true).into()),
                BuiltinName::range_check => vm.builtin_runners.push(
                    RangeCheckBuiltinRunner::<RC_N_PARTS_STANDARD>::new(Some(1), true).into(),
                ),
                BuiltinName::output => vm
                    .builtin_runners
                    .push(OutputBuiltinRunner::new(true).into()),
                BuiltinName::ecdsa => vm
                    .builtin_runners
                    .push(SignatureBuiltinRunner::new(Some(1), true).into()),
                BuiltinName::bitwise => vm
                    .builtin_runners
                    .push(BitwiseBuiltinRunner::new(Some(1), true).into()),
                BuiltinName::ec_op => vm
                    .builtin_runners
                    .push(EcOpBuiltinRunner::new(Some(1), true).into()),
                BuiltinName::keccak => vm
                    .builtin_runners
                    .push(KeccakBuiltinRunner::new(Some(1), true).into()),
                BuiltinName::poseidon => vm
                    .builtin_runners
                    .push(PoseidonBuiltinRunner::new(Some(1), true).into()),
                BuiltinName::segment_arena => vm
                    .builtin_runners
                    .push(SegmentArenaBuiltinRunner::new(true).into()),
                BuiltinName::range_check96 => vm
                    .builtin_runners
                    .push(RangeCheckBuiltinRunner::<RC_N_PARTS_96>::new(Some(1), true).into()),
                BuiltinName::add_mod => vm.builtin_runners.push(
                    ModBuiltinRunner::new_add_mod(&ModInstanceDef::new(Some(1), 1, 96), true)
                        .into(),
                ),
                BuiltinName::mul_mod => vm.builtin_runners.push(
                    ModBuiltinRunner::new_mul_mod(&ModInstanceDef::new(Some(1), 1, 96), true)
                        .into(),
                ),
            }
        }

        for builtin_name in &self.program.builtins {
            initialize_builtin(*builtin_name, &mut self.vm);
        }
        Ok(())
    }

    ///Creates the necessary segments for the program, execution, and each builtin on the MemorySegmentManager and stores the first adress of each of this new segments as each owner's base
    pub fn initialize_segments(&mut self, program_base: Option<Relocatable>) {
        self.program_base = match program_base {
            Some(base) => Some(base),
            None => Some(self.vm.add_memory_segment()),
        };
        self.execution_base = Some(self.vm.add_memory_segment());
        for builtin_runner in self.vm.builtin_runners.iter_mut() {
            builtin_runner.initialize_segments(&mut self.vm.segments);
        }
    }

    fn initialize_state(
        &mut self,
        entrypoint: usize,
        stack: Vec<MaybeRelocatable>,
    ) -> Result<(), RunnerError> {
        let prog_base = self.program_base.ok_or(RunnerError::NoProgBase)?;
        let exec_base = self.execution_base.ok_or(RunnerError::NoExecBase)?;
        self.initial_pc = Some((prog_base + entrypoint)?);
        self.vm
            .load_data(prog_base, &self.program.shared_program_data.data)
            .map_err(RunnerError::MemoryInitializationError)?;

        // Mark all addresses from the program segment as accessed
        for i in 0..self.program.shared_program_data.data.len() {
            self.vm.segments.memory.mark_as_accessed((prog_base + i)?);
        }
        self.vm
            .segments
            .load_data(exec_base, &stack)
            .map_err(RunnerError::MemoryInitializationError)?;
        Ok(())
    }

    pub fn initialize_function_entrypoint(
        &mut self,
        entrypoint: usize,
        mut stack: Vec<MaybeRelocatable>,
        return_fp: MaybeRelocatable,
    ) -> Result<Relocatable, RunnerError> {
        let end = self.vm.add_memory_segment();
        stack.append(&mut vec![
            return_fp,
            MaybeRelocatable::RelocatableValue(end),
        ]);
        if let Some(base) = &self.execution_base {
            self.initial_fp = Some(Relocatable {
                segment_index: base.segment_index,
                offset: base.offset + stack.len(),
            });
            self.initial_ap = self.initial_fp;
        } else {
            return Err(RunnerError::NoExecBase);
        }
        self.initialize_state(entrypoint, stack)?;
        self.final_pc = Some(end);
        Ok(end)
    }

    ///Initializes state for running a program from the main() entrypoint.
    ///If self.is_proof_mode() == True, the execution starts from the start label rather then the main() function.
    ///Returns the value of the program counter after returning from main.
    fn initialize_main_entrypoint(&mut self) -> Result<Relocatable, RunnerError> {
        let mut stack = Vec::new();
        {
            let builtin_runners = self
                .vm
                .builtin_runners
                .iter()
                .map(|b| (b.name(), b))
                .collect::<HashMap<_, _>>();
            for builtin_name in &self.program.builtins {
                if let Some(builtin_runner) = builtin_runners.get(builtin_name) {
                    stack.append(&mut builtin_runner.initial_stack());
                } else {
                    stack.push(Felt252::ZERO.into())
                }
            }
        }

        if self.is_proof_mode() {
            // In canonical proof mode, add the dummy last fp and pc to the public memory, so that the verifier can enforce

            // canonical offset should be 2 for Cairo 0
            let mut target_offset = 2;

            // Cairo1 is not adding data to check [fp - 2] = fp, and has a different initialization of the stack. This should be updated.
            // Cairo0 remains canonical

            if matches!(self.runner_mode, RunnerMode::ProofModeCairo1) {
                target_offset = stack.len() + 2;

                // This values shouldn't be needed with a canonical proof mode
                let return_fp = self.vm.add_memory_segment();
                let end = self.vm.add_memory_segment();
                stack.append(&mut vec![
                    MaybeRelocatable::RelocatableValue(return_fp),
                    MaybeRelocatable::RelocatableValue(end),
                ]);

                self.initialize_state(
                    self.program
                        .shared_program_data
                        .start
                        .ok_or(RunnerError::NoProgramStart)?,
                    stack,
                )?;
            } else {
                let mut stack_prefix = vec![
                    Into::<MaybeRelocatable>::into(
                        (self.execution_base.ok_or(RunnerError::NoExecBase)? + target_offset)?,
                    ),
                    MaybeRelocatable::from(Felt252::zero()),
                ];
                stack_prefix.extend(stack.clone());

                self.execution_public_memory = Some(Vec::from_iter(0..stack_prefix.len()));

                self.initialize_state(
                    self.program
                        .shared_program_data
                        .start
                        .ok_or(RunnerError::NoProgramStart)?,
                    stack_prefix.clone(),
                )?;
            }

            self.initial_fp =
                Some((self.execution_base.ok_or(RunnerError::NoExecBase)? + target_offset)?);

            self.initial_ap = self.initial_fp;
            return Ok((self.program_base.ok_or(RunnerError::NoProgBase)?
                + self
                    .program
                    .shared_program_data
                    .end
                    .ok_or(RunnerError::NoProgramEnd)?)?);
        }

        let return_fp = self.vm.add_memory_segment();
        if let Some(main) = &self.entrypoint {
            let main_clone = *main;
            Ok(self.initialize_function_entrypoint(
                main_clone,
                stack,
                MaybeRelocatable::RelocatableValue(return_fp),
            )?)
        } else {
            Err(RunnerError::MissingMain)
        }
    }

    pub fn initialize_vm(&mut self) -> Result<(), RunnerError> {
        self.vm.run_context.pc = *self.initial_pc.as_ref().ok_or(RunnerError::NoPC)?;
        self.vm.run_context.ap = self.initial_ap.as_ref().ok_or(RunnerError::NoAP)?.offset;
        self.vm.run_context.fp = self.initial_fp.as_ref().ok_or(RunnerError::NoFP)?.offset;
        for builtin in self.vm.builtin_runners.iter() {
            builtin.add_validation_rule(&mut self.vm.segments.memory);
        }

        self.vm
            .segments
            .memory
            .validate_existing_memory()
            .map_err(RunnerError::MemoryValidationError)
    }

    pub fn get_initial_fp(&self) -> Option<Relocatable> {
        self.initial_fp
    }

    /// Gets the data used by the HintProcessor to execute each hint
    pub fn get_hint_data(
        &self,
        references: &[HintReference],
        hint_executor: &mut dyn HintProcessor,
    ) -> Result<Vec<Box<dyn Any>>, VirtualMachineError> {
        self.program
            .shared_program_data
            .hints_collection
            .iter_hints()
            .map(|hint| {
                hint_executor
                    .compile_hint(
                        &hint.code,
                        &hint.flow_tracking_data.ap_tracking,
                        &hint.flow_tracking_data.reference_ids,
                        references,
                    )
                    .map_err(|_| VirtualMachineError::CompileHintFail(hint.code.clone().into()))
            })
            .collect()
    }

    pub fn get_constants(&self) -> &HashMap<String, Felt252> {
        &self.program.constants
    }

    pub fn get_program_builtins(&self) -> &Vec<BuiltinName> {
        &self.program.builtins
    }

    pub fn run_until_pc(
        &mut self,
        address: Relocatable,
        hint_processor: &mut dyn HintProcessor,
    ) -> Result<(), VirtualMachineError> {
        let references = &self.program.shared_program_data.reference_manager;
        #[cfg(not(feature = "extensive_hints"))]
        let hint_data = self.get_hint_data(references, hint_processor)?;
        #[cfg(feature = "extensive_hints")]
        let mut hint_data = self.get_hint_data(references, hint_processor)?;
        #[cfg(feature = "extensive_hints")]
        let mut hint_ranges = self
            .program
            .shared_program_data
            .hints_collection
            .hints_ranges
            .clone();
        #[cfg(feature = "test_utils")]
        self.vm.execute_before_first_step(&hint_data)?;
        while self.vm.get_pc() != address && !hint_processor.consumed() {
            self.vm.step(
                hint_processor,
                &mut self.exec_scopes,
                #[cfg(feature = "extensive_hints")]
                &mut hint_data,
                #[cfg(not(feature = "extensive_hints"))]
                self.program
                    .shared_program_data
                    .hints_collection
                    .get_hint_range_for_pc(self.vm.get_pc().offset)
                    .and_then(|range| {
                        range.and_then(|(start, length)| hint_data.get(start..start + length.get()))
                    })
                    .unwrap_or(&[]),
                #[cfg(feature = "extensive_hints")]
                &mut hint_ranges,
                &self.program.constants,
            )?;

            hint_processor.consume_step();
        }

        if self.vm.get_pc() != address {
            return Err(VirtualMachineError::UnfinishedExecution);
        }

        Ok(())
    }

    /// Execute an exact number of steps on the program from the actual position.
    pub fn run_for_steps(
        &mut self,
        steps: usize,
        hint_processor: &mut dyn HintProcessor,
    ) -> Result<(), VirtualMachineError> {
        let references = &self.program.shared_program_data.reference_manager;
        #[cfg(not(feature = "extensive_hints"))]
        let hint_data = self.get_hint_data(references, hint_processor)?;
        #[cfg(feature = "extensive_hints")]
        let mut hint_data = self.get_hint_data(references, hint_processor)?;
        #[cfg(feature = "extensive_hints")]
        let mut hint_ranges = self
            .program
            .shared_program_data
            .hints_collection
            .hints_ranges
            .clone();
        #[cfg(not(feature = "extensive_hints"))]
        let hint_data = &self
            .program
            .shared_program_data
            .hints_collection
            .get_hint_range_for_pc(self.vm.get_pc().offset)
            .and_then(|range| {
                range.and_then(|(start, length)| hint_data.get(start..start + length.get()))
            })
            .unwrap_or(&[]);

        for remaining_steps in (1..=steps).rev() {
            if self.final_pc.as_ref() == Some(&self.vm.get_pc()) {
                return Err(VirtualMachineError::EndOfProgram(remaining_steps));
            }

            self.vm.step(
                hint_processor,
                &mut self.exec_scopes,
                #[cfg(feature = "extensive_hints")]
                &mut hint_data,
                #[cfg(not(feature = "extensive_hints"))]
                hint_data,
                #[cfg(feature = "extensive_hints")]
                &mut hint_ranges,
                &self.program.constants,
            )?;
        }

        Ok(())
    }

    /// Execute steps until a number of steps since the start of the program is reached.
    pub fn run_until_steps(
        &mut self,
        steps: usize,
        hint_processor: &mut dyn HintProcessor,
    ) -> Result<(), VirtualMachineError> {
        self.run_for_steps(steps.saturating_sub(self.vm.current_step), hint_processor)
    }

    /// Execute steps until the step counter reaches a power of two.
    pub fn run_until_next_power_of_2(
        &mut self,
        hint_processor: &mut dyn HintProcessor,
    ) -> Result<(), VirtualMachineError> {
        self.run_until_steps(self.vm.current_step.next_power_of_two(), hint_processor)
    }

    pub fn get_perm_range_check_limits(&self) -> Option<(isize, isize)> {
        let runner_usages = self
            .vm
            .builtin_runners
            .iter()
            .filter_map(|runner| runner.get_range_check_usage(&self.vm.segments.memory))
            .map(|(rc_min, rc_max)| (rc_min as isize, rc_max as isize));
        let rc_bounds = self.vm.rc_limits.iter().copied().chain(runner_usages);
        rc_bounds.reduce(|(min1, max1), (min2, max2)| (min1.min(min2), max1.max(max2)))
    }

    /// Checks that there are enough trace cells to fill the entire range check
    /// range.
    pub fn check_range_check_usage(&self) -> Result<(), VirtualMachineError> {
        let Some((rc_min, rc_max)) = self.get_perm_range_check_limits() else {
            return Ok(());
        };

        let rc_units_used_by_builtins: usize = self
            .vm
            .builtin_runners
            .iter()
            .map(|runner| runner.get_used_perm_range_check_units(&self.vm))
            .sum::<Result<usize, MemoryError>>()
            .map_err(Into::<VirtualMachineError>::into)?;

        let unused_rc_units =
            (self.layout.rc_units as usize - 3) * self.vm.current_step - rc_units_used_by_builtins;
        if unused_rc_units < (rc_max - rc_min) as usize {
            return Err(MemoryError::InsufficientAllocatedCells(
                InsufficientAllocatedCellsError::RangeCheckUnits(Box::new((
                    unused_rc_units,
                    (rc_max - rc_min) as usize,
                ))),
            )
            .into());
        }

        Ok(())
    }

    /// Count the number of holes present in the segments.
    pub fn get_memory_holes(&self) -> Result<usize, MemoryError> {
        // Grab builtin segment indexes, except for the output builtin
        let builtin_segment_indexes: HashSet<usize> = self
            .vm
            .builtin_runners
            .iter()
            .filter(|b| b.name() != BuiltinName::output)
            .map(|b| b.base())
            .collect();

        self.vm.segments.get_memory_holes(builtin_segment_indexes)
    }

    /// Check if there are enough trace cells to fill the entire diluted checks.
    pub fn check_diluted_check_usage(&self) -> Result<(), VirtualMachineError> {
        let diluted_pool_instance = match &self.layout.diluted_pool_instance_def {
            Some(x) => x,
            None => return Ok(()),
        };

        let mut used_units_by_builtins = 0;
        for builtin_runner in &self.vm.builtin_runners {
            let used_units = builtin_runner.get_used_diluted_check_units(
                diluted_pool_instance.spacing,
                diluted_pool_instance.n_bits,
            );

            let multiplier = builtin_runner.get_allocated_instances(&self.vm)?;

            used_units_by_builtins += used_units * multiplier;
        }

        let diluted_units = if !diluted_pool_instance.fractional_units_per_step {
            diluted_pool_instance.units_per_step as usize * self.vm.current_step
        } else {
            safe_div_usize(
                self.vm.current_step,
                diluted_pool_instance.units_per_step as usize,
            )?
        };

        let unused_diluted_units = diluted_units.saturating_sub(used_units_by_builtins);

        let diluted_usage_upper_bound = 1usize << diluted_pool_instance.n_bits;
        if unused_diluted_units < diluted_usage_upper_bound {
            return Err(MemoryError::InsufficientAllocatedCells(
                InsufficientAllocatedCellsError::DilutedCells(Box::new((
                    unused_diluted_units,
                    diluted_usage_upper_bound,
                ))),
            )
            .into());
        }

        Ok(())
    }

    pub fn end_run(
        &mut self,
        disable_trace_padding: bool,
        disable_finalize_all: bool,
        hint_processor: &mut dyn HintProcessor,
    ) -> Result<(), VirtualMachineError> {
        if self.run_ended {
            return Err(RunnerError::EndRunCalledTwice.into());
        }

        self.vm.segments.memory.relocate_memory()?;
        self.vm.end_run(&self.exec_scopes)?;

        if disable_finalize_all {
            return Ok(());
        }

        self.vm.segments.compute_effective_sizes();
        if self.is_proof_mode() && !disable_trace_padding {
            self.run_until_next_power_of_2(hint_processor)?;
            loop {
                match self.check_used_cells() {
                    Ok(_) => break,
                    Err(e) => match e {
                        VirtualMachineError::Memory(MemoryError::InsufficientAllocatedCells(_)) => {
                        }
                        e => return Err(e),
                    },
                }

                self.run_for_steps(1, hint_processor)?;
                self.run_until_next_power_of_2(hint_processor)?;
            }
        }

        self.run_ended = true;
        Ok(())
    }

    ///Relocates the VM's trace, turning relocatable registers to numbered ones
    pub fn relocate_trace(&mut self, relocation_table: &[usize]) -> Result<(), TraceError> {
        if self.relocated_trace.is_some() {
            return Err(TraceError::AlreadyRelocated);
        }

        let trace = self
            .vm
            .trace
            .as_ref()
            .ok_or(TraceError::TraceNotEnabled)?
            .iter();
        let mut relocated_trace = Vec::<RelocatedTraceEntry>::with_capacity(trace.len());
        let segment_1_base = relocation_table
            .get(1)
            .ok_or(TraceError::NoRelocationFound)?;

        for entry in trace {
            relocated_trace.push(RelocatedTraceEntry {
                pc: relocate_trace_register(entry.pc, relocation_table)?,
                ap: entry.ap + segment_1_base,
                fp: entry.fp + segment_1_base,
            })
        }
        self.relocated_trace = Some(relocated_trace);
        Ok(())
    }

    /// Relocates the VM's memory, turning bidimensional indexes into contiguous numbers, and values
    /// into Felt252s. Uses the relocation_table to asign each index a number according to the value
    /// on its segment number.
    fn relocate_memory(&mut self, relocation_table: &[usize]) -> Result<(), MemoryError> {
        if !(self.relocated_memory.is_empty()) {
            return Err(MemoryError::Relocation);
        }
        //Relocated addresses start at 1
        self.relocated_memory.push(None);
        for (index, segment) in self.vm.segments.memory.data.iter().enumerate() {
            for (seg_offset, cell) in segment.iter().enumerate() {
                match cell.get_value() {
                    Some(cell) => {
                        let relocated_addr = relocate_address(
                            Relocatable::from((index as isize, seg_offset)),
                            relocation_table,
                        )?;
                        let value = relocate_value(cell, relocation_table)?;
                        if self.relocated_memory.len() <= relocated_addr {
                            self.relocated_memory.resize(relocated_addr + 1, None);
                        }
                        self.relocated_memory[relocated_addr] = Some(value);
                    }
                    None => self.relocated_memory.push(None),
                }
            }
        }
        Ok(())
    }

    pub fn relocate(&mut self, relocate_mem: bool) -> Result<(), TraceError> {
        self.vm.segments.compute_effective_sizes();
        if !relocate_mem && self.vm.trace.is_none() {
            return Ok(());
        }
        // relocate_segments can fail if compute_effective_sizes is not called before.
        // The expect should be unreachable.
        let relocation_table = self
            .vm
            .segments
            .relocate_segments()
            .expect("compute_effective_sizes called but relocate_memory still returned error");

        if relocate_mem {
            if let Err(memory_error) = self.relocate_memory(&relocation_table) {
                return Err(TraceError::MemoryError(memory_error));
            }
        }
        if self.vm.trace.is_some() {
            self.relocate_trace(&relocation_table)?;
        }
        self.vm.relocation_table = Some(relocation_table);
        Ok(())
    }

    // Returns a map from builtin base's segment index to stop_ptr offset
    // Aka the builtin's segment number and its maximum offset
    pub fn get_builtin_segments_info(&self) -> Result<Vec<(usize, usize)>, RunnerError> {
        let mut builtin_segment_info = Vec::new();

        for builtin in &self.vm.builtin_runners {
            let (index, stop_ptr) = builtin.get_memory_segment_addresses();

            builtin_segment_info.push((
                index,
                stop_ptr.ok_or_else(|| RunnerError::NoStopPointer(Box::new(builtin.name())))?,
            ));
        }

        Ok(builtin_segment_info)
    }

    // Returns a map from builtin's name wihout the "_builtin" suffix to its base's segment index and stop_ptr offset
    // Aka the builtin's segment number and its maximum offset
    pub fn get_builtin_segment_info_for_pie(
        &self,
    ) -> Result<HashMap<BuiltinName, cairo_pie::SegmentInfo>, RunnerError> {
        let mut builtin_segment_info = HashMap::new();

        for builtin in &self.vm.builtin_runners {
            let (index, stop_ptr) = builtin.get_memory_segment_addresses();

            builtin_segment_info.insert(
                builtin.name(),
                (
                    index as isize,
                    stop_ptr.ok_or_else(|| RunnerError::NoStopPointer(Box::new(builtin.name())))?,
                )
                    .into(),
            );
        }

        Ok(builtin_segment_info)
    }

    pub fn get_execution_resources(&self) -> Result<ExecutionResources, RunnerError> {
        let n_steps = self
            .vm
            .trace
            .as_ref()
            .map(|x| x.len())
            .unwrap_or(self.vm.current_step);
        let n_memory_holes = self.get_memory_holes()?;

        let mut builtin_instance_counter = HashMap::new();
        for builtin_runner in &self.vm.builtin_runners {
            builtin_instance_counter.insert(
                builtin_runner.name(),
                builtin_runner.get_used_instances(&self.vm.segments)?,
            );
        }

        Ok(ExecutionResources {
            n_steps,
            n_memory_holes,
            builtin_instance_counter,
        })
    }

    // Finalizes the segments.
    //     Note:
    //     1.  end_run() must precede a call to this method.
    //     2.  Call read_return_values() *before* finalize_segments(), otherwise the return values
    //         will not be included in the public memory.
    pub fn finalize_segments(&mut self) -> Result<(), RunnerError> {
        if self.segments_finalized {
            return Ok(());
        }
        if !self.run_ended {
            return Err(RunnerError::FinalizeNoEndRun);
        }
        let size = self.program.shared_program_data.data.len();
        let mut public_memory = Vec::with_capacity(size);
        for i in 0..size {
            public_memory.push((i, 0_usize))
        }
        self.vm.segments.finalize(
            Some(size),
            self.program_base
                .as_ref()
                .ok_or(RunnerError::NoProgBase)?
                .segment_index as usize,
            Some(&public_memory),
        );
        let mut public_memory = Vec::with_capacity(size);
        let exec_base = self
            .execution_base
            .as_ref()
            .ok_or(RunnerError::NoExecBase)?;
        for elem in self
            .execution_public_memory
            .as_ref()
            .ok_or(RunnerError::FinalizeSegmentsNoProofMode)?
            .iter()
        {
            public_memory.push((elem + exec_base.offset, 0))
        }
        self.vm
            .segments
            .finalize(None, exec_base.segment_index as usize, Some(&public_memory));
        for builtin_runner in self.vm.builtin_runners.iter() {
            let (_, size) = builtin_runner
                .get_used_cells_and_allocated_size(&self.vm)
                .map_err(RunnerError::FinalizeSegements)?;
            if let BuiltinRunner::Output(output_builtin) = builtin_runner {
                let public_memory = output_builtin.get_public_memory(&self.vm.segments)?;
                self.vm
                    .segments
                    .finalize(Some(size), builtin_runner.base(), Some(&public_memory))
            } else {
                self.vm
                    .segments
                    .finalize(Some(size), builtin_runner.base(), None)
            }
        }
        self.vm.segments.finalize_zero_segment();
        self.segments_finalized = true;
        Ok(())
    }

    /// Runs a cairo program from a give entrypoint, indicated by its pc offset, with the given arguments.
    /// If `verify_secure` is set to true, [verify_secure_runner] will be called to run extra verifications.
    /// `program_segment_size` is only used by the [verify_secure_runner] function and will be ignored if `verify_secure` is set to false.
    pub fn run_from_entrypoint(
        &mut self,
        entrypoint: usize,
        args: &[&CairoArg],
        verify_secure: bool,
        program_segment_size: Option<usize>,
        hint_processor: &mut dyn HintProcessor,
    ) -> Result<(), CairoRunError> {
        let stack = args
            .iter()
            .map(|arg| self.vm.segments.gen_cairo_arg(arg))
            .collect::<Result<Vec<MaybeRelocatable>, VirtualMachineError>>()?;
        let return_fp = MaybeRelocatable::from(0);
        let end = self.initialize_function_entrypoint(entrypoint, stack, return_fp)?;

        self.initialize_vm()?;

        self.run_until_pc(end, hint_processor)
            .map_err(|err| VmException::from_vm_error(self, err))?;
        self.end_run(true, false, hint_processor)?;

        if verify_secure {
            verify_secure_runner(self, false, program_segment_size)?;
        }

        Ok(())
    }

    // Returns Ok(()) if there are enough allocated cells for the builtins.
    // If not, the number of steps should be increased or a different layout should be used.
    pub fn check_used_cells(&self) -> Result<(), VirtualMachineError> {
        self.vm
            .builtin_runners
            .iter()
            .map(|builtin_runner| builtin_runner.get_used_cells_and_allocated_size(&self.vm))
            .collect::<Result<Vec<(usize, usize)>, MemoryError>>()?;
        self.check_range_check_usage()?;
        self.check_memory_usage()?;
        self.check_diluted_check_usage()?;
        Ok(())
    }

    // Checks that there are enough trace cells to fill the entire memory range.
    pub fn check_memory_usage(&self) -> Result<(), VirtualMachineError> {
        let instance = &self.layout;

        let builtins_memory_units: usize = self
            .vm
            .builtin_runners
            .iter()
            .map(|builtin_runner| builtin_runner.get_allocated_memory_units(&self.vm))
            .collect::<Result<Vec<usize>, MemoryError>>()?
            .iter()
            .sum();

        let builtins_memory_units = builtins_memory_units as u32;

        let vm_current_step_u32 = self.vm.current_step as u32;

        // Out of the memory units available per step, a fraction is used for public memory, and
        // four are used for the instruction.
        let total_memory_units = instance.memory_units_per_step * vm_current_step_u32;
        let (public_memory_units, rem) =
            div_rem(total_memory_units, instance.public_memory_fraction);
        if rem != 0 {
            return Err(MathError::SafeDivFailU32(
                total_memory_units,
                instance.public_memory_fraction,
            )
            .into());
        }

        let instruction_memory_units = 4 * vm_current_step_u32;

        let unused_memory_units = total_memory_units
            - (public_memory_units + instruction_memory_units + builtins_memory_units);
        let memory_address_holes = self.get_memory_holes()?;
        if unused_memory_units < memory_address_holes as u32 {
            Err(MemoryError::InsufficientAllocatedCells(
                InsufficientAllocatedCellsError::MemoryAddresses(Box::new((
                    unused_memory_units,
                    memory_address_holes,
                ))),
            ))?
        }
        Ok(())
    }

    /// Intitializes the runner in order to run cairo 1 contract entrypoints
    /// Swaps the program's builtins field with program_builtins
    /// Initializes program builtins & segments
    pub fn initialize_function_runner_cairo_1(
        &mut self,
        program_builtins: &[BuiltinName],
    ) -> Result<(), RunnerError> {
        self.program.builtins = program_builtins.to_vec();
        self.initialize_program_builtins()?;
        self.initialize_segments(self.program_base);
        Ok(())
    }

    /// Intitializes the runner in order to run cairo 0 contract entrypoints
    /// Initializes program builtins & segments
    pub fn initialize_function_runner(&mut self) -> Result<(), RunnerError> {
        self.initialize_program_builtins()?;
        self.initialize_segments(self.program_base);
        Ok(())
    }

    /// Overrides the previous entrypoint with a custom one, or "main" if none
    /// is specified.
    pub fn set_entrypoint(&mut self, new_entrypoint: Option<&str>) -> Result<(), ProgramError> {
        let new_entrypoint = new_entrypoint.unwrap_or("main");
        self.entrypoint = Some(
            self.program
                .shared_program_data
                .identifiers
                .get(&format!("__main__.{new_entrypoint}"))
                .and_then(|x| x.pc)
                .ok_or_else(|| ProgramError::EntrypointNotFound(new_entrypoint.to_string()))?,
        );

        Ok(())
    }

    pub fn read_return_values(&mut self, allow_missing_builtins: bool) -> Result<(), RunnerError> {
        if !self.run_ended {
            return Err(RunnerError::ReadReturnValuesNoEndRun);
        }
        let mut pointer = self.vm.get_ap();
        for builtin_name in self.program.builtins.iter().rev() {
            if let Some(builtin_runner) = self
                .vm
                .builtin_runners
                .iter_mut()
                .find(|b| b.name() == *builtin_name)
            {
                let new_pointer = builtin_runner.final_stack(&self.vm.segments, pointer)?;
                pointer = new_pointer;
            } else {
                if !allow_missing_builtins {
                    return Err(RunnerError::MissingBuiltin(*builtin_name));
                }
                pointer.offset = pointer.offset.saturating_sub(1);

                if !self.vm.get_integer(pointer)?.is_zero() {
                    return Err(RunnerError::MissingBuiltinStopPtrNotZero(*builtin_name));
                }
            }
        }
        if self.segments_finalized {
            return Err(RunnerError::FailedAddingReturnValues);
        }
        if self.is_proof_mode() {
            let exec_base = *self
                .execution_base
                .as_ref()
                .ok_or(RunnerError::NoExecBase)?;
            let begin = pointer.offset - exec_base.offset;
            let ap = self.vm.get_ap();
            let end = ap.offset - exec_base.offset;
            self.execution_public_memory
                .as_mut()
                .ok_or(RunnerError::NoExecPublicMemory)?
                .extend(begin..end);
        }
        Ok(())
    }

    // Iterates over the program builtins in reverse, calling BuiltinRunner::final_stack on each of them and returns the final pointer
    // This method is used by cairo-vm-py to replace starknet functionality
    pub fn get_builtins_final_stack(
        &mut self,
        stack_ptr: Relocatable,
    ) -> Result<Relocatable, RunnerError> {
        let mut stack_ptr = Relocatable::from(&stack_ptr);
        for runner in self
            .vm
            .builtin_runners
            .iter_mut()
            .rev()
            .filter(|builtin_runner| {
                self.program
                    .builtins
                    .iter()
                    .any(|bn| *bn == builtin_runner.name())
            })
        {
            stack_ptr = runner.final_stack(&self.vm.segments, stack_ptr)?
        }
        Ok(stack_ptr)
    }

    /// Return CairoRunner.program
    pub fn get_program(&self) -> &Program {
        &self.program
    }

    // Constructs and returns a CairoPie representing the current VM run.
    pub fn get_cairo_pie(&self) -> Result<CairoPie, RunnerError> {
        let program_base = self.program_base.ok_or(RunnerError::NoProgBase)?;
        let execution_base = self.execution_base.ok_or(RunnerError::NoExecBase)?;

        let builtin_segments = self.get_builtin_segment_info_for_pie()?;
        let mut known_segment_indices = HashSet::new();
        for info in builtin_segments.values() {
            known_segment_indices.insert(info.index);
        }
        let n_used_builtins = self.program.builtins_len();
        let return_fp_addr = (execution_base + n_used_builtins)?;
        let return_fp = self.vm.get_relocatable(return_fp_addr)?;
        let return_pc = self.vm.get_relocatable((return_fp_addr + 1)?)?;

        if let None | Some(false) = return_fp
            .segment_index
            .to_usize()
            .and_then(|u| self.vm.get_segment_size(u))
            .map(|u| u.is_zero())
        {
            // return_fp negative index / no size / size is zero
            return Err(RunnerError::UnexpectedRetFpSegmentSize);
        }

        if let None | Some(false) = return_pc
            .segment_index
            .to_usize()
            .and_then(|u| self.vm.get_segment_size(u))
            .map(|u| u.is_zero())
        {
            // return_pc negative index / no size / size is zero
            return Err(RunnerError::UnexpectedRetPcSegmentSize);
        }

        if program_base.offset != 0 {
            return Err(RunnerError::ProgramBaseOffsetNotZero);
        }
        known_segment_indices.insert(program_base.segment_index);

        if execution_base.offset != 0 {
            return Err(RunnerError::ExecBaseOffsetNotZero);
        }
        known_segment_indices.insert(execution_base.segment_index);

        if return_fp.offset != 0 {
            return Err(RunnerError::RetFpOffsetNotZero);
        }
        known_segment_indices.insert(return_fp.segment_index);

        if return_pc.offset != 0 {
            return Err(RunnerError::RetPcOffsetNotZero);
        }
        known_segment_indices.insert(return_pc.segment_index);

        // Put all the remaining segments in extra_segments.
        let mut extra_segments = Vec::default();
        for index in 0..self.vm.segments.num_segments() {
            if !known_segment_indices.contains(&(index as isize)) {
                extra_segments.push(
                    (
                        index as isize,
                        self.vm
                            .get_segment_size(index)
                            .ok_or(MemoryError::MissingSegmentUsedSizes)?,
                    )
                        .into(),
                );
            }
        }

        let execution_size = (self.vm.get_ap() - execution_base)?;
        let metadata = CairoPieMetadata {
            program: self
                .get_program()
                .get_stripped_program()
                .map_err(|_| RunnerError::StrippedProgramNoMain)?,
            program_segment: (program_base.segment_index, self.program.data_len()).into(),
            execution_segment: (execution_base.segment_index, execution_size).into(),
            ret_fp_segment: (return_fp.segment_index, 0).into(),
            ret_pc_segment: (return_pc.segment_index, 0).into(),
            builtin_segments,
            extra_segments,
        };

        Ok(CairoPie {
            metadata,
            memory: (&self.vm.segments.memory).into(),
            execution_resources: self.get_execution_resources()?,
            additional_data: CairoPieAdditionalData(
                self.vm
                    .builtin_runners
                    .iter()
                    .map(|b| (b.name(), b.get_additional_data()))
                    .collect(),
            ),
            version: CairoPieVersion { cairo_pie: () },
        })
    }

    pub fn get_air_public_input(&self) -> Result<PublicInput, PublicInputError> {
        PublicInput::new(
            &self.relocated_memory,
            self.layout.name.to_str(),
            &self.vm.get_public_memory_addresses()?,
            self.get_memory_segment_addresses()?,
            self.relocated_trace
                .as_ref()
                .ok_or(PublicInputError::EmptyTrace)?,
            self.get_perm_range_check_limits()
                .ok_or(PublicInputError::NoRangeCheckLimits)?,
        )
    }

    pub fn get_air_private_input(&self) -> AirPrivateInput {
        let mut private_inputs = HashMap::new();
        for builtin in self.vm.builtin_runners.iter() {
            private_inputs.insert(builtin.name(), builtin.air_private_input(&self.vm.segments));
        }
        AirPrivateInput(private_inputs)
    }

    pub fn get_memory_segment_addresses(
        &self,
    ) -> Result<HashMap<&'static str, (usize, usize)>, VirtualMachineError> {
        let relocation_table = self
            .vm
            .relocation_table
            .as_ref()
            .ok_or(MemoryError::UnrelocatedMemory)?;

        let relocate = |segment: (usize, usize)| -> Result<(usize, usize), VirtualMachineError> {
            let (index, stop_ptr_offset) = segment;
            let base = relocation_table
                .get(index)
                .ok_or(VirtualMachineError::RelocationNotFound(index))?;
            Ok((*base, base + stop_ptr_offset))
        };

        self.vm
            .builtin_runners
            .iter()
            .map(|builtin| -> Result<_, VirtualMachineError> {
                let (base, stop_ptr) = builtin.get_memory_segment_addresses();
                let stop_ptr = if self.program.builtins.contains(&builtin.name()) {
                    stop_ptr.ok_or_else(|| RunnerError::NoStopPointer(Box::new(builtin.name())))?
                } else {
                    stop_ptr.unwrap_or_default()
                };

                Ok((builtin.name().to_str(), relocate((base, stop_ptr))?))
            })
            .collect()
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SegmentInfo {
    pub index: isize,
    pub size: usize,
}

//* ----------------------
//*   ExecutionResources
//* ----------------------

#[derive(Clone, Debug, Default, Deserialize, Serialize, Eq, PartialEq)]
pub struct ExecutionResources {
    pub n_steps: usize,
    pub n_memory_holes: usize,
    #[serde(with = "crate::types::builtin_name::serde_generic_map_impl")]
    pub builtin_instance_counter: HashMap<BuiltinName, usize>,
}

/// Returns a copy of the execution resources where all the builtins with a usage counter
/// of 0 are omitted.
impl ExecutionResources {
    pub fn filter_unused_builtins(&self) -> ExecutionResources {
        ExecutionResources {
            n_steps: self.n_steps,
            n_memory_holes: self.n_memory_holes,
            builtin_instance_counter: self
                .clone()
                .builtin_instance_counter
                .into_iter()
                .filter(|builtin| !builtin.1.is_zero())
                .collect(),
        }
    }
}

impl Add<&ExecutionResources> for &ExecutionResources {
    type Output = ExecutionResources;

    fn add(self, rhs: &ExecutionResources) -> ExecutionResources {
        let mut new = self.clone();
        new.add_assign(rhs);
        new
    }
}

impl AddAssign<&ExecutionResources> for ExecutionResources {
    fn add_assign(&mut self, rhs: &ExecutionResources) {
        self.n_steps += rhs.n_steps;
        self.n_memory_holes += rhs.n_memory_holes;
        for (k, v) in rhs.builtin_instance_counter.iter() {
            // FIXME: remove k's clone, use &'static str
            *self.builtin_instance_counter.entry(*k).or_insert(0) += v;
        }
    }
}

impl Sub<&ExecutionResources> for &ExecutionResources {
    type Output = ExecutionResources;

    fn sub(self, rhs: &ExecutionResources) -> ExecutionResources {
        let mut new = self.clone();
        new.sub_assign(rhs);
        new
    }
}

impl SubAssign<&ExecutionResources> for ExecutionResources {
    fn sub_assign(&mut self, rhs: &ExecutionResources) {
        self.n_steps -= rhs.n_steps;
        self.n_memory_holes -= rhs.n_memory_holes;
        for (k, v) in rhs.builtin_instance_counter.iter() {
            // FIXME: remove k's clone, use &'static str
            let entry = self.builtin_instance_counter.entry(*k).or_insert(0);
            *entry = (*entry).saturating_sub(*v);
        }
    }
}

impl Mul<usize> for &ExecutionResources {
    type Output = ExecutionResources;

    fn mul(self, rhs: usize) -> ExecutionResources {
        let mut new = self.clone();
        new.mul_assign(rhs);
        new
    }
}

impl MulAssign<usize> for ExecutionResources {
    fn mul_assign(&mut self, rhs: usize) {
        self.n_steps *= rhs;
        self.n_memory_holes *= rhs;
        for (_builtin_name, counter) in self.builtin_instance_counter.iter_mut() {
            *counter *= rhs;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::air_private_input::{PrivateInput, PrivateInputSignature, SignatureInput};
    use crate::cairo_run::{cairo_run, CairoRunConfig};
    use crate::stdlib::collections::{HashMap, HashSet};
    use crate::vm::vm_memory::memory::MemoryCell;

    use crate::felt_hex;
    use crate::{
        hint_processor::builtin_hint_processor::builtin_hint_processor_definition::BuiltinHintProcessor,
        relocatable,
        serde::deserialize_program::{Identifier, ReferenceManager},
        utils::test_utils::*,
        vm::trace::trace_entry::TraceEntry,
    };
    use assert_matches::assert_matches;

    #[cfg(target_arch = "wasm32")]
    use wasm_bindgen_test::*;

    #[test]
    #[cfg_attr(target_arch = "wasm32", wasm_bindgen_test)]
    fn check_memory_usage_ok_case() {
        let program = program![BuiltinName::range_check, BuiltinName::output];
        let mut cairo_runner = cairo_runner!(program);
        cairo_runner.vm.segments.segment_used_sizes = Some(vec![4]);

        assert_matches!(cairo_runner.check_memory_usage(), Ok(()));
    }

    #[test]
    #[cfg_attr(target_arch = "wasm32", wasm_bindgen_test)]
    fn check_memory_usage_err_case() {
        let program = program!();

        let mut cairo_runner = cairo_runner!(program);
        cairo_runner.vm.builtin_runners = vec![{
            let mut builtin_runner: BuiltinRunner = OutputBuiltinRunner::new(true).into();
            builtin_runner.initialize_segments(&mut cairo_runner.vm.segments);

            builtin_runner
        }];
        cairo_runner.vm.segments.segment_used_sizes = Some(vec![4, 12]);
        cairo_runner.vm.segments.memory = memory![((0, 0), 0), ((0, 1), 1), ((0, 2), 1)];
        cairo_runner
            .vm
            .segments
            .memory
            .mark_as_accessed((0, 0).into());
        assert_matches!(
            cairo_runner.check_memory_usage(),
            Err(VirtualMachineError::Memory(
                MemoryError::InsufficientAllocatedCells(_)
            ))
        );
    }

    #[test]
    #[cfg_attr(target_arch = "wasm32", wasm_bindgen_test)]
    fn initialize_builtins_with_disordered_builtins() {
        let program = program![BuiltinName::range_check, BuiltinName::output];
        let mut cairo_runner = cairo_runner!(program, LayoutName::plain);
        assert!(cairo_runner.initialize_builtins(false).is_err());
    }

    #[test]
    #[cfg_attr(target_arch = "wasm32", wasm_bindgen_test)]
    fn initialize_builtins_missing_builtins_no_allow_missing() {
        let program = program![BuiltinName::output, BuiltinName::ecdsa];
        let mut cairo_runner = cairo_runner!(program, LayoutName::plain);
        assert_matches!(
            cairo_runner.initialize_builtins(false),
            Err(RunnerError::NoBuiltinForInstance(_))
        )
    }

    #[test]
    #[cfg_attr(target_arch = "wasm32", wasm_bindgen_test)]
    fn initialize_builtins_missing_builtins_allow_missing() {
        let program = program![BuiltinName::output, BuiltinName::ecdsa];
        let mut cairo_runner = cairo_runner!(program);
        assert!(cairo_runner.initialize_builtins(true).is_ok())
    }

    #[test]
    #[cfg_attr(target_arch = "wasm32", wasm_bindgen_test)]
    fn initialize_segments_with_base() {
        let program = program![BuiltinName::output];
        let mut cairo_runner = cairo_runner!(program);
        let program_base = Some(Relocatable {
            segment_index: 5,
            offset: 9,
        });
        add_segments!(&mut cairo_runner.vm, 6);
        cairo_runner.initialize_builtins(false).unwrap();
        cairo_runner.initialize_segments(program_base);
        assert_eq!(
            cairo_runner.program_base,
            Some(Relocatable {
                segment_index: 5,
                offset: 9,
            })
        );
        assert_eq!(
            cairo_runner.execution_base,
            Some(Relocatable {
                segment_index: 6,
                offset: 0,
            })
        );
        assert_eq!(
            cairo_runner.vm.builtin_runners[0].name(),
            BuiltinName::output
        );
        assert_eq!(cairo_runner.vm.builtin_runners[0].base(), 7);

        assert_eq!(cairo_runner.vm.segments.num_segments(), 8);
    }

    #[test]
    #[cfg_attr(target_arch = "wasm32", wasm_bindgen_test)]
    fn initialize_segments_no_base() {
        let program = program![BuiltinName::output];
        let mut cairo_runner = cairo_runner!(program);
        cairo_runner.initialize_builtins(false).unwrap();
        cairo_runner.initialize_segments(None);
        assert_eq!(
            cairo_runner.program_base,
            Some(Relocatable {
                segment_index: 0,
                offset: 0
            })
        );
        assert_eq!(
            cairo_runner.execution_base,
            Some(Relocatable {
                segment_index: 1,
                offset: 0
            })
        );
        assert_eq!(
            cairo_runner.vm.builtin_runners[0].name(),
            BuiltinName::output
        );
        assert_eq!(cairo_runner.vm.builtin_runners[0].base(), 2);

        assert_eq!(cairo_runner.vm.segments.num_segments(), 3);
    }

    #[test]
    #[cfg_attr(target_arch = "wasm32", wasm_bindgen_test)]
    fn initialize_state_empty_data_and_stack() {
        let program = program![BuiltinName::output];
        let mut cairo_runner = cairo_runner!(program);
        cairo_runner.program_base = Some(relocatable!(1, 0));
        cairo_runner.execution_base = Some(relocatable!(2, 0));
        let stack = Vec::new();
        cairo_runner.initialize_builtins(false).unwrap();
        cairo_runner.initialize_state(1, stack).unwrap();
        assert_eq!(
            cairo_runner.initial_pc,
            Some(Relocatable {
                segment_index: 1,
                offset: 1
            })
        );
    }

    #[test]
    #[cfg_attr(target_arch = "wasm32", wasm_bindgen_test)]
    fn initialize_state_some_data_empty_stack() {
        let program = program!(
            builtins = vec![BuiltinName::output],
            data = vec_data!((4), (6)),
        );
        let mut cairo_runner = cairo_runner!(program);
        for _ in 0..2 {
            cairo_runner.vm.segments.add();
        }
        cairo_runner.program_base = Some(Relocatable {
            segment_index: 1,
            offset: 0,
        });
        cairo_runner.execution_base = Some(relocatable!(2, 0));
        let stack = Vec::new();
        cairo_runner.initialize_state(1, stack).unwrap();
        check_memory!(cairo_runner.vm.segments.memory, ((1, 0), 4), ((1, 1), 6));
    }

    #[test]
    #[cfg_attr(target_arch = "wasm32", wasm_bindgen_test)]
    fn initialize_state_empty_data_some_stack() {
        let program = program![BuiltinName::output];
        let mut cairo_runner = cairo_runner!(program);
        for _ in 0..3 {
            cairo_runner.vm.segments.add();
        }
        cairo_runner.program_base = Some(relocatable!(1, 0));
        cairo_runner.execution_base = Some(relocatable!(2, 0));
        let stack = vec![mayberelocatable!(4), mayberelocatable!(6)];
        cairo_runner.initialize_state(1, stack).unwrap();
        check_memory!(cairo_runner.vm.segments.memory, ((2, 0), 4), ((2, 1), 6));
    }

    #[test]
    #[cfg_attr(target_arch = "wasm32", wasm_bindgen_test)]
    fn initialize_state_no_program_base() {
        let program = program![BuiltinName::output];
        let mut cairo_runner = cairo_runner!(program);
        for _ in 0..2 {
            cairo_runner.vm.segments.add();
        }
        cairo_runner.execution_base = Some(Relocatable {
            segment_index: 2,
            offset: 0,
        });
        let stack = vec![
            MaybeRelocatable::from(Felt252::from(4_i32)),
            MaybeRelocatable::from(Felt252::from(6_i32)),
        ];
        assert!(cairo_runner.initialize_state(1, stack).is_err());
    }

    #[test]
    #[should_panic]
    fn initialize_state_no_execution_base() {
        let program = program![BuiltinName::output];
        let mut cairo_runner = cairo_runner!(program);
        for _ in 0..2 {
            cairo_runner.vm.segments.add();
        }
        cairo_runner.program_base = Some(relocatable!(1, 0));
        let stack = vec![
            MaybeRelocatable::from(Felt252::from(4_i32)),
            MaybeRelocatable::from(Felt252::from(6_i32)),
        ];
        cairo_runner.initialize_state(1, stack).unwrap();
    }

    #[test]
    #[cfg_attr(target_arch = "wasm32", wasm_bindgen_test)]
    fn initialize_function_entrypoint_empty_stack() {
        let program = program![BuiltinName::output];
        let mut cairo_runner = cairo_runner!(program);
        for _ in 0..2 {
            cairo_runner.vm.segments.add();
        }
        cairo_runner.program_base = Some(relocatable!(0, 0));
        cairo_runner.execution_base = Some(relocatable!(1, 0));
        let stack = Vec::new();
        let return_fp = MaybeRelocatable::from(Felt252::from(9_i32));
        cairo_runner
            .initialize_function_entrypoint(0, stack, return_fp)
            .unwrap();
        assert_eq!(cairo_runner.initial_fp, cairo_runner.initial_ap);
        assert_eq!(cairo_runner.initial_fp, Some(relocatable!(1, 2)));
        check_memory!(
            cairo_runner.vm.segments.memory,
            ((1, 0), 9),
            ((1, 1), (2, 0))
        );
    }

    #[test]
    #[cfg_attr(target_arch = "wasm32", wasm_bindgen_test)]
    fn initialize_function_entrypoint_some_stack() {
        let program = program![BuiltinName::output];
        let mut cairo_runner = cairo_runner!(program);
        for _ in 0..2 {
            cairo_runner.vm.segments.add();
        }
        cairo_runner.program_base = Some(relocatable!(0, 0));
        cairo_runner.execution_base = Some(relocatable!(1, 0));
        let stack = vec![MaybeRelocatable::from(Felt252::from(7_i32))];
        let return_fp = MaybeRelocatable::from(Felt252::from(9_i32));
        cairo_runner
            .initialize_function_entrypoint(1, stack, return_fp)
            .unwrap();
        assert_eq!(cairo_runner.initial_fp, cairo_runner.initial_ap);
        assert_eq!(cairo_runner.initial_fp, Some(relocatable!(1, 3)));
        check_memory!(
            cairo_runner.vm.segments.memory,
            ((1, 0), 7),
            ((1, 1), 9),
            ((1, 2), (2, 0))
        );
    }

    #[test]
    #[should_panic]
    fn initialize_function_entrypoint_no_execution_base() {
        let program = program![BuiltinName::output];
        let mut cairo_runner = cairo_runner!(program);
        let stack = vec![MaybeRelocatable::from(Felt252::from(7_i32))];
        let return_fp = MaybeRelocatable::from(Felt252::from(9_i32));
        cairo_runner
            .initialize_function_entrypoint(1, stack, return_fp)
            .unwrap();
    }

    #[test]
    #[should_panic]
    fn initialize_main_entrypoint_no_main() {
        let program = program![BuiltinName::output];
        let mut cairo_runner = cairo_runner!(program);
        cairo_runner.initialize_main_entrypoint().unwrap();
    }

    #[test]
    #[cfg_attr(target_arch = "wasm32", wasm_bindgen_test)]
    fn initialize_main_entrypoint() {
        let program = program!(main = Some(1),);
        let mut cairo_runner = cairo_runner!(program);
        cairo_runner.program_base = Some(relocatable!(0, 0));
        cairo_runner.execution_base = Some(relocatable!(0, 0));
        let return_pc = cairo_runner.initialize_main_entrypoint().unwrap();
        assert_eq!(return_pc, Relocatable::from((1, 0)));
    }

    #[test]
    #[cfg_attr(target_arch = "wasm32", wasm_bindgen_test)]
    fn initialize_state_program_segment_accessed_addrs() {
        // This test checks that all addresses from the program segment are marked as accessed at VM state initialization.
        // The fibonacci program has 24 instructions, so there should be 24 accessed addresses,
        // from (0, 0) to (0, 23).
        let program = Program::from_bytes(
            include_bytes!("../../../../cairo_programs/fibonacci.json"),
            Some("main"),
        )
        .unwrap();

        let mut cairo_runner = cairo_runner!(program);

        cairo_runner.initialize(false).unwrap();
        assert_eq!(
            cairo_runner
                .vm
                .segments
                .memory
                .get_amount_of_accessed_addresses_for_segment(0),
            Some(24)
        );
    }

    #[test]
    #[cfg_attr(target_arch = "wasm32", wasm_bindgen_test)]
    fn initialize_vm_no_builtins() {
        let program = program!(main = Some(1),);
        let mut cairo_runner = cairo_runner!(program);
        cairo_runner.program_base = Some(relocatable!(0, 0));
        cairo_runner.initial_pc = Some(relocatable!(0, 1));
        cairo_runner.initial_ap = Some(relocatable!(1, 2));
        cairo_runner.initial_fp = Some(relocatable!(1, 2));
        cairo_runner.initialize_vm().unwrap();
        assert_eq!(cairo_runner.vm.run_context.pc, relocatable!(0, 1));
        assert_eq!(cairo_runner.vm.run_context.ap, 2);
        assert_eq!(cairo_runner.vm.run_context.fp, 2);
    }

    #[test]
    #[cfg_attr(target_arch = "wasm32", wasm_bindgen_test)]
    fn initialize_vm_with_range_check_valid() {
        let program = program!(builtins = vec![BuiltinName::range_check], main = Some(1),);
        let mut cairo_runner = cairo_runner!(program);
        cairo_runner.initial_pc = Some(relocatable!(0, 1));
        cairo_runner.initial_ap = Some(relocatable!(1, 2));
        cairo_runner.initial_fp = Some(relocatable!(1, 2));
        cairo_runner.initialize_builtins(false).unwrap();
        cairo_runner.initialize_segments(None);
        cairo_runner.vm.segments = segments![((2, 0), 23), ((2, 1), 233)];
        assert_eq!(
            cairo_runner.vm.builtin_runners[0].name(),
            BuiltinName::range_check
        );
        assert_eq!(cairo_runner.vm.builtin_runners[0].base(), 2);
        cairo_runner.initialize_vm().unwrap();
        assert!(cairo_runner
            .vm
            .segments
            .memory
            .validated_addresses
            .contains(&Relocatable::from((2, 0))));
        assert!(cairo_runner
            .vm
            .segments
            .memory
            .validated_addresses
            .contains(&Relocatable::from((2, 1))));
        assert_eq!(cairo_runner.vm.segments.memory.validated_addresses.len(), 2);
    }

    #[test]
    #[cfg_attr(target_arch = "wasm32", wasm_bindgen_test)]
    fn initialize_vm_with_range_check_invalid() {
        let program = program!(builtins = vec![BuiltinName::range_check], main = Some(1),);
        let mut cairo_runner = cairo_runner!(program);
        cairo_runner.initial_pc = Some(relocatable!(0, 1));
        cairo_runner.initial_ap = Some(relocatable!(1, 2));
        cairo_runner.initial_fp = Some(relocatable!(1, 2));
        cairo_runner.initialize_builtins(false).unwrap();
        cairo_runner.initialize_segments(None);
        cairo_runner.vm.segments = segments![((2, 1), 23), ((2, 4), (-1))];

        assert_eq!(
            cairo_runner.initialize_vm(),
            Err(RunnerError::MemoryValidationError(
                MemoryError::RangeCheckFoundNonInt(Box::new((2, 0).into()))
            ))
        );
    }

    //Integration tests for initialization phase

    #[test]
    #[cfg_attr(target_arch = "wasm32", wasm_bindgen_test)]
    /* Program used:
    func myfunc(a: felt) -> (r: felt):
        let b = a * 2
        return(b)
    end

    func main():
        let a = 1
        let b = myfunc(a)
        return()
    end

    main = 3
    data = [5207990763031199744, 2, 2345108766317314046, 5189976364521848832, 1, 1226245742482522112, 3618502788666131213697322783095070105623107215331596699973092056135872020476, 2345108766317314046]
    */
    fn initialization_phase_no_builtins() {
        let program = program!(
            data = vec_data!(
                (5207990763031199744_u64),
                (2),
                (2345108766317314046_u64),
                (5189976364521848832_u64),
                (1),
                (1226245742482522112_u64),
                ((
                    "3618502788666131213697322783095070105623107215331596699973092056135872020476",
                    10
                )),
                (2345108766317314046_i64)
            ),
            main = Some(3),
        );
        let mut cairo_runner = cairo_runner!(program);
        cairo_runner.initialize_segments(None);
        cairo_runner.initialize_main_entrypoint().unwrap();
        cairo_runner.initialize_vm().unwrap();

        assert_eq!(cairo_runner.program_base, Some(relocatable!(0, 0)));
        assert_eq!(cairo_runner.execution_base, Some(relocatable!(1, 0)));
        assert_eq!(cairo_runner.final_pc, Some(relocatable!(3, 0)));

        //RunContext check
        //Registers
        assert_eq!(cairo_runner.vm.run_context.pc, relocatable!(0, 3));
        assert_eq!(cairo_runner.vm.run_context.ap, 2);
        assert_eq!(cairo_runner.vm.run_context.fp, 2);
        //Memory
        check_memory!(
            cairo_runner.vm.segments.memory,
            ((0, 0), 5207990763031199744_u64),
            ((0, 1), 2),
            ((0, 2), 2345108766317314046_u64),
            ((0, 3), 5189976364521848832_u64),
            ((0, 4), 1),
            ((0, 5), 1226245742482522112_u64),
            (
                (0, 6),
                (
                    "3618502788666131213697322783095070105623107215331596699973092056135872020476",
                    10
                )
            ),
            ((0, 7), 2345108766317314046_u64),
            ((1, 0), (2, 0)),
            ((1, 1), (3, 0))
        );
    }

    #[test]
    #[cfg_attr(target_arch = "wasm32", wasm_bindgen_test)]
    /*Program used:
    %builtins output

    from starkware.cairo.common.serialize import serialize_word

    func main{output_ptr: felt*}():
        let a = 1
        serialize_word(a)
        return()
    end

    main = 4
    data = [4612671182993129469, 5198983563776393216, 1, 2345108766317314046, 5191102247248822272, 5189976364521848832, 1, 1226245742482522112, 3618502788666131213697322783095070105623107215331596699973092056135872020474, 2345108766317314046]
    */
    fn initialization_phase_output_builtin() {
        let program = program!(
            builtins = vec![BuiltinName::output],
            data = vec_data!(
                (4612671182993129469_u64),
                (5198983563776393216_u64),
                (1),
                (2345108766317314046_u64),
                (5191102247248822272_u64),
                (5189976364521848832_u64),
                (1),
                (1226245742482522112_u64),
                ((
                    "3618502788666131213697322783095070105623107215331596699973092056135872020474",
                    10
                )),
                (2345108766317314046_u64)
            ),
            main = Some(4),
        );
        let mut cairo_runner = cairo_runner!(program);

        cairo_runner.initialize_builtins(false).unwrap();
        cairo_runner.initialize_segments(None);
        cairo_runner.initialize_main_entrypoint().unwrap();
        cairo_runner.initialize_vm().unwrap();

        assert_eq!(cairo_runner.program_base, Some(relocatable!(0, 0)));
        assert_eq!(cairo_runner.execution_base, Some(relocatable!(1, 0)));
        assert_eq!(cairo_runner.final_pc, Some(relocatable!(4, 0)));

        //RunContext check
        //Registers
        assert_eq!(cairo_runner.vm.run_context.pc, relocatable!(0, 4));
        assert_eq!(cairo_runner.vm.run_context.ap, 3);
        assert_eq!(cairo_runner.vm.run_context.fp, 3);
        //Memory
        check_memory!(
            cairo_runner.vm.segments.memory,
            ((0, 0), 4612671182993129469_u64),
            ((0, 1), 5198983563776393216_u64),
            ((0, 2), 1),
            ((0, 3), 2345108766317314046_u64),
            ((0, 4), 5191102247248822272_u64),
            ((0, 5), 5189976364521848832_u64),
            ((0, 6), 1),
            ((0, 7), 1226245742482522112_u64),
            (
                (0, 8),
                (
                    "3618502788666131213697322783095070105623107215331596699973092056135872020474",
                    10
                )
            ),
            ((0, 9), 2345108766317314046_u64),
            ((1, 0), (2, 0)),
            ((1, 1), (3, 0)),
            ((1, 2), (4, 0))
        );
    }

    #[test]
    #[cfg_attr(target_arch = "wasm32", wasm_bindgen_test)]
    /*Program used:
    %builtins range_check

    func check_range{range_check_ptr}(num):

        # Check that 0 <= num < 2**64.
        [range_check_ptr] = num
        assert [range_check_ptr + 1] = 2 ** 64 - 1 - num
        let range_check_ptr = range_check_ptr + 2
        return()
    end

    func main{range_check_ptr}():
        check_range(7)
        return()
    end

    main = 8
    data = [4612671182993129469, 5189976364521848832, 18446744073709551615, 5199546496550207487, 4612389712311386111, 5198983563776393216, 2, 2345108766317314046, 5191102247248822272, 5189976364521848832, 7, 1226245742482522112, 3618502788666131213697322783095070105623107215331596699973092056135872020470, 2345108766317314046]
    */
    fn initialization_phase_range_check_builtin() {
        let program = program!(
            builtins = vec![BuiltinName::range_check],
            data = vec_data!(
                (4612671182993129469_u64),
                (5189976364521848832_u64),
                (18446744073709551615_u128),
                (5199546496550207487_u64),
                (4612389712311386111_u64),
                (5198983563776393216_u64),
                (2),
                (2345108766317314046_u64),
                (5191102247248822272_u64),
                (5189976364521848832_u64),
                (7),
                (1226245742482522112_u64),
                ((
                    "3618502788666131213697322783095070105623107215331596699973092056135872020474",
                    10
                )),
                (2345108766317314046_u64)
            ),
            main = Some(8),
        );

        let mut cairo_runner = cairo_runner!(program);

        cairo_runner.initialize_builtins(false).unwrap();
        cairo_runner.initialize_segments(None);
        cairo_runner.initialize_main_entrypoint().unwrap();
        cairo_runner.initialize_vm().unwrap();

        assert_eq!(cairo_runner.program_base, Some(relocatable!(0, 0)));
        assert_eq!(cairo_runner.execution_base, Some(relocatable!(1, 0)));
        assert_eq!(cairo_runner.final_pc, Some(relocatable!(4, 0)));

        //RunContext check
        //Registers
        assert_eq!(cairo_runner.vm.run_context.pc, relocatable!(0, 8));
        assert_eq!(cairo_runner.vm.run_context.ap, 3);
        assert_eq!(cairo_runner.vm.run_context.fp, 3);
        //Memory
        check_memory!(
            cairo_runner.vm.segments.memory,
            ((0, 0), 4612671182993129469_u64),
            ((0, 1), 5189976364521848832_u64),
            ((0, 2), 18446744073709551615_u128),
            ((0, 3), 5199546496550207487_u64),
            ((0, 4), 4612389712311386111_u64),
            ((0, 5), 5198983563776393216_u64),
            ((0, 6), 2),
            ((0, 7), 2345108766317314046_u64),
            ((0, 8), 5191102247248822272_u64),
            ((0, 9), 5189976364521848832_u64),
            ((0, 10), 7),
            ((0, 11), 1226245742482522112_u64),
            (
                (0, 12),
                (
                    "3618502788666131213697322783095070105623107215331596699973092056135872020474",
                    10
                )
            ),
            ((0, 13), 2345108766317314046_u64),
            ((1, 0), (2, 0)),
            ((1, 1), (3, 0)),
            ((1, 2), (4, 0))
        );
    }

    //Integration tests for initialization + execution phase

    #[test]
    #[cfg_attr(target_arch = "wasm32", wasm_bindgen_test)]
    /*Program used:
    func myfunc(a: felt) -> (r: felt):
        let b = a * 2
        return(b)
    end

    func main():
        let a = 1
        let b = myfunc(a)
        return()
    end

    main = 3
    data = [5207990763031199744, 2, 2345108766317314046, 5189976364521848832, 1, 1226245742482522112, 3618502788666131213697322783095070105623107215331596699973092056135872020476, 2345108766317314046]
    */
    fn initialize_and_run_function_call() {
        //Initialization Phase
        let program = program!(
            data = vec_data!(
                (5207990763031199744_i64),
                (2),
                (2345108766317314046_i64),
                (5189976364521848832_i64),
                (1),
                (1226245742482522112_i64),
                ((
                    "3618502788666131213697322783095070105623107215331596699973092056135872020476",
                    10
                )),
                (2345108766317314046_i64)
            ),
            main = Some(3),
        );
        let mut hint_processor = BuiltinHintProcessor::new_empty();
        let mut cairo_runner = cairo_runner!(program, LayoutName::all_cairo, false, true);
        cairo_runner.initialize_segments(None);
        let end = cairo_runner.initialize_main_entrypoint().unwrap();
        assert_eq!(end, Relocatable::from((3, 0)));
        cairo_runner.initialize_vm().unwrap();
        //Execution Phase
        assert_matches!(cairo_runner.run_until_pc(end, &mut hint_processor), Ok(()));
        //Check final values against Python VM
        //Check final register values
        assert_eq!(cairo_runner.vm.run_context.pc, Relocatable::from((3, 0)));

        assert_eq!(cairo_runner.vm.run_context.ap, 6);

        assert_eq!(cairo_runner.vm.run_context.fp, 0);

        //Check each TraceEntry in trace
        let trace = cairo_runner.vm.trace.unwrap();
        assert_eq!(trace.len(), 5);
        trace_check(
            &trace,
            &[
                ((0, 3).into(), 2, 2),
                ((0, 5).into(), 3, 2),
                ((0, 0).into(), 5, 5),
                ((0, 2).into(), 6, 5),
                ((0, 7).into(), 6, 2),
            ],
        );
    }

    #[test]
    #[cfg_attr(target_arch = "wasm32", wasm_bindgen_test)]
    /*Program used:
    %builtins range_check

    func check_range{range_check_ptr}(num):

        # Check that 0 <= num < 2**64.
        [range_check_ptr] = num
        assert [range_check_ptr + 1] = 2 ** 64 - 1 - num
        let range_check_ptr = range_check_ptr + 2
        return()
    end

    func main{range_check_ptr}():
        check_range(7)
        return()
    end

    main = 8
    data = [4612671182993129469, 5189976364521848832, 18446744073709551615, 5199546496550207487, 4612389712311386111, 5198983563776393216, 2, 2345108766317314046, 5191102247248822272, 5189976364521848832, 7, 1226245742482522112, 3618502788666131213697322783095070105623107215331596699973092056135872020470, 2345108766317314046]
    */
    fn initialize_and_run_range_check_builtin() {
        //Initialization Phase
        let program = program!(
            builtins = vec![BuiltinName::range_check],
            data = vec_data!(
                (4612671182993129469_i64),
                (5189976364521848832_i64),
                (18446744073709551615_i128),
                (5199546496550207487_i64),
                (4612389712311386111_i64),
                (5198983563776393216_i64),
                (2),
                (2345108766317314046_i64),
                (5191102247248822272_i64),
                (5189976364521848832_i64),
                (7),
                (1226245742482522112_i64),
                ((
                    "3618502788666131213697322783095070105623107215331596699973092056135872020470",
                    10
                )),
                (2345108766317314046_i64)
            ),
            main = Some(8),
        );
        let mut hint_processor = BuiltinHintProcessor::new_empty();
        let mut cairo_runner = cairo_runner!(program, LayoutName::all_cairo, false, true);
        cairo_runner.initialize_builtins(false).unwrap();
        cairo_runner.initialize_segments(None);
        let end = cairo_runner.initialize_main_entrypoint().unwrap();
        cairo_runner.initialize_vm().unwrap();
        //Execution Phase
        assert_matches!(cairo_runner.run_until_pc(end, &mut hint_processor), Ok(()));
        //Check final values against Python VM
        //Check final register values
        assert_eq!(cairo_runner.vm.run_context.pc, Relocatable::from((4, 0)));

        assert_eq!(cairo_runner.vm.run_context.ap, 10);

        assert_eq!(cairo_runner.vm.run_context.fp, 0);

        //Check each TraceEntry in trace
        let trace = cairo_runner.vm.trace.unwrap();
        assert_eq!(trace.len(), 10);
        trace_check(
            &trace,
            &[
                ((0, 8).into(), 3, 3),
                ((0, 9).into(), 4, 3),
                ((0, 11).into(), 5, 3),
                ((0, 0).into(), 7, 7),
                ((0, 1).into(), 7, 7),
                ((0, 3).into(), 8, 7),
                ((0, 4).into(), 9, 7),
                ((0, 5).into(), 9, 7),
                ((0, 7).into(), 10, 7),
                ((0, 13).into(), 10, 3),
            ],
        );
        //Check the range_check builtin segment
        assert_eq!(
            cairo_runner.vm.builtin_runners[0].name(),
            BuiltinName::range_check
        );
        assert_eq!(cairo_runner.vm.builtin_runners[0].base(), 2);

        check_memory!(
            cairo_runner.vm.segments.memory,
            ((2, 0), 7),
            ((2, 1), 18446744073709551608_i128)
        );
        assert!(cairo_runner
            .vm
            .segments
            .memory
            .get(&MaybeRelocatable::from((2, 2)))
            .is_none());
    }

    #[test]
    #[cfg_attr(target_arch = "wasm32", wasm_bindgen_test)]
    /*Program used:
    %builtins output

    from starkware.cairo.common.serialize import serialize_word

    func main{output_ptr: felt*}():
        let a = 1
        serialize_word(a)
        let b = 17 * a
        serialize_word(b)
        return()
    end

    main = 4
    data = [
    4612671182993129469,
    5198983563776393216,
    1,
    2345108766317314046,
    5191102247248822272,
    5189976364521848832,
    1,
    1226245742482522112,
    3618502788666131213697322783095070105623107215331596699973092056135872020474,
    5189976364521848832,
    17,
    1226245742482522112,
    3618502788666131213697322783095070105623107215331596699973092056135872020470,
    2345108766317314046
    ]
    */
    fn initialize_and_run_output_builtin() {
        //Initialization Phase
        let program = program!(
            builtins = vec![BuiltinName::output],
            data = vec_data!(
                (4612671182993129469_i64),
                (5198983563776393216_i64),
                (1),
                (2345108766317314046_i64),
                (5191102247248822272_i64),
                (5189976364521848832_i64),
                (1),
                (1226245742482522112_i64),
                ((
                    "3618502788666131213697322783095070105623107215331596699973092056135872020474",
                    10
                )),
                (5189976364521848832_i64),
                (17),
                (1226245742482522112_i64),
                ((
                    "3618502788666131213697322783095070105623107215331596699973092056135872020470",
                    10
                )),
                (2345108766317314046_i64)
            ),
            main = Some(4),
        );
        let mut hint_processor = BuiltinHintProcessor::new_empty();
        let mut cairo_runner = cairo_runner!(program, LayoutName::all_cairo, false, true);
        cairo_runner.initialize_builtins(false).unwrap();
        cairo_runner.initialize_segments(None);
        let end = cairo_runner.initialize_main_entrypoint().unwrap();
        cairo_runner.initialize_vm().unwrap();
        //Execution Phase
        assert_matches!(cairo_runner.run_until_pc(end, &mut hint_processor), Ok(()));
        //Check final values against Python VM
        //Check final register values
        //todo
        assert_eq!(cairo_runner.vm.run_context.pc, Relocatable::from((4, 0)));

        assert_eq!(cairo_runner.vm.run_context.ap, 12);

        assert_eq!(cairo_runner.vm.run_context.fp, 0);

        //Check each TraceEntry in trace
        let trace = cairo_runner.vm.trace.unwrap();
        assert_eq!(trace.len(), 12);
        trace_check(
            &trace,
            &[
                ((0, 4).into(), 3, 3),
                ((0, 5).into(), 4, 3),
                ((0, 7).into(), 5, 3),
                ((0, 0).into(), 7, 7),
                ((0, 1).into(), 7, 7),
                ((0, 3).into(), 8, 7),
                ((0, 9).into(), 8, 3),
                ((0, 11).into(), 9, 3),
                ((0, 0).into(), 11, 11),
                ((0, 1).into(), 11, 11),
                ((0, 3).into(), 12, 11),
                ((0, 13).into(), 12, 3),
            ],
        );
        //Check that the output to be printed is correct
        assert_eq!(
            cairo_runner.vm.builtin_runners[0].name(),
            BuiltinName::output
        );
        assert_eq!(cairo_runner.vm.builtin_runners[0].base(), 2);
        check_memory!(cairo_runner.vm.segments.memory, ((2, 0), 1), ((2, 1), 17));
        assert!(cairo_runner
            .vm
            .segments
            .memory
            .get(&MaybeRelocatable::from((2, 2)))
            .is_none());
    }

    #[test]
    #[cfg_attr(target_arch = "wasm32", wasm_bindgen_test)]
    /*Program used:
    %builtins output range_check

    from starkware.cairo.common.serialize import serialize_word

    func check_range{range_check_ptr}(num) -> (num : felt):

        # Check that 0 <= num < 2**64.
        [range_check_ptr] = num
        assert [range_check_ptr + 1] = 2 ** 64 - 1 - num
        let range_check_ptr = range_check_ptr + 2
        return(num)
    end

    func main{output_ptr: felt*, range_check_ptr: felt}():
        let num: felt = check_range(7)
        serialize_word(num)
        return()
    end

    main = 13
    data = [
    4612671182993129469,
    5198983563776393216,
    1,
    2345108766317314046,
    4612671182993129469,
    5189976364521848832,
    18446744073709551615,
    5199546496550207487,
    4612389712311386111,
    5198983563776393216,
    2,
    5191102247248822272,
    2345108766317314046,
    5191102247248822272,
    5189976364521848832,
    7,
    1226245742482522112,
    3618502788666131213697322783095070105623107215331596699973092056135872020469,
    5191102242953854976,
    5193354051357474816,
    1226245742482522112,
    3618502788666131213697322783095070105623107215331596699973092056135872020461,
    5193354029882638336,
    2345108766317314046]
    */
    fn initialize_and_run_output_range_check_builtin() {
        //Initialization Phase
        let program = program!(
            builtins = vec![BuiltinName::output, BuiltinName::range_check],
            data = vec_data!(
                (4612671182993129469_i64),
                (5198983563776393216_i64),
                (1),
                (2345108766317314046_i64),
                (4612671182993129469_i64),
                (5189976364521848832_i64),
                (18446744073709551615_i128),
                (5199546496550207487_i64),
                (4612389712311386111_i64),
                (5198983563776393216_i64),
                (2),
                (5191102247248822272_i64),
                (2345108766317314046_i64),
                (5191102247248822272_i64),
                (5189976364521848832_i64),
                (7),
                (1226245742482522112_i64),
                ((
                    "3618502788666131213697322783095070105623107215331596699973092056135872020469",
                    10
                )),
                (5191102242953854976_i64),
                (5193354051357474816_i64),
                (1226245742482522112_i64),
                ((
                    "3618502788666131213697322783095070105623107215331596699973092056135872020461",
                    10
                )),
                (5193354029882638336_i64),
                (2345108766317314046_i64)
            ),
            main = Some(13),
        );
        let mut hint_processor = BuiltinHintProcessor::new_empty();
        let mut cairo_runner = cairo_runner!(program, LayoutName::all_cairo, false, true);
        cairo_runner.initialize_builtins(false).unwrap();
        cairo_runner.initialize_segments(None);
        let end = cairo_runner.initialize_main_entrypoint().unwrap();
        cairo_runner.initialize_vm().unwrap();
        //Execution Phase
        assert_matches!(cairo_runner.run_until_pc(end, &mut hint_processor), Ok(()));
        //Check final values against Python VM
        //Check final register values
        assert_eq!(cairo_runner.vm.run_context.pc, Relocatable::from((5, 0)));

        assert_eq!(cairo_runner.vm.run_context.ap, 18);

        assert_eq!(cairo_runner.vm.run_context.fp, 0);

        //Check each TraceEntry in trace
        let trace = cairo_runner.vm.trace.unwrap();
        assert_eq!(trace.len(), 18);
        trace_check(
            &trace,
            &[
                ((0, 13).into(), 4, 4),
                ((0, 14).into(), 5, 4),
                ((0, 16).into(), 6, 4),
                ((0, 4).into(), 8, 8),
                ((0, 5).into(), 8, 8),
                ((0, 7).into(), 9, 8),
                ((0, 8).into(), 10, 8),
                ((0, 9).into(), 10, 8),
                ((0, 11).into(), 11, 8),
                ((0, 12).into(), 12, 8),
                ((0, 18).into(), 12, 4),
                ((0, 19).into(), 13, 4),
                ((0, 20).into(), 14, 4),
                ((0, 0).into(), 16, 16),
                ((0, 1).into(), 16, 16),
                ((0, 3).into(), 17, 16),
                ((0, 22).into(), 17, 4),
                ((0, 23).into(), 18, 4),
            ],
        );
        //Check the range_check builtin segment
        assert_eq!(
            cairo_runner.vm.builtin_runners[1].name(),
            BuiltinName::range_check
        );
        assert_eq!(cairo_runner.vm.builtin_runners[1].base(), 3);

        check_memory!(
            cairo_runner.vm.segments.memory,
            ((3, 0), 7),
            ((3, 1), 18446744073709551608_i128)
        );
        assert!(cairo_runner
            .vm
            .segments
            .memory
            .get(&MaybeRelocatable::from((2, 2)))
            .is_none());

        //Check the output segment
        assert_eq!(
            cairo_runner.vm.builtin_runners[0].name(),
            BuiltinName::output
        );
        assert_eq!(cairo_runner.vm.builtin_runners[0].base(), 2);

        check_memory!(cairo_runner.vm.segments.memory, ((2, 0), 7));
        assert!(cairo_runner
            .vm
            .segments
            .memory
            .get(&(MaybeRelocatable::from((2, 1))))
            .is_none());
    }

    #[test]
    #[cfg_attr(target_arch = "wasm32", wasm_bindgen_test)]
    /*Memory from this test is taken from a cairo program execution
    Program used:
        func main():
        let a = 1
        [ap + 3] = 5
        return()

    end
    Final Memory:
    {RelocatableValue(segment_index=0, offset=0): 4613515612218425347,
     RelocatableValue(segment_index=0, offset=1): 5,
     RelocatableValue(segment_index=0, offset=2): 2345108766317314046,
     RelocatableValue(segment_index=1, offset=0): RelocatableValue(segment_index=2, offset=0),
     RelocatableValue(segment_index=1, offset=1): RelocatableValue(segment_index=3, offset=0),
     RelocatableValue(segment_index=1, offset=5): 5}
    Relocated Memory:
        1     4613515612218425347
        2     5
        3     2345108766317314046
        4     10
        5     10
        ⋮
        9     5
    */
    fn relocate_memory_with_gap() {
        let program = program!();
        let mut cairo_runner = cairo_runner!(program, LayoutName::all_cairo, false, true);
        for _ in 0..4 {
            cairo_runner.vm.segments.add();
        }
        // Memory initialization without macro
        cairo_runner
            .vm
            .segments
            .memory
            .insert(
                Relocatable::from((0, 0)),
                &MaybeRelocatable::from(Felt252::from(4613515612218425347_i64)),
            )
            .unwrap();
        cairo_runner
            .vm
            .segments
            .memory
            .insert(
                Relocatable::from((0, 1)),
                &MaybeRelocatable::from(Felt252::from(5)),
            )
            .unwrap();
        cairo_runner
            .vm
            .segments
            .memory
            .insert(
                Relocatable::from((0, 2)),
                &MaybeRelocatable::from(Felt252::from(2345108766317314046_i64)),
            )
            .unwrap();
        cairo_runner
            .vm
            .segments
            .memory
            .insert(Relocatable::from((1, 0)), &MaybeRelocatable::from((2, 0)))
            .unwrap();
        cairo_runner
            .vm
            .segments
            .memory
            .insert(Relocatable::from((1, 1)), &MaybeRelocatable::from((3, 0)))
            .unwrap();
        cairo_runner
            .vm
            .segments
            .memory
            .insert(
                Relocatable::from((1, 5)),
                &MaybeRelocatable::from(Felt252::from(5)),
            )
            .unwrap();
        cairo_runner.vm.segments.compute_effective_sizes();
        let rel_table = cairo_runner
            .vm
            .segments
            .relocate_segments()
            .expect("Couldn't relocate after compute effective sizes");
        assert_eq!(cairo_runner.relocate_memory(&rel_table), Ok(()));
        assert_eq!(cairo_runner.relocated_memory[0], None);
        assert_eq!(
            cairo_runner.relocated_memory[1],
            Some(Felt252::from(4613515612218425347_i64))
        );
        assert_eq!(cairo_runner.relocated_memory[2], Some(Felt252::from(5)));
        assert_eq!(
            cairo_runner.relocated_memory[3],
            Some(Felt252::from(2345108766317314046_i64))
        );
        assert_eq!(cairo_runner.relocated_memory[4], Some(Felt252::from(10)));
        assert_eq!(cairo_runner.relocated_memory[5], Some(Felt252::from(10)));
        assert_eq!(cairo_runner.relocated_memory[6], None);
        assert_eq!(cairo_runner.relocated_memory[7], None);
        assert_eq!(cairo_runner.relocated_memory[8], None);
        assert_eq!(cairo_runner.relocated_memory[9], Some(Felt252::from(5)));
    }

    #[test]
    #[cfg_attr(target_arch = "wasm32", wasm_bindgen_test)]
    /* Program used:
    %builtins output

    from starkware.cairo.common.serialize import serialize_word

    func main{output_ptr: felt*}():
        let a = 1
        serialize_word(a)
        let b = 17 * a
        serialize_word(b)
        return()
    end
    Relocated Memory:
        1     4612671182993129469
        2     5198983563776393216
        3     1
        4     2345108766317314046
        5     5191102247248822272
        6     5189976364521848832
        7     1
        8     1226245742482522112
        9     -7
        10    5189976364521848832
        11    17
        12    1226245742482522112
        13    -11
        14    2345108766317314046
        15    27
        16    29
        17    29
        18    27
        19    1
        20    18
        21    10
        22    28
        23    17
        24    18
        25    14
        26    29
        27    1
        28    17
     */
    fn initialize_run_and_relocate_output_builtin() {
        let program = program!(
            builtins = vec![BuiltinName::output],
            data = vec_data!(
                (4612671182993129469_i64),
                (5198983563776393216_i64),
                (1),
                (2345108766317314046_i64),
                (5191102247248822272_i64),
                (5189976364521848832_i64),
                (1),
                (1226245742482522112_i64),
                ((
                    "3618502788666131213697322783095070105623107215331596699973092056135872020474",
                    10
                )),
                (5189976364521848832_i64),
                (17),
                (1226245742482522112_i64),
                ((
                    "3618502788666131213697322783095070105623107215331596699973092056135872020470",
                    10
                )),
                (2345108766317314046_i64)
            ),
            main = Some(4),
        );
        let mut hint_processor = BuiltinHintProcessor::new_empty();
        let mut cairo_runner = cairo_runner!(program, LayoutName::all_cairo, false, true);
        cairo_runner.initialize_builtins(false).unwrap();
        cairo_runner.initialize_segments(None);
        let end = cairo_runner.initialize_main_entrypoint().unwrap();
        cairo_runner.initialize_vm().unwrap();
        assert_matches!(cairo_runner.run_until_pc(end, &mut hint_processor), Ok(()));
        cairo_runner.vm.segments.compute_effective_sizes();
        let rel_table = cairo_runner
            .vm
            .segments
            .relocate_segments()
            .expect("Couldn't relocate after compute effective sizes");
        assert_eq!(cairo_runner.relocate_memory(&rel_table), Ok(()));
        assert_eq!(cairo_runner.relocated_memory[0], None);
        assert_eq!(
            cairo_runner.relocated_memory[1],
            Some(Felt252::from(4612671182993129469_u64))
        );
        assert_eq!(
            cairo_runner.relocated_memory[2],
            Some(Felt252::from(5198983563776393216_u64))
        );
        assert_eq!(cairo_runner.relocated_memory[3], Some(Felt252::ONE));
        assert_eq!(
            cairo_runner.relocated_memory[4],
            Some(Felt252::from(2345108766317314046_u64))
        );
        assert_eq!(
            cairo_runner.relocated_memory[5],
            Some(Felt252::from(5191102247248822272_u64))
        );
        assert_eq!(
            cairo_runner.relocated_memory[6],
            Some(Felt252::from(5189976364521848832_u64))
        );
        assert_eq!(cairo_runner.relocated_memory[7], Some(Felt252::ONE));
        assert_eq!(
            cairo_runner.relocated_memory[8],
            Some(Felt252::from(1226245742482522112_u64))
        );
        assert_eq!(
            cairo_runner.relocated_memory[9],
            Some(felt_hex!(
                "0x800000000000010fffffffffffffffffffffffffffffffffffffffffffffffa"
            ))
        );
        assert_eq!(
            cairo_runner.relocated_memory[10],
            Some(Felt252::from(5189976364521848832_u64))
        );
        assert_eq!(cairo_runner.relocated_memory[11], Some(Felt252::from(17)));
        assert_eq!(
            cairo_runner.relocated_memory[12],
            Some(Felt252::from(1226245742482522112_u64))
        );
        assert_eq!(
            cairo_runner.relocated_memory[13],
            Some(felt_hex!(
                "0x800000000000010fffffffffffffffffffffffffffffffffffffffffffffff6"
            ))
        );
        assert_eq!(
            cairo_runner.relocated_memory[14],
            Some(Felt252::from(2345108766317314046_u64))
        );
        assert_eq!(
            cairo_runner.relocated_memory[15],
            Some(Felt252::from(27_u64))
        );
        assert_eq!(cairo_runner.relocated_memory[16], Some(Felt252::from(29)));
        assert_eq!(cairo_runner.relocated_memory[17], Some(Felt252::from(29)));
        assert_eq!(cairo_runner.relocated_memory[18], Some(Felt252::from(27)));
        assert_eq!(cairo_runner.relocated_memory[19], Some(Felt252::ONE));
        assert_eq!(cairo_runner.relocated_memory[20], Some(Felt252::from(18)));
        assert_eq!(cairo_runner.relocated_memory[21], Some(Felt252::from(10)));
        assert_eq!(cairo_runner.relocated_memory[22], Some(Felt252::from(28)));
        assert_eq!(cairo_runner.relocated_memory[23], Some(Felt252::from(17)));
        assert_eq!(cairo_runner.relocated_memory[24], Some(Felt252::from(18)));
        assert_eq!(cairo_runner.relocated_memory[25], Some(Felt252::from(14)));
        assert_eq!(cairo_runner.relocated_memory[26], Some(Felt252::from(29)));
        assert_eq!(cairo_runner.relocated_memory[27], Some(Felt252::ONE));
        assert_eq!(cairo_runner.relocated_memory[28], Some(Felt252::from(17)));
    }

    #[test]
    #[cfg_attr(target_arch = "wasm32", wasm_bindgen_test)]
    /* Program used:
    %builtins output

    from starkware.cairo.common.serialize import serialize_word

    func main{output_ptr: felt*}():
        let a = 1
        serialize_word(a)
        let b = 17 * a
        serialize_word(b)
        return()
    end

    Relocated Trace:
    [TraceEntry(pc=5, ap=18, fp=18),
     TraceEntry(pc=6, ap=19, fp=18),
     TraceEntry(pc=8, ap=20, fp=18),
     TraceEntry(pc=1, ap=22, fp=22),
     TraceEntry(pc=2, ap=22, fp=22),
     TraceEntry(pc=4, ap=23, fp=22),
     TraceEntry(pc=10, ap=23, fp=18),
    */
    fn relocate_trace_output_builtin() {
        let program = program!(
            builtins = vec![BuiltinName::output],
            data = vec_data!(
                (4612671182993129469_i64),
                (5198983563776393216_i64),
                (1),
                (2345108766317314046_i64),
                (5191102247248822272_i64),
                (5189976364521848832_i64),
                (1),
                (1226245742482522112_i64),
                ((
                    "3618502788666131213697322783095070105623107215331596699973092056135872020474",
                    10
                )),
                (5189976364521848832_i64),
                (17),
                (1226245742482522112_i64),
                ((
                    "3618502788666131213697322783095070105623107215331596699973092056135872020470",
                    10
                )),
                (2345108766317314046_i64)
            ),
            main = Some(4),
        );
        let mut hint_processor = BuiltinHintProcessor::new_empty();
        let mut cairo_runner = cairo_runner!(program, LayoutName::all_cairo, false, true);
        cairo_runner.initialize_builtins(false).unwrap();
        cairo_runner.initialize_segments(None);
        let end = cairo_runner.initialize_main_entrypoint().unwrap();
        cairo_runner.initialize_vm().unwrap();
        assert_matches!(cairo_runner.run_until_pc(end, &mut hint_processor), Ok(()));
        cairo_runner.vm.segments.compute_effective_sizes();
        let rel_table = cairo_runner
            .vm
            .segments
            .relocate_segments()
            .expect("Couldn't relocate after compute effective sizes");
        cairo_runner.relocate_trace(&rel_table).unwrap();
        let relocated_trace = cairo_runner.relocated_trace.unwrap();
        assert_eq!(relocated_trace.len(), 12);
        assert_eq!(
            relocated_trace[0],
            RelocatedTraceEntry {
                pc: 5,
                ap: 18,
                fp: 18
            }
        );
        assert_eq!(
            relocated_trace[1],
            RelocatedTraceEntry {
                pc: 6,
                ap: 19,
                fp: 18
            }
        );
        assert_eq!(
            relocated_trace[2],
            RelocatedTraceEntry {
                pc: 8,
                ap: 20,
                fp: 18
            }
        );
        assert_eq!(
            relocated_trace[3],
            RelocatedTraceEntry {
                pc: 1,
                ap: 22,
                fp: 22
            }
        );
        assert_eq!(
            relocated_trace[4],
            RelocatedTraceEntry {
                pc: 2,
                ap: 22,
                fp: 22
            }
        );
        assert_eq!(
            relocated_trace[5],
            RelocatedTraceEntry {
                pc: 4,
                ap: 23,
                fp: 22
            }
        );
        assert_eq!(
            relocated_trace[6],
            RelocatedTraceEntry {
                pc: 10,
                ap: 23,
                fp: 18
            }
        );
        assert_eq!(
            relocated_trace[7],
            RelocatedTraceEntry {
                pc: 12,
                ap: 24,
                fp: 18
            }
        );
        assert_eq!(
            relocated_trace[8],
            RelocatedTraceEntry {
                pc: 1,
                ap: 26,
                fp: 26
            }
        );
        assert_eq!(
            relocated_trace[9],
            RelocatedTraceEntry {
                pc: 2,
                ap: 26,
                fp: 26
            }
        );
        assert_eq!(
            relocated_trace[10],
            RelocatedTraceEntry {
                pc: 4,
                ap: 27,
                fp: 26
            }
        );
        assert_eq!(
            relocated_trace[11],
            RelocatedTraceEntry {
                pc: 14,
                ap: 27,
                fp: 18
            }
        );
    }

    #[test]
    #[cfg_attr(target_arch = "wasm32", wasm_bindgen_test)]
    fn write_output_from_preset_memory() {
        let program = program![BuiltinName::output];
        let mut cairo_runner = cairo_runner!(program);
        cairo_runner.initialize_builtins(false).unwrap();
        cairo_runner.initialize_segments(None);
        assert_eq!(
            cairo_runner.vm.builtin_runners[0].name(),
            BuiltinName::output
        );
        assert_eq!(cairo_runner.vm.builtin_runners[0].base(), 2);

        cairo_runner.vm.segments = segments![((2, 0), 1), ((2, 1), 2)];
        cairo_runner.vm.segments.segment_used_sizes = Some(vec![0, 0, 2]);

        let mut output_buffer = String::new();
        cairo_runner.vm.write_output(&mut output_buffer).unwrap();
        assert_eq!(&output_buffer, "1\n2\n");
    }

    #[test]
    #[cfg_attr(target_arch = "wasm32", wasm_bindgen_test)]
    /*Program used:
    %builtins output

    from starkware.cairo.common.serialize import serialize_word

    func main{output_ptr: felt*}():
        let a = 1
        serialize_word(a)
        return()
    end */
    fn get_output_from_program() {
        //Initialization Phase
        let program = program!(
            builtins = vec![BuiltinName::output],
            data = vec_data!(
                (4612671182993129469_i64),
                (5198983563776393216_i64),
                (1),
                (2345108766317314046_i64),
                (5191102247248822272_i64),
                (5189976364521848832_i64),
                (1),
                (1226245742482522112_i64),
                ((
                    "3618502788666131213697322783095070105623107215331596699973092056135872020474",
                    10
                )),
                (5189976364521848832_i64),
                (17),
                (1226245742482522112_i64),
                ((
                    "3618502788666131213697322783095070105623107215331596699973092056135872020470",
                    10
                )),
                (2345108766317314046_i64)
            ),
            main = Some(4),
        );
        let mut cairo_runner = cairo_runner!(program);
        cairo_runner.initialize_builtins(false).unwrap();
        cairo_runner.initialize_segments(None);
        let end = cairo_runner.initialize_main_entrypoint().unwrap();
        cairo_runner.initialize_vm().unwrap();
        //Execution Phase
        let mut hint_processor = BuiltinHintProcessor::new_empty();
        assert_matches!(cairo_runner.run_until_pc(end, &mut hint_processor), Ok(()));

        let mut output_buffer = String::new();
        cairo_runner.vm.write_output(&mut output_buffer).unwrap();
        assert_eq!(&output_buffer, "1\n17\n");
    }

    #[test]
    #[cfg_attr(target_arch = "wasm32", wasm_bindgen_test)]
    /*Program used:
    %builtins output

    func main{output_ptr: felt*}() {
        //Memory Gap + Relocatable value
        assert [output_ptr + 1] = cast(output_ptr, felt);
        let output_ptr = output_ptr + 2;
        return ();
    }*/
    fn write_output_from_program_gap_relocatable_output() {
        //Initialization Phase
        let program = program!(
            builtins = vec![BuiltinName::output],
            data = vec_data!(
                (4612671187288162301),
                (5198983563776458752),
                (2),
                (2345108766317314046)
            ),
            main = Some(0),
        );
        let mut cairo_runner = cairo_runner!(program);
        cairo_runner.initialize_builtins(false).unwrap();
        cairo_runner.initialize_segments(None);
        let end = cairo_runner.initialize_main_entrypoint().unwrap();
        cairo_runner.initialize_vm().unwrap();
        //Execution Phase
        let mut hint_processor = BuiltinHintProcessor::new_empty();
        assert_matches!(cairo_runner.run_until_pc(end, &mut hint_processor), Ok(()));

        let mut output_buffer = String::new();
        cairo_runner.vm.write_output(&mut output_buffer).unwrap();
        assert_eq!(&output_buffer, "<missing>\n2:0\n");
    }

    #[test]
    #[cfg_attr(target_arch = "wasm32", wasm_bindgen_test)]
    fn write_output_from_preset_memory_neg_output() {
        let program = program![BuiltinName::output];
        let mut cairo_runner = cairo_runner!(program);
        cairo_runner.initialize_builtins(false).unwrap();
        cairo_runner.initialize_segments(None);
        assert_eq!(
            cairo_runner.vm.builtin_runners[0].name(),
            BuiltinName::output
        );
        assert_eq!(cairo_runner.vm.builtin_runners[0].base(), 2);
        cairo_runner.vm.segments = segments![(
            (2, 0),
            (
                "800000000000011000000000000000000000000000000000000000000000000",
                16
            )
        )];
        cairo_runner.vm.segments.segment_used_sizes = Some(vec![0, 0, 1]);

        let mut output_buffer = String::new();
        cairo_runner.vm.write_output(&mut output_buffer).unwrap();
        assert_eq!(&output_buffer, "-1\n");
    }

    /// Test that `get_output()` works when the `output` builtin is not the first one.
    #[test]
    #[cfg_attr(target_arch = "wasm32", wasm_bindgen_test)]
    fn get_output_unordered_builtins() {
        //Initialization Phase
        let program = program!(
            builtins = vec![BuiltinName::output, BuiltinName::bitwise],
            data = vec_data!(
                (4612671182993129469_i64),
                (5198983563776393216_i64),
                (1),
                (2345108766317314046_i64),
                (5191102247248822272_i64),
                (5189976364521848832_i64),
                (1),
                (1226245742482522112_i64),
                ((
                    "3618502788666131213697322783095070105623107215331596699973092056135872020474",
                    10
                )),
                (5189976364521848832_i64),
                (17),
                (1226245742482522112_i64),
                ((
                    "3618502788666131213697322783095070105623107215331596699973092056135872020470",
                    10
                )),
                (2345108766317314046_i64)
            ),
            main = Some(4),
        );

        let mut cairo_runner = cairo_runner!(program);

        cairo_runner
            .initialize_builtins(false)
            .expect("Couldn't initialize builtins.");

        // Swap the first and second builtins (first should be `output`).
        cairo_runner.vm.builtin_runners.swap(0, 1);
        cairo_runner.program.builtins.swap(0, 1);

        cairo_runner.initialize_segments(None);

        let end = cairo_runner
            .initialize_main_entrypoint()
            .expect("Couldn't initialize the main entrypoint.");
        cairo_runner
            .initialize_vm()
            .expect("Couldn't initialize the cairo_runner.VM.");

        let mut hint_processor = BuiltinHintProcessor::new_empty();
        assert_matches!(cairo_runner.run_until_pc(end, &mut hint_processor), Ok(()));

        let mut output_buffer = String::new();
        cairo_runner.vm.write_output(&mut output_buffer).unwrap();
        assert_eq!(&output_buffer, "1\n17\n");
    }

    #[test]
    #[cfg_attr(target_arch = "wasm32", wasm_bindgen_test)]
    fn insert_all_builtins_in_order() {
        let program = program![
            BuiltinName::output,
            BuiltinName::pedersen,
            BuiltinName::range_check,
            BuiltinName::bitwise,
            BuiltinName::ec_op
        ];
        let mut cairo_runner = cairo_runner!(program);
        cairo_runner.initialize_builtins(false).unwrap();
        assert_eq!(
            cairo_runner.vm.builtin_runners[0].name(),
            BuiltinName::output
        );
        assert_eq!(
            cairo_runner.vm.builtin_runners[1].name(),
            BuiltinName::pedersen
        );
        assert_eq!(
            cairo_runner.vm.builtin_runners[2].name(),
            BuiltinName::range_check
        );
        assert_eq!(
            cairo_runner.vm.builtin_runners[3].name(),
            BuiltinName::bitwise
        );
        assert_eq!(
            cairo_runner.vm.builtin_runners[4].name(),
            BuiltinName::ec_op
        );
    }

    #[test]
    #[cfg_attr(target_arch = "wasm32", wasm_bindgen_test)]
    /*Program used:
    %builtins range_check

    func check_range{range_check_ptr}(num):
        # Check that 0 <= num < 2**64.
        [range_check_ptr] = num
        assert [range_check_ptr + 1] = 2 ** 64 - 1 - num
        let range_check_ptr = range_check_ptr + 2
        return()
    end

    func main{range_check_ptr}():
        check_range(7)
        return()
    end

    main = 8
    data = [4612671182993129469, 5189976364521848832, 18446744073709551615, 5199546496550207487, 4612389712311386111, 5198983563776393216, 2, 2345108766317314046, 5191102247248822272, 5189976364521848832, 7, 1226245742482522112, 3618502788666131213697322783095070105623107215331596699973092056135872020470, 2345108766317314046]
    */
    fn run_for_steps() {
        let program = program!(
            builtins = vec![BuiltinName::range_check],
            data = vec_data!(
                (4612671182993129469_i64),
                (5189976364521848832_i64),
                (18446744073709551615_i128),
                (5199546496550207487_i64),
                (4612389712311386111_i64),
                (5198983563776393216_i64),
                (2),
                (2345108766317314046_i64),
                (5191102247248822272_i64),
                (5189976364521848832_i64),
                (7),
                (1226245742482522112_i64),
                ((
                    "3618502788666131213697322783095070105623107215331596699973092056135872020470",
                    10
                )),
                (2345108766317314046_i64)
            ),
            main = Some(8),
        );

        let mut hint_processor = BuiltinHintProcessor::new_empty();
        let mut cairo_runner = cairo_runner!(program, LayoutName::all_cairo, false, true);
        cairo_runner.initialize_builtins(false).unwrap();
        cairo_runner.initialize_segments(None);

        cairo_runner.initialize_main_entrypoint().unwrap();
        cairo_runner.initialize_vm().unwrap();

        // Full takes 10 steps.
        assert_matches!(cairo_runner.run_for_steps(8, &mut hint_processor), Ok(()));
        assert_matches!(
            cairo_runner.run_for_steps(8, &mut hint_processor),
            Err(VirtualMachineError::EndOfProgram(x)) if x == 8 - 2
        );
    }

    #[test]
    #[cfg_attr(target_arch = "wasm32", wasm_bindgen_test)]
    fn run_empty() {
        let program = program!();
        let mut cairo_runner = cairo_runner!(&program, LayoutName::all_cairo, false, true);
        assert_matches!(
            cairo_runner.initialize(false),
            Err(RunnerError::MissingMain)
        );
    }

    #[test]
    #[cfg_attr(target_arch = "wasm32", wasm_bindgen_test)]
    /*Program used:
    %builtins range_check

    func check_range{range_check_ptr}(num):
        # Check that 0 <= num < 2**64.
        [range_check_ptr] = num
        assert [range_check_ptr + 1] = 2 ** 64 - 1 - num
        let range_check_ptr = range_check_ptr + 2
        return()
    end

    func main{range_check_ptr}():
        check_range(7)
        return()
    end

    main = 8
    data = [4612671182993129469, 5189976364521848832, 18446744073709551615, 5199546496550207487, 4612389712311386111, 5198983563776393216, 2, 2345108766317314046, 5191102247248822272, 5189976364521848832, 7, 1226245742482522112, 3618502788666131213697322783095070105623107215331596699973092056135872020470, 2345108766317314046]
    */
    fn run_until_steps() {
        let program = program!(
            builtins = vec![BuiltinName::range_check],
            data = vec_data!(
                (4612671182993129469_i64),
                (5189976364521848832_i64),
                (18446744073709551615_i128),
                (5199546496550207487_i64),
                (4612389712311386111_i64),
                (5198983563776393216_i64),
                (2),
                (2345108766317314046_i64),
                (5191102247248822272_i64),
                (5189976364521848832_i64),
                (7),
                (1226245742482522112_i64),
                ((
                    "3618502788666131213697322783095070105623107215331596699973092056135872020470",
                    10
                )),
                (2345108766317314046_i64)
            ),
            main = Some(8),
        );

        let mut hint_processor = BuiltinHintProcessor::new_empty();
        let mut cairo_runner = cairo_runner!(program, LayoutName::all_cairo, false, true);
        cairo_runner.initialize_builtins(false).unwrap();
        cairo_runner.initialize_segments(None);

        cairo_runner.initialize_main_entrypoint().unwrap();
        cairo_runner.initialize_vm().unwrap();

        // Full takes 10 steps.
        assert_matches!(cairo_runner.run_until_steps(8, &mut hint_processor), Ok(()));
        assert_matches!(
            cairo_runner.run_until_steps(10, &mut hint_processor),
            Ok(())
        );
        assert_matches!(
            cairo_runner.run_until_steps(11, &mut hint_processor),
            Err(VirtualMachineError::EndOfProgram(1))
        );
    }

    #[test]
    #[cfg_attr(target_arch = "wasm32", wasm_bindgen_test)]
    /*Program used:
    %builtins range_check

    func check_range{range_check_ptr}(num):
        # Check that 0 <= num < 2**64.
        [range_check_ptr] = num
        assert [range_check_ptr + 1] = 2 ** 64 - 1 - num
        let range_check_ptr = range_check_ptr + 2
        return()
    end

    func main{range_check_ptr}():
        check_range(7)
        return()
    end

    main = 8
    data = [4612671182993129469, 5189976364521848832, 18446744073709551615, 5199546496550207487, 4612389712311386111, 5198983563776393216, 2, 2345108766317314046, 5191102247248822272, 5189976364521848832, 7, 1226245742482522112, 3618502788666131213697322783095070105623107215331596699973092056135872020470, 2345108766317314046]
    */
    /// Verify that run_until_next_power_2() executes steps until the current
    /// step reaches a power of two, or an error occurs.
    fn run_until_next_power_of_2() {
        let program = program!(
            builtins = vec![BuiltinName::range_check],
            data = vec_data!(
                (4612671182993129469_i64),
                (5189976364521848832_i64),
                (18446744073709551615_i128),
                (5199546496550207487_i64),
                (4612389712311386111_i64),
                (5198983563776393216_i64),
                (2),
                (2345108766317314046_i64),
                (5191102247248822272_i64),
                (5189976364521848832_i64),
                (7),
                (1226245742482522112_i64),
                ((
                    "3618502788666131213697322783095070105623107215331596699973092056135872020470",
                    10
                )),
                (2345108766317314046_i64)
            ),
            main = Some(8),
        );

        let mut hint_processor = BuiltinHintProcessor::new_empty();
        let mut cairo_runner = cairo_runner!(program, LayoutName::all_cairo, false, true);
        cairo_runner.initialize_builtins(false).unwrap();
        cairo_runner.initialize_segments(None);

        cairo_runner.initialize_main_entrypoint().unwrap();
        cairo_runner.initialize_vm().unwrap();

        // Full takes 10 steps.
        assert_matches!(cairo_runner.run_for_steps(1, &mut hint_processor), Ok(()));
        assert_matches!(
            cairo_runner.run_until_next_power_of_2(&mut hint_processor),
            Ok(())
        );
        assert_eq!(cairo_runner.vm.current_step, 1);

        assert_matches!(cairo_runner.run_for_steps(1, &mut hint_processor), Ok(()));
        assert_matches!(
            cairo_runner.run_until_next_power_of_2(&mut hint_processor),
            Ok(())
        );
        assert_eq!(cairo_runner.vm.current_step, 2);

        assert_matches!(cairo_runner.run_for_steps(1, &mut hint_processor), Ok(()));
        assert_matches!(
            cairo_runner.run_until_next_power_of_2(&mut hint_processor),
            Ok(())
        );
        assert_eq!(cairo_runner.vm.current_step, 4);

        assert_matches!(cairo_runner.run_for_steps(1, &mut hint_processor), Ok(()));
        assert_matches!(
            cairo_runner.run_until_next_power_of_2(&mut hint_processor),
            Ok(())
        );
        assert_eq!(cairo_runner.vm.current_step, 8);

        assert_matches!(cairo_runner.run_for_steps(1, &mut hint_processor), Ok(()));
        assert_matches!(
            cairo_runner.run_until_next_power_of_2(&mut hint_processor),
            Err(VirtualMachineError::EndOfProgram(6))
        );
        assert_eq!(cairo_runner.vm.current_step, 10);
    }

    #[test]
    #[cfg_attr(target_arch = "wasm32", wasm_bindgen_test)]
    fn get_constants() {
        let program_constants = HashMap::from([
            ("MAX".to_string(), Felt252::from(300)),
            ("MIN".to_string(), Felt252::from(20)),
        ]);
        let program = program!(constants = program_constants.clone(),);
        let cairo_runner = cairo_runner!(program);
        assert_eq!(cairo_runner.get_constants(), &program_constants);
    }

    #[test]
    #[cfg_attr(target_arch = "wasm32", wasm_bindgen_test)]
    fn get_memory_holes_missing_segment_used_sizes() {
        let program = program!();

        let mut cairo_runner = cairo_runner!(program);
        // Add element into memory and mark it as accessed so that get_memory_holes tries to access a segment size
        cairo_runner.vm.segments.memory = memory![((0, 0), 9)];
        cairo_runner
            .vm
            .segments
            .memory
            .mark_as_accessed((0, 0).into());

        cairo_runner.vm.builtin_runners = Vec::new();
        assert_eq!(
            cairo_runner.get_memory_holes(),
            Err(MemoryError::MissingSegmentUsedSizes),
        );
    }

    #[test]
    #[cfg_attr(target_arch = "wasm32", wasm_bindgen_test)]
    fn get_memory_holes_empty() {
        let program = program!();

        let mut cairo_runner = cairo_runner!(program);

        cairo_runner.vm.builtin_runners = Vec::new();
        cairo_runner.vm.segments.segment_used_sizes = Some(Vec::new());
        assert_eq!(cairo_runner.get_memory_holes(), Ok(0));
    }

    #[test]
    #[cfg_attr(target_arch = "wasm32", wasm_bindgen_test)]
    fn get_memory_holes_empty_builtins() {
        let program = program!();

        let mut cairo_runner = cairo_runner!(program);
        cairo_runner.vm.segments.memory = memory![((0, 0), 0), ((0, 2), 0)];
        cairo_runner
            .vm
            .segments
            .memory
            .mark_as_accessed((0, 0).into());
        cairo_runner
            .vm
            .segments
            .memory
            .mark_as_accessed((0, 2).into());
        cairo_runner.vm.builtin_runners = Vec::new();
        cairo_runner.vm.segments.segment_used_sizes = Some(vec![4]);
        assert_eq!(cairo_runner.get_memory_holes(), Ok(2));
    }

    #[test]
    #[cfg_attr(target_arch = "wasm32", wasm_bindgen_test)]
    fn get_memory_holes_empty_accesses() {
        let program = program!();

        let mut cairo_runner = cairo_runner!(program);

        cairo_runner.vm.builtin_runners = vec![{
            let mut builtin_runner: BuiltinRunner = OutputBuiltinRunner::new(true).into();
            builtin_runner.initialize_segments(&mut cairo_runner.vm.segments);

            builtin_runner
        }];
        cairo_runner.vm.segments.segment_used_sizes = Some(vec![4]);
        assert_eq!(cairo_runner.get_memory_holes(), Ok(0));
    }

    #[test]
    #[cfg_attr(target_arch = "wasm32", wasm_bindgen_test)]
    fn get_memory_holes() {
        let program = program!();

        let mut cairo_runner = cairo_runner!(program);
        cairo_runner.vm.segments.memory = memory![((1, 0), 0), ((1, 2), 2)];
        cairo_runner
            .vm
            .segments
            .memory
            .mark_as_accessed((1, 0).into());
        cairo_runner
            .vm
            .segments
            .memory
            .mark_as_accessed((1, 2).into());
        cairo_runner.vm.builtin_runners = vec![{
            let mut builtin_runner: BuiltinRunner = OutputBuiltinRunner::new(true).into();
            builtin_runner.initialize_segments(&mut cairo_runner.vm.segments);

            builtin_runner
        }];
        cairo_runner.vm.segments.segment_used_sizes = Some(vec![4, 4]);
        assert_eq!(cairo_runner.get_memory_holes(), Ok(2));
    }

    /// Test that check_diluted_check_usage() works without a diluted pool
    /// instance.
    #[test]
    #[cfg_attr(target_arch = "wasm32", wasm_bindgen_test)]
    fn check_diluted_check_usage_without_pool_instance() {
        let program = program!();

        let mut cairo_runner = cairo_runner!(program);

        cairo_runner.layout.diluted_pool_instance_def = None;
        assert_matches!(cairo_runner.check_diluted_check_usage(), Ok(()));
    }

    /// Test that check_diluted_check_usage() works without builtin runners.
    #[test]
    #[cfg_attr(target_arch = "wasm32", wasm_bindgen_test)]
    fn check_diluted_check_usage_without_builtin_runners() {
        let program = program!();

        let mut cairo_runner = cairo_runner!(program);

        cairo_runner.vm.current_step = 10000;
        cairo_runner.vm.builtin_runners = vec![];
        assert_matches!(cairo_runner.check_diluted_check_usage(), Ok(()));
    }

    /// Test that check_diluted_check_usage() fails when there aren't enough
    /// allocated units.
    #[test]
    #[cfg_attr(target_arch = "wasm32", wasm_bindgen_test)]
    fn check_diluted_check_usage_insufficient_allocated_cells() {
        let program = program!();

        let mut cairo_runner = cairo_runner!(program);

        cairo_runner.vm.current_step = 100;
        cairo_runner.vm.builtin_runners = vec![];
        assert_matches!(
            cairo_runner.check_diluted_check_usage(),
            Err(VirtualMachineError::Memory(
                MemoryError::InsufficientAllocatedCells(_)
            ))
        );
    }

    /// Test that check_diluted_check_usage() succeeds when all the conditions
    /// are met.
    #[test]
    #[cfg_attr(target_arch = "wasm32", wasm_bindgen_test)]
    fn check_diluted_check_usage() {
        let program = program!();

        let mut cairo_runner = cairo_runner!(program);

        cairo_runner.vm.current_step = 8192;
        cairo_runner.vm.builtin_runners = vec![BitwiseBuiltinRunner::new(Some(256), true).into()];
        assert_matches!(cairo_runner.check_diluted_check_usage(), Ok(()));
    }

    #[test]
    #[cfg_attr(target_arch = "wasm32", wasm_bindgen_test)]
    fn end_run_run_already_finished() {
        let program = program!();

        let mut hint_processor = BuiltinHintProcessor::new_empty();
        let mut cairo_runner = cairo_runner!(program);

        cairo_runner.run_ended = true;
        assert_matches!(
            cairo_runner.end_run(true, false, &mut hint_processor),
            Err(VirtualMachineError::RunnerError(
                RunnerError::EndRunCalledTwice
            ))
        );
    }

    #[test]
    #[cfg_attr(target_arch = "wasm32", wasm_bindgen_test)]
    fn end_run() {
        let program = program!();

        let mut hint_processor = BuiltinHintProcessor::new_empty();
        let mut cairo_runner = cairo_runner!(program);

        assert_matches!(
            cairo_runner.end_run(true, false, &mut hint_processor),
            Ok(())
        );

        cairo_runner.run_ended = false;
        cairo_runner.relocated_memory.clear();
        assert_matches!(
            cairo_runner.end_run(true, true, &mut hint_processor),
            Ok(())
        );
        assert!(!cairo_runner.run_ended);
    }

    #[test]
    #[cfg_attr(target_arch = "wasm32", wasm_bindgen_test)]
    fn end_run_proof_mode_insufficient_allocated_cells() {
        let program = Program::from_bytes(
            include_bytes!("../../../../cairo_programs/proof_programs/fibonacci.json"),
            Some("main"),
        )
        .unwrap();

        let mut hint_processor = BuiltinHintProcessor::new_empty();
        let mut cairo_runner = cairo_runner!(program, LayoutName::all_cairo, true, true);

        let end = cairo_runner.initialize(false).unwrap();
        cairo_runner
            .run_until_pc(end, &mut hint_processor)
            .expect("Call to `CairoRunner::run_until_pc()` failed.");
        assert_matches!(
            cairo_runner.end_run(false, false, &mut hint_processor),
            Ok(())
        );
    }

    #[test]
    #[cfg_attr(target_arch = "wasm32", wasm_bindgen_test)]
    fn get_builtin_segments_info_empty() {
        let program = program!();

        let cairo_runner = cairo_runner!(program);

        assert_eq!(cairo_runner.get_builtin_segments_info(), Ok(Vec::new()),);
    }

    #[test]
    #[cfg_attr(target_arch = "wasm32", wasm_bindgen_test)]
    fn get_builtin_segments_info_base_not_finished() {
        let program = program!();

        let mut cairo_runner = cairo_runner!(program);

        cairo_runner.vm.builtin_runners =
            vec![BuiltinRunner::Output(OutputBuiltinRunner::new(true))];
        assert_eq!(
            cairo_runner.get_builtin_segments_info(),
            Err(RunnerError::NoStopPointer(Box::new(BuiltinName::output))),
        );
    }

    #[test]
    #[cfg_attr(target_arch = "wasm32", wasm_bindgen_test)]
    fn get_execution_resources_trace_not_enabled() {
        let program = program!();

        let mut cairo_runner = cairo_runner!(program);

        cairo_runner.vm.segments.segment_used_sizes = Some(vec![4]);
        cairo_runner.vm.current_step = 10;
        assert_eq!(
            cairo_runner.get_execution_resources(),
            Ok(ExecutionResources {
                n_steps: 10,
                n_memory_holes: 0,
                builtin_instance_counter: HashMap::new(),
            }),
        );
    }

    #[test]
    #[cfg_attr(target_arch = "wasm32", wasm_bindgen_test)]
    fn get_execution_resources_run_program() {
        let program_data = include_bytes!("../../../../cairo_programs/fibonacci.json");
        let cairo_run_config = CairoRunConfig {
            entrypoint: "main",
            trace_enabled: true,
            relocate_mem: false,
            layout: LayoutName::all_cairo,
            proof_mode: false,
            secure_run: Some(false),
            ..Default::default()
        };
        let mut hint_executor = BuiltinHintProcessor::new_empty();
        let runner = cairo_run(program_data, &cairo_run_config, &mut hint_executor).unwrap();
        assert_eq!(runner.get_execution_resources().unwrap().n_steps, 80);
    }

    #[test]
    #[cfg_attr(target_arch = "wasm32", wasm_bindgen_test)]
    fn get_execution_resources_run_program_no_trace() {
        let program_data = include_bytes!("../../../../cairo_programs/fibonacci.json");
        let cairo_run_config = CairoRunConfig {
            entrypoint: "main",
            trace_enabled: false,
            relocate_mem: false,
            layout: LayoutName::all_cairo,
            proof_mode: false,
            secure_run: Some(false),
            ..Default::default()
        };
        let mut hint_executor = BuiltinHintProcessor::new_empty();
        let runner = cairo_run(program_data, &cairo_run_config, &mut hint_executor).unwrap();
        assert_eq!(runner.get_execution_resources().unwrap().n_steps, 80);
    }

    #[test]
    #[cfg_attr(target_arch = "wasm32", wasm_bindgen_test)]
    fn get_execution_resources_empty_builtins() {
        let program = program!();

        let mut cairo_runner = cairo_runner!(program);

        cairo_runner.vm.current_step = 10;
        cairo_runner.vm.segments.segment_used_sizes = Some(vec![4]);
        assert_eq!(
            cairo_runner.get_execution_resources(),
            Ok(ExecutionResources {
                n_steps: 10,
                n_memory_holes: 0,
                builtin_instance_counter: HashMap::new(),
            }),
        );
    }

    #[test]
    #[cfg_attr(target_arch = "wasm32", wasm_bindgen_test)]
    fn get_execution_resources() {
        let program = program!();

        let mut cairo_runner = cairo_runner!(program);

        cairo_runner.vm.current_step = 10;
        cairo_runner.vm.segments.segment_used_sizes = Some(vec![4]);
        cairo_runner.vm.builtin_runners = vec![{
            let mut builtin = OutputBuiltinRunner::new(true);
            builtin.initialize_segments(&mut cairo_runner.vm.segments);

            BuiltinRunner::Output(builtin)
        }];
        assert_eq!(
            cairo_runner.get_execution_resources(),
            Ok(ExecutionResources {
                n_steps: 10,
                n_memory_holes: 0,
                builtin_instance_counter: HashMap::from([(BuiltinName::output, 4)]),
            }),
        );
    }

    #[test]
    #[cfg_attr(target_arch = "wasm32", wasm_bindgen_test)]
    fn finalize_segments_run_not_ended() {
        let program = program!();
        let mut cairo_runner = cairo_runner!(program);
        assert_eq!(
            cairo_runner.finalize_segments(),
            Err(RunnerError::FinalizeNoEndRun)
        )
    }

    #[test]
    #[cfg_attr(target_arch = "wasm32", wasm_bindgen_test)]
    fn finalize_segments_run_ended_empty_no_prog_base() {
        let program = program!();
        let mut cairo_runner = cairo_runner!(program);
        cairo_runner.execution_base = Some(Relocatable::from((1, 0)));
        cairo_runner.run_ended = true;
        assert_eq!(
            cairo_runner.finalize_segments(),
            Err(RunnerError::NoProgBase)
        )
    }

    #[test]
    #[cfg_attr(target_arch = "wasm32", wasm_bindgen_test)]
    fn finalize_segments_run_ended_empty_no_exec_base() {
        let program = program!();
        let mut cairo_runner = cairo_runner!(program);
        cairo_runner.runner_mode = RunnerMode::ProofModeCanonical;
        cairo_runner.program_base = Some(Relocatable::from((0, 0)));
        cairo_runner.run_ended = true;
        assert_eq!(
            cairo_runner.finalize_segments(),
            Err(RunnerError::NoExecBase)
        )
    }

    #[test]
    #[cfg_attr(target_arch = "wasm32", wasm_bindgen_test)]
    fn finalize_segments_run_ended_empty_noproof_mode() {
        let program = program!();
        let mut cairo_runner = cairo_runner!(program);
        cairo_runner.program_base = Some(Relocatable::from((0, 0)));
        cairo_runner.execution_base = Some(Relocatable::from((1, 0)));
        cairo_runner.run_ended = true;
        assert_eq!(
            cairo_runner.finalize_segments(),
            Err(RunnerError::FinalizeSegmentsNoProofMode)
        )
    }

    #[test]
    #[cfg_attr(target_arch = "wasm32", wasm_bindgen_test)]
    fn finalize_segments_run_ended_emptyproof_mode() {
        let program = program!();
        let mut cairo_runner = cairo_runner!(program, LayoutName::plain, true);
        cairo_runner.program_base = Some(Relocatable::from((0, 0)));
        cairo_runner.execution_base = Some(Relocatable::from((1, 0)));
        cairo_runner.run_ended = true;
        assert_eq!(cairo_runner.finalize_segments(), Ok(()));
        assert!(cairo_runner.segments_finalized);
        assert!(cairo_runner.execution_public_memory.unwrap().is_empty())
    }

    #[test]
    #[cfg_attr(target_arch = "wasm32", wasm_bindgen_test)]
    fn finalize_segments_run_ended_not_emptyproof_mode_empty_execution_public_memory() {
        let mut program = program!();
        Arc::get_mut(&mut program.shared_program_data).unwrap().data =
            vec_data![(1), (2), (3), (4), (5), (6), (7), (8)];
        //Program data len = 8
        let mut cairo_runner = cairo_runner!(program, LayoutName::plain, true);
        cairo_runner.program_base = Some(Relocatable::from((0, 0)));
        cairo_runner.execution_base = Some(Relocatable::from((1, 0)));
        cairo_runner.run_ended = true;
        assert_eq!(cairo_runner.finalize_segments(), Ok(()));
        assert!(cairo_runner.segments_finalized);
        //Check values written by first call to segments.finalize()
        assert_eq!(
            cairo_runner.vm.segments.segment_sizes.get(&0),
            Some(&8_usize)
        );
        assert_eq!(
            cairo_runner.vm.segments.public_memory_offsets.get(&0),
            Some(&vec![
                (0_usize, 0_usize),
                (1_usize, 0_usize),
                (2_usize, 0_usize),
                (3_usize, 0_usize),
                (4_usize, 0_usize),
                (5_usize, 0_usize),
                (6_usize, 0_usize),
                (7_usize, 0_usize)
            ])
        );
        //Check values written by second call to segments.finalize()
        assert_eq!(cairo_runner.vm.segments.segment_sizes.get(&1), None);
        assert_eq!(
            cairo_runner.vm.segments.public_memory_offsets.get(&1),
            Some(&vec![])
        );
    }

    #[test]
    #[cfg_attr(target_arch = "wasm32", wasm_bindgen_test)]
    fn finalize_segments_run_ended_not_emptyproof_mode_with_execution_public_memory() {
        let mut program = program!();
        Arc::get_mut(&mut program.shared_program_data).unwrap().data =
            vec_data![(1), (2), (3), (4)];
        //Program data len = 4
        let mut cairo_runner = cairo_runner!(program, LayoutName::plain, true);
        cairo_runner.program_base = Some(Relocatable::from((0, 0)));
        cairo_runner.execution_base = Some(Relocatable::from((1, 1)));
        cairo_runner.execution_public_memory = Some(vec![1_usize, 3_usize, 5_usize, 4_usize]);
        cairo_runner.run_ended = true;
        assert_eq!(cairo_runner.finalize_segments(), Ok(()));
        assert!(cairo_runner.segments_finalized);
        //Check values written by first call to segments.finalize()
        assert_eq!(
            cairo_runner.vm.segments.segment_sizes.get(&0),
            Some(&4_usize)
        );
        assert_eq!(
            cairo_runner.vm.segments.public_memory_offsets.get(&0),
            Some(&vec![
                (0_usize, 0_usize),
                (1_usize, 0_usize),
                (2_usize, 0_usize),
                (3_usize, 0_usize)
            ])
        );
        //Check values written by second call to segments.finalize()
        assert_eq!(cairo_runner.vm.segments.segment_sizes.get(&1), None);
        assert_eq!(
            cairo_runner.vm.segments.public_memory_offsets.get(&1),
            Some(&vec![
                (2_usize, 0_usize),
                (4_usize, 0_usize),
                (6_usize, 0_usize),
                (5_usize, 0_usize)
            ])
        );
    }

    /// Test that get_perm_range_check_limits() works correctly when there are
    /// no builtins.
    #[test]
    #[cfg_attr(target_arch = "wasm32", wasm_bindgen_test)]
    fn get_perm_range_check_limits_no_builtins() {
        let program = program!();
        let mut hint_processor = BuiltinHintProcessor::new(HashMap::new(), RunResources::default());

        let mut cairo_runner = cairo_runner!(program);

        cairo_runner.vm.segments.memory.data = vec![
            vec![
                MemoryCell::new(Felt252::from(0x8000_8023_8012u64).into()),
                MemoryCell::new(Felt252::from(0xBFFF_8000_0620u64).into()),
                MemoryCell::new(Felt252::from(0x8FFF_8000_0750u64).into()),
            ],
            vec![MemoryCell::new((0isize, 0usize).into()); 128 * 1024],
        ];

        cairo_runner.run_for_steps(1, &mut hint_processor).unwrap();

        assert_matches!(
            cairo_runner.get_perm_range_check_limits(),
            Some((32768, 32803))
        );
    }

    /// Test that get_perm_range_check_limits() works correctly when there are
    /// builtins.
    #[test]
    #[cfg_attr(target_arch = "wasm32", wasm_bindgen_test)]
    fn get_perm_range_check_limits() {
        let program = program!();

        let mut cairo_runner = cairo_runner!(program);

        cairo_runner.vm.segments.memory.data = vec![vec![MemoryCell::new(mayberelocatable!(
            0x80FF_8000_0530u64
        ))]];
        cairo_runner.vm.builtin_runners =
            vec![RangeCheckBuiltinRunner::<RC_N_PARTS_STANDARD>::new(Some(12), true).into()];

        assert_matches!(cairo_runner.get_perm_range_check_limits(), Some((0, 33023)));
    }

    /// Test that check_range_check_usage() returns successfully when trace is
    /// not enabled.
    #[test]
    #[cfg_attr(target_arch = "wasm32", wasm_bindgen_test)]
    fn check_range_check_usage_perm_range_limits_none() {
        let program = program!();

        let mut cairo_runner = cairo_runner!(program);
        cairo_runner.vm.trace = Some(vec![]);

        assert_matches!(cairo_runner.check_range_check_usage(), Ok(()));
    }

    /// Test that check_range_check_usage() returns successfully when all the
    /// conditions are met.
    #[test]
    #[cfg_attr(target_arch = "wasm32", wasm_bindgen_test)]
    fn check_range_check_usage_without_builtins() {
        let program = program!();

        let mut cairo_runner = cairo_runner!(program, LayoutName::plain);
        cairo_runner.vm.builtin_runners = vec![];
        cairo_runner.vm.current_step = 10000;
        cairo_runner.vm.segments.memory.data = vec![vec![MemoryCell::new(mayberelocatable!(
            0x80FF_8000_0530u64
        ))]];
        cairo_runner.vm.trace = Some(vec![TraceEntry {
            pc: (0, 0).into(),
            ap: 0,
            fp: 0,
        }]);

        assert_matches!(cairo_runner.check_range_check_usage(), Ok(()));
    }

    /// Test that check_range_check_usage() returns an error if there are
    /// insufficient allocated cells.
    #[test]
    #[cfg_attr(target_arch = "wasm32", wasm_bindgen_test)]
    fn check_range_check_usage_insufficient_allocated_cells() {
        let program = program!();

        let mut cairo_runner = cairo_runner!(program);
        cairo_runner.vm.builtin_runners =
            vec![RangeCheckBuiltinRunner::<RC_N_PARTS_STANDARD>::new(Some(8), true).into()];
        cairo_runner.vm.segments.memory.data = vec![vec![MemoryCell::new(mayberelocatable!(
            0x80FF_8000_0530u64
        ))]];
        cairo_runner.vm.trace = Some(vec![TraceEntry {
            pc: (0, 0).into(),
            ap: 0,
            fp: 0,
        }]);
        cairo_runner.vm.segments.compute_effective_sizes();

        assert_matches!(
            cairo_runner.check_range_check_usage(),
            Err(VirtualMachineError::Memory(
                MemoryError::InsufficientAllocatedCells(_)
            ))
        );
    }

    #[test]
    #[cfg_attr(target_arch = "wasm32", wasm_bindgen_test)]
    fn get_initial_fp_is_none_without_initialization() {
        let program = program!();

        let runner = cairo_runner!(program);

        assert_eq!(None, runner.get_initial_fp());
    }

    #[test]
    #[cfg_attr(target_arch = "wasm32", wasm_bindgen_test)]
    fn get_initial_fp_can_be_obtained() {
        let program = program![BuiltinName::output];
        let mut cairo_runner = cairo_runner!(program);
        for _ in 0..2 {
            cairo_runner.vm.segments.add();
        }
        cairo_runner.program_base = Some(relocatable!(0, 0));
        cairo_runner.execution_base = Some(relocatable!(1, 0));
        let return_fp = Felt252::from(9_i32).into();
        cairo_runner
            .initialize_function_entrypoint(0, vec![], return_fp)
            .unwrap();
        assert_eq!(Some(relocatable!(1, 2)), cairo_runner.get_initial_fp());
    }

    #[test]
    #[cfg_attr(target_arch = "wasm32", wasm_bindgen_test)]
    fn check_used_cells_valid_case() {
        let program = program![BuiltinName::range_check, BuiltinName::output];
        let mut cairo_runner = cairo_runner!(program);
        cairo_runner.vm.segments.segment_used_sizes = Some(vec![4]);
        cairo_runner.vm.trace = Some(vec![]);
        cairo_runner.layout.diluted_pool_instance_def = None;

        assert_matches!(cairo_runner.check_used_cells(), Ok(()));
    }

    #[test]
    #[cfg_attr(target_arch = "wasm32", wasm_bindgen_test)]
    fn check_used_cells_get_used_cells_and_allocated_size_error() {
        let program = program!();

        let mut cairo_runner = cairo_runner!(program);
        cairo_runner.vm.builtin_runners =
            vec![RangeCheckBuiltinRunner::<RC_N_PARTS_STANDARD>::new(Some(8), true).into()];
        cairo_runner.vm.segments.memory.data = vec![vec![MemoryCell::new(mayberelocatable!(
            0x80FF_8000_0530u64
        ))]];
        cairo_runner.vm.trace = Some(vec![TraceEntry {
            pc: (0, 0).into(),
            ap: 0,
            fp: 0,
        }]);
        cairo_runner.vm.segments.compute_effective_sizes();
        assert_matches!(
            cairo_runner.check_used_cells(),
            Err(VirtualMachineError::Memory(
                MemoryError::InsufficientAllocatedCells(_)
            ))
        );
    }

    #[test]
    #[cfg_attr(target_arch = "wasm32", wasm_bindgen_test)]
    fn check_used_cells_check_memory_usage_error() {
        let program = program!();

        let mut cairo_runner = cairo_runner!(program);
        cairo_runner
            .vm
            .segments
            .memory
            .mark_as_accessed((1, 0).into());
        cairo_runner
            .vm
            .segments
            .memory
            .mark_as_accessed((1, 3).into());
        cairo_runner.vm.builtin_runners = vec![{
            let mut builtin_runner: BuiltinRunner = OutputBuiltinRunner::new(true).into();
            builtin_runner.initialize_segments(&mut cairo_runner.vm.segments);

            builtin_runner
        }];
        cairo_runner.vm.segments.segment_used_sizes = Some(vec![4, 12]);
        cairo_runner.vm.trace = Some(vec![]);

        assert_matches!(
            cairo_runner.check_used_cells(),
            Err(VirtualMachineError::Memory(
                MemoryError::InsufficientAllocatedCells(_)
            ))
        );
    }

    #[test]
    #[cfg_attr(target_arch = "wasm32", wasm_bindgen_test)]
    fn check_used_cells_check_diluted_check_usage_error() {
        let program = program![BuiltinName::range_check, BuiltinName::output];
        let mut cairo_runner = cairo_runner!(program);
        cairo_runner.vm.segments.segment_used_sizes = Some(vec![4]);
        cairo_runner.vm.trace = Some(vec![]);

        assert_matches!(
            cairo_runner.check_used_cells(),
            Err(VirtualMachineError::Memory(
                MemoryError::InsufficientAllocatedCells(_)
            ))
        );
    }

    #[test]
    #[cfg_attr(target_arch = "wasm32", wasm_bindgen_test)]
    fn initialize_all_program_builtins() {
        let mut program = program!();

        // we manually add the builtins and check that they exist later
        program.builtins = vec![
            BuiltinName::pedersen,
            BuiltinName::range_check,
            BuiltinName::output,
            BuiltinName::ecdsa,
            BuiltinName::bitwise,
            BuiltinName::ec_op,
            BuiltinName::keccak,
            BuiltinName::poseidon,
        ];

        let mut cairo_runner = cairo_runner!(program);

        cairo_runner
            .initialize_program_builtins()
            .expect("Builtin initialization failed.");

        let given_output = cairo_runner.vm.get_builtin_runners();

        assert_eq!(given_output[0].name(), BuiltinName::pedersen);
        assert_eq!(given_output[1].name(), BuiltinName::range_check);
        assert_eq!(given_output[2].name(), BuiltinName::output);
        assert_eq!(given_output[3].name(), BuiltinName::ecdsa);
        assert_eq!(given_output[4].name(), BuiltinName::bitwise);
        assert_eq!(given_output[5].name(), BuiltinName::ec_op);
        assert_eq!(given_output[6].name(), BuiltinName::keccak);
        assert_eq!(given_output[7].name(), BuiltinName::poseidon);
    }

    #[test]
    #[cfg_attr(target_arch = "wasm32", wasm_bindgen_test)]
    fn initialize_function_runner_without_builtins() {
        let program = program!();

        let mut cairo_runner = cairo_runner!(program);

        cairo_runner
            .initialize_function_runner()
            .expect("initialize_function_runner failed.");

        assert_eq!(
            cairo_runner.program_base,
            Some(Relocatable {
                segment_index: 0,
                offset: 0,
            })
        );
        assert_eq!(
            cairo_runner.execution_base,
            Some(Relocatable {
                segment_index: 1,
                offset: 0,
            })
        );
        assert_eq!(cairo_runner.vm.segments.num_segments(), 2);
    }

    #[test]
    #[cfg_attr(target_arch = "wasm32", wasm_bindgen_test)]
    fn initialize_function_runner_with_segment_arena_builtin() {
        let program = program!();

        let mut cairo_runner = cairo_runner!(program);

        cairo_runner
            .initialize_function_runner_cairo_1(&[BuiltinName::segment_arena])
            .expect("initialize_function_runner failed.");

        let builtin_runners = cairo_runner.vm.get_builtin_runners();

        assert_eq!(builtin_runners[0].name(), BuiltinName::segment_arena);

        assert_eq!(
            cairo_runner.program_base,
            Some(Relocatable {
                segment_index: 0,
                offset: 0,
            })
        );
        assert_eq!(
            cairo_runner.execution_base,
            Some(Relocatable {
                segment_index: 1,
                offset: 0,
            })
        );
        // segment arena builtin adds 2 segments
        assert_eq!(cairo_runner.vm.segments.num_segments(), 4);
    }

    #[test]
    #[cfg_attr(target_arch = "wasm32", wasm_bindgen_test)]
    fn initialize_segments_incorrect_layout_plain_one_builtin() {
        let program = program![BuiltinName::output];
        let mut cairo_runner = cairo_runner!(program, LayoutName::plain);
        assert_eq!(
            cairo_runner.initialize_builtins(false),
            Err(RunnerError::NoBuiltinForInstance(Box::new((
                HashSet::from([BuiltinName::output]),
                LayoutName::plain
            ))))
        );
    }

    #[test]
    #[cfg_attr(target_arch = "wasm32", wasm_bindgen_test)]
    fn initialize_segments_incorrect_layout_plain_two_builtins() {
        let program = program![BuiltinName::output, BuiltinName::pedersen];
        let mut cairo_runner = cairo_runner!(program, LayoutName::plain);
        assert_eq!(
            cairo_runner.initialize_builtins(false),
            Err(RunnerError::NoBuiltinForInstance(Box::new((
                HashSet::from([BuiltinName::output, BuiltinName::pedersen]),
                LayoutName::plain
            ))))
        );
    }

    #[test]
    #[cfg_attr(target_arch = "wasm32", wasm_bindgen_test)]
    fn initialize_segments_incorrect_layout_small_two_builtins() {
        let program = program![BuiltinName::output, BuiltinName::bitwise];
        let mut cairo_runner = cairo_runner!(program, LayoutName::small);
        assert_eq!(
            cairo_runner.initialize_builtins(false),
            Err(RunnerError::NoBuiltinForInstance(Box::new((
                HashSet::from([BuiltinName::bitwise]),
                LayoutName::small,
            ))))
        );
    }
    #[test]
    #[cfg_attr(target_arch = "wasm32", wasm_bindgen_test)]
    fn initialize_main_entrypoint_proof_mode_empty_program() {
        let program = program!(start = Some(0), end = Some(0), main = Some(8),);
        let mut runner = cairo_runner!(program);
        runner.runner_mode = RunnerMode::ProofModeCanonical;
        runner.initialize_segments(None);
        assert_eq!(runner.execution_base, Some(Relocatable::from((1, 0))));
        assert_eq!(runner.program_base, Some(Relocatable::from((0, 0))));
        assert_eq!(
            runner.initialize_main_entrypoint(),
            Ok(Relocatable::from((0, 0)))
        );
        assert_eq!(runner.initial_ap, Some(Relocatable::from((1, 2))));
        assert_eq!(runner.initial_fp, runner.initial_ap);
        assert_eq!(runner.execution_public_memory, Some(vec![0, 1]));
    }

    #[test]
    #[cfg_attr(target_arch = "wasm32", wasm_bindgen_test)]
    fn initialize_main_entrypoint_proof_mode_empty_program_two_builtins() {
        let program = program!(
            start = Some(0),
            end = Some(0),
            main = Some(8),
            builtins = vec![BuiltinName::output, BuiltinName::ec_op],
        );
        let mut runner = cairo_runner!(program);
        runner.runner_mode = RunnerMode::ProofModeCanonical;
        runner.initialize_builtins(false).unwrap();
        runner.initialize_segments(None);
        assert_eq!(runner.execution_base, Some(Relocatable::from((1, 0))));
        assert_eq!(runner.program_base, Some(Relocatable::from((0, 0))));
        assert_eq!(
            runner.initialize_main_entrypoint(),
            Ok(Relocatable::from((0, 0)))
        );
        assert_eq!(runner.initial_ap, Some(Relocatable::from((1, 2))));
        assert_eq!(runner.initial_fp, runner.initial_ap);
        assert_eq!(runner.execution_public_memory, Some(vec![0, 1, 2, 3]));
    }

    #[test]
    #[cfg_attr(target_arch = "wasm32", wasm_bindgen_test)]
    fn can_get_the_runner_program_builtins() {
        let program = program!(
            start = Some(0),
            end = Some(0),
            main = Some(8),
            builtins = vec![BuiltinName::output, BuiltinName::ec_op],
        );
        let runner = cairo_runner!(program);

        assert_eq!(&program.builtins, runner.get_program_builtins());
    }

    #[test]
    #[cfg_attr(target_arch = "wasm32", wasm_bindgen_test)]
    fn set_entrypoint_main_default() {
        let program = program!(
            identifiers = [(
                "__main__.main",
                Identifier {
                    pc: Some(0),
                    type_: None,
                    value: None,
                    full_name: None,
                    members: None,
                    cairo_type: None,
                    size: None,
                },
            )]
            .into_iter()
            .map(|(k, v)| (k.to_string(), v))
            .collect(),
        );
        let mut cairo_runner = cairo_runner!(program);

        cairo_runner
            .set_entrypoint(None)
            .expect("Call to `set_entrypoint()` failed.");
        assert_eq!(cairo_runner.entrypoint, Some(0));
    }

    #[test]
    #[cfg_attr(target_arch = "wasm32", wasm_bindgen_test)]
    fn set_entrypoint_main() {
        let program = program!(
            identifiers = [
                (
                    "__main__.main",
                    Identifier {
                        pc: Some(0),
                        type_: None,
                        value: None,
                        full_name: None,
                        members: None,
                        cairo_type: None,
                        size: None,
                    },
                ),
                (
                    "__main__.alternate_main",
                    Identifier {
                        pc: Some(1),
                        type_: None,
                        value: None,
                        full_name: None,
                        members: None,
                        cairo_type: None,
                        size: None,
                    },
                ),
            ]
            .into_iter()
            .map(|(k, v)| (k.to_string(), v))
            .collect(),
        );
        let mut cairo_runner = cairo_runner!(program);

        cairo_runner
            .set_entrypoint(Some("alternate_main"))
            .expect("Call to `set_entrypoint()` failed.");
        assert_eq!(cairo_runner.entrypoint, Some(1));
    }

    /// Test that set_entrypoint() fails when the entrypoint doesn't exist.
    #[test]
    #[cfg_attr(target_arch = "wasm32", wasm_bindgen_test)]
    fn set_entrypoint_main_non_existent() {
        let program = program!(
            identifiers = [(
                "__main__.main",
                Identifier {
                    pc: Some(0),
                    type_: None,
                    value: None,
                    full_name: None,
                    members: None,
                    cairo_type: None,
                    size: None,
                },
            )]
            .into_iter()
            .map(|(k, v)| (k.to_string(), v))
            .collect(),
        );
        let mut cairo_runner = cairo_runner!(program);

        cairo_runner
            .set_entrypoint(Some("nonexistent_main"))
            .expect_err("Call to `set_entrypoint()` succeeded (should've failed).");
        assert_eq!(cairo_runner.entrypoint, None);
    }

    #[test]
    #[cfg_attr(target_arch = "wasm32", wasm_bindgen_test)]
    fn read_return_values_test() {
        let mut program = program!();
        Arc::get_mut(&mut program.shared_program_data).unwrap().data =
            vec_data![(1), (2), (3), (4), (5), (6), (7), (8)];
        //Program data len = 8
        let mut cairo_runner = cairo_runner!(program, LayoutName::plain, true);
        cairo_runner.program_base = Some(Relocatable::from((0, 0)));
        cairo_runner.execution_base = Some(Relocatable::from((1, 0)));
        cairo_runner.run_ended = true;
        cairo_runner.segments_finalized = false;
        //Check values written by first call to segments.finalize()

        assert_eq!(cairo_runner.read_return_values(false), Ok(()));
        assert_eq!(
            cairo_runner
                .execution_public_memory
                .expect("missing execution public memory"),
            Vec::<usize>::new()
        );
    }

    #[test]
    #[cfg_attr(target_arch = "wasm32", wasm_bindgen_test)]
    fn read_return_values_test_with_run_not_ended() {
        let mut program = program!();
        Arc::get_mut(&mut program.shared_program_data).unwrap().data =
            vec_data![(1), (2), (3), (4), (5), (6), (7), (8)];
        //Program data len = 8
        let mut cairo_runner = cairo_runner!(program, LayoutName::plain, true);
        cairo_runner.program_base = Some(Relocatable::from((0, 0)));
        cairo_runner.execution_base = Some(Relocatable::from((1, 0)));
        cairo_runner.run_ended = false;
        assert_eq!(
            cairo_runner.read_return_values(false),
            Err(RunnerError::ReadReturnValuesNoEndRun)
        );
    }

    #[test]
    #[cfg_attr(target_arch = "wasm32", wasm_bindgen_test)]
    fn read_return_values_test_with_segments_finalized() {
        let mut program = program!();
        Arc::get_mut(&mut program.shared_program_data).unwrap().data =
            vec_data![(1), (2), (3), (4), (5), (6), (7), (8)];
        //Program data len = 8
        let mut cairo_runner = cairo_runner!(program, LayoutName::plain, true);
        cairo_runner.program_base = Some(Relocatable::from((0, 0)));
        cairo_runner.execution_base = Some(Relocatable::from((1, 0)));
        cairo_runner.run_ended = true;
        cairo_runner.segments_finalized = true;
        assert_eq!(
            cairo_runner.read_return_values(false),
            Err(RunnerError::FailedAddingReturnValues)
        );
    }

    #[test]
    #[cfg_attr(target_arch = "wasm32", wasm_bindgen_test)]
    fn read_return_values_updates_builtin_stop_ptr_one_builtin_empty() {
        let mut program = program![BuiltinName::output];
        Arc::get_mut(&mut program.shared_program_data).unwrap().data =
            vec_data![(1), (2), (3), (4), (5), (6), (7), (8)];
        //Program data len = 8
        let mut cairo_runner = cairo_runner!(program, LayoutName::all_cairo, true);
        cairo_runner.program_base = Some(Relocatable::from((0, 0)));
        cairo_runner.execution_base = Some(Relocatable::from((1, 0)));
        cairo_runner.run_ended = true;
        cairo_runner.segments_finalized = false;
        let output_builtin = OutputBuiltinRunner::new(true);
        cairo_runner.vm.builtin_runners.push(output_builtin.into());
        cairo_runner.vm.segments.memory.data = vec![
            vec![],
            vec![MemoryCell::new(MaybeRelocatable::from((0, 0)))],
            vec![],
        ];
        cairo_runner.vm.set_ap(1);
        cairo_runner.vm.segments.segment_used_sizes = Some(vec![0, 1, 0]);
        //Check values written by first call to segments.finalize()
        assert_eq!(cairo_runner.read_return_values(false), Ok(()));
        let output_builtin = match &cairo_runner.vm.builtin_runners[0] {
            BuiltinRunner::Output(runner) => runner,
            _ => unreachable!(),
        };
        assert_eq!(output_builtin.stop_ptr, Some(0))
    }

    #[test]
    #[cfg_attr(target_arch = "wasm32", wasm_bindgen_test)]
    fn read_return_values_updates_builtin_stop_ptr_one_builtin_one_element() {
        let mut program = program![BuiltinName::output];
        Arc::get_mut(&mut program.shared_program_data).unwrap().data =
            vec_data![(1), (2), (3), (4), (5), (6), (7), (8)];
        //Program data len = 8
        let mut cairo_runner = cairo_runner!(program, LayoutName::all_cairo, true);
        cairo_runner.program_base = Some(Relocatable::from((0, 0)));
        cairo_runner.execution_base = Some(Relocatable::from((1, 0)));
        cairo_runner.run_ended = true;
        cairo_runner.segments_finalized = false;
        let output_builtin = OutputBuiltinRunner::new(true);
        cairo_runner.vm.builtin_runners.push(output_builtin.into());
        cairo_runner.vm.segments.memory.data = vec![
            vec![MemoryCell::new(MaybeRelocatable::from((0, 0)))],
            vec![MemoryCell::new(MaybeRelocatable::from((0, 1)))],
            vec![],
        ];
        cairo_runner.vm.set_ap(1);
        cairo_runner.vm.segments.segment_used_sizes = Some(vec![1, 1, 0]);
        //Check values written by first call to segments.finalize()
        assert_eq!(cairo_runner.read_return_values(false), Ok(()));
        let output_builtin = match &cairo_runner.vm.builtin_runners[0] {
            BuiltinRunner::Output(runner) => runner,
            _ => unreachable!(),
        };
        assert_eq!(output_builtin.stop_ptr, Some(1))
    }

    #[test]
    #[cfg_attr(target_arch = "wasm32", wasm_bindgen_test)]
    fn read_return_values_updates_builtin_stop_ptr_two_builtins() {
        let mut program = program![BuiltinName::output, BuiltinName::bitwise];
        Arc::get_mut(&mut program.shared_program_data).unwrap().data =
            vec_data![(1), (2), (3), (4), (5), (6), (7), (8)];
        //Program data len = 8
        let mut cairo_runner = cairo_runner!(program, LayoutName::all_cairo, true);
        cairo_runner.program_base = Some(Relocatable::from((0, 0)));
        cairo_runner.execution_base = Some(Relocatable::from((1, 0)));
        cairo_runner.run_ended = true;
        cairo_runner.segments_finalized = false;
        let output_builtin = OutputBuiltinRunner::new(true);
        let bitwise_builtin = BitwiseBuiltinRunner::new(Some(256), true);
        cairo_runner.vm.builtin_runners.push(output_builtin.into());
        cairo_runner.vm.builtin_runners.push(bitwise_builtin.into());
        cairo_runner.initialize_segments(None);
        cairo_runner.vm.segments.memory.data = vec![
            vec![MemoryCell::new(MaybeRelocatable::from((0, 0)))],
            vec![
                MemoryCell::new(MaybeRelocatable::from((2, 0))),
                MemoryCell::new(MaybeRelocatable::from((3, 5))),
            ],
            vec![],
        ];
        cairo_runner.vm.set_ap(2);
        // We use 5 as bitwise builtin's segment size as a bitwise instance is 5 cells
        cairo_runner.vm.segments.segment_used_sizes = Some(vec![0, 2, 0, 5]);
        //Check values written by first call to segments.finalize()
        assert_eq!(cairo_runner.read_return_values(false), Ok(()));
        let output_builtin = match &cairo_runner.vm.builtin_runners[0] {
            BuiltinRunner::Output(runner) => runner,
            _ => unreachable!(),
        };
        assert_eq!(output_builtin.stop_ptr, Some(0));
        assert_eq!(cairo_runner.read_return_values(false), Ok(()));
        let bitwise_builtin = match &cairo_runner.vm.builtin_runners[1] {
            BuiltinRunner::Bitwise(runner) => runner,
            _ => unreachable!(),
        };
        assert_eq!(bitwise_builtin.stop_ptr, Some(5));
    }

    #[test]
    #[cfg_attr(target_arch = "wasm32", wasm_bindgen_test)]
    fn run_from_entrypoint_custom_program_test() {
        let program = Program::from_bytes(
            include_bytes!("../../../../cairo_programs/example_program.json"),
            None,
        )
        .unwrap();
        let mut cairo_runner = cairo_runner!(program, LayoutName::all_cairo, false, true);
        let mut hint_processor = BuiltinHintProcessor::new_empty();

        //this entrypoint tells which function to run in the cairo program
        let main_entrypoint = program
            .shared_program_data
            .identifiers
            .get("__main__.main")
            .unwrap()
            .pc
            .unwrap();

        cairo_runner.initialize_builtins(false).unwrap();
        cairo_runner.initialize_segments(None);
        assert_matches!(
            cairo_runner.run_from_entrypoint(
                main_entrypoint,
                &[
                    &mayberelocatable!(2).into(),
                    &MaybeRelocatable::from((2, 0)).into()
                ], //range_check_ptr
                true,
                None,
                &mut hint_processor,
            ),
            Ok(())
        );

        let mut new_cairo_runner = cairo_runner!(program, LayoutName::all_cairo, false, true);
        let mut hint_processor = BuiltinHintProcessor::new_empty();

        new_cairo_runner.initialize_builtins(false).unwrap();
        new_cairo_runner.initialize_segments(None);

        let fib_entrypoint = program
            .shared_program_data
            .identifiers
            .get("__main__.evaluate_fib")
            .unwrap()
            .pc
            .unwrap();

        assert_matches!(
            new_cairo_runner.run_from_entrypoint(
                fib_entrypoint,
                &[
                    &mayberelocatable!(2).into(),
                    &MaybeRelocatable::from((2, 0)).into()
                ],
                true,
                None,
                &mut hint_processor,
            ),
            Ok(())
        );
    }

    #[test]
    #[cfg_attr(target_arch = "wasm32", wasm_bindgen_test)]
    fn run_from_entrypoint_bitwise_test_check_memory_holes() {
        let program = Program::from_bytes(
            include_bytes!("../../../../cairo_programs/bitwise_builtin_test.json"),
            None,
        )
        .unwrap();
        let mut cairo_runner = cairo_runner!(program, LayoutName::all_cairo, false, true);
        let mut hint_processor = BuiltinHintProcessor::new_empty();

        //this entrypoint tells which function to run in the cairo program
        let main_entrypoint = program
            .shared_program_data
            .identifiers
            .get("__main__.main")
            .unwrap()
            .pc
            .unwrap();

        cairo_runner.initialize_function_runner().unwrap();

        assert!(cairo_runner
            .run_from_entrypoint(
                main_entrypoint,
                &[
                    &MaybeRelocatable::from((2, 0)).into() //bitwise_ptr
                ],
                true,
                None,
                &mut hint_processor,
            )
            .is_ok());

        // Check that memory_holes == 0
        assert!(cairo_runner.get_memory_holes().unwrap().is_zero());
    }

    #[test]
    #[cfg_attr(target_arch = "wasm32", wasm_bindgen_test)]
    fn cairo_arg_from_single() {
        let expected = CairoArg::Single(MaybeRelocatable::from((0, 0)));
        let value = MaybeRelocatable::from((0, 0));
        assert_eq!(expected, value.into())
    }

    #[test]
    #[cfg_attr(target_arch = "wasm32", wasm_bindgen_test)]
    fn cairo_arg_from_array() {
        let expected = CairoArg::Array(vec![MaybeRelocatable::from((0, 0))]);
        let value = vec![MaybeRelocatable::from((0, 0))];
        assert_eq!(expected, value.into())
    }

    fn setup_execution_resources() -> (ExecutionResources, ExecutionResources) {
        let mut builtin_instance_counter: HashMap<BuiltinName, usize> = HashMap::new();
        builtin_instance_counter.insert(BuiltinName::output, 8);

        let execution_resources_1 = ExecutionResources {
            n_steps: 100,
            n_memory_holes: 5,
            builtin_instance_counter: builtin_instance_counter.clone(),
        };

        //Test that the combined Execution Resources only contains the shared builtins
        builtin_instance_counter.insert(BuiltinName::range_check, 8);

        let execution_resources_2 = ExecutionResources {
            n_steps: 100,
            n_memory_holes: 5,
            builtin_instance_counter,
        };

        (execution_resources_1, execution_resources_2)
    }

    #[test]
    #[cfg_attr(target_arch = "wasm32", wasm_bindgen_test)]
    fn execution_resources_add() {
        let (execution_resources_1, execution_resources_2) = setup_execution_resources();
        let combined_resources = &execution_resources_1 + &execution_resources_2;

        assert_eq!(combined_resources.n_steps, 200);
        assert_eq!(combined_resources.n_memory_holes, 10);
        assert_eq!(
            combined_resources
                .builtin_instance_counter
                .get(&BuiltinName::output)
                .unwrap(),
            &16
        );
        assert!(combined_resources
            .builtin_instance_counter
            .contains_key(&BuiltinName::range_check));
    }

    #[test]
    #[cfg_attr(target_arch = "wasm32", wasm_bindgen_test)]
    fn execution_resources_sub() {
        let (execution_resources_1, execution_resources_2) = setup_execution_resources();

        let combined_resources = &execution_resources_1 - &execution_resources_2;

        assert_eq!(combined_resources.n_steps, 0);
        assert_eq!(combined_resources.n_memory_holes, 0);
        assert_eq!(
            combined_resources
                .builtin_instance_counter
                .get(&BuiltinName::output)
                .unwrap(),
            &0
        );
        assert!(combined_resources
            .builtin_instance_counter
            .contains_key(&BuiltinName::range_check));
    }

    #[test]
    #[cfg_attr(target_arch = "wasm32", wasm_bindgen_test)]
    fn run_from_entrypoint_substitute_error_message_test() {
        let program = Program::from_bytes(
            include_bytes!("../../../../cairo_programs/bad_programs/error_msg_function.json"),
            None,
        )
        .unwrap();
        let mut cairo_runner = cairo_runner!(program, LayoutName::all_cairo, false, true);
        let mut hint_processor = BuiltinHintProcessor::new_empty();

        //this entrypoint tells which function to run in the cairo program
        let main_entrypoint = program
            .shared_program_data
            .identifiers
            .get("__main__.main")
            .unwrap()
            .pc
            .unwrap();

        cairo_runner.initialize_builtins(false).unwrap();
        cairo_runner.initialize_segments(None);

        let result =
            cairo_runner.run_from_entrypoint(main_entrypoint, &[], true, None, &mut hint_processor);
        match result {
            Err(CairoRunError::VmException(exception)) => {
                assert_eq!(
                    exception.error_attr_value,
                    Some(String::from("Error message: Test error\n"))
                )
            }
            Err(_) => panic!("Wrong error returned, expected VmException"),
            Ok(_) => panic!("Expected run to fail"),
        }
    }

    #[test]
    #[cfg_attr(target_arch = "wasm32", wasm_bindgen_test)]
    fn get_builtins_final_stack_range_check_builtin() {
        let program = Program::from_bytes(
            include_bytes!("../../../../cairo_programs/assert_le_felt_hint.json"),
            Some("main"),
        )
        .unwrap();
        let mut runner = cairo_runner!(program);
        let end = runner.initialize(false).unwrap();
        runner
            .run_until_pc(end, &mut BuiltinHintProcessor::new_empty())
            .unwrap();
        runner.vm.segments.compute_effective_sizes();
        let initial_pointer = runner.vm.get_ap();
        let expected_pointer = (runner.vm.get_ap() - 1).unwrap();
        assert_eq!(
            runner.get_builtins_final_stack(initial_pointer),
            Ok(expected_pointer)
        );
    }

    #[test]
    #[cfg_attr(target_arch = "wasm32", wasm_bindgen_test)]
    fn get_builtins_final_stack_4_builtins() {
        let program = Program::from_bytes(
            include_bytes!("../../../../cairo_programs/integration.json"),
            Some("main"),
        )
        .unwrap();
        let mut runner = cairo_runner!(program);
        let end = runner.initialize(false).unwrap();
        runner
            .run_until_pc(end, &mut BuiltinHintProcessor::new_empty())
            .unwrap();
        runner.vm.segments.compute_effective_sizes();
        let initial_pointer = runner.vm.get_ap();
        let expected_pointer = (runner.vm.get_ap() - 4).unwrap();
        assert_eq!(
            runner.get_builtins_final_stack(initial_pointer),
            Ok(expected_pointer)
        );
    }

    #[test]
    #[cfg_attr(target_arch = "wasm32", wasm_bindgen_test)]
    fn get_builtins_final_stack_no_builtins() {
        let program = Program::from_bytes(
            include_bytes!("../../../../cairo_programs/fibonacci.json"),
            Some("main"),
        )
        .unwrap();
        let mut runner = cairo_runner!(program);
        let end = runner.initialize(false).unwrap();
        runner
            .run_until_pc(end, &mut BuiltinHintProcessor::new_empty())
            .unwrap();
        runner.vm.segments.compute_effective_sizes();
        let initial_pointer = runner.vm.get_ap();
        let expected_pointer = runner.vm.get_ap();
        assert_eq!(
            runner.get_builtins_final_stack(initial_pointer),
            Ok(expected_pointer)
        );
    }

    #[test]
    #[cfg_attr(target_arch = "wasm32", wasm_bindgen_test)]

    fn filter_unused_builtins_test() {
        let program = Program::from_bytes(
            include_bytes!("../../../../cairo_programs/integration.json"),
            Some("main"),
        )
        .unwrap();
        let mut runner = cairo_runner!(program);
        let end = runner.initialize(false).unwrap();
        runner
            .run_until_pc(end, &mut BuiltinHintProcessor::new_empty())
            .unwrap();
        runner.vm.segments.compute_effective_sizes();
        let mut exec = runner.get_execution_resources().unwrap();
        exec.builtin_instance_counter.insert(BuiltinName::keccak, 0);
        assert_eq!(exec.builtin_instance_counter.len(), 5);
        let rsc = exec.filter_unused_builtins();
        assert_eq!(rsc.builtin_instance_counter.len(), 4);
    }

    #[test]
    fn execution_resources_mul() {
        let execution_resources_1 = ExecutionResources {
            n_steps: 800,
            n_memory_holes: 0,
            builtin_instance_counter: HashMap::from([
                (BuiltinName::pedersen, 7),
                (BuiltinName::range_check, 16),
            ]),
        };

        assert_eq!(
            &execution_resources_1 * 2,
            ExecutionResources {
                n_steps: 1600,
                n_memory_holes: 0,
                builtin_instance_counter: HashMap::from([
                    (BuiltinName::pedersen, 14),
                    (BuiltinName::range_check, 32)
                ])
            }
        );

        let execution_resources_2 = ExecutionResources {
            n_steps: 545,
            n_memory_holes: 0,
            builtin_instance_counter: HashMap::from([(BuiltinName::range_check, 17)]),
        };

        assert_eq!(
            &execution_resources_2 * 8,
            ExecutionResources {
                n_steps: 4360,
                n_memory_holes: 0,
                builtin_instance_counter: HashMap::from([(BuiltinName::range_check, 136)])
            }
        );

        let execution_resources_3 = ExecutionResources {
            n_steps: 42,
            n_memory_holes: 0,
            builtin_instance_counter: HashMap::new(),
        };

        assert_eq!(
            &execution_resources_3 * 18,
            ExecutionResources {
                n_steps: 756,
                n_memory_holes: 0,
                builtin_instance_counter: HashMap::new()
            }
        );
    }

    #[test]
    fn test_get_program() {
        let program = program!(
            builtins = vec![BuiltinName::output],
            data = vec_data!((4), (6)),
        );
        let runner = cairo_runner!(program);

        assert_eq!(runner.get_program().data_len(), 2)
    }

    #[test]
    #[cfg_attr(target_arch = "wasm32", wasm_bindgen_test)]
    fn test_run_resources_none() {
        let program = Program::from_bytes(
            include_bytes!("../../../../cairo_programs/fibonacci.json"),
            Some("main"),
        )
        .unwrap();
        let mut runner = cairo_runner!(program);
        let end = runner.initialize(false).unwrap();

        // program takes 80 steps
        assert_matches!(
            runner.run_until_pc(end, &mut BuiltinHintProcessor::new_empty(),),
            Ok(())
        )
    }

    #[test]
    #[cfg_attr(target_arch = "wasm32", wasm_bindgen_test)]
    fn test_run_resources_ok() {
        let program = Program::from_bytes(
            include_bytes!("../../../../cairo_programs/fibonacci.json"),
            Some("main"),
        )
        .unwrap();
        let mut runner = cairo_runner!(program);
        let end = runner.initialize(false).unwrap();
        let mut hint_processor = BuiltinHintProcessor::new(HashMap::new(), RunResources::new(81));
        // program takes 81 steps
        assert_matches!(runner.run_until_pc(end, &mut hint_processor), Ok(()));

        assert_eq!(hint_processor.run_resources().get_n_steps(), Some(1));
    }

    #[test]
    #[cfg_attr(target_arch = "wasm32", wasm_bindgen_test)]
    fn test_run_resources_ok_2() {
        let program = Program::from_bytes(
            include_bytes!("../../../../cairo_programs/fibonacci.json"),
            Some("main"),
        )
        .unwrap();
        let mut runner = cairo_runner!(program);
        let end = runner.initialize(false).unwrap();
        let mut hint_processor = BuiltinHintProcessor::new(HashMap::new(), RunResources::new(80));
        // program takes 80 steps
        assert_matches!(runner.run_until_pc(end, &mut hint_processor), Ok(()));

        assert_eq!(hint_processor.run_resources(), &RunResources::new(0));
    }

    #[test]
    #[cfg_attr(target_arch = "wasm32", wasm_bindgen_test)]
    fn test_run_resources_error() {
        let program = Program::from_bytes(
            include_bytes!("../../../../cairo_programs/fibonacci.json"),
            Some("main"),
        )
        .unwrap();
        let mut runner = cairo_runner!(program);
        let end = runner.initialize(false).unwrap();
        let mut hint_processor = BuiltinHintProcessor::new(HashMap::new(), RunResources::new(9));
        // program takes 9 steps
        assert_matches!(
            runner.run_until_pc(end, &mut hint_processor,),
            Err(VirtualMachineError::UnfinishedExecution)
        );
        assert_eq!(hint_processor.run_resources(), &RunResources::new(0));
    }

    #[test]
    fn get_cairo_pie_no_program_base() {
        let runner = cairo_runner!(Default::default());
        assert_eq!(runner.get_cairo_pie(), Err(RunnerError::NoProgBase))
    }

    #[test]
    fn get_cairo_pie_no_execution_base() {
        let mut runner = cairo_runner!(Default::default());
        runner.program_base = Some(Relocatable::from((0, 0)));
        assert_eq!(runner.get_cairo_pie(), Err(RunnerError::NoExecBase))
    }

    #[test]
    fn get_cairo_pie_no_segment_sizes() {
        let mut runner = cairo_runner!(Default::default());
        runner.program_base = Some(Relocatable::from((0, 0)));
        runner.execution_base = Some(Relocatable::from((1, 0)));
        runner.vm.add_memory_segment();
        runner.vm.add_memory_segment();
        // return_fp
        runner
            .vm
            .insert_value::<Relocatable>((1, 0).into(), (2, 0).into())
            .unwrap();
        // return_pc
        runner
            .vm
            .insert_value::<Relocatable>((1, 1).into(), (3, 0).into())
            .unwrap();
        assert_eq!(
            runner.get_cairo_pie(),
            Err(RunnerError::UnexpectedRetFpSegmentSize)
        );
    }

    #[test]
    fn get_cairo_pie_ret_pc_segment_size_not_zero() {
        let mut runner = cairo_runner!(Default::default());
        runner.program_base = Some(Relocatable::from((0, 0)));
        runner.execution_base = Some(Relocatable::from((1, 0)));
        runner.vm.add_memory_segment();
        runner.vm.add_memory_segment();
        // return_fp
        runner
            .vm
            .insert_value::<Relocatable>((1, 0).into(), (2, 0).into())
            .unwrap();
        // return_pc
        runner
            .vm
            .insert_value::<Relocatable>((1, 1).into(), (3, 0).into())
            .unwrap();
        // segment sizes
        runner.vm.segments.segment_sizes = HashMap::from([(0, 0), (1, 2), (2, 0), (3, 1)]);
        assert_eq!(
            runner.get_cairo_pie(),
            Err(RunnerError::UnexpectedRetPcSegmentSize)
        );
    }

    #[test]
    fn get_cairo_pie_program_base_offset_not_zero() {
        let mut runner = cairo_runner!(Default::default());
        runner.program_base = Some(Relocatable::from((0, 1)));
        runner.execution_base = Some(Relocatable::from((1, 0)));
        runner.vm.add_memory_segment();
        runner.vm.add_memory_segment();
        // return_fp
        runner
            .vm
            .insert_value::<Relocatable>((1, 0).into(), (2, 0).into())
            .unwrap();
        // return_pc
        runner
            .vm
            .insert_value::<Relocatable>((1, 1).into(), (3, 0).into())
            .unwrap();
        // segment sizes
        runner.vm.segments.segment_sizes = HashMap::from([(0, 0), (1, 2), (2, 0), (3, 0)]);
        assert_eq!(
            runner.get_cairo_pie(),
            Err(RunnerError::ProgramBaseOffsetNotZero)
        );
    }

    #[test]
    fn get_cairo_pie_execution_base_offset_not_zero() {
        let mut runner = cairo_runner!(Default::default());
        runner.program_base = Some(Relocatable::from((0, 0)));
        runner.execution_base = Some(Relocatable::from((1, 1)));
        runner.vm.add_memory_segment();
        runner.vm.add_memory_segment();
        // return_fp
        runner
            .vm
            .insert_value::<Relocatable>((1, 1).into(), (2, 0).into())
            .unwrap();
        // return_pc
        runner
            .vm
            .insert_value::<Relocatable>((1, 2).into(), (3, 0).into())
            .unwrap();
        // segment sizes
        runner.vm.segments.segment_sizes = HashMap::from([(0, 0), (1, 2), (2, 0), (3, 0)]);
        assert_eq!(
            runner.get_cairo_pie(),
            Err(RunnerError::ExecBaseOffsetNotZero)
        );
    }

    #[test]
    fn get_cairo_pie_ret_fp_offset_not_zero() {
        let mut runner = cairo_runner!(Default::default());
        runner.program_base = Some(Relocatable::from((0, 0)));
        runner.execution_base = Some(Relocatable::from((1, 0)));
        runner.vm.add_memory_segment();
        runner.vm.add_memory_segment();
        // return_fp
        runner
            .vm
            .insert_value::<Relocatable>((1, 0).into(), (2, 1).into())
            .unwrap();
        // return_pc
        runner
            .vm
            .insert_value::<Relocatable>((1, 1).into(), (3, 0).into())
            .unwrap();
        // segment sizes
        runner.vm.segments.segment_sizes = HashMap::from([(0, 0), (1, 2), (2, 0), (3, 0)]);
        assert_eq!(runner.get_cairo_pie(), Err(RunnerError::RetFpOffsetNotZero));
    }

    #[test]
    fn get_cairo_pie_ret_pc_offset_not_zero() {
        let mut runner = cairo_runner!(Default::default());
        runner.program_base = Some(Relocatable::from((0, 0)));
        runner.execution_base = Some(Relocatable::from((1, 0)));
        runner.vm.add_memory_segment();
        runner.vm.add_memory_segment();
        // return_fp
        runner
            .vm
            .insert_value::<Relocatable>((1, 0).into(), (2, 0).into())
            .unwrap();
        // return_pc
        runner
            .vm
            .insert_value::<Relocatable>((1, 1).into(), (3, 1).into())
            .unwrap();
        // segment sizes
        runner.vm.segments.segment_sizes = HashMap::from([(0, 0), (1, 2), (2, 0), (3, 0)]);
        assert_eq!(runner.get_cairo_pie(), Err(RunnerError::RetPcOffsetNotZero));
    }

    #[test]
    fn get_cairo_pie_ok() {
        let mut runner = cairo_runner!(Default::default());
        runner.program_base = Some(Relocatable::from((0, 0)));
        runner.execution_base = Some(Relocatable::from((1, 0)));
        runner.vm.add_memory_segment();
        runner.vm.add_memory_segment();
        // return_fp
        runner
            .vm
            .insert_value::<Relocatable>((1, 0).into(), (2, 0).into())
            .unwrap();
        // return_pc
        runner
            .vm
            .insert_value::<Relocatable>((1, 1).into(), (3, 0).into())
            .unwrap();
        // segment sizes
        runner.vm.segments.segment_sizes = HashMap::from([(0, 0), (1, 2), (2, 0), (3, 0)]);
    }

    #[test]
    fn get_air_private_input() {
        let program_content =
            include_bytes!("../../../../cairo_programs/proof_programs/common_signature.json");
        let runner = crate::cairo_run::cairo_run(
            program_content,
            &CairoRunConfig {
                proof_mode: true,
                layout: LayoutName::all_cairo,
                ..Default::default()
            },
            &mut BuiltinHintProcessor::new_empty(),
        )
        .unwrap();
        let air_private_input = runner.get_air_private_input();
        assert!(air_private_input.0[&BuiltinName::pedersen].is_empty());
        assert!(air_private_input.0[&BuiltinName::range_check].is_empty());
        assert!(air_private_input.0[&BuiltinName::bitwise].is_empty());
        assert!(air_private_input.0[&BuiltinName::ec_op].is_empty());
        assert!(air_private_input.0[&BuiltinName::keccak].is_empty());
        assert!(air_private_input.0[&BuiltinName::poseidon].is_empty());
        assert_eq!(
            air_private_input.0[&BuiltinName::ecdsa],
            vec![PrivateInput::Signature(PrivateInputSignature {
                index: 0,
                pubkey: felt_hex!(
                    "0x3d60886c2353d93ec2862e91e23036cd9999a534481166e5a616a983070434d"
                ),
                msg: felt_hex!("0xa9e"),
                signature_input: SignatureInput {
                    r: felt_hex!(
                        "0x6d2e2e00dfceffd6a375db04764da249a5a1534c7584738dfe01cb3944a33ee"
                    ),
                    w: felt_hex!(
                        "0x396362a34ff391372fca63f691e27753ce8f0c2271a614cbd240e1dc1596b28"
                    )
                }
            })]
        );
    }
}
