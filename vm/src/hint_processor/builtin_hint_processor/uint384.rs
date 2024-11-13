use crate::Felt252;
use num_bigint::BigUint;
use num_integer::Integer;
use num_traits::Zero;

use crate::math_utils::isqrt;
use crate::stdlib::{boxed::Box, collections::HashMap, prelude::*};
use crate::types::errors::math_errors::MathError;
use crate::{
    hint_processor::hint_processor_definition::HintReference,
    math_utils::pow2_const_nz,
    serde::deserialize_program::ApTracking,
    vm::{errors::hint_errors::HintError, vm_core::VirtualMachine},
};

use super::hint_utils::{
    get_constant_from_var_name, get_integer_from_var_name, get_relocatable_from_var_name,
    insert_value_from_var_name, insert_value_into_ap,
};
use super::secp::bigint_utils::Uint384;
// Notes: Hints in this lib use the type Uint384, which is equal to common lib's BigInt3

/* Implements Hint:
%{
    def split(num: int, num_bits_shift: int, length: int):
        a = []
        for _ in range(length):
            a.append( num & ((1 << num_bits_shift) - 1) )
            num = num >> num_bits_shift
        return tuple(a)

    def pack(z, num_bits_shift: int) -> int:
        limbs = (z.d0, z.d1, z.d2)
        return sum(limb << (num_bits_shift * i) for i, limb in enumerate(limbs))

    a = pack(ids.a, num_bits_shift = 128)
    div = pack(ids.div, num_bits_shift = 128)
    quotient, remainder = divmod(a, div)

    quotient_split = split(quotient, num_bits_shift=128, length=3)
    assert len(quotient_split) == 3

    ids.quotient.d0 = quotient_split[0]
    ids.quotient.d1 = quotient_split[1]
    ids.quotient.d2 = quotient_split[2]

    remainder_split = split(remainder, num_bits_shift=128, length=3)
    ids.remainder.d0 = remainder_split[0]
    ids.remainder.d1 = remainder_split[1]
    ids.remainder.d2 = remainder_split[2]
%}
*/
pub fn uint384_unsigned_div_rem(
    vm: &mut VirtualMachine,
    ids_data: &HashMap<String, HintReference>,
    ap_tracking: &ApTracking,
) -> Result<(), HintError> {
    let a = Uint384::from_var_name("a", vm, ids_data, ap_tracking)?.pack();
    let div = Uint384::from_var_name("div", vm, ids_data, ap_tracking)?.pack();

    if div.is_zero() {
        return Err(MathError::DividedByZero.into());
    }
    let (quotient, remainder) = a.div_mod_floor(&div);

    let quotient_split = Uint384::split(&quotient);
    quotient_split.insert_from_var_name("quotient", vm, ids_data, ap_tracking)?;

    let remainder_split = Uint384::split(&remainder);
    remainder_split.insert_from_var_name("remainder", vm, ids_data, ap_tracking)
}

/* Implements Hint:
    %{
        ids.low = ids.a & ((1<<128) - 1)
        ids.high = ids.a >> 128
    %}
*/
pub fn uint384_split_128(
    vm: &mut VirtualMachine,
    ids_data: &HashMap<String, HintReference>,
    ap_tracking: &ApTracking,
) -> Result<(), HintError> {
    let bound = pow2_const_nz(128);
    let a = get_integer_from_var_name("a", vm, ids_data, ap_tracking)?;
    let (high, low) = a.div_rem(bound);
    insert_value_from_var_name("low", low, vm, ids_data, ap_tracking)?;
    insert_value_from_var_name("high", high, vm, ids_data, ap_tracking)
}

/* Implements Hint:
%{
    sum_d0 = ids.a.d0 + ids.b.d0
    ids.carry_d0 = 1 if sum_d0 >= ids.SHIFT else 0
    sum_d1 = ids.a.d1 + ids.b.d1 + ids.carry_d0
    ids.carry_d1 = 1 if sum_d1 >= ids.SHIFT else 0
    sum_d2 = ids.a.d2 + ids.b.d2 + ids.carry_d1
    ids.carry_d2 = 1 if sum_d2 >= ids.SHIFT else 0
%}
 */
pub fn add_no_uint384_check(
    vm: &mut VirtualMachine,
    ids_data: &HashMap<String, HintReference>,
    ap_tracking: &ApTracking,
    constants: &HashMap<String, Felt252>,
) -> Result<(), HintError> {
    let a = Uint384::from_var_name("a", vm, ids_data, ap_tracking)?;
    let b = Uint384::from_var_name("b", vm, ids_data, ap_tracking)?;
    // This hint is not from the cairo commonlib, and its lib can be found under different paths, so we cant rely on a full path name
    let shift = get_constant_from_var_name("SHIFT", constants)?.to_biguint();

    let sum_d0 = (a.limbs[0].as_ref().to_biguint()) + (b.limbs[0].as_ref().to_biguint());
    let carry_d0 = BigUint::from((sum_d0 >= shift) as usize);
    let sum_d1 =
        (a.limbs[1].as_ref().to_biguint()) + (b.limbs[1].as_ref().to_biguint()) + &carry_d0;
    let carry_d1 = BigUint::from((sum_d1 >= shift) as usize);
    let sum_d2 =
        (a.limbs[2].as_ref().to_biguint()) + (b.limbs[2].as_ref().to_biguint()) + &carry_d1;
    let carry_d2 = Felt252::from((sum_d2 >= shift) as usize);

    insert_value_from_var_name(
        "carry_d0",
        Felt252::from(&carry_d0),
        vm,
        ids_data,
        ap_tracking,
    )?;
    insert_value_from_var_name(
        "carry_d1",
        Felt252::from(&carry_d1),
        vm,
        ids_data,
        ap_tracking,
    )?;
    insert_value_from_var_name("carry_d2", carry_d2, vm, ids_data, ap_tracking)
}

/* Implements Hint
%{
    from starkware.python.math_utils import isqrt

    def split(num: int, num_bits_shift: int, length: int):
        a = []
        for _ in range(length):
            a.append( num & ((1 << num_bits_shift) - 1) )
            num = num >> num_bits_shift
        return tuple(a)

    def pack(z, num_bits_shift: int) -> int:
        limbs = (z.d0, z.d1, z.d2)
        return sum(limb << (num_bits_shift * i) for i, limb in enumerate(limbs))

    a = pack(ids.a, num_bits_shift=128)
    root = isqrt(a)
    assert 0 <= root < 2 ** 192
    root_split = split(root, num_bits_shift=128, length=3)
    ids.root.d0 = root_split[0]
    ids.root.d1 = root_split[1]
    ids.root.d2 = root_split[2]
%}
 */
pub fn uint384_sqrt(
    vm: &mut VirtualMachine,
    ids_data: &HashMap<String, HintReference>,
    ap_tracking: &ApTracking,
) -> Result<(), HintError> {
    let a = Uint384::from_var_name("a", vm, ids_data, ap_tracking)?.pack();

    let root = isqrt(&a)?;

    if root.is_zero() || root.bits() > 192 {
        return Err(HintError::AssertionFailed(
            "assert 0 <= root < 2 ** 192".to_string().into_boxed_str(),
        ));
    }
    let root_split = Uint384::split(&root);
    root_split.insert_from_var_name("root", vm, ids_data, ap_tracking)
}

/* Implements Hint:
   memory[ap] = 1 if 0 <= (ids.a.d2 % PRIME) < 2 ** 127 else 0
*/
pub fn uint384_signed_nn(
    vm: &mut VirtualMachine,
    ids_data: &HashMap<String, HintReference>,
    ap_tracking: &ApTracking,
) -> Result<(), HintError> {
    let a_addr = get_relocatable_from_var_name("a", vm, ids_data, ap_tracking)?;
    let a_d2 = vm.get_integer((a_addr + 2)?).map_err(|_| {
        HintError::IdentifierHasNoMember(Box::new(("a".to_string(), "d2".to_string())))
    })?;
    let res = Felt252::from((a_d2.bits() <= 127) as u32);
    insert_value_into_ap(vm, res)
}

/* Implements Hint:
%{
    def split(num: int, num_bits_shift: int, length: int):
        a = []
        for _ in range(length):
        a.append( num & ((1 << num_bits_shift) - 1) )
        num = num >> num_bits_shift
        return tuple(a)

    def pack(z, num_bits_shift: int) -> int:
        limbs = (z.d0, z.d1, z.d2)
        return sum(limb << (num_bits_shift * i) for i, limb in enumerate(limbs))

    a = pack(ids.a, num_bits_shift = 128)
    b = pack(ids.b, num_bits_shift = 128)
    p = pack(ids.p, num_bits_shift = 128)

    res = (a - b) % p


    res_split = split(res, num_bits_shift=128, length=3)

    ids.res.d0 = res_split[0]
    ids.res.d1 = res_split[1]
    ids.res.d2 = res_split[2]
%}
*/
pub fn sub_reduced_a_and_reduced_b(
    vm: &mut VirtualMachine,
    ids_data: &HashMap<String, HintReference>,
    ap_tracking: &ApTracking,
) -> Result<(), HintError> {
    let a = Uint384::from_var_name("a", vm, ids_data, ap_tracking)?.pack();
    let b = Uint384::from_var_name("b", vm, ids_data, ap_tracking)?.pack();
    let p = Uint384::from_var_name("p", vm, ids_data, ap_tracking)?.pack();
    let res = if a > b {
        (a - b).mod_floor(&p)
    } else {
        &p - (b - &a).mod_floor(&p)
    };

    let res_split = Uint384::split(&res);
    res_split.insert_from_var_name("res", vm, ids_data, ap_tracking)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::hint_processor::builtin_hint_processor::hint_code;

    use crate::felt_str;
    use crate::{
        any_box,
        hint_processor::{
            builtin_hint_processor::builtin_hint_processor_definition::{
                BuiltinHintProcessor, HintProcessorData,
            },
            hint_processor_definition::HintProcessorLogic,
        },
        types::relocatable::{MaybeRelocatable, Relocatable},
        utils::test_utils::*,
        vm::{errors::memory_errors::MemoryError, vm_core::VirtualMachine},
    };
    use assert_matches::assert_matches;

    #[cfg(target_arch = "wasm32")]
    use wasm_bindgen_test::*;

    #[test]
    #[cfg_attr(target_arch = "wasm32", wasm_bindgen_test)]
    fn run_unsigned_div_rem_ok() {
        let mut vm = vm_with_range_check!();
        //Initialize fp
        vm.run_context.fp = 10;
        //Create hint_data
        let ids_data =
            non_continuous_ids_data![("a", -9), ("div", -6), ("quotient", -3), ("remainder", 0)];
        //Insert ids into memory
        vm.segments = segments![
            //a
            ((1, 1), 83434123481193248),
            ((1, 2), 82349321849739284),
            ((1, 3), 839243219401320423),
            //div
            ((1, 4), 9283430921839492319493),
            ((1, 5), 313248123482483248),
            ((1, 6), 3790328402913840)
        ];
        //Execute the hint
        assert_matches!(
            run_hint!(vm, ids_data, hint_code::UINT384_UNSIGNED_DIV_REM),
            Ok(())
        );
        //Check hint memory inserts
        check_memory![
            vm.segments.memory,
            // quotient
            ((1, 7), 221),
            ((1, 8), 0),
            ((1, 9), 0),
            // remainder
            //((1, 10), 340282366920936411825224315027446796751),
            //((1, 11), 340282366920938463394229121463989152931),
            ((1, 12), 1580642357361782)
        ];
        assert_eq!(
            vm.segments
                .memory
                .get_integer((1, 10).into())
                .unwrap()
                .as_ref(),
            &felt_str!("340282366920936411825224315027446796751")
        );
        assert_eq!(
            vm.segments
                .memory
                .get_integer((1, 11).into())
                .unwrap()
                .as_ref(),
            &felt_str!("340282366920938463394229121463989152931")
        );
    }

    #[test]
    #[cfg_attr(target_arch = "wasm32", wasm_bindgen_test)]
    fn run_unsigned_div_rem_divide_by_zero() {
        let mut vm = vm_with_range_check!();
        //Initialize fp
        vm.run_context.fp = 10;
        //Create hint_data
        let ids_data =
            non_continuous_ids_data![("a", -9), ("div", -6), ("quotient", -3), ("remainder", 0)];
        //Insert ids into memory
        vm.segments = segments![
            //a
            ((1, 1), 83434123481193248),
            ((1, 2), 82349321849739284),
            ((1, 3), 839243219401320423),
            //div
            ((1, 4), 0),
            ((1, 5), 0),
            ((1, 6), 0)
        ];
        //Execute the hint
        assert_matches!(
            run_hint!(vm, ids_data, hint_code::UINT384_UNSIGNED_DIV_REM),
            Err(HintError::Math(MathError::DividedByZero))
        );
    }

    #[test]
    #[cfg_attr(target_arch = "wasm32", wasm_bindgen_test)]
    fn run_unsigned_div_rem_invalid_memory_insert() {
        let mut vm = vm_with_range_check!();
        //Initialize fp
        vm.run_context.fp = 10;
        //Create hint_data
        let ids_data =
            non_continuous_ids_data![("a", -9), ("div", -6), ("quotient", -3), ("remainder", 0)];
        //Insert ids into memory
        vm.segments = segments![
            //a
            ((1, 1), 83434123481193248),
            ((1, 2), 82349321849739284),
            ((1, 3), 839243219401320423),
            //div
            ((1, 4), 9283430921839492319493),
            ((1, 5), 313248123482483248),
            ((1, 6), 3790328402913840),
            //quotient
            ((1, 7), 2)
        ];
        //Execute the hint
        assert_matches!(
            run_hint!(vm, ids_data, hint_code::UINT384_UNSIGNED_DIV_REM),
            Err(HintError::Memory(
                MemoryError::InconsistentMemory(bx)
            )) if *bx == (Relocatable::from((1, 7)),
                    MaybeRelocatable::from(Felt252::from(2)),
                    MaybeRelocatable::from(Felt252::from(221)))
        );
    }

    #[test]
    #[cfg_attr(target_arch = "wasm32", wasm_bindgen_test)]
    fn run_split_128_ok() {
        let mut vm = vm_with_range_check!();
        //Initialize fp
        vm.run_context.fp = 3;
        //Create hint_data
        let ids_data = ids_data!["a", "low", "high"];
        //Insert ids into memory
        vm.segments = segments![((1, 0), 34895349583295832495320945304)];
        //Execute the hint
        assert_matches!(
            run_hint!(vm, ids_data, hint_code::UINT384_SPLIT_128),
            Ok(())
        );
        //Check hint memory inserts
        check_memory![
            vm.segments.memory,
            // low
            ((1, 1), 34895349583295832495320945304),
            // high
            ((1, 2), 0)
        ];
    }

    #[test]
    #[cfg_attr(target_arch = "wasm32", wasm_bindgen_test)]
    fn run_split_128_ok_big_number() {
        let mut vm = vm_with_range_check!();
        //Initialize fp
        vm.run_context.fp = 3;
        //Create hint_data
        let ids_data = ids_data!["a", "low", "high"];
        //Insert ids into memory
        vm.segments.add();
        vm.segments.add();
        vm.segments
            .memory
            .insert(
                (1, 0).into(),
                Felt252::from(u128::MAX) * Felt252::from(20_u32),
            )
            .unwrap();
        //Execute the hint
        assert_matches!(
            run_hint!(vm, ids_data, hint_code::UINT384_SPLIT_128),
            Ok(())
        );
        //Check hint memory inserts
        check_memory![
            vm.segments.memory,
            // low
            //((1, 1), 340282366920938463463374607431768211454)
            // high
            ((1, 2), 19)
        ];
        assert_eq!(
            vm.segments
                .memory
                .get_integer((1, 1).into())
                .unwrap()
                .as_ref(),
            &felt_str!("340282366920938463463374607431768211436")
        );
    }

    #[test]
    #[cfg_attr(target_arch = "wasm32", wasm_bindgen_test)]
    fn run_split_128_invalid_memory_insert() {
        let mut vm = vm_with_range_check!();
        //Initialize fp
        vm.run_context.fp = 3;
        //Create hint_data
        let ids_data = ids_data!["a", "low", "high"];
        //Insert ids into memory
        vm.segments = segments![((1, 0), 34895349583295832495320945304), ((1, 1), 2)];
        //Execute the hint
        assert_matches!(
            run_hint!(vm, ids_data, hint_code::UINT384_SPLIT_128),
            Err(HintError::Memory(
                MemoryError::InconsistentMemory(bx)
            )) if *bx == (Relocatable::from((1, 1)),
                    MaybeRelocatable::from(Felt252::from(2)),
                    MaybeRelocatable::from(Felt252::from(34895349583295832495320945304_i128)))
        );
    }

    #[test]
    #[cfg_attr(target_arch = "wasm32", wasm_bindgen_test)]
    fn run_add_no_check_ok() {
        let mut vm = vm_with_range_check!();
        //Initialize fp
        vm.run_context.fp = 10;
        //Create hint_data
        let ids_data = non_continuous_ids_data![
            ("a", -10),
            ("b", -7),
            ("carry_d0", -4),
            ("carry_d1", -3),
            ("carry_d2", -2)
        ];
        //Insert ids into memory
        vm.segments = segments![
            // a
            ((1, 0), 3789423292314891293),
            ((1, 1), 21894),
            ((1, 2), 340282366920938463463374607431768211455_u128),
            // b
            ((1, 3), 32838232),
            ((1, 4), 17),
            ((1, 5), 8)
        ];
        //Execute the hint
        assert_matches!(
            run_hint!(
                vm,
                ids_data,
                hint_code::ADD_NO_UINT384_CHECK,
                &mut exec_scopes_ref!(),
                &[("path.path.path.SHIFT", crate::math_utils::pow2_const(128))]
                    .into_iter()
                    .map(|(k, v)| (k.to_string(), v))
                    .collect()
            ),
            Ok(())
        );
        //Check hint memory inserts
        check_memory![
            vm.segments.memory,
            // carry_d0
            ((1, 6), 0),
            // carry_d1
            ((1, 7), 0),
            // carry_d2
            ((1, 8), 1)
        ];
    }

    #[test]
    #[cfg_attr(target_arch = "wasm32", wasm_bindgen_test)]
    fn run_add_no_check_missing_constant() {
        let mut vm = vm_with_range_check!();
        //Initialize fp
        vm.run_context.fp = 10;
        //Create hint_data
        let ids_data = non_continuous_ids_data![
            ("a", -10),
            ("b", -7),
            ("carry_d0", -3),
            ("carry_d1", -2),
            ("carry_d2", -1)
        ];
        //Insert ids into memory
        vm.segments = segments![
            // a
            ((1, 0), 3789423292314891293),
            ((1, 1), 21894),
            ((1, 2), 340282366920938463463374607431768211455_u128),
            // b
            ((1, 3), 32838232),
            ((1, 4), 17),
            ((1, 5), 8)
        ];
        //Execute the hint
        assert_matches!(
            run_hint!(vm, ids_data, hint_code::ADD_NO_UINT384_CHECK),
            Err(HintError::MissingConstant(bx)) if &*bx == "SHIFT"
        );
    }

    #[test]
    #[cfg_attr(target_arch = "wasm32", wasm_bindgen_test)]
    fn run_sqrt_ok() {
        let mut vm = vm_with_range_check!();
        //Initialize fp
        vm.run_context.fp = 5;
        //Create hint_data
        let ids_data = non_continuous_ids_data![("a", -5), ("root", -2)];
        //Insert ids into memory
        vm.segments = segments![
            //a
            ((1, 0), 83434123481193248),
            ((1, 1), 82349321849739284),
            ((1, 2), 839243219401320423)
        ];
        //Execute the hint
        assert_matches!(run_hint!(vm, ids_data, hint_code::UINT384_SQRT), Ok(()));
        //Check hint memory inserts
        check_memory![
            vm.segments.memory,
            // root
            ((1, 3), 100835122758113432298839930225328621183),
            ((1, 4), 916102188),
            ((1, 5), 0)
        ];
    }

    #[test]
    #[cfg_attr(target_arch = "wasm32", wasm_bindgen_test)]
    fn run_sqrt_assertion_fail() {
        let mut vm = vm_with_range_check!();
        //Initialize fp
        vm.run_context.fp = 5;
        //Create hint_data
        let ids_data = non_continuous_ids_data![("a", -5), ("root", -2)];
        //Insert ids into memory
        //Insert ids into memory
        vm.segments = segments![
            //a
            ((1, 0), (-1)),
            ((1, 1), (-1)),
            ((1, 2), (-1))
        ];
        //Execute the hint
        assert_matches!(
            run_hint!(vm, ids_data, hint_code::UINT384_SQRT),
            Err(HintError::AssertionFailed(bx)) if bx.as_ref() == "assert 0 <= root < 2 ** 192"
        );
    }

    #[test]
    #[cfg_attr(target_arch = "wasm32", wasm_bindgen_test)]
    fn run_signed_nn_ok_positive() {
        let mut vm = vm_with_range_check!();
        //Initialize fp
        vm.run_context.fp = 3;
        //Create hint_data
        let ids_data = non_continuous_ids_data![("a", -2)];
        //Insert ids into memory
        vm.segments = segments![
            //a.d2
            ((1, 3), 1)
        ];
        //Execute the hint
        assert!(run_hint!(vm, ids_data, hint_code::UINT384_SIGNED_NN).is_ok());
        //Check hint memory inserts
        check_memory![
            vm.segments.memory,
            // ap
            ((1, 0), 1)
        ];
    }

    #[test]
    #[cfg_attr(target_arch = "wasm32", wasm_bindgen_test)]
    fn run_signed_nn_missing_identifier() {
        let mut vm = vm_with_range_check!();
        //Initialize fp
        vm.run_context.fp = 3;
        //Create hint_data
        let ids_data = non_continuous_ids_data![("a", -2)];
        //Insert ids into memory
        vm.segments = segments![
            //a.d0
            ((1, 1), 1),
            //a.d1
            ((1, 2), 1) //a.d2
        ];
        //Execute the hint
        assert_matches!(run_hint!(vm, ids_data, hint_code::UINT384_SIGNED_NN),
            Err(HintError::IdentifierHasNoMember(bx)) if *bx == ("a".to_string(), "d2".to_string())
        );
    }

    #[test]
    #[cfg_attr(target_arch = "wasm32", wasm_bindgen_test)]
    fn run_signed_nn_ok_negative() {
        let mut vm = vm_with_range_check!();
        //Initialize fp
        vm.run_context.fp = 3;
        //Create hint_data
        let ids_data = non_continuous_ids_data![("a", -2)];
        //Insert ids into memory
        vm.segments = segments![
            //a.d0
            ((1, 3), 170141183460469231731687303715884105729_u128)
        ];
        //Execute the hint
        assert!(run_hint!(vm, ids_data, hint_code::UINT384_SIGNED_NN).is_ok());
        //Check hint memory inserts
        check_memory![
            vm.segments.memory,
            // ap
            ((1, 0), 0)
        ];
    }

    #[test]
    #[cfg_attr(target_arch = "wasm32", wasm_bindgen_test)]
    fn run_uint384_sub_a_b_ok_a_max() {
        let mut vm = vm_with_range_check!();
        //Initialize fp
        vm.run_context.fp = 10;
        //Create hint_data
        let ids_data = non_continuous_ids_data![("a", -10), ("b", -7), ("p", -4), ("res", -1)];
        //Insert ids into memory
        vm.segments = segments![
            // a
            ((1, 0), 6),
            ((1, 1), 6),
            ((1, 2), 6),
            // b
            ((1, 3), 1),
            ((1, 4), 1),
            ((1, 5), 1),
            // p
            ((1, 6), 7),
            ((1, 7), 7),
            ((1, 8), 7)
        ];
        //Execute the hint
        assert!(run_hint!(vm, ids_data, hint_code::SUB_REDUCED_A_AND_REDUCED_B).is_ok());
        //Check hint memory inserts
        check_memory![
            vm.segments.memory,
            // res
            ((1, 9), 5),
            ((1, 10), 5),
            ((1, 11), 5)
        ];
    }

    #[test]
    #[cfg_attr(target_arch = "wasm32", wasm_bindgen_test)]
    fn run_uint384_sub_a_b_ok_b_max() {
        let mut vm = vm_with_range_check!();
        //Initialize fp
        vm.run_context.fp = 10;
        //Create hint_data
        let ids_data = non_continuous_ids_data![("a", -10), ("b", -7), ("p", -4), ("res", -1)];
        //Insert ids into memory
        vm.segments = segments![
            // a
            ((1, 0), 3),
            ((1, 1), 3),
            ((1, 2), 3),
            // b
            ((1, 3), 5),
            ((1, 4), 5),
            ((1, 5), 5),
            // p
            ((1, 6), 7),
            ((1, 7), 7),
            ((1, 8), 7)
        ];
        //Execute the hint
        assert!(run_hint!(vm, ids_data, hint_code::SUB_REDUCED_A_AND_REDUCED_B).is_ok());
        //Check hint memory inserts
        check_memory![
            vm.segments.memory,
            // res
            ((1, 9), 5),
            ((1, 10), 5),
            ((1, 11), 5)
        ];
    }
}
