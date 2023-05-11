use crate::tests::*;

#[test]
#[cfg_attr(target_arch = "wasm32", wasm_bindgen_test)]
fn test_init_squash_data() {
    let program_data =
        include_bytes!("../../cairo_programs/cairo-1-contracts/init_squash_data.casm");

    run_cairo_1_entrypoint(
        program_data.as_slice(),
        0,
        &[10_usize.into()],
        &[10_usize.into()],
    );
}

#[test]
#[cfg_attr(target_arch = "wasm32", wasm_bindgen_test)]
fn dict_hint_test() {
    let program_data = include_bytes!("../../cairo_programs/cairo-1-contracts/dict_test.casm");
    run_cairo_1_entrypoint(program_data.as_slice(), 0, &[], &[5_usize.into()]);
}

#[test]
#[cfg_attr(target_arch = "wasm32", wasm_bindgen_test)]
fn test_uint256_div_mod_hint() {
    let program_data =
        include_bytes!("../../cairo_programs/cairo-1-contracts/uint256_div_mod.casm");

    run_cairo_1_entrypoint(
        program_data.as_slice(),
        0,
        &[8_usize.into(), 2_usize.into()],
        &[Felt252::from(4_usize)],
    );
}

#[test]
#[cfg_attr(target_arch = "wasm32", wasm_bindgen_test)]
fn test_less_than_or_equal() {
    let program_data = include_bytes!("../../cairo_programs/cairo-1-contracts/test_less_than.casm");
    run_cairo_1_entrypoint(
        program_data.as_slice(),
        0,
        &[8_usize.into()],
        &[1_u8.into()],
    );

    run_cairo_1_entrypoint(
        program_data.as_slice(),
        0,
        &[290_usize.into()],
        &[0_u8.into()],
    );

    run_cairo_1_entrypoint(
        program_data.as_slice(),
        0,
        &[250_usize.into()],
        &[1_u8.into()],
    );
}

#[test]
#[cfg_attr(target_arch = "wasm32", wasm_bindgen_test)]
fn fibonacci_1() {
    let program_data = include_bytes!("../../cairo_programs/cairo-1-contracts/fib.casm");
    run_cairo_1_entrypoint(
        program_data.as_slice(),
        0,
        &[1_usize.into(), 1_usize.into(), 1_usize.into()],
        &[1_usize.into()],
    );
}

#[test]
#[cfg_attr(target_arch = "wasm32", wasm_bindgen_test)]
fn fibonacci_3() {
    let program_data = include_bytes!("../../cairo_programs/cairo-1-contracts/fib.casm");
    run_cairo_1_entrypoint(
        program_data.as_slice(),
        0,
        &[3_usize.into(), 3_usize.into(), 3_usize.into()],
        &[9_usize.into()],
    );
}

#[test]
#[cfg_attr(target_arch = "wasm32", wasm_bindgen_test)]
fn u8_sqrt_9() {
    let program_data = include_bytes!("../../cairo_programs/cairo-1-contracts/u8_sqrt.casm");
    run_cairo_1_entrypoint(
        program_data.as_slice(),
        0,
        &[9_usize.into()],
        &[3_usize.into()],
    );
}

#[test]
#[cfg_attr(target_arch = "wasm32", wasm_bindgen_test)]
fn u8_sqrt_10() {
    let program_data = include_bytes!("../../cairo_programs/cairo-1-contracts/u8_sqrt.casm");
    run_cairo_1_entrypoint(
        program_data.as_slice(),
        0,
        &[10_usize.into()],
        &[3_usize.into()],
    );
}

#[test]
#[cfg_attr(target_arch = "wasm32", wasm_bindgen_test)]
fn u8_sqrt_one() {
    let program_data = include_bytes!("../../cairo_programs/cairo-1-contracts/u8_sqrt.casm");
    run_cairo_1_entrypoint(
        program_data.as_slice(),
        0,
        &[1_usize.into()],
        &[1_usize.into()],
    );
}

#[test]
#[cfg_attr(target_arch = "wasm32", wasm_bindgen_test)]
fn u8_sqrt_zero() {
    let program_data = include_bytes!("../../cairo_programs/cairo-1-contracts/u8_sqrt.casm");
    run_cairo_1_entrypoint(
        program_data.as_slice(),
        0,
        &[0_usize.into()],
        &[0_usize.into()],
    );
}

#[test]
#[cfg_attr(target_arch = "wasm32", wasm_bindgen_test)]
fn u16_sqrt_9() {
    let program_data = include_bytes!("../../cairo_programs/cairo-1-contracts/u16_sqrt.casm");
    run_cairo_1_entrypoint(
        program_data.as_slice(),
        0,
        &[9_usize.into()],
        &[3_usize.into()],
    );
}

#[test]
#[cfg_attr(target_arch = "wasm32", wasm_bindgen_test)]
fn u16_sqrt_10() {
    let program_data = include_bytes!("../../cairo_programs/cairo-1-contracts/u16_sqrt.casm");
    run_cairo_1_entrypoint(
        program_data.as_slice(),
        0,
        &[10_usize.into()],
        &[3_usize.into()],
    );
}

#[test]
#[cfg_attr(target_arch = "wasm32", wasm_bindgen_test)]
fn u16_sqrt_one() {
    let program_data = include_bytes!("../../cairo_programs/cairo-1-contracts/u16_sqrt.casm");
    run_cairo_1_entrypoint(
        program_data.as_slice(),
        0,
        &[1_usize.into()],
        &[1_usize.into()],
    );
}

#[test]
#[cfg_attr(target_arch = "wasm32", wasm_bindgen_test)]
fn u16_sqrt_zero() {
    let program_data = include_bytes!("../../cairo_programs/cairo-1-contracts/u16_sqrt.casm");
    run_cairo_1_entrypoint(
        program_data.as_slice(),
        0,
        &[0_usize.into()],
        &[0_usize.into()],
    );
}

#[test]
#[cfg_attr(target_arch = "wasm32", wasm_bindgen_test)]
fn u32_sqrt_9() {
    let program_data = include_bytes!("../../cairo_programs/cairo-1-contracts/u32_sqrt.casm");
    run_cairo_1_entrypoint(
        program_data.as_slice(),
        0,
        &[9_usize.into()],
        &[3_usize.into()],
    );
}

#[test]
#[cfg_attr(target_arch = "wasm32", wasm_bindgen_test)]
fn u32_sqrt_10() {
    let program_data = include_bytes!("../../cairo_programs/cairo-1-contracts/u32_sqrt.casm");
    run_cairo_1_entrypoint(
        program_data.as_slice(),
        0,
        &[10_usize.into()],
        &[3_usize.into()],
    );
}

#[test]
#[cfg_attr(target_arch = "wasm32", wasm_bindgen_test)]
fn u32_sqrt_one() {
    let program_data = include_bytes!("../../cairo_programs/cairo-1-contracts/u32_sqrt.casm");
    run_cairo_1_entrypoint(
        program_data.as_slice(),
        0,
        &[1_usize.into()],
        &[1_usize.into()],
    );
}

#[test]
#[cfg_attr(target_arch = "wasm32", wasm_bindgen_test)]
fn u32_sqrt_zero() {
    let program_data = include_bytes!("../../cairo_programs/cairo-1-contracts/u32_sqrt.casm");
    run_cairo_1_entrypoint(
        program_data.as_slice(),
        0,
        &[0_usize.into()],
        &[0_usize.into()],
    );
}

#[test]
#[cfg_attr(target_arch = "wasm32", wasm_bindgen_test)]
fn u64_sqrt_9() {
    let program_data = include_bytes!("../../cairo_programs/cairo-1-contracts/u64_sqrt.casm");
    run_cairo_1_entrypoint(
        program_data.as_slice(),
        0,
        &[9_usize.into()],
        &[3_usize.into()],
    );
}

#[test]
#[cfg_attr(target_arch = "wasm32", wasm_bindgen_test)]
fn u64_sqrt_10() {
    let program_data = include_bytes!("../../cairo_programs/cairo-1-contracts/u64_sqrt.casm");
    run_cairo_1_entrypoint(
        program_data.as_slice(),
        0,
        &[10_usize.into()],
        &[3_usize.into()],
    );
}

#[test]
#[cfg_attr(target_arch = "wasm32", wasm_bindgen_test)]
fn u64_sqrt_one() {
    let program_data = include_bytes!("../../cairo_programs/cairo-1-contracts/u64_sqrt.casm");
    run_cairo_1_entrypoint(
        program_data.as_slice(),
        0,
        &[1_usize.into()],
        &[1_usize.into()],
    );
}

#[test]
#[cfg_attr(target_arch = "wasm32", wasm_bindgen_test)]
fn u64_sqrt_zero() {
    let program_data = include_bytes!("../../cairo_programs/cairo-1-contracts/u64_sqrt.casm");
    run_cairo_1_entrypoint(
        program_data.as_slice(),
        0,
        &[0_usize.into()],
        &[0_usize.into()],
    );
}

#[test]
#[cfg_attr(target_arch = "wasm32", wasm_bindgen_test)]
fn u128_sqrt_9() {
    let program_data = include_bytes!("../../cairo_programs/cairo-1-contracts/u128_sqrt.casm");
    run_cairo_1_entrypoint(
        program_data.as_slice(),
        0,
        &[9_usize.into()],
        &[3_usize.into()],
    );
}

#[test]
#[cfg_attr(target_arch = "wasm32", wasm_bindgen_test)]
fn u128_sqrt_10() {
    let program_data = include_bytes!("../../cairo_programs/cairo-1-contracts/u128_sqrt.casm");
    run_cairo_1_entrypoint(
        program_data.as_slice(),
        0,
        &[10_usize.into()],
        &[3_usize.into()],
    );
}

#[test]
#[cfg_attr(target_arch = "wasm32", wasm_bindgen_test)]
fn u128_sqrt_one() {
    let program_data = include_bytes!("../../cairo_programs/cairo-1-contracts/u128_sqrt.casm");
    run_cairo_1_entrypoint(
        program_data.as_slice(),
        0,
        &[1_usize.into()],
        &[1_usize.into()],
    );
}

#[test]
#[cfg_attr(target_arch = "wasm32", wasm_bindgen_test)]
fn u128_sqrt_zero() {
    let program_data = include_bytes!("../../cairo_programs/cairo-1-contracts/u128_sqrt.casm");
    run_cairo_1_entrypoint(
        program_data.as_slice(),
        0,
        &[0_usize.into()],
        &[0_usize.into()],
    );
}

#[test]
#[cfg_attr(target_arch = "wasm32", wasm_bindgen_test)]
fn u256_sqrt_9() {
    let program_data = include_bytes!("../../cairo_programs/cairo-1-contracts/u256_sqrt.casm");
    run_cairo_1_entrypoint(
        program_data.as_slice(),
        0,
        &[9_usize.into()],
        &[3_usize.into()],
    );
}

#[test]
#[cfg_attr(target_arch = "wasm32", wasm_bindgen_test)]
fn u256_sqrt_10() {
    let program_data = include_bytes!("../../cairo_programs/cairo-1-contracts/u256_sqrt.casm");
    run_cairo_1_entrypoint(
        program_data.as_slice(),
        0,
        &[10_usize.into()],
        &[3_usize.into()],
    );
}

#[test]
#[cfg_attr(target_arch = "wasm32", wasm_bindgen_test)]
fn u256_sqrt_one() {
    let program_data = include_bytes!("../../cairo_programs/cairo-1-contracts/u256_sqrt.casm");
    run_cairo_1_entrypoint(
        program_data.as_slice(),
        0,
        &[1_usize.into()],
        &[1_usize.into()],
    );
}

#[test]
#[cfg_attr(target_arch = "wasm32", wasm_bindgen_test)]
fn u256_sqrt_zero() {
    let program_data = include_bytes!("../../cairo_programs/cairo-1-contracts/u256_sqrt.casm");
    run_cairo_1_entrypoint(
        program_data.as_slice(),
        0,
        &[0_usize.into()],
        &[0_usize.into()],
    );
}

#[test]
#[cfg_attr(target_arch = "wasm32", wasm_bindgen_test)]
fn u256_sqrt_big_num() {
    let program_data = include_bytes!("../../cairo_programs/cairo-1-contracts/u256_sqrt.casm");
    run_cairo_1_entrypoint(
        program_data.as_slice(),
        144,
        &[],
        &[1125899906842624_usize.into()],
    );
}

#[test]
#[cfg_attr(target_arch = "wasm32", wasm_bindgen_test)]
fn divmod_hint_test() {
    let program_data = include_bytes!("../../cairo_programs/cairo-1-contracts/divmod.casm");
    run_cairo_1_entrypoint(
        program_data.as_slice(),
        0,
        &[16_usize.into(), 2_usize.into()],
        &[8_usize.into()],
    );
}

#[test]
#[cfg_attr(target_arch = "wasm32", wasm_bindgen_test)]
fn alloc_segment_hint_test() {
    let program_data = include_bytes!("../../cairo_programs/cairo-1-contracts/alloc_segment.casm");
    run_cairo_1_entrypoint(program_data.as_slice(), 0, &[], &[]);
}

#[test]
#[cfg_attr(target_arch = "wasm32", wasm_bindgen_test)]
fn should_skip_squash_loop_hint_test() {
    let program_data =
        include_bytes!("../../cairo_programs/cairo-1-contracts/should_skip_squash_loop.casm");
    run_cairo_1_entrypoint(program_data.as_slice(), 0, &[], &[]);
}

#[test]
#[cfg_attr(target_arch = "wasm32", wasm_bindgen_test)]
fn get_segment_arena_test() {
    let program_data =
        include_bytes!("../../cairo_programs/cairo-1-contracts/get_segment_arena_index.casm");
    run_cairo_1_entrypoint(program_data.as_slice(), 0, &[], &[1_usize.into()]);
}

#[test]
#[cfg_attr(target_arch = "wasm32", wasm_bindgen_test)]
fn boxed_fibonacci() {
    let program_data =
        include_bytes!("../../cairo_programs/cairo-1-contracts/alloc_constant_size.casm");
    run_cairo_1_entrypoint(
        program_data.as_slice(),
        0,
        &[3_usize.into(), 3_usize.into(), 3_usize.into()],
        &[9_usize.into()],
    );
}

#[test]
#[cfg_attr(target_arch = "wasm32", wasm_bindgen_test)]
fn linear_split() {
    let program_data = include_bytes!("../../cairo_programs/cairo-1-contracts/linear_split.casm");
    run_cairo_1_entrypoint(
        program_data.as_slice(),
        0,
        &[1_usize.into()],
        &[0.into(), 1.into()],
    );

    run_cairo_1_entrypoint(
        program_data.as_slice(),
        0,
        &[100_usize.into()],
        &[0.into(), 100.into()],
    );

    run_cairo_1_entrypoint(
        program_data.as_slice(),
        0,
        &[1000_usize.into()],
        &[0.into(), 1000.into()],
    );
}

#[test]
#[cfg_attr(target_arch = "wasm32", wasm_bindgen_test)]
fn assert_le_find_small_arcs() {
    let program_data =
        include_bytes!("../../cairo_programs/cairo-1-contracts/assert_le_find_small_arcs.casm");
    run_cairo_1_entrypoint(program_data.as_slice(), 0, &[], &[]);
}
