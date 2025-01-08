use crate::Felt252;
use crate::{
    hint_processor::{
        builtin_hint_processor::{
            hint_utils::{
                get_integer_from_var_name, get_relocatable_from_var_name,
                insert_value_from_var_name, insert_value_into_ap,
            },
            secp::{
                bigint_utils::BigInt3,
                secp_utils::{SECP256R1_ALPHA, SECP256R1_N, SECP_P},
            },
        },
        hint_processor_definition::HintReference,
    },
    math_utils::{ec_double_slope, line_slope},
    serde::deserialize_program::ApTracking,
    stdlib::{collections::HashMap, prelude::*},
    types::exec_scope::ExecutionScopes,
    vm::{errors::hint_errors::HintError, vm_core::VirtualMachine},
};
use num_bigint::{BigInt, BigUint};
use num_integer::Integer;

use num_traits::{One, ToPrimitive, Zero};

use super::secp_utils::SECP256R1_P;

#[derive(Debug, PartialEq)]
pub(crate) struct EcPoint<'a> {
    pub(crate) x: BigInt3<'a>,
    pub(crate) y: BigInt3<'a>,
}
impl EcPoint<'_> {
    pub(crate) fn from_var_name<'a>(
        name: &'a str,
        vm: &'a VirtualMachine,
        ids_data: &'a HashMap<String, HintReference>,
        ap_tracking: &'a ApTracking,
    ) -> Result<EcPoint<'a>, HintError> {
        // Get first addr of EcPoint struct
        let point_addr = get_relocatable_from_var_name(name, vm, ids_data, ap_tracking)?;
        Ok(EcPoint {
            x: BigInt3::from_base_addr(point_addr, &format!("{}.x", name), vm)?,
            y: BigInt3::from_base_addr((point_addr + 3)?, &format!("{}.y", name), vm)?,
        })
    }
}

/*
Implements main logic for `EC_NEGATE` and `EC_NEGATE_EMBEDDED_SECP` hints
*/
pub fn ec_negate(
    vm: &mut VirtualMachine,
    exec_scopes: &mut ExecutionScopes,
    ids_data: &HashMap<String, HintReference>,
    ap_tracking: &ApTracking,
    secp_p: BigInt,
) -> Result<(), HintError> {
    //ids.point
    let point_y = (get_relocatable_from_var_name("point", vm, ids_data, ap_tracking)? + 3i32)?;
    let y_bigint3 = BigInt3::from_base_addr(point_y, "point.y", vm)?;
    let y = y_bigint3.pack86();
    let value = (-y).mod_floor(&secp_p);
    exec_scopes.insert_value("value", value);
    exec_scopes.insert_value("SECP_P", secp_p);
    Ok(())
}

/*
Implements hint:
%{
    from starkware.cairo.common.cairo_secp.secp_utils import SECP_P, pack

    y = pack(ids.point.y, PRIME) % SECP_P
    # The modulo operation in python always returns a nonnegative number.
    value = (-y) % SECP_P
%}
*/
pub fn ec_negate_import_secp_p(
    vm: &mut VirtualMachine,
    exec_scopes: &mut ExecutionScopes,
    ids_data: &HashMap<String, HintReference>,
    ap_tracking: &ApTracking,
) -> Result<(), HintError> {
    ec_negate(vm, exec_scopes, ids_data, ap_tracking, SECP_P.clone())
}

/*
Implements hint:
%{
    from starkware.cairo.common.cairo_secp.secp_utils import pack
    SECP_P = 2**255-19

    y = pack(ids.point.y, PRIME) % SECP_P
    # The modulo operation in python always returns a nonnegative number.
    value = (-y) % SECP_P
%}
*/
pub fn ec_negate_embedded_secp_p(
    vm: &mut VirtualMachine,
    exec_scopes: &mut ExecutionScopes,
    ids_data: &HashMap<String, HintReference>,
    ap_tracking: &ApTracking,
) -> Result<(), HintError> {
    let secp_p = (BigInt::one() << 255) - 19;
    ec_negate(vm, exec_scopes, ids_data, ap_tracking, secp_p)
}

/*
Implements hint:
%{
    from starkware.cairo.common.cairo_secp.secp_utils import SECP_P, pack
    from starkware.python.math_utils import ec_double_slope

    # Compute the slope.
    x = pack(ids.point.x, PRIME)
    y = pack(ids.point.y, PRIME)
    value = slope = ec_double_slope(point=(x, y), alpha=0, p=SECP_P)
%}
*/
pub fn compute_doubling_slope(
    vm: &mut VirtualMachine,
    exec_scopes: &mut ExecutionScopes,
    ids_data: &HashMap<String, HintReference>,
    ap_tracking: &ApTracking,
    point_alias: &str,
    secp_p: &BigInt,
    alpha: &BigInt,
) -> Result<(), HintError> {
    exec_scopes.insert_value("SECP_P", secp_p.clone());
    //ids.point
    let point = EcPoint::from_var_name(point_alias, vm, ids_data, ap_tracking)?;

    let value = ec_double_slope(&(point.x.pack86(), point.y.pack86()), alpha, secp_p)?;
    exec_scopes.insert_value("value", value.clone());
    exec_scopes.insert_value("slope", value);
    Ok(())
}

/*
Implements hint:
%{
    from starkware.cairo.common.cairo_secp.secp_utils import pack
    from starkware.python.math_utils import ec_double_slope

    # Compute the slope.
    x = pack(ids.point.x, PRIME)
    y = pack(ids.point.y, PRIME)
    value = slope = ec_double_slope(point=(x, y), alpha=ALPHA, p=SECP_P)
%}
*/
pub fn compute_doubling_slope_external_consts(
    vm: &mut VirtualMachine,
    exec_scopes: &mut ExecutionScopes,
    ids_data: &HashMap<String, HintReference>,
    ap_tracking: &ApTracking,
) -> Result<(), HintError> {
    //ids.point
    let point = EcPoint::from_var_name("point", vm, ids_data, ap_tracking)?;
    let secp_p: BigInt = exec_scopes.get("SECP_P")?;
    let alpha: BigInt = exec_scopes.get("ALPHA")?;

    let value = ec_double_slope(&(point.x.pack86(), point.y.pack86()), &alpha, &secp_p)?;
    exec_scopes.insert_value("value", value.clone());
    exec_scopes.insert_value("slope", value);
    Ok(())
}

/*
Implements hint:
%{
    from starkware.cairo.common.cairo_secp.secp_utils import SECP_P, pack
    from starkware.python.math_utils import line_slope

    # Compute the slope.
    x0 = pack(ids.point0.x, PRIME)
    y0 = pack(ids.point0.y, PRIME)
    x1 = pack(ids.point1.x, PRIME)
    y1 = pack(ids.point1.y, PRIME)
    value = slope = line_slope(point1=(x0, y0), point2=(x1, y1), p=SECP_P)
%}
*/
pub fn compute_slope_and_assing_secp_p(
    vm: &mut VirtualMachine,
    exec_scopes: &mut ExecutionScopes,
    ids_data: &HashMap<String, HintReference>,
    ap_tracking: &ApTracking,
    point0_alias: &str,
    point1_alias: &str,
    secp_p: &BigInt,
) -> Result<(), HintError> {
    exec_scopes.insert_value("SECP_P", secp_p.clone());
    compute_slope(
        vm,
        exec_scopes,
        ids_data,
        ap_tracking,
        point0_alias,
        point1_alias,
        "SECP_P",
    )
}

pub fn compute_slope(
    vm: &mut VirtualMachine,
    exec_scopes: &mut ExecutionScopes,
    ids_data: &HashMap<String, HintReference>,
    ap_tracking: &ApTracking,
    point0_alias: &str,
    point1_alias: &str,
    secp_p_name: &str,
) -> Result<(), HintError> {
    //ids.point0
    let point0 = EcPoint::from_var_name(point0_alias, vm, ids_data, ap_tracking)?;
    //ids.point1
    let point1 = EcPoint::from_var_name(point1_alias, vm, ids_data, ap_tracking)?;

    let secp_p: BigInt = exec_scopes.get(secp_p_name)?;

    let value = line_slope(
        &(point0.x.pack86(), point0.y.pack86()),
        &(point1.x.pack86(), point1.y.pack86()),
        &secp_p,
    )?;
    exec_scopes.insert_value("value", value.clone());
    exec_scopes.insert_value("slope", value);
    Ok(())
}

/*
Implements hint:
%{from starkware.cairo.common.cairo_secp.secp_utils import pack

slope = pack(ids.slope, PRIME)
x0 = pack(ids.point0.x, PRIME)
x1 = pack(ids.point1.x, PRIME)
y0 = pack(ids.point0.y, PRIME)

value = new_x = (pow(slope, 2, SECP_P) - x0 - x1) % SECP_P
%}
*/
pub fn square_slope_minus_xs(
    vm: &mut VirtualMachine,
    exec_scopes: &mut ExecutionScopes,
    ids_data: &HashMap<String, HintReference>,
    ap_tracking: &ApTracking,
) -> Result<(), HintError> {
    let secp_p = exec_scopes.get::<BigInt>("SECP_P")?;
    let point0 = EcPoint::from_var_name("point0", vm, ids_data, ap_tracking)?;
    let point1 = EcPoint::from_var_name("point1", vm, ids_data, ap_tracking)?;

    let slope = BigInt3::from_var_name("slope", vm, ids_data, ap_tracking)?;
    let slope = slope.pack86();
    let x0 = point0.x.pack86();
    let x1 = point1.x.pack86();
    let y0 = point0.y.pack86();

    let value = (slope.pow(2) - &x0 - &x1).mod_floor(&secp_p);

    exec_scopes.insert_value("slope", slope);
    exec_scopes.insert_value("x0", x0);
    exec_scopes.insert_value("x1", x1);
    exec_scopes.insert_value("y0", y0);
    exec_scopes.insert_value("value", value.clone());
    exec_scopes.insert_value("new_x", value);

    Ok(())
}

pub fn ec_double_assign_new_x_v2(
    vm: &mut VirtualMachine,
    exec_scopes: &mut ExecutionScopes,
    ids_data: &HashMap<String, HintReference>,
    ap_tracking: &ApTracking,
    point_alias: &str,
) -> Result<(), HintError> {
    let secp_p: BigInt = exec_scopes.get("SECP_P")?;
    ec_double_assign_new_x(vm, exec_scopes, ids_data, ap_tracking, &secp_p, point_alias)
}

/*
Implements hint:
%{
    from starkware.cairo.common.cairo_secp.secp_utils import SECP_P, pack

    slope = pack(ids.slope, PRIME)
    x = pack(ids.point.x, PRIME)
    y = pack(ids.point.y, PRIME)

    value = new_x = (pow(slope, 2, SECP_P) - 2 * x) % SECP_P
%}
*/
pub fn ec_double_assign_new_x(
    vm: &mut VirtualMachine,
    exec_scopes: &mut ExecutionScopes,
    ids_data: &HashMap<String, HintReference>,
    ap_tracking: &ApTracking,
    secp_p: &BigInt,
    point_alias: &str,
) -> Result<(), HintError> {
    exec_scopes.insert_value("SECP_P", secp_p.clone());
    //ids.slope
    let slope = BigInt3::from_var_name("slope", vm, ids_data, ap_tracking)?;
    //ids.point
    let point = EcPoint::from_var_name(point_alias, vm, ids_data, ap_tracking)?;

    let slope = slope.pack86().mod_floor(secp_p);
    let x = point.x.pack86().mod_floor(secp_p);
    let y = point.y.pack86().mod_floor(secp_p);

    let value = (slope.pow(2) - (&x << 1u32)).mod_floor(secp_p);

    //Assign variables to vm scope
    exec_scopes.insert_value("slope", slope);
    exec_scopes.insert_value("x", x);
    exec_scopes.insert_value("y", y);
    exec_scopes.insert_value("value", value.clone());
    exec_scopes.insert_value("new_x", value);
    Ok(())
}

/*
Implements hint:
%{ value = new_y = (slope * (x - new_x) - y) % SECP_P %}
*/
pub fn ec_double_assign_new_y(exec_scopes: &mut ExecutionScopes) -> Result<(), HintError> {
    //Get variables from vm scope
    let (slope, x, new_x, y, secp_p) = (
        exec_scopes.get::<BigInt>("slope")?,
        exec_scopes.get::<BigInt>("x")?,
        exec_scopes.get::<BigInt>("new_x")?,
        exec_scopes.get::<BigInt>("y")?,
        exec_scopes.get::<BigInt>("SECP_P")?,
    );

    let value = (slope * (x - new_x) - y).mod_floor(&secp_p);
    exec_scopes.insert_value("value", value.clone());
    exec_scopes.insert_value("new_y", value);
    Ok(())
}

/*
Implements hint:
%{
    from starkware.cairo.common.cairo_secp.secp_utils import SECP_P, pack

    slope = pack(ids.slope, PRIME)
    x0 = pack(ids.point0.x, PRIME)
    x1 = pack(ids.point1.x, PRIME)
    y0 = pack(ids.point0.y, PRIME)

    value = new_x = (pow(slope, 2, SECP_P) - x0 - x1) % SECP_P
%}
*/
pub fn fast_ec_add_assign_new_x(
    vm: &mut VirtualMachine,
    exec_scopes: &mut ExecutionScopes,
    ids_data: &HashMap<String, HintReference>,
    ap_tracking: &ApTracking,
    secp_p: &BigInt,
    point0_alias: &str,
    point1_alias: &str,
) -> Result<(), HintError> {
    exec_scopes.insert_value("SECP_P", secp_p.clone());
    //ids.slope
    let slope = BigInt3::from_var_name("slope", vm, ids_data, ap_tracking)?;
    //ids.point0
    let point0 = EcPoint::from_var_name(point0_alias, vm, ids_data, ap_tracking)?;
    //ids.point1.x
    let point1 = EcPoint::from_var_name(point1_alias, vm, ids_data, ap_tracking)?;

    let slope = slope.pack86().mod_floor(secp_p);
    let x0 = point0.x.pack86().mod_floor(secp_p);
    let x1 = point1.x.pack86().mod_floor(secp_p);
    let y0 = point0.y.pack86().mod_floor(secp_p);

    let value = (&slope * &slope - &x0 - &x1).mod_floor(secp_p);
    //Assign variables to vm scope
    exec_scopes.insert_value("slope", slope);
    exec_scopes.insert_value("x0", x0);
    exec_scopes.insert_value("y0", y0);
    exec_scopes.insert_value("value", value.clone());
    exec_scopes.insert_value("new_x", value);

    Ok(())
}

/*
Implements hint:
%{ value = new_y = (slope * (x0 - new_x) - y0) % SECP_P %}
*/
pub fn fast_ec_add_assign_new_y(exec_scopes: &mut ExecutionScopes) -> Result<(), HintError> {
    //Get variables from vm scope
    let (slope, x0, new_x, y0, secp_p) = (
        exec_scopes.get::<BigInt>("slope")?,
        exec_scopes.get::<BigInt>("x0")?,
        exec_scopes.get::<BigInt>("new_x")?,
        exec_scopes.get::<BigInt>("y0")?,
        exec_scopes.get::<BigInt>("SECP_P")?,
    );
    let value = (slope * (x0 - new_x) - y0).mod_floor(&secp_p);
    exec_scopes.insert_value("value", value.clone());
    exec_scopes.insert_value("new_y", value);

    Ok(())
}

/*
Implements hint:
%{ memory[ap] = (ids.scalar % PRIME) % 2 %}
*/
pub fn ec_mul_inner(
    vm: &mut VirtualMachine,
    ids_data: &HashMap<String, HintReference>,
    ap_tracking: &ApTracking,
) -> Result<(), HintError> {
    //(ids.scalar % PRIME) % 2
    let scalar = get_integer_from_var_name("scalar", vm, ids_data, ap_tracking)?
        .as_ref()
        .mod_floor(&Felt252::TWO.try_into().expect("nonzero by construction"));
    insert_value_into_ap(vm, scalar)
}

/*
Implements hint:
%{ from starkware.cairo.common.cairo_secp.secp256r1_utils import SECP256R1_ALPHA as ALPHA %}
*/
pub fn import_secp256r1_alpha(exec_scopes: &mut ExecutionScopes) -> Result<(), HintError> {
    exec_scopes.insert_value("ALPHA", SECP256R1_ALPHA.clone());
    Ok(())
}

/*
Implements hint:
%{ from starkware.cairo.common.cairo_secp.secp256r1_utils import SECP256R1_N as N %}
*/
pub fn import_secp256r1_n(exec_scopes: &mut ExecutionScopes) -> Result<(), HintError> {
    exec_scopes.insert_value("N", SECP256R1_N.clone());
    Ok(())
}

/*
Implements hint:
%{
from starkware.cairo.common.cairo_secp.secp256r1_utils import SECP256R1_P as SECP_P
%}
*/
pub fn import_secp256r1_p(exec_scopes: &mut ExecutionScopes) -> Result<(), HintError> {
    exec_scopes.insert_value("SECP_P", SECP256R1_P.clone());
    Ok(())
}
/*
Implements hint:
%{
    ids.quad_bit = (
        8 * ((ids.scalar_v >> ids.m) & 1)
        + 4 * ((ids.scalar_u >> ids.m) & 1)
        + 2 * ((ids.scalar_v >> (ids.m - 1)) & 1)
        + ((ids.scalar_u >> (ids.m - 1)) & 1)
    )
%}
*/
pub fn quad_bit(
    vm: &mut VirtualMachine,
    ids_data: &HashMap<String, HintReference>,
    ap_tracking: &ApTracking,
) -> Result<(), HintError> {
    n_pair_bits(vm, ids_data, ap_tracking, "quad_bit", 2)
}

/*
Implements hint:
%{ ids.dibit = ((ids.scalar_u >> ids.m) & 1) + 2 * ((ids.scalar_v >> ids.m) & 1) %}
*/
pub fn di_bit(
    vm: &mut VirtualMachine,
    ids_data: &HashMap<String, HintReference>,
    ap_tracking: &ApTracking,
) -> Result<(), HintError> {
    n_pair_bits(vm, ids_data, ap_tracking, "dibit", 1)
}

pub fn n_pair_bits(
    vm: &mut VirtualMachine,
    ids_data: &HashMap<String, HintReference>,
    ap_tracking: &ApTracking,
    result_name: &str,
    number_of_pairs: u32,
) -> Result<(), HintError> {
    let scalar_v_cow = get_integer_from_var_name("scalar_v", vm, ids_data, ap_tracking)?;
    let scalar_u_cow = get_integer_from_var_name("scalar_u", vm, ids_data, ap_tracking)?;
    let m_cow = get_integer_from_var_name("m", vm, ids_data, ap_tracking)?;

    let scalar_v = scalar_v_cow.as_ref();
    let scalar_u = scalar_u_cow.as_ref();

    // If m is too high the shift result will always be zero
    let m = m_cow.as_ref().to_u32().unwrap_or(253);
    if m >= 253 {
        return insert_value_from_var_name(result_name, 0, vm, ids_data, ap_tracking);
    }
    if m + 1 < number_of_pairs {
        return Err(HintError::NPairBitsTooLowM);
    }

    let (scalar_v, scalar_u) = (scalar_v.to_biguint(), scalar_u.to_biguint());

    // Each step, fetches the bits in mth position for v and u,
    // and appends them to the accumulator. i.e:
    //         10
    //         ↓↓
    //  1010101__ -> 101010110
    let get_bit =
        |x: &BigUint, i| m.checked_sub(i).map(|i| x.bit(i.into())).unwrap_or(false) as u32;
    let res = Felt252::from(
        &(0..number_of_pairs)
            .map(|i| {
                // This code is definitely verbose, but it's the only way I found to avoid a `panic`
                // when `m < number_of_pairs` while still being correct and hopefully fast.
                let bit_1 = get_bit(&scalar_v, i);
                // 1 * ((ids.scalar_u >> ids.m) & 1)
                let bit_0 = get_bit(&scalar_u, i);
                bit_0 | (bit_1 << 1)
            })
            .fold(BigUint::zero(), |mut acc, x| {
                acc <<= 2_u32;
                acc += x;
                acc
            }),
    );
    /*
        ids.quad_bit = (
            8 * ((ids.scalar_v >> ids.m) & 1)
            + 4 * ((ids.scalar_u >> ids.m) & 1)
            + 2 * ((ids.scalar_v >> (ids.m - 1)) & 1)
            + ((ids.scalar_u >> (ids.m - 1)) & 1)
        )
    %{ ids.dibit = ((ids.scalar_u >> ids.m) & 1) + 2 * ((ids.scalar_v >> ids.m) & 1) %}
    */

    insert_value_from_var_name(result_name, res, vm, ids_data, ap_tracking)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::hint_processor::builtin_hint_processor::hint_code;
    use crate::hint_processor::builtin_hint_processor::secp::secp_utils::SECP_P_V2;
    use crate::stdlib::string::ToString;

    use crate::{
        any_box,
        hint_processor::{
            builtin_hint_processor::builtin_hint_processor_definition::{
                BuiltinHintProcessor, HintProcessorData,
            },
            hint_processor_definition::HintProcessorLogic,
        },
        types::{exec_scope::ExecutionScopes, relocatable::Relocatable},
        utils::test_utils::*,
        vm::vm_core::VirtualMachine,
    };
    use assert_matches::assert_matches;

    #[cfg(target_arch = "wasm32")]
    use wasm_bindgen_test::*;

    #[test]
    #[cfg_attr(target_arch = "wasm32", wasm_bindgen_test)]
    fn run_ec_negate_ok() {
        let hint_code = "from starkware.cairo.common.cairo_secp.secp_utils import SECP_P, pack\n\ny = pack(ids.point.y, PRIME) % SECP_P\n# The modulo operation in python always returns a nonnegative number.\nvalue = (-y) % SECP_P";
        let mut vm = vm_with_range_check!();

        vm.segments = segments![((1, 3), 2645i32), ((1, 4), 454i32), ((1, 5), 206i32)];
        //Initialize fp
        vm.run_context.fp = 1;
        //Create hint_data
        let ids_data = ids_data!["point"];
        let mut exec_scopes = ExecutionScopes::new();
        //Execute the hint
        assert_matches!(run_hint!(vm, ids_data, hint_code, &mut exec_scopes), Ok(()));
        //Check 'value' is defined in the vm scope
        assert_matches!(
            exec_scopes.get::<BigInt>("value"),
            Ok(x) if x == bigint_str!(
                "115792089237316195423569751828682367333329274433232027476421668138471189901786"
            )
        );
    }

    #[test]
    #[cfg_attr(target_arch = "wasm32", wasm_bindgen_test)]
    fn run_ec_negate_embedded_secp_p_ok() {
        let hint_code = hint_code::EC_NEGATE_EMBEDDED_SECP;
        let mut vm = vm_with_range_check!();

        let (y0, y1, y2) = (2645i32, 454i32, 206i32);

        let y = (BigInt::from(y2) << (86 * 2)) + (BigInt::from(y1) << 86) + y0;
        let minus_y = (BigInt::one() << 255) - 19 - y;

        vm.segments = segments![((1, 3), y0), ((1, 4), y1), ((1, 5), y2)];
        //Initialize fp
        vm.run_context.fp = 1;
        //Create hint_data
        let ids_data = ids_data!["point"];
        let mut exec_scopes = ExecutionScopes::new();
        //Execute the hint
        assert_matches!(run_hint!(vm, ids_data, hint_code, &mut exec_scopes), Ok(()));
        //Check 'value' is defined in the vm scope
        assert_matches!(
            exec_scopes.get::<BigInt>("value"),
            Ok(x) if x == minus_y
        );
    }

    #[test]
    #[cfg_attr(target_arch = "wasm32", wasm_bindgen_test)]
    fn run_compute_doubling_slope_ok() {
        let hint_code = "from starkware.cairo.common.cairo_secp.secp_utils import SECP_P, pack\nfrom starkware.python.math_utils import ec_double_slope\n\n# Compute the slope.\nx = pack(ids.point.x, PRIME)\ny = pack(ids.point.y, PRIME)\nvalue = slope = ec_double_slope(point=(x, y), alpha=0, p=SECP_P)";
        let mut vm = vm_with_range_check!();
        vm.segments = segments![
            ((1, 0), 614323u64),
            ((1, 1), 5456867u64),
            ((1, 2), 101208u64),
            ((1, 3), 773712524u64),
            ((1, 4), 77371252u64),
            ((1, 5), 5298795u64)
        ];

        //Initialize fp
        vm.run_context.fp = 1;

        let ids_data = ids_data!["point"];
        let mut exec_scopes = ExecutionScopes::new();

        //Execute the hint
        assert_matches!(run_hint!(vm, ids_data, hint_code, &mut exec_scopes), Ok(()));
        check_scope!(
            &exec_scopes,
            [
                (
                    "value",
                    bigint_str!(
                        "40442433062102151071094722250325492738932110061897694430475034100717288403728"
                    )
                ),
                (
                    "slope",
                    bigint_str!(
                        "40442433062102151071094722250325492738932110061897694430475034100717288403728"
                    )
                )
            ]
        );
    }

    #[test]
    #[cfg_attr(target_arch = "wasm32", wasm_bindgen_test)]
    fn run_ec_double_scope_v2_hint_ok() {
        let hint_code = hint_code::EC_DOUBLE_SLOPE_V2;
        let mut vm = vm_with_range_check!();
        vm.segments = segments![
            ((1, 0), 512),
            ((1, 1), 2412),
            ((1, 2), 133),
            ((1, 3), 64),
            ((1, 4), 0),
            ((1, 5), 6546)
        ];

        //Initialize fp
        vm.run_context.fp = 1;

        let ids_data = ids_data!["point"];
        let mut exec_scopes = ExecutionScopes::new();

        //Execute the hint
        assert_matches!(run_hint!(vm, ids_data, hint_code, &mut exec_scopes), Ok(()));
        check_scope!(
            &exec_scopes,
            [
                (
                    "value",
                    bigint_str!(
                        "48268701472940295594394094960749868325610234644833445333946260403470540790234"
                    )
                ),
                (
                    "slope",
                    bigint_str!(
                        "48268701472940295594394094960749868325610234644833445333946260403470540790234"
                    )
                ),
                ("SECP_P", SECP_P_V2.clone())
            ]
        );
    }

    #[test]
    #[cfg_attr(target_arch = "wasm32", wasm_bindgen_test)]
    fn run_compute_doubling_slope_wdivmod_ok() {
        let hint_code = "from starkware.cairo.common.cairo_secp.secp_utils import SECP_P, pack\nfrom starkware.python.math_utils import div_mod\n\n# Compute the slope.\nx = pack(ids.pt.x, PRIME)\ny = pack(ids.pt.y, PRIME)\nvalue = slope = div_mod(3 * x ** 2, 2 * y, SECP_P)";
        let mut vm = vm_with_range_check!();
        vm.segments = segments![
            ((1, 0), 614323u64),
            ((1, 1), 5456867u64),
            ((1, 2), 101208u64),
            ((1, 3), 773712524u64),
            ((1, 4), 77371252u64),
            ((1, 5), 5298795u64)
        ];

        //Initialize fp
        vm.run_context.fp = 1;

        let ids_data = ids_data!["pt"];
        let mut exec_scopes = ExecutionScopes::new();

        //Execute the hint
        assert_matches!(run_hint!(vm, ids_data, hint_code, &mut exec_scopes), Ok(()));
        check_scope!(
            &exec_scopes,
            [
                (
                    "value",
                    bigint_str!(
                        "40442433062102151071094722250325492738932110061897694430475034100717288403728"
                    )
                ),
                (
                    "slope",
                    bigint_str!(
                        "40442433062102151071094722250325492738932110061897694430475034100717288403728"
                    )
                )
            ]
        );
    }

    #[test]
    #[cfg_attr(target_arch = "wasm32", wasm_bindgen_test)]
    fn run_compute_doubling_slope_with_custom_consts_ok() {
        let hint_code = hint_code::EC_DOUBLE_SLOPE_EXTERNAL_CONSTS;
        let mut vm = vm_with_range_check!();
        vm.segments = segments![
            ((1, 0), 614323u64),
            ((1, 1), 5456867u64),
            ((1, 2), 101208u64),
            ((1, 3), 773712524u64),
            ((1, 4), 77371252u64),
            ((1, 5), 5298795u64)
        ];

        //Initialize fp
        vm.run_context.fp = 1;

        let ids_data = ids_data!["point"];
        let mut exec_scopes = ExecutionScopes::new();

        exec_scopes.insert_value("SECP_P", SECP256R1_P.clone());
        exec_scopes.insert_value("ALPHA", SECP256R1_ALPHA.clone());

        //Execute the hint
        assert_matches!(run_hint!(vm, ids_data, hint_code, &mut exec_scopes), Ok(()));
        check_scope!(
            &exec_scopes,
            [
                (
                    "value",
                    bigint_str!(
                        "99065496658741969395000079476826955370154683653966841736214499259699304892273"
                    )
                ),
                (
                    "slope",
                    bigint_str!(
                        "99065496658741969395000079476826955370154683653966841736214499259699304892273"
                    )
                ),
            ],
        );
    }

    #[test]
    #[cfg_attr(target_arch = "wasm32", wasm_bindgen_test)]
    fn run_compute_slope_ok() {
        let hint_code = "from starkware.cairo.common.cairo_secp.secp_utils import SECP_P, pack\nfrom starkware.python.math_utils import line_slope\n\n# Compute the slope.\nx0 = pack(ids.point0.x, PRIME)\ny0 = pack(ids.point0.y, PRIME)\nx1 = pack(ids.point1.x, PRIME)\ny1 = pack(ids.point1.y, PRIME)\nvalue = slope = line_slope(point1=(x0, y0), point2=(x1, y1), p=SECP_P)";
        let mut vm = vm_with_range_check!();

        //Insert ids.point0 and ids.point1 into memory
        vm.segments = segments![
            ((1, 0), 134),
            ((1, 1), 5123),
            ((1, 2), 140),
            ((1, 3), 1232),
            ((1, 4), 4652),
            ((1, 5), 720),
            ((1, 6), 156),
            ((1, 7), 6545),
            ((1, 8), 100010),
            ((1, 9), 1123),
            ((1, 10), 1325),
            ((1, 11), 910)
        ];

        //Initialize fp
        vm.run_context.fp = 14;
        let ids_data = HashMap::from([
            ("point0".to_string(), HintReference::new_simple(-14)),
            ("point1".to_string(), HintReference::new_simple(-8)),
        ]);
        let mut exec_scopes = ExecutionScopes::new();

        //Execute the hint
        assert_matches!(run_hint!(vm, ids_data, hint_code, &mut exec_scopes), Ok(()));
        check_scope!(
            &exec_scopes,
            [
                (
                    "value",
                    bigint_str!(
                        "41419765295989780131385135514529906223027172305400087935755859001910844026631"
                    )
                ),
                (
                    "slope",
                    bigint_str!(
                        "41419765295989780131385135514529906223027172305400087935755859001910844026631"
                    )
                )
            ]
        );
    }

    #[test]
    #[cfg_attr(target_arch = "wasm32", wasm_bindgen_test)]
    fn run_compute_slope_v2_ok() {
        let mut vm = vm_with_range_check!();

        //Insert ids.point0 and ids.point1 into memory
        vm.segments = segments![
            ((1, 0), 512),
            ((1, 1), 2412),
            ((1, 2), 133),
            ((1, 3), 64),
            ((1, 4), 0),
            ((1, 5), 6546),
            ((1, 6), 7),
            ((1, 7), 8),
            ((1, 8), 123),
            ((1, 9), 1),
            ((1, 10), 7),
            ((1, 11), 465)
        ];
        // let point_1 = EcPoint(BigInt3(512,2412,133), BigInt3(64,0,6546));
        // let point_2 = EcPoint(BigInt3(7,8,123), BigInt3(1,7,465));

        //Initialize fp
        vm.run_context.fp = 14;
        let ids_data = HashMap::from([
            ("point0".to_string(), HintReference::new_simple(-14)),
            ("point1".to_string(), HintReference::new_simple(-8)),
        ]);
        let mut exec_scopes = ExecutionScopes::new();

        //Execute the hint
        assert_matches!(
            run_hint!(vm, ids_data, hint_code::COMPUTE_SLOPE_V2, &mut exec_scopes),
            Ok(())
        );
        check_scope!(
            &exec_scopes,
            [
                (
                    "value",
                    bigint_str!(
                        "39376930140709393693483102164172662915882483986415749881375763965703119677959"
                    )
                ),
                (
                    "slope",
                    bigint_str!(
                        "39376930140709393693483102164172662915882483986415749881375763965703119677959"
                    )
                )
            ]
        );
    }

    #[test]
    #[cfg_attr(target_arch = "wasm32", wasm_bindgen_test)]
    fn run_compute_slope_wdivmod_ok() {
        let hint_code = "from starkware.cairo.common.cairo_secp.secp_utils import SECP_P, pack\nfrom starkware.python.math_utils import div_mod\n\n# Compute the slope.\nx0 = pack(ids.pt0.x, PRIME)\ny0 = pack(ids.pt0.y, PRIME)\nx1 = pack(ids.pt1.x, PRIME)\ny1 = pack(ids.pt1.y, PRIME)\nvalue = slope = div_mod(y0 - y1, x0 - x1, SECP_P)";
        let mut vm = vm_with_range_check!();

        // Insert ids.pt0 and ids.pt1 into memory
        vm.segments = segments![
            ((1, 0), 134),
            ((1, 1), 5123),
            ((1, 2), 140),
            ((1, 3), 1232),
            ((1, 4), 4652),
            ((1, 5), 720),
            ((1, 6), 156),
            ((1, 7), 6545),
            ((1, 8), 100010),
            ((1, 9), 1123),
            ((1, 10), 1325),
            ((1, 11), 910)
        ];

        // Initialize fp
        vm.run_context.fp = 14;
        let ids_data = HashMap::from([
            ("pt0".to_string(), HintReference::new_simple(-14)),
            ("pt1".to_string(), HintReference::new_simple(-8)),
        ]);
        let mut exec_scopes = ExecutionScopes::new();

        // Execute the hint
        assert_matches!(run_hint!(vm, ids_data, hint_code, &mut exec_scopes), Ok(()));
        check_scope!(
            &exec_scopes,
            [
                (
                    "value",
                    bigint_str!(
                        "41419765295989780131385135514529906223027172305400087935755859001910844026631"
                    )
                ),
                (
                    "slope",
                    bigint_str!(
                        "41419765295989780131385135514529906223027172305400087935755859001910844026631"
                    )
                )
            ]
        );
    }

    #[test]
    #[cfg_attr(target_arch = "wasm32", wasm_bindgen_test)]
    fn run_ec_double_assign_new_x_ok() {
        let hint_code = hint_code::EC_DOUBLE_ASSIGN_NEW_X_V1;

        let mut vm = vm_with_range_check!();

        //Insert ids.point and ids.slope into memory
        vm.segments = segments![
            ((1, 0), 134),
            ((1, 1), 5123),
            ((1, 2), 140),
            ((1, 3), 1232),
            ((1, 4), 4652),
            ((1, 5), 720),
            ((1, 6), 44186171158942157784255469_i128),
            ((1, 7), 54173758974262696047492534_i128),
            ((1, 8), 8106299688661572814170174_i128)
        ];

        //Initialize fp
        vm.run_context.fp = 10;
        let ids_data = HashMap::from([
            ("point".to_string(), HintReference::new_simple(-10)),
            ("slope".to_string(), HintReference::new_simple(-4)),
        ]);
        let mut exec_scopes = ExecutionScopes::new();

        //Execute the hint
        assert_matches!(run_hint!(vm, ids_data, hint_code, &mut exec_scopes), Ok(()));

        check_scope!(
                &exec_scopes,
                [
                    (
                        "slope",
                        bigint_str!(
                            "48526828616392201132917323266456307435009781900148206102108934970258721901549"
                        )
                    ),
                    (
                        "x",
                        bigint_str!("838083498911032969414721426845751663479194726707495046")
                    ),
                    (
                        "y",
                        bigint_str!("4310143708685312414132851373791311001152018708061750480")
                    ),
                    (
                        "value",
                        bigint_str!(
                            "59479631769792988345961122678598249997181612138456851058217178025444564264149"
                        )
                    ),
                    (
                        "new_x",
                        bigint_str!(
                            "59479631769792988345961122678598249997181612138456851058217178025444564264149"
                        )
                    )
                ]
            );
    }

    #[test]
    #[cfg_attr(target_arch = "wasm32", wasm_bindgen_test)]
    fn run_ec_double_assign_new_x_v2_ok() {
        let hint_code = hint_code::EC_DOUBLE_ASSIGN_NEW_X_V2;

        let mut vm = vm_with_range_check!();

        //Insert ids.point and ids.slope into memory
        vm.segments = segments![
            ((1, 0), 134),
            ((1, 1), 5123),
            ((1, 2), 140),
            ((1, 3), 1232),
            ((1, 4), 4652),
            ((1, 5), 720),
            ((1, 6), 44186171158942157784255469_i128),
            ((1, 7), 54173758974262696047492534_i128),
            ((1, 8), 8106299688661572814170174_i128)
        ];

        //Initialize fp
        vm.run_context.fp = 10;
        let ids_data = HashMap::from([
            ("point".to_string(), HintReference::new_simple(-10)),
            ("slope".to_string(), HintReference::new_simple(-4)),
        ]);
        let mut exec_scopes = ExecutionScopes::new();
        exec_scopes.assign_or_update_variable("SECP_P", any_box!(SECP_P.clone()));

        //Execute the hint
        assert_matches!(run_hint!(vm, ids_data, hint_code, &mut exec_scopes), Ok(()));

        check_scope!(
                &exec_scopes,
                [
                    (
                        "slope",
                        bigint_str!(
                            "48526828616392201132917323266456307435009781900148206102108934970258721901549"
                        )
                    ),
                    (
                        "x",
                        bigint_str!("838083498911032969414721426845751663479194726707495046")
                    ),
                    (
                        "y",
                        bigint_str!("4310143708685312414132851373791311001152018708061750480")
                    ),
                    (
                        "value",
                        bigint_str!(
                            "59479631769792988345961122678598249997181612138456851058217178025444564264149"
                        )
                    ),
                    (
                        "new_x",
                        bigint_str!(
                            "59479631769792988345961122678598249997181612138456851058217178025444564264149"
                        )
                    )
                ]
            );
    }

    #[test]
    #[cfg_attr(target_arch = "wasm32", wasm_bindgen_test)]
    fn run_ec_double_assign_new_y_ok() {
        let hint_code = "value = new_y = (slope * (x - new_x) - y) % SECP_P";
        let mut vm = vm_with_range_check!();
        let mut exec_scopes = scope![
            (
                "slope",
                bigint_str!(
                    "48526828616392201132917323266456307435009781900148206102108934970258721901549"
                )
            ),
            (
                "x",
                bigint_str!("838083498911032969414721426845751663479194726707495046")
            ),
            (
                "new_x",
                bigint_str!(
                    "59479631769792988345961122678598249997181612138456851058217178025444564264149"
                )
            ),
            (
                "y",
                bigint_str!("4310143708685312414132851373791311001152018708061750480")
            ),
            ("SECP_P", (*SECP_P).clone())
        ];
        //Execute the hint
        assert_matches!(
            run_hint!(vm, HashMap::new(), hint_code, &mut exec_scopes),
            Ok(())
        );

        check_scope!(
            &exec_scopes,
            [
                (
                    "value",
                    bigint_str!(
                        "7948634220683381957329555864604318996476649323793038777651086572350147290350"
                    )
                ),
                (
                    "new_y",
                    bigint_str!(
                        "7948634220683381957329555864604318996476649323793038777651086572350147290350"
                    )
                )
            ]
        );
    }

    #[test]
    #[cfg_attr(target_arch = "wasm32", wasm_bindgen_test)]
    fn run_fast_ec_add_assign_new_x_ok() {
        let hint_code = "from starkware.cairo.common.cairo_secp.secp_utils import SECP_P, pack\n\nslope = pack(ids.slope, PRIME)\nx0 = pack(ids.point0.x, PRIME)\nx1 = pack(ids.point1.x, PRIME)\ny0 = pack(ids.point0.y, PRIME)\n\nvalue = new_x = (pow(slope, 2, SECP_P) - x0 - x1) % SECP_P";
        let mut vm = vm_with_range_check!();

        //Insert ids.point0, ids.point1.x and ids.slope into memory
        vm.segments = segments![
            //ids.point0
            ((1, 0), 89712),
            ((1, 1), 56),
            ((1, 2), 1233409),
            ((1, 3), 980126),
            ((1, 4), 10),
            ((1, 5), 8793),
            //ids.point0.x
            ((1, 6), 1235216451),
            ((1, 7), 5967),
            ((1, 8), 2171381),
            //ids.slope
            ((1, 9), 67470097831679799377177424_i128),
            ((1, 10), 43370026683122492246392730_i128),
            ((1, 11), 16032182557092050689870202_i128)
        ];

        //Initialize run_context
        run_context!(vm, 0, 20, 15);

        let ids_data = HashMap::from([
            ("point0".to_string(), HintReference::new_simple(-15)),
            ("point1".to_string(), HintReference::new_simple(-9)),
            ("slope".to_string(), HintReference::new_simple(-6)),
        ]);
        let mut exec_scopes = ExecutionScopes::new();

        //Execute the hint
        assert_matches!(run_hint!(vm, ids_data, hint_code, &mut exec_scopes), Ok(()));

        check_scope!(
            &exec_scopes,
            [
                (
                    "value",
                    bigint_str!(
                        "8891838197222656627233627110766426698842623939023296165598688719819499152657"
                    )
                ),
                (
                    "new_x",
                    bigint_str!(
                        "8891838197222656627233627110766426698842623939023296165598688719819499152657"
                    )
                )
            ]
        );
    }

    #[test]
    #[cfg_attr(target_arch = "wasm32", wasm_bindgen_test)]
    fn run_fast_ec_add_assign_new_y_ok() {
        let hint_code = "value = new_y = (slope * (x0 - new_x) - y0) % SECP_P";
        let mut vm = vm_with_range_check!();

        let mut exec_scopes = scope![
            (
                "slope",
                bigint_str!(
                    "48526828616392201132917323266456307435009781900148206102108934970258721901549"
                )
            ),
            (
                "x0",
                bigint_str!("838083498911032969414721426845751663479194726707495046")
            ),
            (
                "new_x",
                bigint_str!(
                    "59479631769792988345961122678598249997181612138456851058217178025444564264149"
                )
            ),
            (
                "y0",
                bigint_str!("4310143708685312414132851373791311001152018708061750480")
            ),
            ("SECP_P", (*SECP_P).clone())
        ];

        //Execute the hint
        assert_matches!(
            run_hint!(vm, HashMap::new(), hint_code, &mut exec_scopes),
            Ok(())
        );

        check_scope!(
            &exec_scopes,
            [
                (
                    "value",
                    bigint_str!(
                        "7948634220683381957329555864604318996476649323793038777651086572350147290350"
                    )
                ),
                (
                    "new_y",
                    bigint_str!(
                        "7948634220683381957329555864604318996476649323793038777651086572350147290350"
                    )
                )
            ]
        );
    }

    #[test]
    #[cfg_attr(target_arch = "wasm32", wasm_bindgen_test)]
    fn run_ec_mul_inner_ok() {
        let hint_code = "memory[ap] = (ids.scalar % PRIME) % 2";
        let mut vm = vm_with_range_check!();

        let scalar = 89712_i32;
        //Insert ids.scalar into memory
        vm.segments = segments![((1, 0), scalar)];

        //Initialize RunContext
        run_context!(vm, 0, 2, 1);

        let ids_data = ids_data!["scalar"];

        //Execute the hint
        assert_matches!(run_hint!(vm, ids_data, hint_code), Ok(()));

        //Check hint memory inserts
        check_memory![vm.segments.memory, ((1, 2), 0)];
    }

    #[test]
    fn get_ec_point_from_var_name_ok() {
        /*EcPoint {
            x: (1,2,3)
            y: (4,5,6)
        }*/
        let mut vm = vm!();
        vm.set_fp(1);
        vm.segments = segments![
            ((1, 0), 1),
            ((1, 1), 2),
            ((1, 2), 3),
            ((1, 3), 4),
            ((1, 4), 5),
            ((1, 5), 6)
        ];
        let ids_data = ids_data!["e"];
        let ap_tracking = ApTracking::default();
        let e = EcPoint::from_var_name("e", &vm, &ids_data, &ap_tracking).unwrap();
        assert_eq!(e.x.limbs[0].as_ref(), &Felt252::ONE);
        assert_eq!(e.x.limbs[1].as_ref(), &Felt252::from(2));
        assert_eq!(e.x.limbs[2].as_ref(), &Felt252::from(3));
        assert_eq!(e.y.limbs[0].as_ref(), &Felt252::from(4));
        assert_eq!(e.y.limbs[1].as_ref(), &Felt252::from(5));
        assert_eq!(e.y.limbs[2].as_ref(), &Felt252::from(6));
    }

    #[test]
    fn get_ec_point_from_var_name_missing_member() {
        /*EcPoint {
            x: (1,2,3)
            y: (4,_,_)
        }*/
        let mut vm = vm!();
        vm.set_fp(1);
        vm.segments = segments![((1, 0), 1), ((1, 1), 2), ((1, 2), 3), ((1, 3), 4)];
        let ids_data = ids_data!["e"];
        let ap_tracking = ApTracking::default();
        let r = EcPoint::from_var_name("e", &vm, &ids_data, &ap_tracking);
        assert_matches!(r, Err(HintError::IdentifierHasNoMember(bx)) if *bx == ("e.y".to_string(), "d1".to_string()))
    }

    #[test]
    fn get_ec_point_from_var_name_invalid_reference() {
        let mut vm = vm!();
        vm.segments = segments![((1, 0), 1), ((1, 1), 2)];
        let ids_data = ids_data!["e"];
        let ap_tracking = ApTracking::default();
        let r = EcPoint::from_var_name("e", &vm, &ids_data, &ap_tracking);
        assert_matches!(r, Err(HintError::UnknownIdentifier(bx)) if bx.as_ref() == "e")
    }

    #[test]
    fn run_quad_bit_ok() {
        let hint_code = hint_code::QUAD_BIT;
        let mut vm = vm_with_range_check!();

        let scalar_u = 89712;
        let scalar_v = 1478396;
        let m = 4;
        // Insert ids.scalar into memory
        vm.segments = segments![((1, 0), scalar_u), ((1, 1), scalar_v), ((1, 2), m)];

        // Initialize RunContext
        run_context!(vm, 0, 4, 4);

        let ids_data = ids_data!["scalar_u", "scalar_v", "m", "quad_bit"];

        // Execute the hint
        assert_matches!(run_hint!(vm, ids_data, hint_code), Ok(()));

        // Check hint memory inserts
        check_memory![vm.segments.memory, ((1, 3), 14)];
    }

    #[test]
    fn run_quad_bit_for_m_1_ok() {
        let hint_code = hint_code::QUAD_BIT;
        let mut vm = vm_with_range_check!();

        let scalar_u = 89712;
        let scalar_v = 1478396;
        let m = 1;
        // Insert ids.scalar into memory
        vm.segments = segments![((1, 0), scalar_u), ((1, 1), scalar_v), ((1, 2), m)];

        // Initialize RunContext
        run_context!(vm, 0, 4, 4);

        let ids_data = ids_data!["scalar_u", "scalar_v", "m", "quad_bit"];

        // Execute the hint
        assert_matches!(run_hint!(vm, ids_data, hint_code), Ok(()));

        // Check hint memory inserts
        check_memory![vm.segments.memory, ((1, 3), 0)];
    }

    #[test]
    fn run_quad_bit_for_m_0() {
        let hint_code = hint_code::QUAD_BIT;
        let mut vm = vm_with_range_check!();

        let scalar_u = 0b1010101;
        let scalar_v = 0b1010101;
        let m = 0;
        // Insert ids.scalar into memory
        vm.segments = segments![((1, 0), scalar_u), ((1, 1), scalar_v), ((1, 2), m)];

        // Initialize RunContext
        run_context!(vm, 0, 4, 4);

        let ids_data = ids_data!["scalar_u", "scalar_v", "m", "quad_bit"];

        // Execute the hint
        assert_matches!(
            run_hint!(vm, ids_data, hint_code),
            Err(HintError::NPairBitsTooLowM)
        );
    }

    #[test]
    fn run_quad_bit_with_max_m_ok() {
        let hint_code = hint_code::QUAD_BIT;
        let mut vm = vm_with_range_check!();

        let scalar_u = 89712;
        let scalar_v = 1478396;
        // Value is so high the result will always be zero
        let m = i128::MAX;
        // Insert ids.scalar into memory
        vm.segments = segments![((1, 0), scalar_u), ((1, 1), scalar_v), ((1, 2), m)];

        // Initialize RunContext
        run_context!(vm, 0, 4, 4);

        let ids_data = ids_data!["scalar_u", "scalar_v", "m", "quad_bit"];

        // Execute the hint
        assert_matches!(run_hint!(vm, ids_data, hint_code), Ok(()));

        // Check hint memory inserts
        check_memory![vm.segments.memory, ((1, 3), 0)];
    }

    #[test]
    fn run_di_bit_ok() {
        let hint_code = hint_code::DI_BIT;
        let mut vm = vm_with_range_check!();

        let scalar_u = 0b10101111001110000;
        let scalar_v = 0b101101000111011111100;
        let m = 3;
        // Insert ids.scalar into memory
        vm.segments = segments![((1, 0), scalar_u), ((1, 1), scalar_v), ((1, 2), m)];

        // Initialize RunContext
        run_context!(vm, 0, 4, 4);

        let ids_data = ids_data!["scalar_u", "scalar_v", "m", "dibit"];

        // Execute the hint
        assert_matches!(run_hint!(vm, ids_data, hint_code), Ok(()));

        // Check hint memory inserts
        check_memory![vm.segments.memory, ((1, 3), 2)];
    }

    #[test]
    fn run_di_bit_with_max_m_ok() {
        let hint_code = hint_code::DI_BIT;
        let mut vm = vm_with_range_check!();

        let scalar_u = 89712;
        let scalar_v = 1478396;
        // Value is so high the result will always be zero
        let m = i128::MAX;
        // Insert ids.scalar into memory
        vm.segments = segments![((1, 0), scalar_u), ((1, 1), scalar_v), ((1, 2), m)];

        // Initialize RunContext
        run_context!(vm, 0, 4, 4);

        let ids_data = ids_data!["scalar_u", "scalar_v", "m", "dibit"];

        // Execute the hint
        assert_matches!(run_hint!(vm, ids_data, hint_code), Ok(()));

        // Check hint memory inserts
        check_memory![vm.segments.memory, ((1, 3), 0)];
    }

    #[test]
    fn run_di_bit_m_zero_ok() {
        let hint_code = hint_code::DI_BIT;
        let mut vm = vm_with_range_check!();

        let scalar_u = 0b00;
        let scalar_v = 0b01;
        let m = 0;
        // Insert ids.scalar into memory
        vm.segments = segments![((1, 0), scalar_u), ((1, 1), scalar_v), ((1, 2), m)];

        // Initialize RunContext
        run_context!(vm, 0, 4, 4);

        let ids_data = ids_data!["scalar_u", "scalar_v", "m", "dibit"];

        // Execute the hint
        assert_matches!(run_hint!(vm, ids_data, hint_code), Ok(()));

        // Check hint memory inserts
        check_memory![vm.segments.memory, ((1, 3), 0b10)];
    }

    #[test]
    #[cfg_attr(target_arch = "wasm32", wasm_bindgen_test)]
    fn run_import_secp256r1_alpha() {
        let hint_code = "from starkware.cairo.common.cairo_secp.secp256r1_utils import SECP256R1_ALPHA as ALPHA";
        let mut vm = vm_with_range_check!();

        //Initialize fp
        vm.run_context.fp = 1;
        //Create hint_data
        let ids_data = ids_data!["point"];
        let mut exec_scopes = ExecutionScopes::new();
        //Execute the hint
        assert_matches!(run_hint!(vm, ids_data, hint_code, &mut exec_scopes), Ok(()));
        //Check 'ALPHA' is defined in the vm scope
        assert_matches!(
            exec_scopes.get::<BigInt>("ALPHA"),
            Ok(x) if x == bigint_str!(
                "115792089210356248762697446949407573530086143415290314195533631308867097853948"
            )
        );
    }

    #[test]
    #[cfg_attr(target_arch = "wasm32", wasm_bindgen_test)]
    fn run_square_slope_minus_xs_ok() {
        let hint_code = "from starkware.cairo.common.cairo_secp.secp_utils import pack\n\nslope = pack(ids.slope, PRIME)\nx0 = pack(ids.point0.x, PRIME)\nx1 = pack(ids.point1.x, PRIME)\ny0 = pack(ids.point0.y, PRIME)\n\nvalue = new_x = (pow(slope, 2, SECP_P) - x0 - x1) % SECP_P";
        let mut vm = vm_with_range_check!();

        //Insert ids.point0, ids.point1.x and ids.slope into memory
        vm.segments = segments![
            //ids.point0
            ((1, 0), 89712),
            ((1, 1), 56),
            ((1, 2), 1233409),
            ((1, 3), 980126),
            ((1, 4), 10),
            ((1, 5), 8793),
            //ids.point0.x
            ((1, 6), 1235216451),
            ((1, 7), 5967),
            ((1, 8), 2171381),
            //ids.slope
            ((1, 9), 67470097831679799377177424_i128),
            ((1, 10), 43370026683122492246392730_i128),
            ((1, 11), 16032182557092050689870202_i128)
        ];

        //Initialize run_context
        run_context!(vm, 0, 20, 15);

        let ids_data = HashMap::from([
            ("point0".to_string(), HintReference::new_simple(-15)),
            ("point1".to_string(), HintReference::new_simple(-9)),
            ("slope".to_string(), HintReference::new_simple(-6)),
        ]);
        let mut exec_scopes = ExecutionScopes::new();
        exec_scopes.insert_value("SECP_P", SECP_P.clone());

        //Execute the hint
        assert_matches!(run_hint!(vm, ids_data, hint_code, &mut exec_scopes), Ok(()));

        check_scope!(
            &exec_scopes,
            [
                (
                    "value",
                    bigint_str!(
                        "8891838197222656627233627110766426698842623939023296165598688719819499152657"
                    )
                ),
                (
                    "new_x",
                    bigint_str!(
                        "8891838197222656627233627110766426698842623939023296165598688719819499152657"
                    )
                )
            ]
        );
    }
}
