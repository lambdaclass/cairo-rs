use crate::stdlib::{boxed::Box, cmp, collections::HashMap, prelude::*};

use crate::types::errors::math_errors::MathError;
use crate::Felt252;
use crate::{
    hint_processor::{
        builtin_hint_processor::hint_utils::{
            get_integer_from_var_name, get_ptr_from_var_name, get_relocatable_from_var_name,
        },
        hint_processor_definition::HintReference,
    },
    math_utils::pow2_const_nz,
    serde::deserialize_program::ApTracking,
    types::{exec_scope::ExecutionScopes, relocatable::Relocatable},
    vm::{errors::hint_errors::HintError, vm_core::VirtualMachine},
};
use num_integer::Integer;
use num_traits::ToPrimitive;
use sha3::{Digest, Keccak256};

use super::hint_utils::insert_value_from_var_name;

const BYTES_IN_WORD: &str = "starkware.cairo.common.builtin_keccak.keccak.BYTES_IN_WORD";

/* Implements hint:
   %{
       from eth_hash.auto import keccak

       data, length = ids.data, ids.length

       if '__keccak_max_size' in globals():
           assert length <= __keccak_max_size, \
               f'unsafe_keccak() can only be used with length<={__keccak_max_size}. ' \
               f'Got: length={length}.'

       keccak_input = bytearray()
       for word_i, byte_i in enumerate(range(0, length, 16)):
           word = memory[data + word_i]
           n_bytes = min(16, length - byte_i)
           assert 0 <= word < 2 ** (8 * n_bytes)
           keccak_input += word.to_bytes(n_bytes, 'big')

       hashed = keccak(keccak_input)
       ids.high = int.from_bytes(hashed[:16], 'big')
       ids.low = int.from_bytes(hashed[16:32], 'big')
   %}
*/
pub fn unsafe_keccak(
    vm: &mut VirtualMachine,
    exec_scopes: &mut ExecutionScopes,
    ids_data: &HashMap<String, HintReference>,
    ap_tracking: &ApTracking,
) -> Result<(), HintError> {
    let length = get_integer_from_var_name("length", vm, ids_data, ap_tracking)?;

    if let Ok(keccak_max_size) = exec_scopes.get::<Felt252>("__keccak_max_size") {
        if length.as_ref() > &keccak_max_size {
            return Err(HintError::KeccakMaxSize(Box::new((
                length,
                keccak_max_size,
            ))));
        }
    }

    // `data` is an array, represented by a pointer to the first element.
    let data = get_ptr_from_var_name("data", vm, ids_data, ap_tracking)?;

    let high_addr = get_relocatable_from_var_name("high", vm, ids_data, ap_tracking)?;
    let low_addr = get_relocatable_from_var_name("low", vm, ids_data, ap_tracking)?;

    // transform to u64 to make ranges cleaner in the for loop below
    let u64_length = length
        .to_u64()
        .ok_or_else(|| HintError::InvalidKeccakInputLength(Box::new(length)))?;

    const ZEROES: [u8; 32] = [0u8; 32];
    let mut keccak_input = Vec::new();
    for (word_i, byte_i) in (0..u64_length).step_by(16).enumerate() {
        let word_addr = Relocatable {
            segment_index: data.segment_index,
            offset: data.offset + word_i,
        };

        let word = vm.get_integer(word_addr)?;
        let bytes = word.to_bytes_be();
        let n_bytes = cmp::min(16, u64_length - byte_i);
        let start = 32 - n_bytes as usize;

        // word <= 2^(8 * n_bytes) <=> `start` leading zeroes
        if !ZEROES.starts_with(&bytes[..start]) {
            return Err(HintError::InvalidWordSize(Box::new(word.into_owned())));
        }

        keccak_input.extend_from_slice(&bytes[start..]);
    }

    let mut hasher = Keccak256::new();
    hasher.update(keccak_input);

    let hashed = hasher.finalize();

    let mut high_bytes = [0; 16].to_vec();
    let mut low_bytes = [0; 16].to_vec();
    high_bytes.extend_from_slice(&hashed[0..16]);
    low_bytes.extend_from_slice(&hashed[16..32]);

    let high = Felt252::from_bytes_be_slice(&high_bytes);
    let low = Felt252::from_bytes_be_slice(&low_bytes);

    vm.insert_value(high_addr, high)?;
    vm.insert_value(low_addr, low)?;
    Ok(())
}

/*
Implements hint:

    %{
        from eth_hash.auto import keccak
        keccak_input = bytearray()
        n_elms = ids.keccak_state.end_ptr - ids.keccak_state.start_ptr
        for word in memory.get_range(ids.keccak_state.start_ptr, n_elms):
            keccak_input += word.to_bytes(16, 'big')
        hashed = keccak(keccak_input)
        ids.high = int.from_bytes(hashed[:16], 'big')
        ids.low = int.from_bytes(hashed[16:32], 'big')
    %}

 */
pub fn unsafe_keccak_finalize(
    vm: &mut VirtualMachine,
    ids_data: &HashMap<String, HintReference>,
    ap_tracking: &ApTracking,
) -> Result<(), HintError> {
    /* -----------------------------
    Just for reference (cairo code):
    struct KeccakState:
        member start_ptr : felt*
        member end_ptr : felt*
    end
    ----------------------------- */

    let keccak_state_ptr =
        get_relocatable_from_var_name("keccak_state", vm, ids_data, ap_tracking)?;

    // as `keccak_state` is a struct, the pointer to the struct is the same as the pointer to the first element.
    // this is why to get the pointer stored in the field `start_ptr` it is enough to pass the variable name as
    // `keccak_state`, which is the one that appears in the reference manager of the compiled JSON.
    let start_ptr = get_ptr_from_var_name("keccak_state", vm, ids_data, ap_tracking)?;

    // in the KeccakState struct, the field `end_ptr` is the second one, so this variable should be get from
    // the memory cell contiguous to the one where KeccakState is pointing to.
    let end_ptr = vm.get_relocatable(Relocatable {
        segment_index: keccak_state_ptr.segment_index,
        offset: keccak_state_ptr.offset + 1,
    })?;

    let n_elems = (end_ptr - start_ptr)?;

    let mut keccak_input = Vec::new();
    let range = vm.get_integer_range(start_ptr, n_elems)?;

    for word in range.into_iter() {
        keccak_input.extend_from_slice(&word.to_bytes_be()[16..]);
    }

    let mut hasher = Keccak256::new();
    hasher.update(keccak_input);

    let hashed = hasher.finalize();

    let mut high_bytes = [0; 16].to_vec();
    let mut low_bytes = [0; 16].to_vec();
    high_bytes.extend_from_slice(&hashed[0..16]);
    low_bytes.extend_from_slice(&hashed[16..32]);

    let high_addr = get_relocatable_from_var_name("high", vm, ids_data, ap_tracking)?;
    let low_addr = get_relocatable_from_var_name("low", vm, ids_data, ap_tracking)?;

    let high = Felt252::from_bytes_be_slice(&high_bytes);
    let low = Felt252::from_bytes_be_slice(&low_bytes);

    vm.insert_value(high_addr, high)?;
    vm.insert_value(low_addr, low)?;
    Ok(())
}

// Implements hints of type : ids.output{num}_low = ids.output{num} & ((1 << 128) - 1)
// ids.output{num}_high = ids.output{num} >> 128
pub fn split_output(
    vm: &mut VirtualMachine,
    ids_data: &HashMap<String, HintReference>,
    ap_tracking: &ApTracking,
    num: u32,
) -> Result<(), HintError> {
    let output_name = format!("output{}", num);
    let output = get_integer_from_var_name(&output_name, vm, ids_data, ap_tracking)?;
    let (high, low) = output.div_rem(pow2_const_nz(128));
    insert_value_from_var_name(
        &format!("output{}_high", num),
        high,
        vm,
        ids_data,
        ap_tracking,
    )?;
    insert_value_from_var_name(
        &format!("output{}_low", num),
        low,
        vm,
        ids_data,
        ap_tracking,
    )
}

// Implements hints of type: ids.high{input_key}, ids.low{input_key} = divmod(memory[ids.inputs + {input_key}], 256 ** {exponent})
pub fn split_input(
    vm: &mut VirtualMachine,
    ids_data: &HashMap<String, HintReference>,
    ap_tracking: &ApTracking,
    input_key: usize,
    exponent: u32,
) -> Result<(), HintError> {
    let inputs_ptr = get_ptr_from_var_name("inputs", vm, ids_data, ap_tracking)?;
    let binding = vm.get_integer((inputs_ptr + input_key)?)?;
    let split = pow2_const_nz(8 * exponent);
    let (high, low) = binding.div_rem(split);
    insert_value_from_var_name(
        &format!("high{}", input_key),
        high,
        vm,
        ids_data,
        ap_tracking,
    )?;
    insert_value_from_var_name(&format!("low{}", input_key), low, vm, ids_data, ap_tracking)
}

// Implements hint: ids.n_words_to_copy, ids.n_bytes_left = divmod(ids.n_bytes, ids.BYTES_IN_WORD)
pub fn split_n_bytes(
    vm: &mut VirtualMachine,
    ids_data: &HashMap<String, HintReference>,
    ap_tracking: &ApTracking,
    constants: &HashMap<String, Felt252>,
) -> Result<(), HintError> {
    let n_bytes =
        get_integer_from_var_name("n_bytes", vm, ids_data, ap_tracking).and_then(|x| {
            x.to_u64()
                .ok_or_else(|| HintError::Math(MathError::Felt252ToU64Conversion(Box::new(x))))
        })?;
    let bytes_in_word = constants
        .get(BYTES_IN_WORD)
        .and_then(|x| x.to_u64())
        .ok_or_else(|| HintError::MissingConstant(BYTES_IN_WORD.to_string().into_boxed_str()))?;
    let (high, low) = n_bytes.div_mod_floor(&bytes_in_word);
    insert_value_from_var_name(
        "n_words_to_copy",
        Felt252::from(high),
        vm,
        ids_data,
        ap_tracking,
    )?;
    insert_value_from_var_name(
        "n_bytes_left",
        Felt252::from(low),
        vm,
        ids_data,
        ap_tracking,
    )
}

// Implements hint:
// tmp, ids.output1_low = divmod(ids.output1, 256 ** 7)
// ids.output1_high, ids.output1_mid = divmod(tmp, 2 ** 128)
pub fn split_output_mid_low_high(
    vm: &mut VirtualMachine,
    ids_data: &HashMap<String, HintReference>,
    ap_tracking: &ApTracking,
) -> Result<(), HintError> {
    let binding = get_integer_from_var_name("output1", vm, ids_data, ap_tracking)?;
    let output1 = binding.as_ref();
    let (tmp, output1_low) = output1.div_rem(pow2_const_nz(8 * 7));
    let (output1_high, output1_mid) = tmp.div_rem(pow2_const_nz(128));
    insert_value_from_var_name("output1_high", output1_high, vm, ids_data, ap_tracking)?;
    insert_value_from_var_name("output1_mid", output1_mid, vm, ids_data, ap_tracking)?;
    insert_value_from_var_name("output1_low", output1_low, vm, ids_data, ap_tracking)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::any_box;
    use crate::{
        hint_processor::{
            builtin_hint_processor::{
                builtin_hint_processor_definition::{BuiltinHintProcessor, HintProcessorData},
                hint_code,
                keccak_utils::HashMap,
            },
            hint_processor_definition::{HintProcessorLogic, HintReference},
        },
        utils::test_utils::*,
    };
    use assert_matches::assert_matches;

    #[test]
    fn split_output_0() {
        let mut vm = vm!();
        vm.segments = segments![((1, 0), 24)];
        vm.set_fp(3);
        let ids_data = ids_data!["output0", "output0_high", "output0_low"];
        assert_matches!(run_hint!(vm, ids_data, hint_code::SPLIT_OUTPUT_0), Ok(()));
        check_memory!(vm.segments.memory, ((1, 1), 0), ((1, 2), 24));
    }

    #[test]
    fn split_output_1() {
        let mut vm = vm!();
        vm.segments = segments![((1, 0), 24)];
        vm.set_fp(3);
        let ids_data = ids_data!["output1", "output1_high", "output1_low"];
        assert_matches!(run_hint!(vm, ids_data, hint_code::SPLIT_OUTPUT_1), Ok(()));
        check_memory!(vm.segments.memory, ((1, 1), 0), ((1, 2), 24));
    }

    #[test]
    fn split_input_3() {
        let mut vm = vm!();
        vm.segments = segments![((1, 2), (2, 0)), ((2, 3), 300)];
        vm.set_fp(3);
        let ids_data = ids_data!["high3", "low3", "inputs"];
        assert_matches!(run_hint!(vm, ids_data, hint_code::SPLIT_INPUT_3), Ok(()));
        check_memory!(vm.segments.memory, ((1, 0), 1), ((1, 1), 44));
    }

    #[test]
    fn split_input_6() {
        let mut vm = vm!();
        vm.segments = segments![((1, 2), (2, 0)), ((2, 6), 66036)];
        vm.set_fp(3);
        let ids_data = ids_data!["high6", "low6", "inputs"];
        assert_matches!(run_hint!(vm, ids_data, hint_code::SPLIT_INPUT_6), Ok(()));
        check_memory!(vm.segments.memory, ((1, 0), 1), ((1, 1), 500));
    }

    #[test]
    fn split_input_15() {
        let mut vm = vm!();
        vm.segments = segments![((1, 2), (2, 0)), ((2, 15), 15150315)];
        vm.set_fp(3);
        let ids_data = ids_data!["high15", "low15", "inputs"];
        assert_matches!(run_hint!(vm, ids_data, hint_code::SPLIT_INPUT_15), Ok(()));
        check_memory!(vm.segments.memory, ((1, 0), 0), ((1, 1), 15150315));
    }

    #[test]
    fn split_n_bytes() {
        let mut vm = vm!();
        vm.segments = segments![((1, 2), 17)];
        vm.set_fp(3);
        let ids_data = ids_data!["n_words_to_copy", "n_bytes_left", "n_bytes"];
        assert_matches!(
            run_hint!(
                vm,
                ids_data,
                hint_code::SPLIT_N_BYTES,
                exec_scopes_ref!(),
                &HashMap::from([(String::from(BYTES_IN_WORD), Felt252::from(8))])
            ),
            Ok(())
        );
        check_memory!(vm.segments.memory, ((1, 0), 2), ((1, 1), 1));
    }

    #[test]
    fn split_output_mid_low_high() {
        let mut vm = vm!();
        vm.segments = segments![((1, 0), 72057594037927938)];
        vm.set_fp(4);
        let ids_data = ids_data!["output1", "output1_low", "output1_mid", "output1_high"];
        assert_matches!(
            run_hint!(
                vm,
                ids_data,
                hint_code::SPLIT_OUTPUT_MID_LOW_HIGH,
                exec_scopes_ref!(),
                &HashMap::from([(String::from(BYTES_IN_WORD), Felt252::from(8))])
            ),
            Ok(())
        );
        check_memory!(vm.segments.memory, ((1, 1), 2), ((1, 2), 1), ((1, 3), 0));
    }
}
