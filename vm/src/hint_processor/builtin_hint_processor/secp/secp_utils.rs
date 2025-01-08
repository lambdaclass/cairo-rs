use core::str::FromStr;

use crate::stdlib::{boxed::Box, prelude::*};

use crate::vm::errors::hint_errors::HintError;

use lazy_static::lazy_static;
use num_bigint::{BigInt, BigUint};
use num_traits::{Num, Zero};

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
// Constants in package "starkware.cairo.common.cairo_secp.secp_utils"
lazy_static! {
    //SECP_P = 2**256 - 2**32 - 2**9 - 2**8 - 2**7 - 2**6 - 2**4 - 1
    pub(crate) static ref SECP_P: BigInt = BigInt::from_str(
        "115792089237316195423570985008687907853269984665640564039457584007908834671663"
    )
    .unwrap();
    //SECP_P_V2 = 2**255-19
    pub(crate) static ref SECP_P_V2: BigInt = BigInt::from_str(
        "57896044618658097711785492504343953926634992332820282019728792003956564819949"
    )
    .unwrap();

    pub(crate) static ref ALPHA: BigInt = BigInt::zero();

    pub(crate) static ref ALPHA_V2: BigInt = BigInt::from_str(
        "42204101795669822316448953119945047945709099015225996174933988943478124189485"
    )
    .unwrap();

    // BASE = 2**86
    pub(crate) static ref BASE: BigUint = BigUint::from_str(
        "77371252455336267181195264"
    ).unwrap();

    // Convenience constant BASE - 1
    pub(crate) static ref BASE_MINUS_ONE: BigUint = BigUint::from_str(
        "77371252455336267181195263"
    ).unwrap();
    // N = 0xFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFEBAAEDCE6AF48A03BBFD25E8CD0364141
    pub(crate) static ref N: BigInt = BigInt::from_str(
        "115792089237316195423570985008687907852837564279074904382605163141518161494337"
    ).unwrap();
}
// Constants in package "starkware.cairo.common.cairo_secp.secp256r1_utils"
lazy_static! {
    //SECP256R1_P = 2**256 - 2**224 + 2**192 + 2**96 - 1
    pub(crate) static ref SECP256R1_P: BigInt = BigInt::from_str(
        "115792089210356248762697446949407573530086143415290314195533631308867097853951"
    ).unwrap();
    //SECP256R1_N = 0xFFFFFFFF00000000FFFFFFFFFFFFFFFFBCE6FAADA7179E84F3B9CAC2FC632551
    pub(crate) static ref SECP256R1_N: BigInt = BigInt::from_str(
        "115792089210356248762697446949407573529996955224135760342422259061068512044369"
    ).unwrap();
    //SECP256R1_ALPHA = 0xFFFFFFFF00000001000000000000000000000000FFFFFFFFFFFFFFFFFFFFFFFC
    pub(crate) static ref SECP256R1_ALPHA: BigInt = BigInt::from_str(
        "115792089210356248762697446949407573530086143415290314195533631308867097853948"
    ).unwrap();
    pub(crate) static ref SECP256R1_B: BigInt = BigInt::from_str_radix(
        "5AC635D8AA3A93E7B3EBBD55769886BC651D06B0CC53B0F63BCE3C3E27D2604B",
        16,
    )
    .unwrap();
}

/*
Takes a 256-bit integer and returns its canonical representation as:
d0 + BASE * d1 + BASE**2 * d2,
where BASE = 2**86.
*/
pub fn bigint3_split(integer: &num_bigint::BigUint) -> Result<[num_bigint::BigUint; 3], HintError> {
    let mut canonical_repr: [num_bigint::BigUint; 3] = Default::default();
    let mut num = integer.clone();
    for item in &mut canonical_repr {
        *item = &num & &*BASE_MINUS_ONE;
        num >>= 86_usize;
    }

    if !num.is_zero() {
        return Err(HintError::SecpSplitOutOfRange(Box::new(integer.clone())));
    }
    Ok(canonical_repr)
}

#[cfg(test)]
mod tests {
    use super::*;

    use crate::stdlib::{collections::HashMap, string::ToString};
    use crate::utils::test_utils::*;
    use assert_matches::assert_matches;
    use num_bigint::BigUint;

    #[cfg(target_arch = "wasm32")]
    use wasm_bindgen_test::*;

    #[test]
    #[cfg_attr(target_arch = "wasm32", wasm_bindgen_test)]
    fn secp_split() {
        let mut constants = HashMap::new();
        constants.insert(BASE_86.to_string(), crate::math_utils::pow2_const(86));

        let array_1 = bigint3_split(&BigUint::zero());
        let array_2 = bigint3_split(
            &bigint!(999992)
                .to_biguint()
                .expect("Couldn't convert to BigUint"),
        );
        let array_3 = bigint3_split(
            &bigint_str!("7737125245533626718119526477371252455336267181195264773712524553362")
                .to_biguint()
                .expect("Couldn't convert to BigUint"),
        );
        //TODO, Check SecpSplitutOfRange limit
        let array_4 = bigint3_split(
            &bigint_str!(
                "773712524553362671811952647737125245533626718119526477371252455336267181195264"
            )
            .to_biguint()
            .expect("Couldn't convert to BigUint"),
        );

        assert_matches!(
            array_1,
            Ok(x) if x == [BigUint::zero(), BigUint::zero(), BigUint::zero()]
        );
        assert_matches!(
            array_2,
            Ok(x) if x == [
                bigint!(999992)
                    .to_biguint()
                    .expect("Couldn't convert to BigUint"),
                BigUint::zero(),
                BigUint::zero()
            ]
        );
        assert_matches!(
            array_3,
            Ok(x) if x == [
                bigint_str!("773712524553362")
                    .to_biguint()
                    .expect("Couldn't convert to BigUint"),
                bigint_str!("57408430697461422066401280")
                    .to_biguint()
                    .expect("Couldn't convert to BigUint"),
                bigint_str!("1292469707114105")
                    .to_biguint()
                    .expect("Couldn't convert to BigUint")
            ]
        );
        assert_matches!(
            array_4,
            Err(HintError::SecpSplitOutOfRange(bx)) if *bx == bigint_str!(
                    "773712524553362671811952647737125245533626718119526477371252455336267181195264"
                )
                    .to_biguint()
                    .expect("Couldn't convert to BigUint")

        );
    }
}
