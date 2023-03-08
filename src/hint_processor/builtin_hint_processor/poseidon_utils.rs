use std::collections::HashMap;

use felt::Felt;
use num_traits::{One, Zero};

use crate::{
    hint_processor::hint_processor_definition::HintReference,
    serde::deserialize_program::ApTracking,
    vm::{errors::hint_errors::HintError, vm_core::VirtualMachine},
};

use super::hint_utils::{get_integer_from_var_name, insert_value_into_ap};

// Implements hint: "memory[ap] = to_felt_or_relocatable(ids.n >= 10)"
pub fn n_greater_than_10(
    vm: &mut VirtualMachine,
    ids_data: &HashMap<String, HintReference>,
    ap_tracking: &ApTracking,
) -> Result<(), HintError> {
    let n = get_integer_from_var_name("n", vm, ids_data, ap_tracking)?;

    let value = if n.as_ref() >= &Felt::from(10) {
        Felt::one()
    } else {
        Felt::zero()
    };
    insert_value_into_ap(vm, value)
}

// Implements hint: "memory[ap] = to_felt_or_relocatable(ids.n >= 2)"
pub fn n_greater_than_2(
    vm: &mut VirtualMachine,
    ids_data: &HashMap<String, HintReference>,
    ap_tracking: &ApTracking,
) -> Result<(), HintError> {
    let n = get_integer_from_var_name("n", vm, ids_data, ap_tracking)?;

    let value = if n.as_ref() >= &Felt::from(2) {
        Felt::one()
    } else {
        Felt::zero()
    };
    insert_value_into_ap(vm, value)
}

#[cfg(test)]
mod tests {
    use crate::any_box;
    use crate::hint_processor::builtin_hint_processor::builtin_hint_processor_definition::BuiltinHintProcessor;
    use crate::hint_processor::builtin_hint_processor::builtin_hint_processor_definition::HintProcessorData;
    use crate::hint_processor::builtin_hint_processor::poseidon_utils::HashMap;
    use crate::hint_processor::hint_processor_definition::HintProcessor;
    use crate::hint_processor::hint_processor_definition::HintReference;
    use crate::types::exec_scope::ExecutionScopes;
    use crate::types::relocatable::MaybeRelocatable;
    use crate::vm::errors::memory_errors::MemoryError;
    use crate::vm::vm_core::VirtualMachine;
    use crate::vm::vm_memory::memory::Memory;
    use crate::vm::vm_memory::memory_segments::MemorySegmentManager;
    use crate::{hint_processor::builtin_hint_processor::hint_code, utils::test_utils::*};
    use assert_matches::assert_matches;
    use std::any::Any;

    #[test]
    fn run_n_greater_than_10_true() {
        let hint_code = hint_code::NONDET_N_GREATER_THAN_10;
        let mut vm = vm!();
        vm.set_ap(3);
        vm.segments = segments![((1, 0), 21)];
        vm.set_fp(1);
        let ids_data = ids_data!("n");
        assert_matches!(run_hint!(vm, ids_data, hint_code), Ok(()));
        //Check hint memory inserts
        check_memory![vm.segments.memory, ((1, 3), 1)];
    }

    #[test]
    fn run_n_greater_than_10_false() {
        let hint_code = hint_code::NONDET_N_GREATER_THAN_10;
        let mut vm = vm!();
        vm.set_ap(3);
        vm.segments = segments![((1, 0), 9)];
        vm.set_fp(1);
        let ids_data = ids_data!("n");
        assert_matches!(run_hint!(vm, ids_data, hint_code), Ok(()));
        //Check hint memory inserts
        check_memory![vm.segments.memory, ((1, 3), 0)];
    }

    #[test]
    fn run_n_greater_than_2_true() {
        let hint_code = hint_code::NONDET_N_GREATER_THAN_2;
        let mut vm = vm!();
        vm.set_ap(3);
        vm.segments = segments![((1, 0), 6)];
        vm.set_fp(1);
        let ids_data = ids_data!("n");
        assert_matches!(run_hint!(vm, ids_data, hint_code), Ok(()));
        //Check hint memory inserts
        check_memory![vm.segments.memory, ((1, 3), 1)];
    }

    #[test]
    fn run_n_greater_than_2_false() {
        let hint_code = hint_code::NONDET_N_GREATER_THAN_2;
        let mut vm = vm!();
        vm.set_ap(3);
        vm.segments = segments![((1, 0), 1)];
        vm.set_fp(1);
        let ids_data = ids_data!("n");
        assert_matches!(run_hint!(vm, ids_data, hint_code), Ok(()));
        //Check hint memory inserts
        check_memory![vm.segments.memory, ((1, 3), 0)];
    }
}
