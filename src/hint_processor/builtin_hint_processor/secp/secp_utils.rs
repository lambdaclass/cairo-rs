use crate::hint_processor::hint_processor_definition::HintReference;
use crate::math_utils::as_int;
use crate::serde::deserialize_program::ApTracking;
use crate::types::relocatable::Relocatable;
use crate::vm::errors::vm_errors::VirtualMachineError;
use crate::vm::vm_core::VirtualMachine;
use crate::{
    bigint, hint_processor::builtin_hint_processor::hint_utils::get_relocatable_from_var_name,
};
use num_bigint::BigInt;
use num_traits::{Signed, Zero};
use std::collections::HashMap;

// Constants in package "starkware.cairo.common.cairo_secp.constants".
pub const BASE_86: &str = "starkware.cairo.common.cairo_secp.constants.BASE";
pub const BETA: &str = "starkware.cairo.common.cairo_secp.constants.BETA";
pub const N0: &str = "starkware.cairo.common.cairo_secp.constants.N0";
pub const N1: &str = "starkware.cairo.common.cairo_secp.constants.N1";
pub const N2: &str = "starkware.cairo.common.cairo_secp.constants.N2";
pub const P0: &str = "starkware.cairo.common.cairo_secp.constants.P0";
pub const P1: &str = "starkware.cairo.common.cairo_secp.constants.P1";
pub const P2: &str = "starkware.cairo.common.cairo_secp.constants.P2";
pub const SECP_REM: &str = "starkware.cairo.common.cairo_secp.constants.SECP_REM";

/*
Takes a 256-bit integer and returns its canonical representation as:
d0 + BASE * d1 + BASE**2 * d2,
where BASE = 2**86.
*/
pub fn split(
    integer: &BigInt,
    constants: &HashMap<String, BigInt>,
) -> Result<[BigInt; 3], VirtualMachineError> {
    if integer.is_negative() {
        return Err(VirtualMachineError::SecpSplitNegative(integer.clone()));
    }

    let base_86_max = constants
        .get(BASE_86)
        .ok_or(VirtualMachineError::MissingConstant(BASE_86))?
        - &bigint!(1);

    let mut num = integer.clone();
    let mut canonical_repr: [BigInt; 3] = Default::default();
    for item in &mut canonical_repr {
        *item = (&num & &base_86_max).to_owned();
        num >>= 86_usize;
    }
    if !num.is_zero() {
        return Err(VirtualMachineError::SecpSplitutOfRange(integer.clone()));
    }
    Ok(canonical_repr)
}

/*
Takes an UnreducedBigInt3 struct which represents a triple of limbs (d0, d1, d2) of field
elements and reconstructs the corresponding 256-bit integer (see split()).
Note that the limbs do not have to be in the range [0, BASE).
prime should be the Cairo field, and it is used to handle negative values of the limbs.
*/
pub fn pack(d0: &BigInt, d1: &BigInt, d2: &BigInt, prime: &BigInt) -> BigInt {
    let unreduced_big_int_3 = vec![d0, d1, d2];

    unreduced_big_int_3
        .iter()
        .enumerate()
        .map(|(idx, value)| as_int(value, prime) << (idx * 86))
        .sum()
}

pub fn pack_from_var_name(
    name: &str,
    vm: &VirtualMachine,
    ids_data: &HashMap<String, HintReference>,
    ap_tracking: &ApTracking,
) -> Result<BigInt, VirtualMachineError> {
    let to_pack = get_relocatable_from_var_name(name, vm, ids_data, ap_tracking)?;

    let d0 = vm.get_integer(&to_pack)?;
    let d1 = vm.get_integer(&to_pack + 1)?;
    let d2 = vm.get_integer(to_pack + 2)?;

    Ok(pack(d0.as_ref(), d1.as_ref(), d2.as_ref(), vm.get_prime()))
}

pub fn pack_from_relocatable(
    rel: Relocatable,
    vm: &VirtualMachine,
) -> Result<BigInt, VirtualMachineError> {
    let d0 = vm.get_integer(&rel)?;
    let d1 = vm.get_integer(&rel + 1)?;
    let d2 = vm.get_integer(rel + 2)?;

    Ok(pack(d0.as_ref(), d1.as_ref(), d2.as_ref(), vm.get_prime()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::bigint_str;

    #[test]
    fn secp_split() {
        let mut constants = HashMap::new();
        constants.insert(BASE_86.to_string(), bigint!(1) << 86_usize);

        let array_1 = split(&bigint!(0), &constants);
        let array_2 = split(&bigint!(999992), &constants);
        let array_3 = split(
            &bigint_str!(b"7737125245533626718119526477371252455336267181195264773712524553362"),
            &constants,
        );
        let array_4 = split(&bigint!(-1), &constants);
        //TODO, Check SecpSplitutOfRange limit
        let array_5 = split(
            &bigint_str!(
                b"773712524553362671811952647737125245533626718119526477371252455336267181195264"
            ),
            &constants,
        );

        assert_eq!(array_1, Ok([bigint!(0), bigint!(0), bigint!(0)]));
        assert_eq!(array_2, Ok([bigint!(999992), bigint!(0), bigint!(0)]));
        assert_eq!(
            array_3,
            Ok([
                bigint_str!(b"773712524553362"),
                bigint_str!(b"57408430697461422066401280"),
                bigint_str!(b"1292469707114105")
            ])
        );
        assert_eq!(
            array_4,
            Err(VirtualMachineError::SecpSplitNegative(bigint!(-1)))
        );
        assert_eq!(
            array_5,
            Err(VirtualMachineError::SecpSplitutOfRange(bigint_str!(
                b"773712524553362671811952647737125245533626718119526477371252455336267181195264"
            )))
        );
    }

    #[test]
    fn secp_pack() {
        let pack_1 = pack(&bigint!(10), &bigint!(10), &bigint!(10), &bigint!(160));
        assert_eq!(
            pack_1,
            bigint_str!(b"59863107065073783529622931521771477038469668772249610")
        );

        let pack_2 = pack(
            &bigint_str!(b"773712524553362"),
            &bigint_str!(b"57408430697461422066401280"),
            &bigint_str!(b"1292469707114105"),
            &bigint_str!(b"1292469707114105"),
        );
        assert_eq!(
            pack_2,
            bigint_str!(b"4441762184457963985490320281689802156301430343378457")
        );
    }
}
