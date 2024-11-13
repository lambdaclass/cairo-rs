use core::ops::Shl;

use crate::hint_processor::builtin_hint_processor::uint_utils::{pack, split};
use crate::math_utils::signed_felt;
use crate::stdlib::{borrow::Cow, boxed::Box, collections::HashMap, prelude::*};
use crate::Felt252;
use crate::{
    hint_processor::{
        builtin_hint_processor::{
            hint_utils::{get_relocatable_from_var_name, insert_value_from_var_name},
            secp::secp_utils::{bigint3_split, BASE_86},
        },
        hint_processor_definition::HintReference,
    },
    math_utils::pow2_const_nz,
    serde::deserialize_program::ApTracking,
    types::{
        exec_scope::ExecutionScopes,
        relocatable::{MaybeRelocatable, Relocatable},
    },
    vm::{errors::hint_errors::HintError, vm_core::VirtualMachine},
};
use num_bigint::{BigInt, BigUint};

pub(crate) type BigInt3<'a> = BigIntN<'a, 3>;
pub(crate) type Uint384<'a> = BigIntN<'a, 3>;
pub(crate) type Uint512<'a> = BigIntN<'a, 4>;
pub(crate) type BigInt5<'a> = BigIntN<'a, 5>;
pub(crate) type Uint768<'a> = BigIntN<'a, 6>;

#[derive(Debug, PartialEq)]
pub(crate) struct BigIntN<'a, const NUM_LIMBS: usize> {
    pub(crate) limbs: [Cow<'a, Felt252>; NUM_LIMBS],
}

impl<const NUM_LIMBS: usize> BigIntN<'_, NUM_LIMBS> {
    pub(crate) fn from_base_addr<'a>(
        addr: Relocatable,
        name: &str,
        vm: &'a VirtualMachine,
    ) -> Result<BigIntN<'a, NUM_LIMBS>, HintError> {
        let mut limbs = vec![];
        for i in 0..NUM_LIMBS {
            limbs.push(vm.get_integer((addr + i)?).map_err(|_| {
                HintError::IdentifierHasNoMember(Box::new((name.to_string(), format!("d{}", i))))
            })?)
        }
        Ok(BigIntN {
            limbs: limbs
                .try_into()
                .map_err(|_| HintError::FixedSizeArrayFail(NUM_LIMBS))?,
        })
    }

    pub(crate) fn from_var_name<'a>(
        name: &str,
        vm: &'a VirtualMachine,
        ids_data: &HashMap<String, HintReference>,
        ap_tracking: &ApTracking,
    ) -> Result<BigIntN<'a, NUM_LIMBS>, HintError> {
        let base_addr = get_relocatable_from_var_name(name, vm, ids_data, ap_tracking)?;
        BigIntN::from_base_addr(base_addr, name, vm)
    }

    pub(crate) fn from_values(limbs: [Felt252; NUM_LIMBS]) -> Self {
        Self {
            limbs: limbs.map(Cow::Owned),
        }
    }

    pub(crate) fn insert_from_var_name(
        self,
        var_name: &str,
        vm: &mut VirtualMachine,
        ids_data: &HashMap<String, HintReference>,
        ap_tracking: &ApTracking,
    ) -> Result<(), HintError> {
        let addr = get_relocatable_from_var_name(var_name, vm, ids_data, ap_tracking)?;
        for i in 0..NUM_LIMBS {
            vm.insert_value((addr + i)?, *self.limbs[i].as_ref())?;
        }
        Ok(())
    }

    pub(crate) fn pack(self) -> BigUint {
        pack(self.limbs, 128)
    }

    pub(crate) fn pack86(self) -> BigInt {
        self.limbs
            .into_iter()
            .take(3)
            .enumerate()
            .map(|(idx, value)| signed_felt(*value).shl(idx * 86))
            .sum()
    }

    pub(crate) fn split(num: &BigUint) -> Self {
        let limbs = split(num, 128);
        Self::from_values(limbs)
    }
}

impl<'a, const NUM_LIMBS: usize> From<&'a BigUint> for BigIntN<'a, NUM_LIMBS> {
    fn from(value: &'a BigUint) -> Self {
        Self::split(value)
    }
}

/*
Implements hint:
%{
    from starkware.cairo.common.cairo_secp.secp_utils import split

    segments.write_arg(ids.res.address_, split(value))
%}
*/
pub fn nondet_bigint3(
    vm: &mut VirtualMachine,
    exec_scopes: &mut ExecutionScopes,
    ids_data: &HashMap<String, HintReference>,
    ap_tracking: &ApTracking,
) -> Result<(), HintError> {
    let res_reloc = get_relocatable_from_var_name("res", vm, ids_data, ap_tracking)?;
    let value = exec_scopes
        .get_ref::<num_bigint::BigInt>("value")?
        .to_biguint()
        .ok_or(HintError::BigIntToBigUintFail)?;
    let arg: Vec<MaybeRelocatable> = bigint3_split(&value)?
        .into_iter()
        .map(|ref n| Felt252::from(n).into())
        .collect::<Vec<MaybeRelocatable>>();
    vm.write_arg(res_reloc, &arg).map_err(HintError::Memory)?;
    Ok(())
}

// Implements hint
// %{ ids.low = (ids.x.d0 + ids.x.d1 * ids.BASE) & ((1 << 128) - 1) %}
pub fn bigint_to_uint256(
    vm: &mut VirtualMachine,
    ids_data: &HashMap<String, HintReference>,
    ap_tracking: &ApTracking,
    constants: &HashMap<String, Felt252>,
) -> Result<(), HintError> {
    let x_struct = get_relocatable_from_var_name("x", vm, ids_data, ap_tracking)?;
    let d0 = vm.get_integer(x_struct)?;
    let d1 = vm.get_integer((x_struct + 1_i32)?)?;
    let d0 = d0.as_ref();
    let d1 = d1.as_ref();
    let base_86 = constants
        .get(BASE_86)
        .ok_or_else(|| HintError::MissingConstant(BASE_86.to_string().into_boxed_str()))?;
    let mask = pow2_const_nz(128);
    let low = (d0 + (d1 * base_86)).mod_floor(mask);
    insert_value_from_var_name("low", low, vm, ids_data, ap_tracking)
}

// Implements hint
// %{ ids.len_hi = max(ids.scalar_u.d2.bit_length(), ids.scalar_v.d2.bit_length())-1 %}
pub fn hi_max_bitlen(
    vm: &mut VirtualMachine,
    ids_data: &HashMap<String, HintReference>,
    ap_tracking: &ApTracking,
) -> Result<(), HintError> {
    let scalar_u = BigInt3::from_var_name("scalar_u", vm, ids_data, ap_tracking)?;
    let scalar_v = BigInt3::from_var_name("scalar_v", vm, ids_data, ap_tracking)?;

    let len_hi_u = scalar_u.limbs[2].bits();
    let len_hi_v = scalar_v.limbs[2].bits();

    let len_hi = len_hi_u.max(len_hi_v);

    // equal to `len_hi.wrapping_sub(1)`
    let res = if len_hi == 0 {
        Felt252::MAX
    } else {
        (len_hi - 1).into()
    };

    insert_value_from_var_name("len_hi", res, vm, ids_data, ap_tracking)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::hint_processor::builtin_hint_processor::builtin_hint_processor_definition::{
        BuiltinHintProcessor, HintProcessorData,
    };
    use crate::hint_processor::hint_processor_definition::HintProcessorLogic;
    use crate::stdlib::string::ToString;
    use crate::types::exec_scope::ExecutionScopes;
    use crate::{any_box, felt_str};

    use crate::types::relocatable::Relocatable;
    use crate::utils::test_utils::*;

    use crate::vm::vm_core::VirtualMachine;

    use assert_matches::assert_matches;

    #[cfg(target_arch = "wasm32")]
    use wasm_bindgen_test::*;

    #[test]
    #[cfg_attr(target_arch = "wasm32", wasm_bindgen_test)]
    fn run_nondet_bigint3_ok() {
        let hint_code = "from starkware.cairo.common.cairo_secp.secp_utils import split\n\nsegments.write_arg(ids.res.address_, split(value))";
        let mut vm = vm_with_range_check!();
        add_segments!(vm, 3);
        // initialize vm scope with variable `n`
        let mut exec_scopes = scope![(
            "value",
            bigint_str!("7737125245533626718119526477371252455336267181195264773712524553362")
        )];
        //Initialize RubContext
        run_context!(vm, 0, 6, 6);
        //Create hint_data
        let ids_data = non_continuous_ids_data![("res", 5)];
        assert_matches!(
            run_hint!(
                vm,
                ids_data,
                hint_code,
                &mut exec_scopes,
                &[(BASE_86, crate::math_utils::pow2_const(86))]
                    .into_iter()
                    .map(|(k, v)| (k.to_string(), v))
                    .collect()
            ),
            Ok(())
        );
        //Check hint memory inserts
        check_memory![
            vm.segments.memory,
            ((1, 11), 773712524553362_u64),
            ((1, 12), 57408430697461422066401280_u128),
            ((1, 13), 1292469707114105_u64)
        ];
    }

    #[test]
    #[cfg_attr(target_arch = "wasm32", wasm_bindgen_test)]
    fn run_nondet_bigint3_value_not_in_scope() {
        let hint_code = "from starkware.cairo.common.cairo_secp.secp_utils import split\n\nsegments.write_arg(ids.res.address_, split(value))";
        let mut vm = vm_with_range_check!();
        // we don't initialize `value` now:
        //Initialize RubContext
        run_context!(vm, 0, 6, 6);
        //Create hint_data
        let ids_data = non_continuous_ids_data![("res", 5)];
        assert_matches!(
            run_hint!(vm, ids_data, hint_code),
            Err(HintError::VariableNotInScopeError(bx)) if bx.as_ref() == "value"
        );
    }

    #[test]
    #[cfg_attr(target_arch = "wasm32", wasm_bindgen_test)]
    fn run_nondet_bigint3_split_error() {
        let hint_code = "from starkware.cairo.common.cairo_secp.secp_utils import split\n\nsegments.write_arg(ids.res.address_, split(value))";
        let mut vm = vm_with_range_check!();

        // initialize vm scope with variable `n`
        let mut exec_scopes = scope![("value", bigint!(-1))];
        //Create hint_data
        let ids_data = non_continuous_ids_data![("res", 5)];
        assert_matches!(
            run_hint!(vm, ids_data, hint_code, &mut exec_scopes),
            Err(HintError::BigIntToBigUintFail)
        );
    }

    #[test]
    fn get_bigint3_from_base_addr_ok() {
        //BigInt3(1,2,3)
        let mut vm = vm!();
        vm.segments = segments![((0, 0), 1), ((0, 1), 2), ((0, 2), 3)];
        let x = BigInt3::from_base_addr((0, 0).into(), "x", &vm).unwrap();
        assert_eq!(x.limbs[0].as_ref(), &Felt252::ONE);
        assert_eq!(x.limbs[1].as_ref(), &Felt252::from(2));
        assert_eq!(x.limbs[2].as_ref(), &Felt252::from(3));
    }

    #[test]
    fn get_bigint5_from_base_addr_ok() {
        //BigInt3(1,2,3, 4, 5)
        let mut vm = vm!();
        vm.segments = segments![
            ((0, 0), 1),
            ((0, 1), 2),
            ((0, 2), 3),
            ((0, 3), 4),
            ((0, 4), 5)
        ];
        let x = BigInt5::from_base_addr((0, 0).into(), "x", &vm).unwrap();
        assert_eq!(x.limbs[0].as_ref(), &Felt252::ONE);
        assert_eq!(x.limbs[1].as_ref(), &Felt252::from(2));
        assert_eq!(x.limbs[2].as_ref(), &Felt252::from(3));
        assert_eq!(x.limbs[3].as_ref(), &Felt252::from(4));
        assert_eq!(x.limbs[4].as_ref(), &Felt252::from(5));
    }

    #[test]
    fn get_bigint3_from_base_addr_missing_member() {
        //BigInt3(1,2,x)
        let mut vm = vm!();
        vm.segments = segments![((0, 0), 1), ((0, 1), 2)];
        let r = BigInt3::from_base_addr((0, 0).into(), "x", &vm);
        assert_matches!(r,
            Err(HintError::IdentifierHasNoMember(bx))
            if *bx == ("x".to_string(), "d2".to_string())
        )
    }

    #[test]
    fn get_bigint5_from_base_addr_missing_member() {
        let mut vm = vm!();
        vm.segments = segments![((0, 0), 1), ((0, 1), 2), ((0, 2), 3), ((0, 3), 4),];
        let r = BigInt5::from_base_addr((0, 0).into(), "x", &vm);
        assert_matches!(r,
            Err(HintError::IdentifierHasNoMember(bx))
            if *bx == ("x".to_string(), "d4".to_string())
        )
    }

    #[test]
    fn get_bigint3_from_var_name_ok() {
        //BigInt3(1,2,3)
        let mut vm = vm!();
        vm.set_fp(1);
        vm.segments = segments![((1, 0), 1), ((1, 1), 2), ((1, 2), 3)];
        let ids_data = ids_data!["x"];
        let x = BigInt3::from_var_name("x", &vm, &ids_data, &ApTracking::default()).unwrap();
        assert_eq!(x.limbs[0].as_ref(), &Felt252::ONE);
        assert_eq!(x.limbs[1].as_ref(), &Felt252::from(2));
        assert_eq!(x.limbs[2].as_ref(), &Felt252::from(3));
    }

    #[test]
    fn get_bigint5_from_var_name_ok() {
        // BigInt5(1,2,3,4,5)
        let mut vm = vm!();
        vm.set_fp(1);
        vm.segments = segments![
            ((1, 0), 1),
            ((1, 1), 2),
            ((1, 2), 3),
            ((1, 3), 4),
            ((1, 4), 5)
        ];
        let ids_data = ids_data!["x"];
        let x = BigInt5::from_var_name("x", &vm, &ids_data, &ApTracking::default()).unwrap();
        assert_eq!(x.limbs[0].as_ref(), &Felt252::ONE);
        assert_eq!(x.limbs[1].as_ref(), &Felt252::from(2));
        assert_eq!(x.limbs[2].as_ref(), &Felt252::from(3));
        assert_eq!(x.limbs[3].as_ref(), &Felt252::from(4));
        assert_eq!(x.limbs[4].as_ref(), &Felt252::from(5));
    }

    #[test]
    fn get_bigint3_from_var_name_missing_member() {
        //BigInt3(1,2,x)
        let mut vm = vm!();
        vm.set_fp(1);
        vm.segments = segments![((1, 0), 1), ((1, 1), 2)];
        let ids_data = ids_data!["x"];
        let r = BigInt3::from_var_name("x", &vm, &ids_data, &ApTracking::default());
        assert_matches!(r,
            Err(HintError::IdentifierHasNoMember(bx))
            if *bx == ("x".to_string(), "d2".to_string())
        )
    }

    #[test]
    fn get_bigint5_from_var_name_missing_member() {
        //BigInt5(1,2,3,4,x)
        let mut vm = vm!();
        vm.set_fp(1);
        vm.segments = segments![((1, 0), 1), ((1, 1), 2), ((1, 2), 3), ((1, 3), 4)];
        let ids_data = ids_data!["x"];
        let r = BigInt5::from_var_name("x", &vm, &ids_data, &ApTracking::default());
        assert_matches!(r,
            Err(HintError::IdentifierHasNoMember(bx))
            if *bx == ("x".to_string(), "d4".to_string())
        )
    }

    #[test]
    fn get_bigint3_from_var_name_invalid_reference() {
        let mut vm = vm!();
        vm.segments = segments![((1, 0), 1), ((1, 1), 2), ((1, 2), 3)];
        let ids_data = ids_data!["x"];
        let r = BigInt3::from_var_name("x", &vm, &ids_data, &ApTracking::default());
        assert_matches!(r, Err(HintError::UnknownIdentifier(bx)) if bx.as_ref() == "x")
    }

    #[test]
    fn get_bigint5_from_var_name_invalid_reference() {
        let mut vm = vm!();
        // Will fail because fp was not set to 1.
        vm.segments = segments![
            ((1, 0), 1),
            ((1, 1), 2),
            ((1, 2), 3),
            ((1, 3), 4),
            ((1, 4), 5)
        ];
        let ids_data = ids_data!["x"];
        let r = BigInt5::from_var_name("x", &vm, &ids_data, &ApTracking::default());
        assert_matches!(r, Err(HintError::UnknownIdentifier(bx)) if bx.as_ref() == "x")
    }

    #[test]
    fn run_hi_max_bitlen_ok() {
        let hint_code =
            "ids.len_hi = max(ids.scalar_u.d2.bit_length(), ids.scalar_v.d2.bit_length())-1";

        let mut vm = vm_with_range_check!();

        // Initialize RunContext
        run_context!(vm, 0, 7, 0);

        vm.segments = segments![
            ((1, 0), 0),
            ((1, 1), 0),
            ((1, 2), 1),
            ((1, 3), 0),
            ((1, 4), 0),
            ((1, 5), 1)
        ];
        // Create hint_data
        let ids_data = non_continuous_ids_data![("scalar_u", 0), ("scalar_v", 3), ("len_hi", 6)];
        assert!(run_hint!(vm, ids_data, hint_code, exec_scopes_ref!()).is_ok());
        //Check hint memory inserts
        check_memory![vm.segments.memory, ((1, 6), 0)];
    }

    #[test]
    #[cfg_attr(target_arch = "wasm32", wasm_bindgen_test)]
    fn u384_pack86() {
        let pack_1 = Uint384 {
            limbs: [
                Cow::Borrowed(&Felt252::from(10_i32)),
                Cow::Borrowed(&Felt252::from(10_i32)),
                Cow::Borrowed(&Felt252::from(10_i32)),
            ],
        }
        .pack86();
        assert_eq!(
            pack_1,
            bigint_str!("59863107065073783529622931521771477038469668772249610")
        );

        let pack_2 = Uint384 {
            limbs: [
                Cow::Borrowed(&felt_str!("773712524553362")),
                Cow::Borrowed(&felt_str!("57408430697461422066401280")),
                Cow::Borrowed(&felt_str!("1292469707114105")),
            ],
        }
        .pack86();
        assert_eq!(
            pack_2,
            bigint_str!("7737125245533626718119526477371252455336267181195264773712524553362")
        );
    }
}
