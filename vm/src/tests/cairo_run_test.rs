use crate::{tests::*, types::layout_name::LayoutName};
#[cfg(feature = "mod_builtin")]
use crate::{
    utils::test_utils::Program,
    vm::{
        runners::{builtin_runner::BuiltinRunner, cairo_runner::CairoRunner},
        security::verify_secure_runner,
    },
};

use num_traits::Zero;

#[test]
#[cfg_attr(target_arch = "wasm32", wasm_bindgen_test)]
fn fibonacci() {
    let program_data = include_bytes!("../../../cairo_programs/fibonacci.json");
    run_program_small(program_data.as_slice());
}

#[test]
#[cfg_attr(target_arch = "wasm32", wasm_bindgen_test)]
fn array_sum() {
    let program_data = include_bytes!("../../../cairo_programs/array_sum.json");
    run_program_simple(program_data.as_slice());
}

#[test]
#[cfg_attr(target_arch = "wasm32", wasm_bindgen_test)]
fn big_struct() {
    let program_data = include_bytes!("../../../cairo_programs/big_struct.json");
    run_program_simple(program_data.as_slice());
}

#[test]
#[cfg_attr(target_arch = "wasm32", wasm_bindgen_test)]
fn call_function_assign_param_by_name() {
    let program_data =
        include_bytes!("../../../cairo_programs/call_function_assign_param_by_name.json");
    run_program_simple(program_data.as_slice());
}

#[test]
#[cfg_attr(target_arch = "wasm32", wasm_bindgen_test)]
fn function_return() {
    let program_data = include_bytes!("../../../cairo_programs/function_return.json");
    run_program_simple(program_data.as_slice());
}

#[test]
#[cfg_attr(target_arch = "wasm32", wasm_bindgen_test)]
fn function_return_if_print() {
    let program_data = include_bytes!("../../../cairo_programs/function_return_if_print.json");
    run_program_simple(program_data.as_slice());
}

#[test]
#[cfg_attr(target_arch = "wasm32", wasm_bindgen_test)]
fn function_return_to_variable() {
    let program_data = include_bytes!("../../../cairo_programs/function_return_to_variable.json");
    run_program_simple(program_data.as_slice());
}

#[test]
#[cfg_attr(target_arch = "wasm32", wasm_bindgen_test)]
fn if_and_prime() {
    let program_data = include_bytes!("../../../cairo_programs/if_and_prime.json");
    run_program_simple(program_data.as_slice());
}

#[test]
#[cfg_attr(target_arch = "wasm32", wasm_bindgen_test)]
fn if_in_function() {
    let program_data = include_bytes!("../../../cairo_programs/if_in_function.json");
    run_program_simple(program_data.as_slice());
}

#[test]
#[cfg_attr(target_arch = "wasm32", wasm_bindgen_test)]
fn if_list() {
    let program_data = include_bytes!("../../../cairo_programs/if_list.json");
    run_program_simple(program_data.as_slice());
}

#[test]
#[cfg_attr(target_arch = "wasm32", wasm_bindgen_test)]
fn jmp() {
    let program_data = include_bytes!("../../../cairo_programs/jmp.json");
    run_program_simple(program_data.as_slice());
}

#[test]
#[cfg_attr(target_arch = "wasm32", wasm_bindgen_test)]
fn jmp_if_condition() {
    let program_data = include_bytes!("../../../cairo_programs/jmp_if_condition.json");
    run_program_simple(program_data.as_slice());
}

#[test]
#[cfg_attr(target_arch = "wasm32", wasm_bindgen_test)]
fn pointers() {
    let program_data = include_bytes!("../../../cairo_programs/pointers.json");
    run_program_simple(program_data.as_slice());
}

#[test]
#[cfg_attr(target_arch = "wasm32", wasm_bindgen_test)]
fn print() {
    let program_data = include_bytes!("../../../cairo_programs/print.json");
    run_program_simple(program_data.as_slice());
}

#[test]
#[cfg_attr(target_arch = "wasm32", wasm_bindgen_test)]
fn program_return() {
    let program_data = include_bytes!("../../../cairo_programs/return.json");
    run_program_simple(program_data.as_slice());
}

#[test]
#[cfg_attr(target_arch = "wasm32", wasm_bindgen_test)]
fn reversed_register_instructions() {
    let program_data =
        include_bytes!("../../../cairo_programs/reversed_register_instructions.json");
    run_program_simple(program_data.as_slice());
}

#[test]
#[cfg_attr(target_arch = "wasm32", wasm_bindgen_test)]
fn simple_print() {
    let program_data = include_bytes!("../../../cairo_programs/simple_print.json");
    run_program_simple(program_data.as_slice());
}

#[test]
#[cfg_attr(target_arch = "wasm32", wasm_bindgen_test)]
fn test_addition_if() {
    let program_data = include_bytes!("../../../cairo_programs/test_addition_if.json");
    run_program_simple(program_data.as_slice());
}

#[test]
#[cfg_attr(target_arch = "wasm32", wasm_bindgen_test)]
fn test_reverse_if() {
    let program_data = include_bytes!("../../../cairo_programs/test_reverse_if.json");
    run_program_simple(program_data.as_slice());
}

#[test]
#[cfg_attr(target_arch = "wasm32", wasm_bindgen_test)]
fn test_subtraction_if() {
    let program_data = include_bytes!("../../../cairo_programs/test_subtraction_if.json");
    run_program_simple(program_data.as_slice());
}

#[test]
#[cfg_attr(target_arch = "wasm32", wasm_bindgen_test)]
fn use_imported_module() {
    let program_data = include_bytes!("../../../cairo_programs/use_imported_module.json");
    run_program_simple(program_data.as_slice());
}

#[test]
#[cfg_attr(target_arch = "wasm32", wasm_bindgen_test)]
fn bitwise_output() {
    let program_data = include_bytes!("../../../cairo_programs/bitwise_output.json");
    run_program_simple(program_data.as_slice());
}

#[test]
#[cfg_attr(target_arch = "wasm32", wasm_bindgen_test)]
fn bitwise_recursion() {
    let program_data = include_bytes!("../../../cairo_programs/bitwise_recursion.json");
    run_program_simple(program_data.as_slice());
}

#[test]
#[cfg_attr(target_arch = "wasm32", wasm_bindgen_test)]
fn integration() {
    let program_data = include_bytes!("../../../cairo_programs/integration.json");
    run_program_simple(program_data.as_slice());
}

#[test]
#[cfg_attr(target_arch = "wasm32", wasm_bindgen_test)]
fn integration_with_alloc_locals() {
    let program_data = include_bytes!("../../../cairo_programs/integration_with_alloc_locals.json");
    run_program_simple(program_data.as_slice());
}

#[test]
#[cfg_attr(target_arch = "wasm32", wasm_bindgen_test)]
fn compare_arrays() {
    let program_data = include_bytes!("../../../cairo_programs/compare_arrays.json");
    run_program_simple(program_data.as_slice());
}

#[test]
#[cfg_attr(target_arch = "wasm32", wasm_bindgen_test)]
fn compare_greater_array() {
    let program_data = include_bytes!("../../../cairo_programs/compare_greater_array.json");
    run_program_simple(program_data.as_slice());
}

#[test]
#[cfg_attr(target_arch = "wasm32", wasm_bindgen_test)]
fn compare_lesser_array() {
    let program_data = include_bytes!("../../../cairo_programs/compare_lesser_array.json");
    run_program_simple(program_data.as_slice());
}

#[test]
#[cfg_attr(target_arch = "wasm32", wasm_bindgen_test)]
fn assert_le_felt_hint() {
    let program_data = include_bytes!("../../../cairo_programs/assert_le_felt_hint.json");
    run_program_simple(program_data.as_slice());
}

#[test]
#[cfg_attr(target_arch = "wasm32", wasm_bindgen_test)]
fn assert_250_bit_element_array() {
    let program_data = include_bytes!("../../../cairo_programs/assert_250_bit_element_array.json");
    run_program_simple(program_data.as_slice());
}

#[test]
#[cfg_attr(target_arch = "wasm32", wasm_bindgen_test)]
fn abs_value_array() {
    let program_data = include_bytes!("../../../cairo_programs/abs_value_array.json");
    run_program_simple(program_data.as_slice());
}

#[test]
#[cfg_attr(target_arch = "wasm32", wasm_bindgen_test)]
fn compare_different_arrays() {
    let program_data = include_bytes!("../../../cairo_programs/compare_different_arrays.json");
    run_program_simple(program_data.as_slice());
}

#[test]
#[cfg_attr(target_arch = "wasm32", wasm_bindgen_test)]
fn assert_nn() {
    let program_data = include_bytes!("../../../cairo_programs/assert_nn.json");
    run_program_simple(program_data.as_slice());
}

#[test]
#[cfg_attr(target_arch = "wasm32", wasm_bindgen_test)]
fn sqrt() {
    let program_data = include_bytes!("../../../cairo_programs/sqrt.json");
    run_program_simple(program_data.as_slice());
}

#[test]
#[cfg_attr(target_arch = "wasm32", wasm_bindgen_test)]
fn assert_not_zero() {
    let program_data = include_bytes!("../../../cairo_programs/assert_not_zero.json");
    run_program_simple(program_data.as_slice());
}

#[test]
#[cfg_attr(target_arch = "wasm32", wasm_bindgen_test)]
fn split_int() {
    let program_data = include_bytes!("../../../cairo_programs/split_int.json");
    run_program_simple(program_data.as_slice());
}

#[test]
#[cfg_attr(target_arch = "wasm32", wasm_bindgen_test)]
fn split_int_big() {
    let program_data = include_bytes!("../../../cairo_programs/split_int_big.json");
    run_program_simple(program_data.as_slice());
}

#[test]
#[cfg_attr(target_arch = "wasm32", wasm_bindgen_test)]
fn split_felt() {
    let program_data = include_bytes!("../../../cairo_programs/split_felt.json");
    run_program_simple(program_data.as_slice());
}

#[test]
#[cfg_attr(target_arch = "wasm32", wasm_bindgen_test)]
fn math_cmp() {
    let program_data = include_bytes!("../../../cairo_programs/math_cmp.json");
    run_program_simple(program_data.as_slice());
}

#[test]
#[cfg_attr(target_arch = "wasm32", wasm_bindgen_test)]
fn unsigned_div_rem() {
    let program_data = include_bytes!("../../../cairo_programs/unsigned_div_rem.json");
    run_program_simple(program_data.as_slice());
}

#[test]
#[cfg_attr(target_arch = "wasm32", wasm_bindgen_test)]
fn signed_div_rem() {
    let program_data = include_bytes!("../../../cairo_programs/signed_div_rem.json");
    run_program_simple(program_data.as_slice());
}

#[test]
#[cfg_attr(target_arch = "wasm32", wasm_bindgen_test)]
fn assert_lt_felt() {
    let program_data = include_bytes!("../../../cairo_programs/assert_lt_felt.json");
    run_program_simple(program_data.as_slice());
}

#[test]
#[cfg_attr(target_arch = "wasm32", wasm_bindgen_test)]
fn memcpy_test() {
    let program_data = include_bytes!("../../../cairo_programs/memcpy_test.json");
    run_program_simple(program_data.as_slice());
}

#[test]
#[cfg_attr(target_arch = "wasm32", wasm_bindgen_test)]
fn memset() {
    let program_data = include_bytes!("../../../cairo_programs/memset.json");
    run_program_simple(program_data.as_slice());
}

#[test]
#[cfg_attr(target_arch = "wasm32", wasm_bindgen_test)]
fn pow() {
    let program_data = include_bytes!("../../../cairo_programs/pow.json");
    run_program_simple(program_data.as_slice());
}

#[test]
#[cfg_attr(target_arch = "wasm32", wasm_bindgen_test)]
fn dict() {
    let program_data = include_bytes!("../../../cairo_programs/dict.json");
    run_program_simple(program_data.as_slice());
}

#[test]
#[cfg_attr(target_arch = "wasm32", wasm_bindgen_test)]
fn dict_update() {
    let program_data = include_bytes!("../../../cairo_programs/dict_update.json");
    run_program_simple(program_data.as_slice());
}

#[test]
#[cfg_attr(target_arch = "wasm32", wasm_bindgen_test)]
fn uint256() {
    let program_data = include_bytes!("../../../cairo_programs/uint256.json");
    run_program_simple(program_data.as_slice());
}

#[test]
#[cfg_attr(target_arch = "wasm32", wasm_bindgen_test)]
fn find_element() {
    let program_data = include_bytes!("../../../cairo_programs/find_element.json");
    run_program_simple(program_data.as_slice());
}

#[test]
#[cfg_attr(target_arch = "wasm32", wasm_bindgen_test)]
fn search_sorted_lower() {
    let program_data = include_bytes!("../../../cairo_programs/search_sorted_lower.json");
    run_program_simple(program_data.as_slice());
}

#[test]
#[cfg_attr(target_arch = "wasm32", wasm_bindgen_test)]
fn usort() {
    let program_data = include_bytes!("../../../cairo_programs/usort.json");
    run_program_simple(program_data.as_slice());
}

#[test]
#[cfg_attr(target_arch = "wasm32", wasm_bindgen_test)]
fn squash_dict() {
    let program_data = include_bytes!("../../../cairo_programs/squash_dict.json");
    run_program_simple(program_data.as_slice());
}

#[test]
#[cfg_attr(target_arch = "wasm32", wasm_bindgen_test)]
fn dict_squash() {
    let program_data = include_bytes!("../../../cairo_programs/dict_squash.json");
    run_program_simple(program_data.as_slice());
}

#[test]
#[cfg_attr(target_arch = "wasm32", wasm_bindgen_test)]
fn set_add() {
    let program_data = include_bytes!("../../../cairo_programs/set_add.json");
    run_program_simple(program_data.as_slice());
}

#[test]
#[cfg_attr(target_arch = "wasm32", wasm_bindgen_test)]
fn secp() {
    let program_data = include_bytes!("../../../cairo_programs/secp.json");
    run_program_simple(program_data.as_slice());
}

#[test]
#[cfg_attr(target_arch = "wasm32", wasm_bindgen_test)]
fn secp_ec() {
    let program_data = include_bytes!("../../../cairo_programs/secp_ec.json");
    run_program_simple(program_data.as_slice());
}

#[test]
#[cfg_attr(target_arch = "wasm32", wasm_bindgen_test)]
fn signature() {
    let program_data = include_bytes!("../../../cairo_programs/signature.json");
    run_program_simple(program_data.as_slice());
}

#[test]
#[cfg_attr(target_arch = "wasm32", wasm_bindgen_test)]
fn blake2s_hello_world_hash() {
    let program_data = include_bytes!("../../../cairo_programs/blake2s_hello_world_hash.json");
    run_program_simple(program_data.as_slice());
}

#[test]
#[cfg_attr(target_arch = "wasm32", wasm_bindgen_test)]
fn example_blake2s() {
    let program_data = include_bytes!("../../../cairo_programs/example_blake2s.json");
    run_program_simple(program_data.as_slice());
}

#[test]
#[cfg_attr(target_arch = "wasm32", wasm_bindgen_test)]
fn finalize_blake2s() {
    let program_data = include_bytes!("../../../cairo_programs/finalize_blake2s.json");
    run_program_simple(program_data.as_slice());
}

#[test]
#[cfg_attr(target_arch = "wasm32", wasm_bindgen_test)]
fn finalize_blake2s_v2_hint() {
    let program_data = include_bytes!("../../../cairo_programs/finalize_blake2s_v2_hint.json");
    run_program_simple(program_data.as_slice());
}

#[test]
#[cfg_attr(target_arch = "wasm32", wasm_bindgen_test)]
fn unsafe_keccak() {
    let program_data = include_bytes!("../../../cairo_programs/unsafe_keccak.json");
    run_program_simple(program_data.as_slice());
}

#[test]
#[cfg_attr(target_arch = "wasm32", wasm_bindgen_test)]
fn blake2s_felts() {
    let program_data = include_bytes!("../../../cairo_programs/blake2s_felts.json");
    run_program_simple(program_data.as_slice());
}

#[test]
#[cfg_attr(target_arch = "wasm32", wasm_bindgen_test)]
fn unsafe_keccak_finalize() {
    let program_data = include_bytes!("../../../cairo_programs/unsafe_keccak_finalize.json");
    run_program_simple(program_data.as_slice());
}

#[test]
#[cfg_attr(target_arch = "wasm32", wasm_bindgen_test)]
fn keccak_add_uint256() {
    let program_data = include_bytes!("../../../cairo_programs/keccak_add_uint256.json");
    run_program_simple(program_data.as_slice());
}

#[test]
#[cfg_attr(target_arch = "wasm32", wasm_bindgen_test)]
fn keccak() {
    let program_data = include_bytes!("../../../cairo_programs/keccak.json");
    run_program_simple(program_data.as_slice());
}

#[test]
#[cfg_attr(target_arch = "wasm32", wasm_bindgen_test)]
fn keccak_copy_inputs() {
    let program_data = include_bytes!("../../../cairo_programs/keccak_copy_inputs.json");
    run_program_simple(program_data.as_slice());
}

#[test]
#[cfg_attr(target_arch = "wasm32", wasm_bindgen_test)]
fn cairo_finalize_keccak_v1() {
    let program_data = include_bytes!("../../../cairo_programs/cairo_finalize_keccak.json");
    run_program_simple(program_data.as_slice());
}

#[test]
#[cfg_attr(target_arch = "wasm32", wasm_bindgen_test)]
fn cairo_finalize_keccak_v2() {
    let program_data =
        include_bytes!("../../../cairo_programs/cairo_finalize_keccak_block_size_1000.json");
    run_program_simple(program_data.as_slice());
}

#[test]
#[cfg_attr(target_arch = "wasm32", wasm_bindgen_test)]
fn operations_with_data_structures() {
    let program_data =
        include_bytes!("../../../cairo_programs/operations_with_data_structures.json");
    run_program_simple(program_data.as_slice());
}

#[test]
#[cfg_attr(target_arch = "wasm32", wasm_bindgen_test)]
fn packed_sha256() {
    let program_data = include_bytes!("../../../cairo_programs/packed_sha256_test.json");
    run_program_simple(program_data.as_slice());
}

#[test]
#[cfg_attr(target_arch = "wasm32", wasm_bindgen_test)]
fn math_cmp_and_pow_integration_tests() {
    let program_data =
        include_bytes!("../../../cairo_programs/math_cmp_and_pow_integration_tests.json");
    run_program_simple(program_data.as_slice());
}

#[test]
#[cfg_attr(target_arch = "wasm32", wasm_bindgen_test)]
fn uint256_integration_tests() {
    let program_data = include_bytes!("../../../cairo_programs/uint256_integration_tests.json");
    run_program_simple(program_data.as_slice());
}

#[test]
#[cfg_attr(target_arch = "wasm32", wasm_bindgen_test)]
fn set_integration_tests() {
    let program_data = include_bytes!("../../../cairo_programs/set_integration_tests.json");
    run_program_simple(program_data.as_slice());
}

#[test]
#[cfg_attr(target_arch = "wasm32", wasm_bindgen_test)]
fn memory_integration_tests() {
    let program_data = include_bytes!("../../../cairo_programs/memory_integration_tests.json");
    run_program_simple(program_data.as_slice());
}

#[test]
#[cfg_attr(target_arch = "wasm32", wasm_bindgen_test)]
fn dict_integration_tests() {
    let program_data = include_bytes!("../../../cairo_programs/dict_integration_tests.json");
    run_program_simple(program_data.as_slice());
}

#[test]
#[cfg_attr(target_arch = "wasm32", wasm_bindgen_test)]
fn secp_integration_tests() {
    let program_data = include_bytes!("../../../cairo_programs/secp_integration_tests.json");
    run_program_simple(program_data.as_slice());
}

#[test]
#[cfg_attr(target_arch = "wasm32", wasm_bindgen_test)]
fn keccak_integration_tests() {
    let program_data = include_bytes!("../../../cairo_programs/keccak_integration_tests.json");
    run_program_simple(program_data.as_slice());
}

#[test]
#[cfg_attr(target_arch = "wasm32", wasm_bindgen_test)]
fn blake2s_integration_tests() {
    let program_data = include_bytes!("../../../cairo_programs/blake2s_integration_tests.json");
    run_program_simple(program_data.as_slice());
}

#[test]
#[cfg_attr(target_arch = "wasm32", wasm_bindgen_test)]
fn relocate_segments() {
    let program_data = include_bytes!("../../../cairo_programs/relocate_segments.json");
    run_program_simple(program_data.as_slice());
}

#[test]
#[cfg_attr(target_arch = "wasm32", wasm_bindgen_test)]
fn relocate_segments_with_offset() {
    let program_data = include_bytes!("../../../cairo_programs/relocate_segments_with_offset.json");
    run_program_simple(program_data.as_slice());
}

#[test]
#[cfg_attr(target_arch = "wasm32", wasm_bindgen_test)]
fn dict_store_cast_ptr() {
    let program_data = include_bytes!("../../../cairo_programs/dict_store_cast_ptr.json");
    run_program_simple(program_data.as_slice());
}

#[test]
#[cfg_attr(target_arch = "wasm32", wasm_bindgen_test)]
fn common_signature() {
    let program_data = include_bytes!("../../../cairo_programs/common_signature.json");
    run_program_simple(program_data.as_slice());
}

#[test]
#[cfg_attr(target_arch = "wasm32", wasm_bindgen_test)]
fn bad_usort() {
    let program_data = include_bytes!("../../../cairo_programs/bad_programs/bad_usort.json");
    let error_msg = "unexpected verify multiplicity fail: positions length != 0";
    run_program_with_error(program_data.as_slice(), error_msg);
}

#[test]
#[cfg_attr(target_arch = "wasm32", wasm_bindgen_test)]
fn bad_dict_new() {
    let program_data = include_bytes!("../../../cairo_programs/bad_programs/bad_dict_new.json");
    let error_msg = "Dict Error: Tried to create a dict without an initial dict";
    run_program_with_error(program_data.as_slice(), error_msg);
}

#[test]
#[cfg_attr(target_arch = "wasm32", wasm_bindgen_test)]
fn bad_dict_update() {
    let program_data = include_bytes!("../../../cairo_programs/bad_programs/bad_dict_update.json");
    let error_msg =
        "Dict Error: Got the wrong value for dict_update, expected value: 3, got: 5 for key: 2";
    run_program_with_error(program_data.as_slice(), error_msg);
}

#[test]
#[cfg_attr(target_arch = "wasm32", wasm_bindgen_test)]
fn error_msg_attr() {
    let program_data = include_bytes!("../../../cairo_programs/bad_programs/error_msg_attr.json");
    let error_msg = "SafeUint256: addition overflow";
    run_program_with_error(program_data.as_slice(), error_msg);
}

#[test]
#[cfg_attr(target_arch = "wasm32", wasm_bindgen_test)]
fn error_msg_attr_tempvar() {
    let program_data =
        include_bytes!("../../../cairo_programs/bad_programs/error_msg_attr_tempvar.json");

    #[cfg(feature = "std")]
    let error_msg = "Error message: SafeUint256: addition overflow: {x} (Cannot evaluate ap-based or complex references: ['x'])\ncairo_programs/bad_programs/error_msg_attr_tempvar.cairo:4:9: Error at pc=0:2:\nAn ASSERT_EQ instruction failed: 3 != 2.\n        assert x = 2;\n        ^***********^\n";
    #[cfg(not(feature = "std"))]
    let error_msg = "Error message: SafeUint256: addition overflow: {x} (Cannot evaluate ap-based or complex references: ['x'])\ncairo_programs/bad_programs/error_msg_attr_tempvar.cairo:4:9: Error at pc=0:2:\nAn ASSERT_EQ instruction failed: 3 != 2.\n";
    run_program_with_error(program_data.as_slice(), error_msg);
}

#[test]
#[cfg_attr(target_arch = "wasm32", wasm_bindgen_test)]
fn error_msg_attr_struct() {
    let program_data =
        include_bytes!("../../../cairo_programs/bad_programs/error_msg_attr_struct.json");
    let error_msg = "Error message: Cats cannot have more than nine lives: {cat} (Cannot evaluate ap-based or complex references: ['cat'])";
    run_program_with_error(program_data.as_slice(), error_msg);
}
#[test]
#[cfg_attr(target_arch = "wasm32", wasm_bindgen_test)]
fn error_msg_attr_div_by_zero() {
    let program_data = include_bytes!("../../../cairo_programs/bad_programs/div_by_zero.json");
    let error_msg = "Got an exception while executing a hint: Attempted to divide by zero";
    run_program_with_error(program_data.as_slice(), error_msg);
}

#[test]
#[cfg_attr(target_arch = "wasm32", wasm_bindgen_test)]
fn split_felt_bad_constants() {
    let program_data =
        include_bytes!("../../../cairo_programs/bad_programs/split_felt_bad_constants.json");
    let error_msg = "assert PRIME - 1 == ids.MAX_HIGH * 2**128 + ids.MAX_LOW";
    run_program_with_error(program_data.as_slice(), error_msg);
}

#[test]
#[cfg_attr(target_arch = "wasm32", wasm_bindgen_test)]
fn poseidon_builtin() {
    let program_data = include_bytes!("../../../cairo_programs/poseidon_builtin.json");
    run_program_simple(program_data.as_slice());
}

#[test]
#[cfg_attr(target_arch = "wasm32", wasm_bindgen_test)]
fn ec_op() {
    let program_data = include_bytes!("../../../cairo_programs/ec_op.json");
    run_program_simple(program_data.as_slice());
}

#[test]
#[cfg_attr(target_arch = "wasm32", wasm_bindgen_test)]
fn poseidon_hash() {
    let program_data = include_bytes!("../../../cairo_programs/poseidon_hash.json");
    run_program_simple(program_data.as_slice());
}

#[test]
#[cfg_attr(target_arch = "wasm32", wasm_bindgen_test)]
fn cairo_chained_run_ec_op() {
    let program_data = include_bytes!("../../../cairo_programs/chained_ec_op.json");
    run_program_simple(program_data.as_slice());
}

#[test]
#[cfg_attr(target_arch = "wasm32", wasm_bindgen_test)]
fn keccak_builtin() {
    let program_data = include_bytes!("../../../cairo_programs/keccak_builtin.json");
    run_program_simple(program_data.as_slice());
}

#[test]
#[cfg_attr(target_arch = "wasm32", wasm_bindgen_test)]
fn keccak_uint256() {
    let program_data = include_bytes!("../../../cairo_programs/keccak_uint256.json");
    run_program_simple(program_data.as_slice());
}

#[test]
#[cfg_attr(target_arch = "wasm32", wasm_bindgen_test)]
fn recover_y() {
    let program_data = include_bytes!("../../../cairo_programs/recover_y.json");
    run_program_simple(program_data.as_slice());
}

#[test]
#[cfg_attr(target_arch = "wasm32", wasm_bindgen_test)]
fn math_integration() {
    let program_data = include_bytes!("../../../cairo_programs/math_integration_tests.json");
    run_program_simple(program_data.as_slice());
}

#[test]
#[cfg_attr(target_arch = "wasm32", wasm_bindgen_test)]
fn is_quad_residue_test() {
    let program_data = include_bytes!("../../../cairo_programs/is_quad_residue_test.json");
    run_program_simple(program_data.as_slice());
}

#[test]
#[cfg_attr(target_arch = "wasm32", wasm_bindgen_test)]
fn mul_s_inv() {
    let program_data = include_bytes!("../../../cairo_programs/mul_s_inv.json");
    run_program_simple(program_data.as_slice());
}

#[test]
#[cfg_attr(target_arch = "wasm32", wasm_bindgen_test)]
fn keccak_alternative_hint() {
    let program_data = include_bytes!("../../../cairo_programs/keccak_alternative_hint.json");
    run_program_simple(program_data.as_slice());
}

#[test]
#[cfg_attr(target_arch = "wasm32", wasm_bindgen_test)]
fn uint384() {
    let program_data = include_bytes!("../../../cairo_programs/uint384_test.json");
    run_program_simple(program_data.as_slice());
}

#[test]
#[cfg_attr(target_arch = "wasm32", wasm_bindgen_test)]
fn uint384_extension() {
    let program_data = include_bytes!("../../../cairo_programs/uint384_extension_test.json");
    run_program_simple(program_data.as_slice());
}

#[test]
#[cfg_attr(target_arch = "wasm32", wasm_bindgen_test)]
fn field_arithmetic() {
    let program_data = include_bytes!("../../../cairo_programs/field_arithmetic.json");
    run_program_simple(program_data.as_slice());
}

#[test]
#[cfg_attr(target_arch = "wasm32", wasm_bindgen_test)]
fn ed25519_field() {
    let program_data = include_bytes!("../../../cairo_programs/ed25519_field.json");
    run_program_simple(program_data.as_slice());
}

#[test]
#[cfg_attr(target_arch = "wasm32", wasm_bindgen_test)]
fn ed25519_ec() {
    let program_data = include_bytes!("../../../cairo_programs/ed25519_ec.json");
    run_program_simple(program_data.as_slice());
}

#[test]
#[cfg_attr(target_arch = "wasm32", wasm_bindgen_test)]
fn efficient_secp256r1_ec() {
    let program_data = include_bytes!("../../../cairo_programs/efficient_secp256r1_ec.json");
    run_program_simple(program_data.as_slice());
}

#[test]
#[cfg_attr(target_arch = "wasm32", wasm_bindgen_test)]
fn cairo_run_secp256r1_slope() {
    let program_data = include_bytes!("../../../cairo_programs/secp256r1_slope.json");
    run_program_simple(program_data.as_slice());
}

#[test]
#[cfg_attr(target_arch = "wasm32", wasm_bindgen_test)]
fn garaga() {
    let program_data = include_bytes!("../../../cairo_programs/garaga.json");
    run_program_simple(program_data.as_slice());
}

#[test]
#[cfg_attr(target_arch = "wasm32", wasm_bindgen_test)]
fn cairo_run_bigint_div_mod() {
    let program_data = include_bytes!("../../../cairo_programs/bigint.json");
    run_program_simple(program_data.as_slice());
}

#[test]
#[cfg_attr(target_arch = "wasm32", wasm_bindgen_test)]
fn div_mod_n() {
    let program_data = include_bytes!("../../../cairo_programs/div_mod_n.json");
    run_program_simple(program_data.as_slice());
}

#[test]
#[cfg_attr(target_arch = "wasm32", wasm_bindgen_test)]
fn is_zero() {
    let program_data = include_bytes!("../../../cairo_programs/is_zero.json");
    run_program_simple(program_data.as_slice());
}

#[test]
#[cfg_attr(target_arch = "wasm32", wasm_bindgen_test)]
fn cairo_run_secp256r1_div_mod_n() {
    let program_data = include_bytes!("../../../cairo_programs/secp256r1_div_mod_n.json");
    run_program_simple(program_data.as_slice());
}

#[test]
#[cfg_attr(target_arch = "wasm32", wasm_bindgen_test)]
fn is_zero_pack() {
    let program_data = include_bytes!("../../../cairo_programs/is_zero_pack.json");
    run_program_simple(program_data.as_slice());
}

#[test]
#[cfg_attr(target_arch = "wasm32", wasm_bindgen_test)]
fn n_bit() {
    let program_data = include_bytes!("../../../cairo_programs/n_bit.json");
    run_program_simple(program_data.as_slice());
}

#[test]
#[cfg_attr(target_arch = "wasm32", wasm_bindgen_test)]
fn highest_bitlen() {
    let program_data = include_bytes!("../../../cairo_programs/highest_bitlen.json");
    run_program_simple(program_data.as_slice());
}

#[test]
#[cfg_attr(target_arch = "wasm32", wasm_bindgen_test)]
fn uint256_improvements() {
    let program_data = include_bytes!("../../../cairo_programs/uint256_improvements.json");
    run_program_simple(program_data.as_slice());
}

#[test]
#[cfg_attr(target_arch = "wasm32", wasm_bindgen_test)]
fn memory_holes() {
    let program_data = include_bytes!("../../../cairo_programs/memory_holes.json");
    run_program_simple(program_data.as_slice())
}

#[test]
#[cfg_attr(target_arch = "wasm32", wasm_bindgen_test)]
fn ec_recover() {
    let program_data = include_bytes!("../../../cairo_programs/ec_recover.json");
    run_program_simple(program_data.as_slice());
}

#[test]
#[cfg_attr(target_arch = "wasm32", wasm_bindgen_test)]
fn cairo_run_inv_mod_p_uint512() {
    let program_data = include_bytes!("../../../cairo_programs/inv_mod_p_uint512.json");
    run_program_simple(program_data.as_slice());
}

#[test]
#[cfg_attr(target_arch = "wasm32", wasm_bindgen_test)]
fn bitand_hint() {
    let program_data = include_bytes!("../../../cairo_programs/bitand_hint.json");
    run_program_simple(program_data.as_slice());
}

#[test]
#[cfg_attr(target_arch = "wasm32", wasm_bindgen_test)]
fn assert_le_felt_old() {
    let program_data = include_bytes!("../../../cairo_programs/assert_le_felt_old.json");
    run_program_simple(program_data.as_slice());
}

#[test]
#[cfg_attr(target_arch = "wasm32", wasm_bindgen_test)]
fn cairo_run_fq_test() {
    let program_data = include_bytes!("../../../cairo_programs/fq_test.json");
    run_program_simple(program_data.as_slice());
}

#[test]
#[cfg_attr(target_arch = "wasm32", wasm_bindgen_test)]
fn cairo_run_ec_negate() {
    let program_data = include_bytes!("../../../cairo_programs/ec_negate.json");
    run_program_simple(program_data.as_slice());
}

#[test]
#[cfg_attr(target_arch = "wasm32", wasm_bindgen_test)]
fn cairo_run_compute_slope_v2_test() {
    let program_data = include_bytes!("../../../cairo_programs/compute_slope_v2.json");
    run_program_simple(program_data.as_slice());
}

#[test]
#[cfg_attr(target_arch = "wasm32", wasm_bindgen_test)]
fn cairo_run_compute_doubling_slope_v2_test() {
    let program_data = include_bytes!("../../../cairo_programs/compute_doubling_slope_v2.json");
    run_program_simple(program_data.as_slice());
}

#[test]
#[cfg_attr(target_arch = "wasm32", wasm_bindgen_test)]
fn fast_ec_add_v2() {
    let program_data = include_bytes!("../../../cairo_programs/fast_ec_add_v2.json");
    run_program_simple(program_data.as_slice());
}

#[test]
#[cfg_attr(target_arch = "wasm32", wasm_bindgen_test)]
fn fast_ec_add_v3() {
    let program_data = include_bytes!("../../../cairo_programs/fast_ec_add_v3.json");
    run_program_simple(program_data.as_slice());
}

#[test]
#[cfg_attr(target_arch = "wasm32", wasm_bindgen_test)]
fn ec_double_assign_new_x_v3() {
    let program_data = include_bytes!("../../../cairo_programs/ec_double_assign_new_x_v3.json");
    run_program_simple(program_data.as_slice());
}

#[test]
#[cfg_attr(target_arch = "wasm32", wasm_bindgen_test)]
fn secp256r1_fast_ec_add() {
    let program_data = include_bytes!("../../../cairo_programs/secp256r1_fast_ec_add.json");
    run_program_simple(program_data.as_slice());
}

#[test]
#[cfg_attr(target_arch = "wasm32", wasm_bindgen_test)]
fn split_xx_hint() {
    let program_data = include_bytes!("../../../cairo_programs/split_xx_hint.json");
    run_program_simple(program_data.as_slice());
}

#[test]
#[cfg_attr(target_arch = "wasm32", wasm_bindgen_test)]
fn nondet_bigint3_v2() {
    let program_data = include_bytes!("../../../cairo_programs/nondet_bigint3_v2.json");
    run_program_simple(program_data.as_slice());
}

#[test]
#[cfg_attr(target_arch = "wasm32", wasm_bindgen_test)]
fn ec_double_v4() {
    let program_data = include_bytes!("../../../cairo_programs/ec_double_v4.json");
    run_program_simple(program_data.as_slice());
}

#[test]
#[cfg_attr(target_arch = "wasm32", wasm_bindgen_test)]
fn cairo_run_ec_double_slope() {
    let program_data = include_bytes!("../../../cairo_programs/ec_double_slope.json");
    run_program_simple(program_data.as_slice());
}

#[test]
#[cfg_attr(target_arch = "wasm32", wasm_bindgen_test)]
fn cairo_run_normalize_address() {
    let program_data = include_bytes!("../../../cairo_programs/normalize_address.json");
    run_program_simple(program_data.as_slice());
}

#[test]
#[cfg_attr(target_arch = "wasm32", wasm_bindgen_test)]
fn cairo_run_sha256_test() {
    let program_data = include_bytes!("../../../cairo_programs/sha256_test.json");
    run_program_simple(program_data.as_slice());
}

#[test]
#[cfg_attr(target_arch = "wasm32", wasm_bindgen_test)]
fn cairo_run_reduce() {
    let program_data = include_bytes!("../../../cairo_programs/reduce.json");
    run_program_simple(program_data.as_slice());
}

#[test]
fn cairo_run_if_reloc_equal() {
    let program_data = include_bytes!("../../../cairo_programs/if_reloc_equal.json");
    run_program_simple(program_data);
}

#[test]
#[cfg_attr(target_arch = "wasm32", wasm_bindgen_test)]
fn fibonacci_proof_mode_disable_trace_padding() {
    let program_data = include_bytes!("../../../cairo_programs/fibonacci.json");
    let config = CairoRunConfig {
        disable_trace_padding: true,
        ..Default::default()
    };
    let mut hint_processor = BuiltinHintProcessor::new_empty();
    let runner = cairo_run(program_data, &config, &mut hint_processor).unwrap();
    assert!(runner.get_memory_holes().unwrap().is_zero());
}

#[test]
#[cfg_attr(target_arch = "wasm32", wasm_bindgen_test)]
fn value_beyond_segment() {
    let program_data = include_bytes!("../../../cairo_programs/value_beyond_segment.json");
    run_program_simple(program_data.as_slice());
}

#[test]
fn cairo_run_overflowing_dict() {
    let program_data =
        include_bytes!("../../../cairo_programs/manually_compiled/overflowing_dict.json");
    run_program_with_error(program_data, "Unknown memory cell at address");
}

#[test]
fn cairo_run_big_hint_pcs() {
    let program_data =
        include_bytes!("../../../cairo_programs/manually_compiled/invalid_hint_pc.json");
    run_program_with_error(
        program_data,
        "Hint PC (18446744073709551615) is greater or equal to program length (0)",
    );
}

#[test]
#[cfg_attr(target_arch = "wasm32", wasm_bindgen_test)]
fn divmod_igcdex_not_one() {
    let program_data =
        include_bytes!("../../../cairo_programs/bad_programs/divmod_igcdex_not_one.json");
    let error_msg = "Operation failed: divmod(1, 340282366920938463463374607431768211457, 340282366920938463463374607431768211457), igcdex(340282366920938463463374607431768211457, 340282366920938463463374607431768211457) != 1";
    run_program_with_error(program_data.as_slice(), error_msg);
}

#[test]
#[cfg(feature = "test_utils")]
fn cairo_run_print_felt() {
    let program_data = include_bytes!("../../../cairo_programs/print_feature/print_felt.json");
    run_program_simple(program_data);
}

#[test]
#[cfg(feature = "test_utils")]
fn cairo_run_print_array() {
    let program_data = include_bytes!("../../../cairo_programs/print_feature/print_array.json");
    run_program_simple(program_data);
}

#[test]
#[cfg(feature = "test_utils")]
fn cairo_run_print_dict_felt() {
    let program_data = include_bytes!("../../../cairo_programs/print_feature/print_dict_felt.json");
    run_program_simple(program_data);
}

#[test]
#[cfg(feature = "test_utils")]
fn cairo_run_print_dict_array() {
    let program_data =
        include_bytes!("../../../cairo_programs/print_feature/print_dict_array.json");
    run_program_simple(program_data);
}

#[test]
fn run_program_allow_missing_builtins() {
    let program_data = include_bytes!("../../../cairo_programs/pedersen_extra_builtins.json");
    let config = CairoRunConfig {
        allow_missing_builtins: Some(true),
        layout: LayoutName::small, // The program logic only uses builtins in the small layout but contains builtins outside of it
        ..Default::default()
    };
    assert!(crate::cairo_run::cairo_run(
        program_data,
        &config,
        &mut BuiltinHintProcessor::new_empty()
    )
    .is_ok())
}

#[test]
fn run_program_allow_missing_builtins_proof() {
    let program_data =
        include_bytes!("../../../cairo_programs/proof_programs/pedersen_extra_builtins.json");
    let config = CairoRunConfig {
        proof_mode: true,
        allow_missing_builtins: Some(true),
        layout: LayoutName::small, // The program logic only uses builtins in the small layout but contains builtins outside of it
        ..Default::default()
    };
    assert!(crate::cairo_run::cairo_run(
        program_data,
        &config,
        &mut BuiltinHintProcessor::new_empty()
    )
    .is_ok())
}

#[test]
#[cfg(feature = "mod_builtin")]
fn cairo_run_mod_builtin() {
    let program_data =
        include_bytes!("../../../cairo_programs/mod_builtin_feature/mod_builtin.json");
    run_program_with_custom_mod_builtin_params(program_data, false, 1, 3, None);
}

#[test]
#[cfg(feature = "mod_builtin")]
fn cairo_run_mod_builtin_failure() {
    let program_data =
        include_bytes!("../../../cairo_programs/mod_builtin_feature/mod_builtin_failure.json");
    let error_msg = "mul_mod_builtin: Expected a * b == c (mod p). Got: instance=2, batch=0, p=9, a=2, b=2, c=2.";
    run_program_with_custom_mod_builtin_params(program_data, false, 1, 3, Some(error_msg));
}

#[test]
#[cfg(feature = "mod_builtin")]
fn cairo_run_mod_builtin_large_batch_size() {
    let program_data = include_bytes!(
        "../../../cairo_programs/mod_builtin_feature/mod_builtin_large_batch_size.json"
    );
    run_program_with_custom_mod_builtin_params(program_data, false, 8, 3, None);
}

#[test]
#[cfg(feature = "mod_builtin")]
fn cairo_run_mod_builtin_large_batch_size_failure() {
    let program_data = include_bytes!(
        "../../../cairo_programs/mod_builtin_feature/mod_builtin_large_batch_size_failure.json"
    );
    let error_msg = "mul_mod_builtin: Expected a * b == c (mod p). Got: instance=0, batch=2, p=9, a=2, b=2, c=2.";
    run_program_with_custom_mod_builtin_params(program_data, false, 8, 3, Some(error_msg));
}

#[test]
#[cfg(feature = "mod_builtin")]
fn cairo_run_mod_builtin_proof() {
    let program_data =
        include_bytes!("../../../cairo_programs/mod_builtin_feature/proof/mod_builtin.json");
    run_program_with_custom_mod_builtin_params(program_data, true, 1, 3, None);
}

#[test]
#[cfg(feature = "mod_builtin")]
fn cairo_run_mod_builtin_large_batch_size_proof() {
    let program_data = include_bytes!(
        "../../../cairo_programs/mod_builtin_feature/proof/mod_builtin_large_batch_size.json"
    );
    run_program_with_custom_mod_builtin_params(program_data, true, 8, 3, None);
}

#[cfg(feature = "mod_builtin")]
fn run_program_with_custom_mod_builtin_params(
    data: &[u8],
    proof_mode: bool,
    batch_size: usize,
    word_bit_len: u32,
    security_error: Option<&str>,
) {
    let cairo_run_config = CairoRunConfig {
        layout: LayoutName::all_cairo,
        proof_mode,
        ..Default::default()
    };
    let mut hint_processor = BuiltinHintProcessor::new_empty();
    let program = Program::from_bytes(data, Some(cairo_run_config.entrypoint)).unwrap();
    let mut cairo_runner = CairoRunner::new(
        &program,
        cairo_run_config.layout,
        cairo_run_config.dynamic_layout_params,
        cairo_run_config.proof_mode,
        cairo_run_config.trace_enabled,
    )
    .unwrap();

    let end = cairo_runner.initialize(false).unwrap();
    // Modify add_mod & mul_mod params
    for runner in cairo_runner.vm.get_builtin_runners_as_mut() {
        if let BuiltinRunner::Mod(runner) = runner {
            runner.override_layout_params(batch_size, word_bit_len)
        }
    }

    cairo_runner.run_until_pc(end, &mut hint_processor).unwrap();

    if cairo_run_config.proof_mode {
        cairo_runner.run_for_steps(1, &mut hint_processor).unwrap();
    }
    cairo_runner
        .end_run(
            cairo_run_config.disable_trace_padding,
            false,
            &mut hint_processor,
        )
        .unwrap();

    cairo_runner.vm.verify_auto_deductions().unwrap();
    cairo_runner.read_return_values(false).unwrap();
    if cairo_run_config.proof_mode {
        cairo_runner.finalize_segments().unwrap();
    }
    if !cairo_run_config.proof_mode {
        let security_res = verify_secure_runner(&cairo_runner, true, None);
        if let Some(error) = security_error {
            assert!(security_res.is_err());
            assert!(security_res.err().unwrap().to_string().contains(error));
            return;
        }
        security_res.unwrap();
    }
}

#[test]
#[cfg(feature = "mod_builtin")]
fn cairo_run_apply_poly() {
    let program_data =
        include_bytes!("../../../cairo_programs/mod_builtin_feature/apply_poly.json");
    run_program(program_data, false, None, None, None);
}

#[test]
#[cfg(feature = "mod_builtin")]
fn cairo_run_apply_poly_proof() {
    let program_data =
        include_bytes!("../../../cairo_programs/mod_builtin_feature/proof/apply_poly.json");
    run_program(program_data, true, None, None, None);
}

#[test]
#[cfg_attr(target_arch = "wasm32", wasm_bindgen_test)]
#[cfg(feature = "cairo-0-secp-hints")]
fn cairo_run_secp_cairo0_reduce_value() {
    let program_data = include_bytes!(
        "../../../cairo_programs/cairo-0-secp-hints-feature/secp_cairo0_reduce_value.json"
    );
    run_program_simple(program_data.as_slice());
}

#[test]
#[cfg_attr(target_arch = "wasm32", wasm_bindgen_test)]
#[cfg(feature = "cairo-0-secp-hints")]
fn cairo_run_secp_cairo0_ec() {
    let program_data =
        include_bytes!("../../../cairo_programs/cairo-0-secp-hints-feature/secp_cairo0_ec.json");
    run_program_simple(program_data.as_slice());
}

#[test]
#[cfg_attr(target_arch = "wasm32", wasm_bindgen_test)]
#[cfg(feature = "cairo-0-secp-hints")]
fn cairo_run_secp_cairo0_reduce_x() {
    let program_data = include_bytes!(
        "../../../cairo_programs/cairo-0-secp-hints-feature/secp_cairo0_reduce_x.json"
    );
    run_program_simple(program_data.as_slice());
}

#[test]
#[cfg_attr(target_arch = "wasm32", wasm_bindgen_test)]
#[cfg(feature = "cairo-0-secp-hints")]
fn cairo_run_secp_cairo0_get_point_from_x() {
    let program_data = include_bytes!(
        "../../../cairo_programs/cairo-0-secp-hints-feature/secp_cairo0_get_point_from_x.json"
    );
    run_program_simple(program_data.as_slice());
}

#[test]
#[cfg_attr(target_arch = "wasm32", wasm_bindgen_test)]
#[cfg(feature = "cairo-0-secp-hints")]
fn cairo_run_secp_cairo0_compute_q_mod_prime() {
    let program_data = include_bytes!(
        "../../../cairo_programs/cairo-0-secp-hints-feature/secp_cairo0_compute_q_mod_prime.json"
    );
    run_program_simple(program_data.as_slice());
}

#[test]
#[cfg_attr(target_arch = "wasm32", wasm_bindgen_test)]
#[cfg(feature = "cairo-0-secp-hints")]
fn cairo_run_secp_cairo0_ec_double_assign_new_x() {
    let program_data =
        include_bytes!("../../../cairo_programs/cairo-0-secp-hints-feature/secp_cairo0_ec_double_assign_new_x.json");
    run_program_simple(program_data.as_slice());
}

#[test]
#[cfg_attr(target_arch = "wasm32", wasm_bindgen_test)]
#[cfg(feature = "cairo-0-secp-hints")]
fn cairo_run_secp_cairo0_ec_mul_by_uint256() {
    let program_data = include_bytes!(
        "../../../cairo_programs/cairo-0-secp-hints-feature/secp_cairo0_ec_mul_by_uint256.json"
    );
    run_program_simple(program_data.as_slice());
}

#[test]
#[cfg_attr(target_arch = "wasm32", wasm_bindgen_test)]
#[cfg(feature = "cairo-0-data-availability-hints")]
fn cairo_run_data_availability_reduced_mul() {
    let program_data =
        include_bytes!("../../../cairo_programs/cairo-0-kzg-da-hints/reduced_mul.json");
    run_program_simple(program_data.as_slice());
}
