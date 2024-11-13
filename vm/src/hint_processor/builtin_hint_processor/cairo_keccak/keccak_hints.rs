use crate::stdlib::{
    borrow::{Cow, ToOwned},
    boxed::Box,
    collections::HashMap,
    prelude::*,
};
use crate::{
    hint_processor::{
        builtin_hint_processor::hint_utils::{
            get_integer_from_var_name, get_ptr_from_var_name, insert_value_from_var_name,
            insert_value_into_ap,
        },
        hint_processor_definition::HintReference,
    },
    math_utils::pow2_const_nz,
    serde::deserialize_program::ApTracking,
    types::{errors::math_errors::MathError, relocatable::MaybeRelocatable},
    vm::{
        errors::{hint_errors::HintError, vm_errors::VirtualMachineError},
        vm_core::VirtualMachine,
    },
    Felt252,
};
use num_traits::ToPrimitive;

// Constants in package "starkware.cairo.common.cairo_keccak.keccak".
const BYTES_IN_WORD: &str = "starkware.cairo.common.cairo_keccak.keccak.BYTES_IN_WORD";
const KECCAK_FULL_RATE_IN_BYTES_CAIRO_KECCAK: &str =
    "starkware.cairo.common.cairo_keccak.keccak.KECCAK_FULL_RATE_IN_BYTES";
const KECCAK_FULL_RATE_IN_BYTES_BUILTIN_KECCAK: &str =
    "starkware.cairo.common.builtin_keccak.keccak.KECCAK_FULL_RATE_IN_BYTES";

const KECCAK_FULL_RATE_IN_BYTES: &str = "KECCAK_FULL_RATE_IN_BYTES";

const KECCAK_STATE_SIZE_FELTS: &str =
    "starkware.cairo.common.cairo_keccak.keccak.KECCAK_STATE_SIZE_FELTS";

// Constants in package "starkware.cairo.common.cairo_keccak.packed_keccak".
const BLOCK_SIZE: &str = "starkware.cairo.common.cairo_keccak.packed_keccak.BLOCK_SIZE";

/*
Implements hint:
    %{
      segments.write_arg(ids.inputs, [ids.low % 2 ** 64, ids.low // 2 ** 64])
      segments.write_arg(ids.inputs + 2, [ids.high % 2 ** 64, ids.high // 2 ** 64])
    %}
*/
pub fn keccak_write_args(
    vm: &mut VirtualMachine,
    ids_data: &HashMap<String, HintReference>,
    ap_tracking: &ApTracking,
) -> Result<(), HintError> {
    let inputs_ptr = get_ptr_from_var_name("inputs", vm, ids_data, ap_tracking)?;

    let low = get_integer_from_var_name("low", vm, ids_data, ap_tracking)?;
    let high = get_integer_from_var_name("high", vm, ids_data, ap_tracking)?;

    let bound = pow2_const_nz(64);
    let (d1, d0) = low.div_rem(bound);
    let (d3, d2) = high.div_rem(bound);
    let args: Vec<_> = [d0, d1, d2, d3]
        .into_iter()
        .map(MaybeRelocatable::from)
        .collect();

    vm.write_arg(inputs_ptr, &args)?;

    Ok(())
}

/*
Implements hint:
    Cairo code:
    if nondet %{ ids.n_bytes < ids.BYTES_IN_WORD %} != 0:

    Compiled code:
    memory[ap] = to_felt_or_relocatable(ids.n_bytes < ids.BYTES_IN_WORD)
*/
pub fn compare_bytes_in_word_nondet(
    vm: &mut VirtualMachine,
    ids_data: &HashMap<String, HintReference>,
    ap_tracking: &ApTracking,
    constants: &HashMap<String, Felt252>,
) -> Result<(), HintError> {
    let n_bytes = get_integer_from_var_name("n_bytes", vm, ids_data, ap_tracking)?;
    let n_bytes = n_bytes.as_ref();

    // This works fine, but it should be checked for a performance improvement.
    // One option is to try to convert n_bytes into usize, with failure to do so simply
    // making value be 0 (if it can't convert then it's either negative, which can't be in Cairo memory
    // or too big, which also means n_bytes > BYTES_IN_WORD). The other option is to exctract
    // Felt252::from(BYTES_INTO_WORD) into a lazy_static!
    let bytes_in_word = constants
        .get(BYTES_IN_WORD)
        .ok_or_else(|| HintError::MissingConstant(BYTES_IN_WORD.to_string().into_boxed_str()))?;
    let value = Felt252::from((n_bytes < bytes_in_word) as usize);
    insert_value_into_ap(vm, value)
}

/*
Implements hint:
    Cairo code:
    if nondet %{ ids.n_bytes >= ids.KECCAK_FULL_RATE_IN_BYTES %} != 0:

    Compiled code:
    "memory[ap] = to_felt_or_relocatable(ids.n_bytes >= ids.KECCAK_FULL_RATE_IN_BYTES)"
*/
pub fn compare_keccak_full_rate_in_bytes_nondet(
    vm: &mut VirtualMachine,
    ids_data: &HashMap<String, HintReference>,
    ap_tracking: &ApTracking,
    constants: &HashMap<String, Felt252>,
) -> Result<(), HintError> {
    let n_bytes = get_integer_from_var_name("n_bytes", vm, ids_data, ap_tracking)?;
    let n_bytes = n_bytes.as_ref();

    let keccak_full_rate_in_bytes = constants
        .get(KECCAK_FULL_RATE_IN_BYTES_CAIRO_KECCAK)
        .or_else(|| constants.get(KECCAK_FULL_RATE_IN_BYTES_BUILTIN_KECCAK))
        .ok_or_else(|| {
            HintError::MissingConstant(KECCAK_FULL_RATE_IN_BYTES.to_string().into_boxed_str())
        })?;
    let value = Felt252::from((n_bytes >= keccak_full_rate_in_bytes) as usize);
    insert_value_into_ap(vm, value)
}

/*
Implements hints:
    %{
        from starkware.cairo.common.cairo_keccak.keccak_utils import keccak_func
        _keccak_state_size_felts = int(ids.KECCAK_STATE_SIZE_FELTS)
        assert 0 <= _keccak_state_size_felts < 100

        output_values = keccak_func(memory.get_range(
            ids.keccak_ptr - _keccak_state_size_felts, _keccak_state_size_felts))
        segments.write_arg(ids.keccak_ptr, output_values)
    %}
    %{
        from starkware.cairo.common.cairo_keccak.keccak_utils import keccak_func
        _keccak_state_size_felts = int(ids.KECCAK_STATE_SIZE_FELTS)
        assert 0 <= _keccak_state_size_felts < 100

        output_values = keccak_func(memory.get_range(
            ids.keccak_ptr - _keccak_state_size_felts, _keccak_state_size_felts))
        segments.write_arg(ids.keccak_ptr, output_values)
    %}
*/
pub(crate) fn block_permutation_v1(
    vm: &mut VirtualMachine,
    ids_data: &HashMap<String, HintReference>,
    ap_tracking: &ApTracking,
    constants: &HashMap<String, Felt252>,
) -> Result<(), HintError> {
    let keccak_state_size_felts = constants.get(KECCAK_STATE_SIZE_FELTS).ok_or_else(|| {
        HintError::MissingConstant(KECCAK_STATE_SIZE_FELTS.to_string().into_boxed_str())
    })?;
    if keccak_state_size_felts >= &Felt252::from(100_i32) {
        return Err(HintError::InvalidKeccakStateSizeFelt252s(Box::new(
            *keccak_state_size_felts,
        )));
    }

    let keccak_ptr = get_ptr_from_var_name("keccak_ptr", vm, ids_data, ap_tracking)?;

    let keccak_state_size_felts = keccak_state_size_felts.to_usize().unwrap();
    let values = vm.get_range(
        (keccak_ptr - keccak_state_size_felts)?,
        keccak_state_size_felts,
    );
    let mut u64_values = maybe_reloc_vec_to_u64_array(&values)?
        .try_into()
        .map_err(|_| VirtualMachineError::SliceToArrayError)?;

    // this function of the keccak crate is the one used instead of keccak_func from
    // keccak_utils.py
    keccak::f1600(&mut u64_values);

    let bigint_values = u64_array_to_mayberelocatable_vec(&u64_values);

    vm.write_arg(keccak_ptr, &bigint_values)
        .map_err(HintError::Memory)?;

    Ok(())
}

/*
Implements hint:
    %{
        ids.full_word = int(ids.n_bytes >= 8)
    %}
*/
pub(crate) fn cairo_keccak_is_full_word(
    vm: &mut VirtualMachine,
    ids_data: &HashMap<String, HintReference>,
    ap_tracking: &ApTracking,
) -> Result<(), HintError> {
    let n_bytes = get_integer_from_var_name("n_bytes", vm, ids_data, ap_tracking)?
        .to_usize()
        .unwrap_or(8); // Hack: if it doesn't fit `usize` then it's >= 8
    let full_word = Felt252::from((n_bytes >= 8) as usize);
    insert_value_from_var_name("full_word", full_word, vm, ids_data, ap_tracking)
}

/*
Implements hint:
    %{
        from starkware.cairo.common.cairo_keccak.keccak_utils import keccak_func
        _keccak_state_size_felts = int(ids.KECCAK_STATE_SIZE_FELTS)
        assert 0 <= _keccak_state_size_felts < 100
        output_values = keccak_func(memory.get_range(
            ids.keccak_ptr_start, _keccak_state_size_felts))
        segments.write_arg(ids.output, output_values)
    %}
*/
pub(crate) fn block_permutation_v2(
    vm: &mut VirtualMachine,
    ids_data: &HashMap<String, HintReference>,
    ap_tracking: &ApTracking,
    constants: &HashMap<String, Felt252>,
) -> Result<(), HintError> {
    let keccak_state_size_felts = constants.get(KECCAK_STATE_SIZE_FELTS).ok_or_else(|| {
        HintError::MissingConstant(KECCAK_STATE_SIZE_FELTS.to_string().into_boxed_str())
    })?;
    if keccak_state_size_felts >= &Felt252::from(100_i32) {
        return Err(HintError::InvalidKeccakStateSizeFelt252s(Box::new(
            *keccak_state_size_felts,
        )));
    }

    let keccak_ptr = get_ptr_from_var_name("keccak_ptr_start", vm, ids_data, ap_tracking)?;

    let keccak_state_size_felts = keccak_state_size_felts.to_usize().unwrap();
    let values = vm.get_range(keccak_ptr, keccak_state_size_felts);
    let mut u64_values = maybe_reloc_vec_to_u64_array(&values)?
        .try_into()
        .map_err(|_| VirtualMachineError::SliceToArrayError)?;

    // this function of the keccak crate is the one used instead of keccak_func from
    // keccak_utils.py
    keccak::f1600(&mut u64_values);

    let bigint_values = u64_array_to_mayberelocatable_vec(&u64_values);

    let output = get_ptr_from_var_name("output", vm, ids_data, ap_tracking)?;
    vm.write_arg(output, &bigint_values)
        .map_err(HintError::Memory)?;

    Ok(())
}

fn cairo_keccak_finalize(
    vm: &mut VirtualMachine,
    ids_data: &HashMap<String, HintReference>,
    ap_tracking: &ApTracking,
    constants: &HashMap<String, Felt252>,
    block_size_limit: usize,
) -> Result<(), HintError> {
    let keccak_state_size_felts = constants.get(KECCAK_STATE_SIZE_FELTS).ok_or_else(|| {
        HintError::MissingConstant(KECCAK_STATE_SIZE_FELTS.to_string().into_boxed_str())
    })?;
    let block_size = constants
        .get(BLOCK_SIZE)
        .ok_or_else(|| HintError::MissingConstant(BLOCK_SIZE.to_string().into_boxed_str()))?;

    if keccak_state_size_felts >= &Felt252::from(100_i32) {
        return Err(HintError::InvalidKeccakStateSizeFelt252s(Box::new(
            *keccak_state_size_felts,
        )));
    }

    if block_size >= &Felt252::from(block_size_limit) {
        return Err(HintError::InvalidBlockSize(Box::new(*block_size)));
    };

    let keccak_state_size_felts = keccak_state_size_felts.to_usize().unwrap();
    let block_size = block_size.to_usize().unwrap();

    let mut inp = vec![0; keccak_state_size_felts]
        .try_into()
        .map_err(|_| VirtualMachineError::SliceToArrayError)?;
    keccak::f1600(&mut inp);

    let mut padding = vec![Felt252::ZERO.into(); keccak_state_size_felts];
    padding.extend(u64_array_to_mayberelocatable_vec(&inp));

    let base_padding = padding.clone();

    for _ in 0..(block_size - 1) {
        padding.extend_from_slice(base_padding.as_slice());
    }

    let keccak_ptr_end = get_ptr_from_var_name("keccak_ptr_end", vm, ids_data, ap_tracking)?;

    vm.write_arg(keccak_ptr_end, &padding)
        .map_err(HintError::Memory)?;

    Ok(())
}

/* Implements hint:
    %{
        # Add dummy pairs of input and output.
        _keccak_state_size_felts = int(ids.KECCAK_STATE_SIZE_FELTS)
        _block_size = int(ids.BLOCK_SIZE)
        assert 0 <= _keccak_state_size_felts < 100
        assert 0 <= _block_size < 10
        inp = [0] * _keccak_state_size_felts
        padding = (inp + keccak_func(inp)) * _block_size
        segments.write_arg(ids.keccak_ptr_end, padding)
    %}
*/
pub(crate) fn cairo_keccak_finalize_v1(
    vm: &mut VirtualMachine,
    ids_data: &HashMap<String, HintReference>,
    ap_tracking: &ApTracking,
    constants: &HashMap<String, Felt252>,
) -> Result<(), HintError> {
    cairo_keccak_finalize(vm, ids_data, ap_tracking, constants, 10)
}

/* Implements hint:
    %{
        # Add dummy pairs of input and output.
        _keccak_state_size_felts = int(ids.KECCAK_STATE_SIZE_FELTS)
        _block_size = int(ids.BLOCK_SIZE)
        assert 0 <= _keccak_state_size_felts < 100
        assert 0 <= _block_size < 1000
        inp = [0] * _keccak_state_size_felts
        padding = (inp + keccak_func(inp)) * _block_size
        segments.write_arg(ids.keccak_ptr_end, padding)
    %}
*/
pub(crate) fn cairo_keccak_finalize_v2(
    vm: &mut VirtualMachine,
    ids_data: &HashMap<String, HintReference>,
    ap_tracking: &ApTracking,
    constants: &HashMap<String, Felt252>,
) -> Result<(), HintError> {
    cairo_keccak_finalize(vm, ids_data, ap_tracking, constants, 1000)
}

// Helper function to transform a vector of MaybeRelocatables into a vector
// of u64. Raises error if there are None's or if MaybeRelocatables are not Bigints.
pub(crate) fn maybe_reloc_vec_to_u64_array(
    vec: &[Option<Cow<MaybeRelocatable>>],
) -> Result<Vec<u64>, HintError> {
    let array = vec
        .iter()
        .map(|n| match n {
            Some(Cow::Owned(MaybeRelocatable::Int(ref num)))
            | Some(Cow::Borrowed(MaybeRelocatable::Int(ref num))) => num
                .to_u64()
                .ok_or_else(|| MathError::Felt252ToU64Conversion(Box::new(*num)).into()),
            _ => Err(VirtualMachineError::ExpectedIntAtRange(Box::new(
                n.as_ref().map(|x| x.as_ref().to_owned()),
            ))),
        })
        .collect::<Result<Vec<u64>, VirtualMachineError>>()?;

    Ok(array)
}

pub fn u64_array_to_mayberelocatable_vec(array: &[u64]) -> Vec<MaybeRelocatable> {
    array.iter().map(|n| Felt252::from(*n).into()).collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::stdlib::string::ToString;

    use crate::{
        any_box,
        hint_processor::{
            builtin_hint_processor::builtin_hint_processor_definition::{
                BuiltinHintProcessor, HintProcessorData,
            },
            hint_processor_definition::{HintProcessorLogic, HintReference},
        },
        types::relocatable::Relocatable,
        utils::test_utils::*,
        vm::vm_core::VirtualMachine,
    };
    use assert_matches::assert_matches;
    use rstest::*;

    #[cfg(target_arch = "wasm32")]
    use wasm_bindgen_test::*;

    #[rstest]
    #[cfg_attr(target_arch = "wasm32", wasm_bindgen_test)]
    #[case(0, 0)]
    #[case(1, 0)]
    #[case(7, 0)]
    #[case(8, 1)]
    #[case(16, 1)]
    #[case(usize::MAX as i128, 1)]
    #[case(i128::MAX, 1)]
    fn cairo_keccak_is_full_word(#[case] n_bytes: i128, #[case] full_bytes: usize) {
        let hint_code = "ids.full_word = int(ids.n_bytes >= 8)";
        let mut vm = vm_with_range_check!();
        vm.segments = segments![((1, 1), n_bytes)];
        vm.run_context.fp = 2;
        let ids_data = ids_data!["full_word", "n_bytes"];
        assert_matches!(run_hint!(vm, ids_data, hint_code), Ok(()));
        check_memory![vm.segments.memory, ((1, 0), full_bytes)];
    }

    #[test]
    #[cfg_attr(target_arch = "wasm32", wasm_bindgen_test)]
    fn keccak_write_args_valid_test() {
        let hint_code = "segments.write_arg(ids.inputs, [ids.low % 2 ** 64, ids.low // 2 ** 64])\nsegments.write_arg(ids.inputs + 2, [ids.high % 2 ** 64, ids.high // 2 ** 64])";
        let mut vm = vm_with_range_check!();
        vm.segments = segments![
            ((1, 0), 233),
            ((1, 1), 351),
            ((1, 2), (2, 0)),
            ((2, 4), 5_i32)
        ];
        //Initialize fp
        vm.run_context.fp = 3;
        //Create ids
        let ids_data = ids_data!["low", "high", "inputs"];
        assert_matches!(run_hint!(vm, ids_data, hint_code), Ok(()));
    }

    #[test]
    #[cfg_attr(target_arch = "wasm32", wasm_bindgen_test)]
    fn keccak_write_args_write_error() {
        let hint_code = "segments.write_arg(ids.inputs, [ids.low % 2 ** 64, ids.low // 2 ** 64])\nsegments.write_arg(ids.inputs + 2, [ids.high % 2 ** 64, ids.high // 2 ** 64])";
        let mut vm = vm_with_range_check!();
        vm.segments = segments![((1, 0), 233), ((1, 1), 351), ((1, 2), (2, 0))];
        //Initialize fp
        vm.run_context.fp = 3;
        //Create ids
        let ids_data = ids_data!["low", "high", "inputs"];
        let error = run_hint!(vm, ids_data, hint_code);
        assert_matches!(error, Err(HintError::Memory(_)));
    }

    #[test]
    #[cfg_attr(target_arch = "wasm32", wasm_bindgen_test)]
    fn compare_bytes_in_word_nondet_valid() {
        let hint_code =
            "memory[ap] = to_felt_or_relocatable(ids.n_bytes >= ids.KECCAK_FULL_RATE_IN_BYTES)";
        let mut vm = vm_with_range_check!();

        vm.segments.add();
        vm.segments = segments![((1, 0), 24)];

        run_context!(vm, 0, 1, 1);
        let ids_data = ids_data!["n_bytes"];
        assert_matches!(
            run_hint!(
                vm,
                ids_data,
                hint_code,
                exec_scopes_ref!(),
                &[(KECCAK_FULL_RATE_IN_BYTES_CAIRO_KECCAK, Felt252::from(136))]
                    .into_iter()
                    .map(|(k, v)| (k.to_string(), v))
                    .collect()
            ),
            Ok(())
        );
    }

    #[test]
    #[cfg_attr(target_arch = "wasm32", wasm_bindgen_test)]
    fn compare_keccak_full_rate_in_bytes_nondet_valid() {
        let hint_code =
            "memory[ap] = to_felt_or_relocatable(ids.n_bytes >= ids.KECCAK_FULL_RATE_IN_BYTES)";

        let mut vm = vm_with_range_check!();

        vm.segments.add();
        vm.segments = segments![((1, 0), 24)];

        run_context!(vm, 0, 1, 1);

        let ids_data = ids_data!["n_bytes"];
        assert_matches!(
            run_hint!(
                vm,
                ids_data,
                hint_code,
                exec_scopes_ref!(),
                &[(KECCAK_FULL_RATE_IN_BYTES_CAIRO_KECCAK, Felt252::from(136))]
                    .into_iter()
                    .map(|(k, v)| (k.to_string(), v))
                    .collect()
            ),
            Ok(())
        );
    }

    #[test]
    #[cfg_attr(target_arch = "wasm32", wasm_bindgen_test)]
    fn block_permutation_valid_test() {
        let hint_code =
            "memory[ap] = to_felt_or_relocatable(ids.n_bytes >= ids.KECCAK_FULL_RATE_IN_BYTES)";
        let mut vm = vm_with_range_check!();

        vm.segments.add();
        vm.segments = segments![((1, 0), 24)];

        run_context!(vm, 0, 1, 1);

        let ids_data = ids_data!["n_bytes"];
        assert_matches!(
            run_hint!(
                vm,
                ids_data,
                hint_code,
                exec_scopes_ref!(),
                &[(KECCAK_FULL_RATE_IN_BYTES_CAIRO_KECCAK, Felt252::from(136))]
                    .into_iter()
                    .map(|(k, v)| (k.to_string(), v))
                    .collect()
            ),
            Ok(())
        );
    }
}
