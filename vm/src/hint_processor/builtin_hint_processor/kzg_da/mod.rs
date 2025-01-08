use core::str::FromStr;

use super::{
    hint_utils::get_relocatable_from_var_name,
    secp::{bigint_utils::BigInt3, secp_utils::SECP_P},
};
use crate::{
    hint_processor::hint_processor_definition::HintReference,
    serde::deserialize_program::ApTracking,
    types::relocatable::MaybeRelocatable,
    vm::{errors::hint_errors::HintError, vm_core::VirtualMachine},
    Felt252,
};
use crate::{
    stdlib::{collections::HashMap, ops::Deref, prelude::*},
    types::exec_scope::ExecutionScopes,
};
use lazy_static::lazy_static;
use num_bigint::BigInt;
use num_integer::Integer;
use num_traits::FromPrimitive;
use num_traits::Zero;

lazy_static! {
    static ref BLS_BASE: BigInt = BigInt::from_u64(2).unwrap().pow(86);
    static ref BLS_PRIME: BigInt = BigInt::from_str(
        "52435875175126190479447740508185965837690552500527637822603658699938581184513"
    )
    .unwrap();
}
pub const WRITE_DIVMOD_SEGMENT: &str = r#"from starkware.starknet.core.os.data_availability.bls_utils import BLS_PRIME, pack, split

a = pack(ids.a, PRIME)
b = pack(ids.b, PRIME)

q, r = divmod(a * b, BLS_PRIME)

# By the assumption: |a|, |b| < 2**104 * ((2**86) ** 2 + 2**86 + 1) < 2**276.001.
# Therefore |q| <= |ab| / BLS_PRIME < 2**299.
# Hence the absolute value of the high limb of split(q) < 2**127.
segments.write_arg(ids.q.address_, split(q))
segments.write_arg(ids.res.address_, split(r))"#;

pub fn write_div_mod_segment(
    vm: &mut VirtualMachine,
    _exec_scopes: &mut ExecutionScopes,
    ids_data: &HashMap<String, HintReference>,
    ap_tracking: &ApTracking,
    _constants: &HashMap<String, Felt252>,
) -> Result<(), HintError> {
    let a = bls_pack(
        &BigInt3::from_var_name("a", vm, ids_data, ap_tracking)?,
        &SECP_P,
    );
    let b = bls_pack(
        &BigInt3::from_var_name("b", vm, ids_data, ap_tracking)?,
        &SECP_P,
    );
    let (q, r) = (a * b).div_mod_floor(&BLS_PRIME);
    let q_reloc = get_relocatable_from_var_name("q", vm, ids_data, ap_tracking)?;
    let res_reloc = get_relocatable_from_var_name("res", vm, ids_data, ap_tracking)?;

    let q_arg: Vec<MaybeRelocatable> = bls_split(q)
        .into_iter()
        .map(|ref n| Felt252::from(n).into())
        .collect::<Vec<MaybeRelocatable>>();
    let res_arg: Vec<MaybeRelocatable> = bls_split(r)
        .into_iter()
        .map(|ref n| Felt252::from(n).into())
        .collect::<Vec<MaybeRelocatable>>();
    vm.write_arg(q_reloc, &q_arg).map_err(HintError::Memory)?;
    vm.write_arg(res_reloc, &res_arg)
        .map_err(HintError::Memory)?;
    Ok(())
}

fn bls_split(mut num: BigInt) -> Vec<BigInt> {
    use num_traits::Signed;
    let mut a = Vec::new();
    for _ in 0..2 {
        let residue = &num % BLS_BASE.deref();
        num /= BLS_BASE.deref();
        a.push(residue);
    }
    assert!(num.abs() < BigInt::from_u128(1 << 127).unwrap());
    a.push(num);
    a
}

fn as_int(value: BigInt, prime: &BigInt) -> BigInt {
    let half_prime = prime / 2u32;
    if value > half_prime {
        value - prime
    } else {
        value
    }
}

fn bls_pack(z: &BigInt3, prime: &BigInt) -> BigInt {
    let limbs = &z.limbs;
    limbs
        .iter()
        .enumerate()
        .fold(BigInt::zero(), |acc, (i, limb)| {
            let limb_as_int = as_int(limb.to_bigint(), prime);
            acc + limb_as_int * &BLS_BASE.pow(i as u32)
        })
}
