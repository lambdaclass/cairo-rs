use crate::Felt252;
use crate::{
    any_box,
    hint_processor::{
        builtin_hint_processor::{hint_utils::get_integer_from_var_name, secp::secp_utils::BETA},
        hint_processor_definition::HintReference,
    },
    math_utils::{div_mod, safe_div_bigint},
    serde::deserialize_program::ApTracking,
    stdlib::{collections::HashMap, ops::Shr, prelude::*},
    types::exec_scope::ExecutionScopes,
    vm::{errors::hint_errors::HintError, vm_core::VirtualMachine},
};
use core::ops::Add;
use num_bigint::BigInt;
use num_integer::Integer;

use super::{
    bigint_utils::Uint384,
    secp_utils::{N, SECP_P},
};

/* Implements hint:
from starkware.cairo.common.cairo_secp.secp_utils import N, pack
from starkware.python.math_utils import div_mod, safe_div

a = pack(ids.a, PRIME)
b = pack(ids.b, PRIME)
value = res = div_mod(a, b, N)
*/
pub fn div_mod_n_packed(
    vm: &mut VirtualMachine,
    exec_scopes: &mut ExecutionScopes,
    ids_data: &HashMap<String, HintReference>,
    ap_tracking: &ApTracking,
    n: &BigInt,
) -> Result<(), HintError> {
    let a = Uint384::from_var_name("a", vm, ids_data, ap_tracking)?.pack86();
    let b = Uint384::from_var_name("b", vm, ids_data, ap_tracking)?.pack86();

    let value = div_mod(&a, &b, n)?;
    exec_scopes.insert_value("a", a);
    exec_scopes.insert_value("b", b);
    exec_scopes.insert_value("value", value.clone());
    exec_scopes.insert_value("res", value);
    Ok(())
}

pub fn div_mod_n_packed_divmod(
    vm: &mut VirtualMachine,
    exec_scopes: &mut ExecutionScopes,
    ids_data: &HashMap<String, HintReference>,
    ap_tracking: &ApTracking,
) -> Result<(), HintError> {
    exec_scopes.assign_or_update_variable("N", any_box!(N.clone()));
    div_mod_n_packed(vm, exec_scopes, ids_data, ap_tracking, &N)
}

pub fn div_mod_n_packed_external_n(
    vm: &mut VirtualMachine,
    exec_scopes: &mut ExecutionScopes,
    ids_data: &HashMap<String, HintReference>,
    ap_tracking: &ApTracking,
) -> Result<(), HintError> {
    let n = exec_scopes.get::<BigInt>("N")?;
    div_mod_n_packed(vm, exec_scopes, ids_data, ap_tracking, &n)
}

// Implements hint:
// value = k = safe_div(res * b - a, N)
pub fn div_mod_n_safe_div(
    exec_scopes: &mut ExecutionScopes,
    a_alias: &str,
    b_alias: &str,
    to_add: u64,
) -> Result<(), HintError> {
    let a = exec_scopes.get_ref::<BigInt>(a_alias)?;
    let b = exec_scopes.get_ref::<BigInt>(b_alias)?;
    let res = exec_scopes.get_ref::<BigInt>("res")?;

    let n = exec_scopes.get("N")?;

    let value = safe_div_bigint(&(res * b - a), &n)?.add(to_add);

    exec_scopes.insert_value("value", value);
    Ok(())
}

/* Implements hint:
%{
    from starkware.cairo.common.cairo_secp.secp_utils import SECP_P, pack

    x_cube_int = pack(ids.x_cube, PRIME) % SECP_P
    y_square_int = (x_cube_int + ids.BETA) % SECP_P
    y = pow(y_square_int, (SECP_P + 1) // 4, SECP_P)

    # We need to decide whether to take y or SECP_P - y.
    if ids.v % 2 == y % 2:
        value = y
    else:
        value = (-y) % SECP_P
%}
*/
pub fn get_point_from_x(
    vm: &mut VirtualMachine,
    exec_scopes: &mut ExecutionScopes,
    ids_data: &HashMap<String, HintReference>,
    ap_tracking: &ApTracking,
    constants: &HashMap<String, Felt252>,
) -> Result<(), HintError> {
    exec_scopes.insert_value("SECP_P", SECP_P.clone());
    let beta = constants
        .get(BETA)
        .ok_or_else(|| HintError::MissingConstant(Box::new(BETA)))?
        .to_bigint();

    let x_cube_int = Uint384::from_var_name("x_cube", vm, ids_data, ap_tracking)?
        .pack86()
        .mod_floor(&SECP_P);
    let y_cube_int = (x_cube_int + beta).mod_floor(&SECP_P);
    exec_scopes.insert_value("y_square_int", y_cube_int.clone());
    // Divide by 4
    let mut y = y_cube_int.modpow(&(&*SECP_P + 1_u32).shr(2_u32), &SECP_P);
    exec_scopes.insert_value::<BigInt>("y", y.clone());

    let v = get_integer_from_var_name("v", vm, ids_data, ap_tracking)?.to_bigint();
    if v.is_even() != y.is_even() {
        y = &*SECP_P - y;
    }
    exec_scopes.insert_value("value", y);
    Ok(())
}
/* Implements hint:
    from starkware.cairo.common.cairo_secp.secp_utils import pack
    from starkware.python.math_utils import div_mod, safe_div

    N = 0xfffffffffffffffffffffffffffffffebaaedce6af48a03bbfd25e8cd0364141
    x = pack(ids.x, PRIME) % N
    s = pack(ids.s, PRIME) % N
    value = res = div_mod(x, s, N)
*/
pub fn pack_modn_div_modn(
    vm: &mut VirtualMachine,
    exec_scopes: &mut ExecutionScopes,
    ids_data: &HashMap<String, HintReference>,
    ap_tracking: &ApTracking,
) -> Result<(), HintError> {
    let x = Uint384::from_var_name("x", vm, ids_data, ap_tracking)?
        .pack86()
        .mod_floor(&N);
    let s = Uint384::from_var_name("s", vm, ids_data, ap_tracking)?
        .pack86()
        .mod_floor(&N);

    let value = div_mod(&x, &s, &N)?;
    exec_scopes.insert_value("x", x);
    exec_scopes.insert_value("s", s);
    exec_scopes.insert_value("N", N.clone());
    exec_scopes.insert_value("value", value.clone());
    exec_scopes.insert_value("res", value);
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::stdlib::string::ToString;
    use crate::types::errors::math_errors::MathError;

    use crate::{
        any_box,
        hint_processor::{
            builtin_hint_processor::{
                builtin_hint_processor_definition::{BuiltinHintProcessor, HintProcessorData},
                hint_code,
            },
            hint_processor_definition::HintProcessorLogic,
        },
        types::exec_scope::ExecutionScopes,
        utils::test_utils::*,
    };
    use assert_matches::assert_matches;
    use num_traits::{One, Zero};

    #[cfg(target_arch = "wasm32")]
    use wasm_bindgen_test::*;

    #[test]
    #[cfg_attr(target_arch = "wasm32", wasm_bindgen_test)]
    fn safe_div_ok() {
        // "import N"
        let mut exec_scopes = ExecutionScopes::new();
        exec_scopes.assign_or_update_variable("N", any_box!(N.clone()));

        let hint_codes = vec![
            hint_code::DIV_MOD_N_PACKED_DIVMOD_V1,
            hint_code::DIV_MOD_N_PACKED_DIVMOD_EXTERNAL_N,
        ];
        for hint_code in hint_codes {
            let mut vm = vm!();

            vm.segments = segments![
                ((1, 0), 15),
                ((1, 1), 3),
                ((1, 2), 40),
                ((1, 3), 0),
                ((1, 4), 10),
                ((1, 5), 1)
            ];
            vm.run_context.fp = 3;
            let ids_data = non_continuous_ids_data![("a", -3), ("b", 0)];

            assert_matches!(run_hint!(vm, ids_data, hint_code, &mut exec_scopes), Ok(()));

            assert_matches!(div_mod_n_safe_div(&mut exec_scopes, "a", "b", 0), Ok(()));
            assert_matches!(div_mod_n_safe_div(&mut exec_scopes, "a", "b", 1), Ok(()));
        }
    }

    #[test]
    #[cfg_attr(target_arch = "wasm32", wasm_bindgen_test)]
    fn safe_div_fail() {
        let mut exec_scopes = scope![
            ("a", BigInt::zero()),
            ("b", BigInt::one()),
            ("res", BigInt::one()),
            ("N", N.clone())
        ];
        assert_matches!(
            div_mod_n_safe_div(
                &mut exec_scopes,
                "a",
                "b",
                0,
            ),
            Err(
                HintError::Math(MathError::SafeDivFailBigInt(bx)
            )) if *bx == (BigInt::one(), bigint_str!("115792089237316195423570985008687907852837564279074904382605163141518161494337"))
        );
    }

    #[test]
    #[cfg_attr(target_arch = "wasm32", wasm_bindgen_test)]
    fn get_point_from_x_ok() {
        let hint_code = hint_code::GET_POINT_FROM_X;
        let mut vm = vm!();
        vm.segments = segments![
            ((1, 0), 18),
            ((1, 1), 2147483647),
            ((1, 2), 2147483647),
            ((1, 3), 2147483647)
        ];
        vm.run_context.fp = 1;
        let ids_data = non_continuous_ids_data![("v", -1), ("x_cube", 0)];
        assert_matches!(
            run_hint!(
                vm,
                ids_data,
                hint_code,
                exec_scopes_ref!(),
                &[(BETA, Felt252::from(7)),]
                    .into_iter()
                    .map(|(k, v)| (k.to_string(), v))
                    .collect()
            ),
            Ok(())
        )
    }

    #[test]
    #[cfg_attr(target_arch = "wasm32", wasm_bindgen_test)]
    fn get_point_from_x_negative_y() {
        let hint_code = hint_code::GET_POINT_FROM_X;
        let mut vm = vm!();
        let mut exec_scopes = ExecutionScopes::new();
        vm.segments = segments![
            ((1, 0), 1),
            ((1, 1), 2147483647),
            ((1, 2), 2147483647),
            ((1, 3), 2147483647)
        ];
        vm.run_context.fp = 2;

        let ids_data = ids_data!["v", "x_cube"];
        assert_matches!(
            run_hint!(
                vm,
                ids_data,
                hint_code,
                &mut exec_scopes,
                &[(BETA, Felt252::from(7)),]
                    .into_iter()
                    .map(|(k, v)| (k.to_string(), v))
                    .collect()
            ),
            Ok(())
        );

        check_scope!(
            &exec_scopes,
            [(
                "value",
                bigint_str!(
                    "94274691440067846579164151740284923997007081248613730142069408045642476712539"
                )
            )]
        );
    }

    #[test]
    #[cfg_attr(target_arch = "wasm32", wasm_bindgen_test)]
    fn pack_modn_div_modn_ok() {
        let hint_code = hint_code::PACK_MODN_DIV_MODN;
        let mut exec_scopes = scope![("N", N.clone())];
        let mut vm = vm!();

        vm.segments = segments![
            ((1, 0), 15),
            ((1, 1), 3),
            ((1, 2), 40),
            ((1, 3), 0),
            ((1, 4), 10),
            ((1, 5), 1)
        ];
        vm.run_context.fp = 3;
        let ids_data = non_continuous_ids_data![("x", -3), ("s", 0)];
        assert_matches!(run_hint!(vm, ids_data, hint_code, &mut exec_scopes), Ok(()));
        assert_matches!(div_mod_n_safe_div(&mut exec_scopes, "x", "s", 0), Ok(()));
    }
}
