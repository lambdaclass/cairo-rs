use crate::{
    math_utils::{div_mod, safe_div},
    serde::deserialize_program::ApTracking,
    types::exec_scope::PyValueType,
    vm::{
        errors::vm_errors::VirtualMachineError,
        hints::{
            hint_utils::get_int_from_scope_ref,
            secp::secp_utils::{pack_from_var_name, N},
        },
        vm_core::VirtualMachine,
    },
};
use num_bigint::BigInt;
use std::collections::HashMap;

/* Implements hint:
from starkware.cairo.common.cairo_secp.secp_utils import N, pack
from starkware.python.math_utils import div_mod, safe_div

a = pack(ids.a, PRIME)
b = pack(ids.b, PRIME)
value = res = div_mod(a, b, N)
*/
pub fn div_mod_n_packed_divmod(
    vm: &mut VirtualMachine,
    ids: &HashMap<String, BigInt>,
    hint_ap_tracking: Option<&ApTracking>,
) -> Result<(), VirtualMachineError> {
    let a = pack_from_var_name("a", ids, vm, hint_ap_tracking)?;
    let b = pack_from_var_name("b", ids, vm, hint_ap_tracking)?;

    let value = div_mod(&a, &b, &N);

    vm.exec_scopes
        .assign_or_update_variable("a", PyValueType::BigInt(a));
    vm.exec_scopes
        .assign_or_update_variable("b", PyValueType::BigInt(b));
    vm.exec_scopes
        .assign_or_update_variable("value", PyValueType::BigInt(value.clone()));
    vm.exec_scopes
        .assign_or_update_variable("res", PyValueType::BigInt(value));
    Ok(())
}

// Implements hint:
// value = k = safe_div(res * b - a, N)
pub fn div_mod_n_safe_div(vm: &mut VirtualMachine) -> Result<(), VirtualMachineError> {
    let a = get_int_from_scope_ref(vm, "a")?.clone();
    let b = get_int_from_scope_ref(vm, "b")?.clone();
    let res = get_int_from_scope_ref(vm, "res")?;

    let value = safe_div(&(res * b - a), &N)?;

    vm.exec_scopes
        .assign_or_update_variable("value", PyValueType::BigInt(value));
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        bigint, bigint_str,
        types::{instruction::Register, relocatable::MaybeRelocatable},
        utils::test_utils::{mayberelocatable, memory, memory_from_memory, memory_inner},
        vm::{
            errors::memory_errors::MemoryError,
            hints::execute_hint::{BuiltinHintExecutor, HintReference},
            vm_memory::memory::Memory,
        },
    };
    use num_bigint::Sign;

    static HINT_EXECUTOR: BuiltinHintExecutor = BuiltinHintExecutor {};
    #[test]
    fn safe_div_ok() {
        let mut vm = VirtualMachine::new(
            BigInt::new(Sign::Plus, vec![1, 0, 0, 0, 0, 0, 17, 134217728]),
            Vec::new(),
            false,
            &HINT_EXECUTOR,
        );

        vm.memory = memory![
            ((0, 0), 15),
            ((0, 1), 3),
            ((0, 2), 40),
            ((0, 3), 0),
            ((0, 4), 10),
            ((0, 5), 1)
        ];
        vm.run_context.fp = mayberelocatable!(0, 3);

        vm.references = HashMap::new();
        for i in 0..=3 {
            vm.references.insert(
                i,
                HintReference {
                    dereference: true,
                    register: Register::FP,
                    offset1: i as i32 - 3,
                    offset2: 0,
                    inner_dereference: false,
                    ap_tracking_data: None,
                    immediate: None,
                },
            );
        }

        let ids: HashMap<String, BigInt> = HashMap::from([
            ("a".to_string(), bigint!(0_i32)),
            ("b".to_string(), bigint!(3_i32)),
        ]);
        assert!(div_mod_n_packed_divmod(&mut vm, &ids, None).is_ok());
        assert!(div_mod_n_safe_div(&mut vm).is_ok());
    }

    #[test]
    fn safe_div_fail() {
        let mut vm = VirtualMachine::new(
            BigInt::new(Sign::Plus, vec![1, 0, 0, 0, 0, 0, 17, 134217728]),
            Vec::new(),
            false,
            &HINT_EXECUTOR,
        );

        vm.exec_scopes
            .assign_or_update_variable("a", PyValueType::BigInt(bigint!(0_usize)));
        vm.exec_scopes
            .assign_or_update_variable("b", PyValueType::BigInt(bigint!(1_usize)));
        vm.exec_scopes
            .assign_or_update_variable("res", PyValueType::BigInt(bigint!(1_usize)));

        assert_eq!(Err(VirtualMachineError::SafeDivFail(bigint!(1_usize), bigint_str!(b"115792089237316195423570985008687907852837564279074904382605163141518161494337"))), div_mod_n_safe_div(&mut vm));
    }
}
