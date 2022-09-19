use crate::bigint;
use crate::hint_processor::proxies::memory_proxy::MemoryProxy;
use crate::hint_processor::proxies::vm_proxy::VMProxy;
use crate::serde::deserialize_program::ApTracking;
use crate::types::relocatable::MaybeRelocatable;
use crate::vm::errors::vm_errors::VirtualMachineError;
use num_bigint::BigInt;
use num_traits::{ToPrimitive, Zero};
use std::cell::Ref;
use std::collections::HashMap;

use crate::hint_processor::builtin_hint_processor::hint_utils::{
    get_integer_from_var_name, get_ptr_from_var_name, insert_value_from_var_name,
};
use crate::hint_processor::hint_processor_definition::HintReference;

pub fn set_add<'a>(
    vm_proxy: &'a mut VMProxy,
    ids_data: &HashMap<String, HintReference>,
    ap_tracking: &ApTracking,
) -> Result<(), VirtualMachineError> {
    let set_ptr = get_ptr_from_var_name("set_ptr", vm_proxy, ids_data, ap_tracking)?;
    let elm_size = get_integer_from_var_name("elm_size", vm_proxy, ids_data, ap_tracking)?
        .to_usize()
        .ok_or(VirtualMachineError::BigintToUsizeFail)?;
    let elm_ptr = get_ptr_from_var_name("elm_ptr", vm_proxy, ids_data, ap_tracking)?;
    let set_end_ptr = get_ptr_from_var_name("set_end_ptr", vm_proxy, ids_data, ap_tracking)?;

    if elm_size.is_zero() {
        return Err(VirtualMachineError::ValueNotPositive(bigint!(elm_size)));
    }

    let elm = {
        let memory: Ref<MemoryProxy> = vm_proxy.memory.borrow();
        memory
            .get_range(&MaybeRelocatable::from(elm_ptr), elm_size)
            .map_err(VirtualMachineError::MemoryError)?
    };

    if set_ptr > set_end_ptr {
        return Err(VirtualMachineError::InvalidSetRange(
            MaybeRelocatable::from(set_ptr),
            MaybeRelocatable::from(set_end_ptr),
        ));
    }

    let range_limit = set_end_ptr.sub_rel(&set_ptr)?;
    for i in (0..range_limit).step_by(elm_size) {
        let set_iter = {
            let memory: Ref<MemoryProxy> = vm_proxy.memory.borrow();
            memory
                .get_range(
                    &MaybeRelocatable::from(set_ptr.clone() + i as usize),
                    elm_size,
                )
                .map_err(VirtualMachineError::MemoryError)?
                .clone()
        };
        if set_iter == elm {
            insert_value_from_var_name(
                "index",
                bigint!(i / elm_size),
                vm_proxy,
                ids_data,
                ap_tracking,
            )?;
            return insert_value_from_var_name(
                "is_elm_in_set",
                bigint!(1),
                vm_proxy,
                ids_data,
                ap_tracking,
            );
        }
    }
    insert_value_from_var_name("is_elm_in_set", bigint!(0), vm_proxy, ids_data, ap_tracking)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::any_box;
    use crate::hint_processor::builtin_hint_processor::builtin_hint_processor_definition::BuiltinHintProcessor;
    use crate::hint_processor::builtin_hint_processor::builtin_hint_processor_definition::HintProcessorData;
    use crate::hint_processor::hint_processor_definition::HintProcessor;
    use crate::hint_processor::proxies::exec_scopes_proxy::get_exec_scopes_proxy;
    use crate::hint_processor::proxies::vm_proxy::get_vm_proxy;
    use crate::types::exec_scope::ExecutionScopes;
    use crate::utils::test_utils::*;
    use crate::vm::errors::memory_errors::MemoryError;
    use crate::vm::runners::builtin_runner::RangeCheckBuiltinRunner;
    use crate::vm::vm_core::VirtualMachine;
    use crate::vm::vm_memory::memory::Memory;
    use num_bigint::Sign;
    use std::any::Any;
    use std::{cell::RefCell, rc::Rc};

    const HINT_CODE: &str = "assert ids.elm_size > 0\nassert ids.set_ptr <= ids.set_end_ptr\nelm_list = memory.get_range(ids.elm_ptr, ids.elm_size)\nfor i in range(0, ids.set_end_ptr - ids.set_ptr, ids.elm_size):\n    if memory.get_range(ids.set_ptr + i, ids.elm_size) == elm_list:\n        ids.index = i // ids.elm_size\n        ids.is_elm_in_set = 1\n        break\nelse:\n    ids.is_elm_in_set = 0";

    fn init_vm_ids_data(
        set_ptr: Option<(usize, usize)>,
        elm_size: Option<i32>,
        elm_a: Option<usize>,
        elm_b: Option<usize>,
    ) -> (VirtualMachine, HashMap<String, HintReference>) {
        let mut vm = vm_with_range_check!();

        vm.run_context.fp = 6;

        let set_ptr = set_ptr.unwrap_or((2, 0));
        let elm_size = elm_size.unwrap_or(2);
        let elm_a = elm_a.unwrap_or(2);
        let elm_b = elm_b.unwrap_or(3);

        vm.memory = memory![
            ((1, 2), (set_ptr.0, set_ptr.1)),
            ((1, 3), elm_size),
            ((1, 4), (3, 0)),
            ((1, 5), (2, 2)),
            ((2, 0), 1),
            ((2, 1), 3),
            ((2, 2), 5),
            ((2, 3), 7),
            ((3, 0), elm_a),
            ((3, 1), elm_b)
        ];
        let ids_data = ids_data![
            "is_elm_in_set",
            "index",
            "set_ptr",
            "elm_size",
            "elm_ptr",
            "set_end_ptr"
        ];

        (vm, ids_data)
    }

    #[test]
    fn set_add_new_elem() {
        let (mut vm, ids_data) = init_vm_ids_data(None, None, None, None);
        assert_eq!(run_hint!(vm, ids_data, HINT_CODE), Ok(()));
        assert_eq!(
            vm.memory.borrow().get(&MaybeRelocatable::from((1, 0))),
            Ok(Some(&MaybeRelocatable::Int(bigint!(0))))
        );
    }

    #[test]
    fn set_add_already_exists() {
        let (mut vm, ids_data) = init_vm_ids_data(None, None, Some(1), Some(3));
        assert_eq!(run_hint!(vm, ids_data, HINT_CODE), Ok(()));
        check_memory![vm.memory, ((1, 0), 1), ((1, 1), 0)];
    }

    #[test]
    fn elm_size_negative() {
        let (mut vm, ids_data) = init_vm_ids_data(None, Some(-2), None, None);
        assert_eq!(
            run_hint!(vm, ids_data, HINT_CODE),
            Err(VirtualMachineError::BigintToUsizeFail)
        );
    }

    #[test]
    fn elm_size_zero() {
        let int = bigint!(0_i32);
        let (mut vm, ids_data) = init_vm_ids_data(None, Some(0), None, None);
        assert_eq!(
            run_hint!(vm, ids_data, HINT_CODE),
            Err(VirtualMachineError::ValueNotPositive(int))
        );
    }
    #[test]
    fn set_ptr_gt_set_end_ptr() {
        let (mut vm, ids_data) = init_vm_ids_data(Some((2, 3)), None, None, None);
        assert_eq!(
            run_hint!(vm, ids_data, HINT_CODE),
            Err(VirtualMachineError::InvalidSetRange(
                MaybeRelocatable::from((2, 3)),
                MaybeRelocatable::from((2, 2)),
            ))
        );
    }
}
