#![cfg(feature = "extensive_hints")]
/* This file contains a test that runs the program: cairo_programs/starknet_os_deprecated_cc.cairo
   For testsing purposes, the contract ran by this program is hardcoded, with values taken from compiling:

   %lang starknet

   @view
   func get_number() -> (number: felt) {
       let number = 14;
       %{print("hello world")%}
       return (number=number);
   }

   The purpose of this test is to check the functionality of the HintProcessor::execute_hint_extensive functionality
   And to show a very simplified example on how it can be used to achieve the `vm_load_data` functionality used by starknet os programs
*/
use crate::stdlib::{collections::HashMap, prelude::*};

use crate::Felt252;
use num_traits::Zero;
#[cfg(target_arch = "wasm32")]
use wasm_bindgen_test::*;

use crate::{
    cairo_run::{cairo_run, CairoRunConfig},
    hint_processor::{
        builtin_hint_processor::{
            builtin_hint_processor_definition::{BuiltinHintProcessor, HintProcessorData},
            hint_utils::{get_ptr_from_var_name, insert_value_from_var_name},
        },
        hint_processor_definition::{
            HintExtension, HintProcessor, HintProcessorLogic, HintReference,
        },
    },
    serde::deserialize_program::ApTracking,
    vm::{
        errors::hint_errors::HintError,
        runners::cairo_runner::{ResourceTracker, RunResources},
        trace::trace_entry::RelocatedTraceEntry,
        vm_core::VirtualMachine,
    },
};

struct SimplifiedOsHintProcessor {
    builtin_hint_processor: BuiltinHintProcessor,
    run_resources: RunResources,
}

impl ResourceTracker for SimplifiedOsHintProcessor {
    fn consumed(&self) -> bool {
        self.run_resources.consumed()
    }

    fn consume_step(&mut self) {
        self.run_resources.consume_step()
    }

    fn get_n_steps(&self) -> Option<usize> {
        self.run_resources.get_n_steps()
    }

    fn run_resources(&self) -> &RunResources {
        &self.run_resources
    }
}

impl Default for SimplifiedOsHintProcessor {
    fn default() -> Self {
        Self {
            builtin_hint_processor: BuiltinHintProcessor::new_empty(),
            run_resources: Default::default(),
        }
    }
}

impl HintProcessorLogic for SimplifiedOsHintProcessor {
    fn execute_hint(
        &mut self,
        _vm: &mut crate::vm::vm_core::VirtualMachine,
        _exec_scopes: &mut crate::types::exec_scope::ExecutionScopes,
        //Data structure that can be downcasted to the structure generated by compile_hint
        _hint_data: &Box<dyn core::any::Any>,
        //Constant values extracted from the program specification.
        _constants: &HashMap<String, Felt252>,
    ) -> Result<(), crate::vm::errors::hint_errors::HintError> {
        // Empty impl as we are using `execute_hint_extensive` instead for this case
        Ok(())
    }

    fn execute_hint_extensive(
        &mut self,
        vm: &mut crate::vm::vm_core::VirtualMachine,
        exec_scopes: &mut crate::types::exec_scope::ExecutionScopes,
        //Data structure that can be downcasted to the structure generated by compile_hint
        hint_data: &Box<dyn core::any::Any>,
        //Constant values extracted from the program specification.
        constants: &HashMap<String, Felt252>,
    ) -> Result<
        crate::hint_processor::hint_processor_definition::HintExtension,
        crate::vm::errors::hint_errors::HintError,
    > {
        // First attempt to execute with builtin hint processor
        match self.builtin_hint_processor.execute_hint_extensive(
            vm,
            exec_scopes,
            hint_data,
            constants,
        ) {
            Err(HintError::UnknownHint(_)) => {}
            res => return res,
        }
        // Execute os-specific hints
        let hint_data = hint_data
            .downcast_ref::<HintProcessorData>()
            .ok_or(HintError::WrongHintData)?;
        match &*hint_data.code {
            ALLOC_FACTS => alloc_facts(vm, &hint_data.ids_data, &hint_data.ap_tracking),
            COMPILE_CLASS => compile_class(vm, &hint_data.ids_data, &hint_data.ap_tracking),
            VM_LOAD_PROGRAM => {
                vm_load_program(self, vm, &hint_data.ids_data, &hint_data.ap_tracking)
            }
            HELLO_WORLD => hello_world(),
            code => Err(HintError::UnknownHint(code.to_string().into_boxed_str())),
        }
    }
}

// Hints & Hint impls
const ALLOC_FACTS: &str = "ids.compiled_class_facts = segments.add()";
pub fn alloc_facts(
    vm: &mut VirtualMachine,
    ids_data: &HashMap<String, HintReference>,
    ap_tracking: &ApTracking,
) -> Result<HintExtension, HintError> {
    insert_value_from_var_name(
        "compiled_class_facts",
        vm.add_memory_segment(),
        vm,
        ids_data,
        ap_tracking,
    )?;
    Ok(HintExtension::default())
}
const COMPILE_CLASS: &str = "from starkware.starknet.services.api.contract_class.contract_class import DeprecatedCompiledClass\nfrom starkware.starknet.core.os.contract_class.deprecated_class_hash import (\n    get_deprecated_contract_class_struct,\n)\nwith open(\"test_contract.json\", \"r\") as f:\n    compiled_class = DeprecatedCompiledClass.loads(f.read())\n \ncairo_contract = get_deprecated_contract_class_struct(\n    identifiers=ids._context.identifiers, contract_class=compiled_class)\nids.compiled_class = segments.gen_arg(cairo_contract)";
pub fn compile_class(
    vm: &mut VirtualMachine,
    ids_data: &HashMap<String, HintReference>,
    ap_tracking: &ApTracking,
) -> Result<HintExtension, HintError> {
    // We wil use a hardcoded contract to avoid importing starknet-related code for this test
    // What this hint does is load the  "test_contract.json" compiled contract into the `ids.compiled_class` variable of type *DeprecatedCompiledClass
    // First we need to allocate the struct
    let compiled_class_ptr = vm.add_memory_segment();
    insert_value_from_var_name(
        "compiled_class",
        compiled_class_ptr,
        vm,
        ids_data,
        ap_tracking,
    )?;
    // Now we can fill each struct field with our hardcoded values
    let mut ptr = compiled_class_ptr;
    // For this test's purpose we will be using the following cairo 0 contract:
    /*
    %lang starknet

    @view
    func get_number() -> (number: felt) {
        let number = 14;
        %{print("hello world")%}
        return (number=number);
    }
    */

    // struct DeprecatedCompiledClass {
    // compiled_class_version: felt,
    vm.insert_value(ptr, Felt252::default())?; // Not relevant
    ptr.offset += 1;
    // n_external_functions: felt,
    vm.insert_value(ptr, Felt252::ONE)?; // Only one external entrypoint
    ptr.offset += 1;
    // external_functions: DeprecatedContractEntryPoint*,
    let mut entrypoints_ptr = vm.add_memory_segment();
    // struct DeprecatedContractEntryPoint {
    //     selector: felt,
    let selector =
        Felt252::from_hex("0x23180acc053dfb2dbc82a0da33515906d37498b42f34ee4ed308f9d5fb51b6c")
            .unwrap();
    vm.insert_value(entrypoints_ptr, selector)?;
    entrypoints_ptr.offset += 1;
    //     offset: felt,
    let offset = Felt252::from(12);
    vm.insert_value(entrypoints_ptr, offset)?;
    // }
    vm.insert_value(ptr, entrypoints_ptr)?; // Only one external entrypoint
    ptr.offset += 1;
    // n_l1_handlers: felt,
    vm.insert_value(ptr, Felt252::zero())?;
    ptr.offset += 1;
    // l1_handlers: DeprecatedContractEntryPoint*,
    let l1_handler_entrypoints_ptr = vm.add_memory_segment();
    vm.insert_value(ptr, l1_handler_entrypoints_ptr)?;
    ptr.offset += 1;
    // n_constructors: felt,
    vm.insert_value(ptr, Felt252::zero())?;
    ptr.offset += 1;
    // constructors: DeprecatedContractEntryPoint*,
    let constructor_entrypoints_ptr = vm.add_memory_segment();
    vm.insert_value(ptr, constructor_entrypoints_ptr)?;
    ptr.offset += 1;
    // n_builtins: felt,
    vm.insert_value(ptr, Felt252::ONE)?;
    ptr.offset += 1;
    // builtin_list: felt*,
    let builtins_ptr = vm.add_memory_segment();
    // One builtin: range_check = 138277649577220228665140075
    vm.insert_value(
        builtins_ptr,
        Felt252::from_dec_str("138277649577220228665140075").unwrap(),
    )?;
    vm.insert_value(ptr, builtins_ptr)?;
    ptr.offset += 1;
    // hinted_class_hash: felt,
    vm.insert_value(ptr, Felt252::zero())?;
    ptr.offset += 1;
    // bytecode_length: felt,
    let byte_code = vec![
        Felt252::from_hex("0x480680017fff8000").unwrap().into(),
        Felt252::from_hex("0xe").unwrap().into(),
        Felt252::from_hex("0x208b7fff7fff7ffe").unwrap().into(),
        Felt252::from_hex("0x40780017fff7fff").unwrap().into(),
        Felt252::from_hex("0x1").unwrap().into(),
        Felt252::from_hex("0x4003800080007ffc").unwrap().into(),
        Felt252::from_hex("0x4826800180008000").unwrap().into(),
        Felt252::from_hex("0x1").unwrap().into(),
        Felt252::from_hex("0x480a7ffd7fff8000").unwrap().into(),
        Felt252::from_hex("0x4828800080007ffe").unwrap().into(),
        Felt252::from_hex("0x480a80007fff8000").unwrap().into(),
        Felt252::from_hex("0x208b7fff7fff7ffe").unwrap().into(),
        Felt252::from_hex("0x402b7ffd7ffc7ffd").unwrap().into(),
        Felt252::from_hex("0x1104800180018000").unwrap().into(),
        Felt252::from_hex("0x800000000000010fffffffffffffffffffffffffffffffffffffffffffffff4")
            .unwrap()
            .into(),
        Felt252::from_hex("0x480280017ffb8000").unwrap().into(),
        Felt252::from_hex("0x1104800180018000").unwrap().into(),
        Felt252::from_hex("0x800000000000010fffffffffffffffffffffffffffffffffffffffffffffff4")
            .unwrap()
            .into(),
        Felt252::from_hex("0x480280007ffb8000").unwrap().into(),
        Felt252::from_hex("0x48127ffc7fff8000").unwrap().into(),
        Felt252::from_hex("0x48127ffc7fff8000").unwrap().into(),
        Felt252::from_hex("0x48127ffc7fff8000").unwrap().into(),
        Felt252::from_hex("0x208b7fff7fff7ffe").unwrap().into(),
    ];
    vm.insert_value(ptr, Felt252::from(byte_code.len()))?;
    ptr.offset += 1;
    // bytecode_ptr: felt*,
    let byte_code_ptr = vm.add_memory_segment();
    vm.load_data(byte_code_ptr, &byte_code)?;
    vm.insert_value(ptr, byte_code_ptr)?;

    Ok(HintExtension::default())
}

const VM_LOAD_PROGRAM: &str =
    "vm_load_program(compiled_class.program, ids.compiled_class.bytecode_ptr)";
pub fn vm_load_program(
    hint_processor: &dyn HintProcessor,
    vm: &mut VirtualMachine,
    ids_data: &HashMap<String, HintReference>,
    ap_tracking: &ApTracking,
) -> Result<HintExtension, HintError> {
    // We will be hardcoding the hint-related values instead of taking them from the compiled contract
    // The contract has the following hint:
    /*
    "0": [
                {
                    "accessible_scopes": [
                        "__main__",
                        "__main__",
                        "__main__.get_number"
                    ],
                    "code": "print(\"hello world\")",
                    "flow_tracking_data": {
                        "ap_tracking": {
                            "group": 0,
                            "offset": 0
                        },
                        "reference_ids": {}
                    }
                }
            ],
     */
    let hint_code = "print(\"hello world\")";
    let hint_ap_tracking_data = ApTracking::default();
    let reference_ids = HashMap::default();
    let references = vec![];
    // Compile the hint
    let compiled_hint = hint_processor.compile_hint(
        hint_code,
        &hint_ap_tracking_data,
        &reference_ids,
        &references,
    )?;
    // Create the hint extension
    // As the hint from the compiled constract has offset 0, the hint pc will be equal to the loaded contract's program base:
    // This ptr can be found at ids.compiled_class.bytecode_ptr
    let compiled_class = get_ptr_from_var_name("compiled_class", vm, ids_data, ap_tracking)?;
    // ids.compiled_class.bytecode_ptr = [ids.compiled_class + 11]
    let byte_code_ptr = vm.get_relocatable((compiled_class + 11)?)?;
    let hint_extension = HashMap::from([(byte_code_ptr, vec![compiled_hint])]);
    Ok(hint_extension)
}

const HELLO_WORLD: &str = "print(\"hello world\")";
pub fn hello_world() -> Result<HintExtension, HintError> {
    #[cfg(feature = "std")]
    println!("hello world");
    Ok(HintExtension::default())
}

#[test]
#[cfg_attr(target_arch = "wasm32", wasm_bindgen_test)]
fn run_deprecated_cc() {
    let mut hint_processor = SimplifiedOsHintProcessor::default();
    let program_content =
        include_bytes!("../../../cairo_programs/noretrocompat/starknet_os_deprecated_cc.json");
    let (runner, _) = cairo_run(
        program_content,
        &CairoRunConfig {
            trace_enabled: true,
            ..Default::default()
        },
        &mut hint_processor,
    )
    .unwrap();
    // Check trace against cairo-lang vm run
    assert_eq!(
        runner.relocated_trace,
        Some(vec![
            RelocatedTraceEntry {
                pc: 4,
                ap: 12,
                fp: 12
            },
            RelocatedTraceEntry {
                pc: 6,
                ap: 15,
                fp: 12
            },
            RelocatedTraceEntry {
                pc: 7,
                ap: 15,
                fp: 12
            },
            RelocatedTraceEntry {
                pc: 8,
                ap: 15,
                fp: 12
            },
            RelocatedTraceEntry {
                pc: 35,
                ap: 17,
                fp: 17
            },
            RelocatedTraceEntry {
                pc: 37,
                ap: 18,
                fp: 17
            },
            RelocatedTraceEntry {
                pc: 9,
                ap: 18,
                fp: 12
            }
        ])
    );
}
