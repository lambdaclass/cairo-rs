use num_integer::Integer;

use crate::hint_processor::builtin_hint_processor::secp::bigint_utils::BigInt3;
use crate::hint_processor::builtin_hint_processor::secp::secp_utils::SECP_P;
use crate::hint_processor::hint_processor_definition::HintReference;
use crate::serde::deserialize_program::ApTracking;
use crate::stdlib::collections::HashMap;
use crate::stdlib::prelude::String;
use crate::types::exec_scope::ExecutionScopes;
use crate::vm::errors::hint_errors::HintError;
use crate::vm::vm_core::VirtualMachine;

/// Implements hint:
/// ```python
/// from starkware.cairo.common.cairo_secp.secp_utils import pack
/// SECP_P=2**255-19
///
/// x = pack(ids.x, PRIME) % SECP_P
/// ```
pub fn assign_pack_mod_secp_prime_to_x(
    vm: &mut VirtualMachine,
    exec_scopes: &mut ExecutionScopes,
    ids_data: &HashMap<String, HintReference>,
    ap_tracking: &ApTracking,
) -> Result<(), HintError> {
    let x = BigInt3::from_var_name("x", vm, ids_data, ap_tracking)?.pack86();
    exec_scopes.insert_value("x", x.mod_floor(&SECP_P));

    Ok(())
}

/// Implements hint:
/// ```python
/// from starkware.cairo.common.cairo_secp.secp_utils import pack
/// SECP_P=2**255-19
///
/// value = pack(ids.x, PRIME) % SECP_P
/// ```
pub fn assign_pack_mod_secp_prime_to_value() -> Result<(), HintError> {
    todo!()
}

/// Implements hint:
/// ```python
/// SECP_P=2**255-19
/// from starkware.python.math_utils import div_mod
///
/// value = x_inv = div_mod(1, x, SECP_P)
/// ```
pub fn assign_div_mod_1_x_secp_prime_to_x_inv_and_value() -> Result<(), HintError> {
    todo!()
}

#[cfg(test)]
mod test {
    use crate::any_box;
    use crate::hint_processor::builtin_hint_processor::builtin_hint_processor_definition::BuiltinHintProcessor;
    use crate::hint_processor::builtin_hint_processor::builtin_hint_processor_definition::HintProcessorData;
    use crate::hint_processor::builtin_hint_processor::hint_code;
    use crate::hint_processor::builtin_hint_processor::secp::secp_utils::SECP_P;
    use crate::hint_processor::hint_processor_definition::HintProcessor;
    use crate::hint_processor::hint_processor_definition::HintReference;
    use crate::stdlib::collections::HashMap;
    use crate::types::exec_scope::ExecutionScopes;
    use crate::utils::test_utils::*;
    use crate::vm::vm_core::VirtualMachine;
    use assert_matches::assert_matches;
    use num_bigint::BigInt;
    use num_traits::Zero;

    fn assert_assign_pack_mod_secp_prime_to_x_ok(
        x_d0: i128,
        x_d1: i128,
        x_d2: i128,
        expected: BigInt,
    ) {
        let ids_data = non_continuous_ids_data![("x", 0)];

        let mut vm = vm!();
        vm.run_context.fp = 0;
        add_segments!(vm, 3); // Alloc space for `ids.x.d0`, `ids.x.d1` and `ids.x.d2`.

        vm.segments = segments![((1, 0), x_d0), ((1, 1), x_d1), ((1, 2), x_d2)];

        let mut exec_scopes = ExecutionScopes::new();
        assert_matches!(
            run_hint!(
                vm,
                ids_data,
                hint_code::ASSIGN_PACK_MOD_SECP_PRIME_TO_X,
                &mut exec_scopes
            ),
            Ok(())
        );

        let x_result = exec_scopes.get::<BigInt>("x");
        assert!(x_result.is_ok());
        assert_eq!(x_result.unwrap(), expected);
    }

    #[test]
    fn run_assign_pack_mod_secp_prime_to_x_with_zero() {
        assert_assign_pack_mod_secp_prime_to_x_ok(0_i128, 0_i128, 0_i128, BigInt::zero());
    }

    #[test]
    fn run_assign_pack_mod_secp_prime_to_x_with_secp_prime_minus_one() {
        assert_assign_pack_mod_secp_prime_to_x_ok(
            // SECP_P - 1:
            77371252455336262886226990_i128,
            77371252455336267181195263_i128,
            19342813113834066795298815_i128,
            SECP_P.clone() - 1,
        );
    }

    #[test]
    fn run_assign_pack_mod_secp_prime_to_x_with_secp_prime() {
        assert_assign_pack_mod_secp_prime_to_x_ok(
            // SECP_P:
            77371252455336262886226991_i128,
            77371252455336267181195263_i128,
            19342813113834066795298815_i128,
            BigInt::zero(),
        );
    }

    #[test]
    fn run_assign_pack_mod_secp_prime_to_value_with_zero() {
        assert_assign_pack_mod_secp_prime_to_value_ok(0, 0, 0, BigInt::zero());
    }

    #[test]
    fn run_assign_div_mod_1_x_secp_prime_to_x_inv_and_value_ok() {
        todo!()
    }
}
