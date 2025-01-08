use crate::{tests::*, vm::runners::cairo_runner::ResourceTracker};
use assert_matches::assert_matches;

#[test]
#[cfg_attr(target_arch = "wasm32", wasm_bindgen_test)]
fn test_init_squash_data() {
    let program_data =
        include_bytes!("../../../cairo_programs/cairo-1-contracts/init_squash_data.casm");

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
    let program_data = include_bytes!("../../../cairo_programs/cairo-1-contracts/dict_test.casm");
    run_cairo_1_entrypoint(program_data.as_slice(), 0, &[], &[5_usize.into()]);
}

#[test]
#[cfg_attr(target_arch = "wasm32", wasm_bindgen_test)]
fn test_uint256_div_mod_hint_max_value() {
    let program_data =
        include_bytes!("../../../cairo_programs/cairo-2-contracts/uint256_div_mod.casm");

    run_cairo_1_entrypoint(program_data.as_slice(), 0, &[], &[]);
}

#[test]
#[cfg_attr(target_arch = "wasm32", wasm_bindgen_test)]
fn test_uint256_div_mod_hint() {
    let program_data =
        include_bytes!("../../../cairo_programs/cairo-2-contracts/uint256_div_mod.casm");

    run_cairo_1_entrypoint(
        program_data.as_slice(),
        219,
        &[36_usize.into(), 2_usize.into()],
        &[Felt252::from(18_usize)],
    );
}

#[test]
#[cfg_attr(target_arch = "wasm32", wasm_bindgen_test)]
fn test_less_than_or_equal() {
    let program_data =
        include_bytes!("../../../cairo_programs/cairo-1-contracts/test_less_than.casm");
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
    let program_data = include_bytes!("../../../cairo_programs/cairo-1-contracts/fib.casm");
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
    let program_data = include_bytes!("../../../cairo_programs/cairo-1-contracts/fib.casm");
    run_cairo_1_entrypoint(
        program_data.as_slice(),
        0,
        &[3_usize.into(), 3_usize.into(), 3_usize.into()],
        &[9_usize.into()],
    );
}

#[test]
#[cfg_attr(target_arch = "wasm32", wasm_bindgen_test)]
fn factorial_50() {
    let program_data = include_bytes!("../../../cairo_programs/cairo-1-contracts/factorial.casm");
    run_cairo_1_entrypoint(
        program_data.as_slice(),
        0,
        &[50.into()],
        &[Felt252::from_dec_str(
            "30414093201713378043612608166064768844377641568960512000000000000",
        )
        .unwrap()],
    );
}
#[test]
#[cfg_attr(target_arch = "wasm32", wasm_bindgen_test)]
fn factorial_2000() {
    let program_data = include_bytes!("../../../cairo_programs/cairo-1-contracts/factorial.casm");
    run_cairo_1_entrypoint(
        program_data.as_slice(),
        0,
        &[2000.into()],
        &[Felt252::from_dec_str(
            "2570376556569900799903105814841036176886569861654260254942280653735904624674",
        )
        .unwrap()],
    );
}

#[test]
#[cfg_attr(target_arch = "wasm32", wasm_bindgen_test)]
fn u8_sqrt_9() {
    let program_data = include_bytes!("../../../cairo_programs/cairo-1-contracts/u8_sqrt.casm");
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
    let program_data = include_bytes!("../../../cairo_programs/cairo-1-contracts/u8_sqrt.casm");
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
    let program_data = include_bytes!("../../../cairo_programs/cairo-1-contracts/u8_sqrt.casm");
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
    let program_data = include_bytes!("../../../cairo_programs/cairo-1-contracts/u8_sqrt.casm");
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
    let program_data = include_bytes!("../../../cairo_programs/cairo-1-contracts/u16_sqrt.casm");
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
    let program_data = include_bytes!("../../../cairo_programs/cairo-1-contracts/u16_sqrt.casm");
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
    let program_data = include_bytes!("../../../cairo_programs/cairo-1-contracts/u16_sqrt.casm");
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
    let program_data = include_bytes!("../../../cairo_programs/cairo-1-contracts/u16_sqrt.casm");
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
    let program_data = include_bytes!("../../../cairo_programs/cairo-1-contracts/u32_sqrt.casm");
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
    let program_data = include_bytes!("../../../cairo_programs/cairo-1-contracts/u32_sqrt.casm");
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
    let program_data = include_bytes!("../../../cairo_programs/cairo-1-contracts/u32_sqrt.casm");
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
    let program_data = include_bytes!("../../../cairo_programs/cairo-1-contracts/u32_sqrt.casm");
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
    let program_data = include_bytes!("../../../cairo_programs/cairo-1-contracts/u64_sqrt.casm");
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
    let program_data = include_bytes!("../../../cairo_programs/cairo-1-contracts/u64_sqrt.casm");
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
    let program_data = include_bytes!("../../../cairo_programs/cairo-1-contracts/u64_sqrt.casm");
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
    let program_data = include_bytes!("../../../cairo_programs/cairo-1-contracts/u64_sqrt.casm");
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
    let program_data = include_bytes!("../../../cairo_programs/cairo-1-contracts/u128_sqrt.casm");
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
    let program_data = include_bytes!("../../../cairo_programs/cairo-1-contracts/u128_sqrt.casm");
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
    let program_data = include_bytes!("../../../cairo_programs/cairo-1-contracts/u128_sqrt.casm");
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
    let program_data = include_bytes!("../../../cairo_programs/cairo-1-contracts/u128_sqrt.casm");
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
    let program_data = include_bytes!("../../../cairo_programs/cairo-1-contracts/u256_sqrt.casm");
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
    let program_data = include_bytes!("../../../cairo_programs/cairo-1-contracts/u256_sqrt.casm");
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
    let program_data = include_bytes!("../../../cairo_programs/cairo-1-contracts/u256_sqrt.casm");
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
    let program_data = include_bytes!("../../../cairo_programs/cairo-1-contracts/u256_sqrt.casm");
    run_cairo_1_entrypoint(
        program_data.as_slice(),
        0,
        &[0_usize.into()],
        &[0_usize.into()],
    );
}

#[test]
#[cfg_attr(target_arch = "wasm32", wasm_bindgen_test)]
fn u256_sqrt_max_num() {
    let program_data = include_bytes!("../../../cairo_programs/cairo-1-contracts/u256_sqrt.casm");

    run_cairo_1_entrypoint(program_data.as_slice(), 257, &[], &[1.into()]);
}

#[test]
#[cfg_attr(target_arch = "wasm32", wasm_bindgen_test)]
fn u256_sqrt_big_num() {
    let program_data = include_bytes!("../../../cairo_programs/cairo-1-contracts/u256_sqrt.casm");
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
    let program_data = include_bytes!("../../../cairo_programs/cairo-1-contracts/divmod.casm");
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
    let program_data =
        include_bytes!("../../../cairo_programs/cairo-1-contracts/alloc_segment.casm");
    run_cairo_1_entrypoint(program_data.as_slice(), 0, &[], &[]);
}

#[test]
#[cfg_attr(target_arch = "wasm32", wasm_bindgen_test)]
fn should_skip_squash_loop_hint_test() {
    let program_data =
        include_bytes!("../../../cairo_programs/cairo-1-contracts/should_skip_squash_loop.casm");
    run_cairo_1_entrypoint(program_data.as_slice(), 0, &[], &[]);
}

#[test]
#[cfg_attr(target_arch = "wasm32", wasm_bindgen_test)]
fn get_segment_arena_test() {
    let program_data =
        include_bytes!("../../../cairo_programs/cairo-1-contracts/get_segment_arena_index.casm");
    run_cairo_1_entrypoint(program_data.as_slice(), 0, &[], &[1_usize.into()]);
}

#[test]
#[cfg_attr(target_arch = "wasm32", wasm_bindgen_test)]
fn boxed_fibonacci() {
    let program_data =
        include_bytes!("../../../cairo_programs/cairo-1-contracts/alloc_constant_size.casm");
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
    let program_data =
        include_bytes!("../../../cairo_programs/cairo-1-contracts/linear_split.casm");
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
fn alloc_felt_252_dict() {
    let program_data =
        include_bytes!("../../../cairo_programs/cairo-1-contracts/felt_252_dict.casm");
    run_cairo_1_entrypoint(program_data.as_slice(), 0, &[], &[1.into()]);
}

#[test]
#[cfg_attr(target_arch = "wasm32", wasm_bindgen_test)]
fn random_ec_point() {
    let program_data =
        include_bytes!("../../../cairo_programs/cairo-1-contracts/random_ec_point.casm");
    run_cairo_1_entrypoint(program_data.as_slice(), 0, &[], &[1.into()]);
}

#[test]
#[cfg_attr(target_arch = "wasm32", wasm_bindgen_test)]
fn assert_le_find_small_arcs() {
    let program_data =
        include_bytes!("../../../cairo_programs/cairo-1-contracts/assert_le_find_small_arcs.casm");
    run_cairo_1_entrypoint(program_data.as_slice(), 0, &[], &[]);
}

#[test]
#[cfg_attr(target_arch = "wasm32", wasm_bindgen_test)]
fn felt252_dict_entry_init() {
    let program_data =
        include_bytes!("../../../cairo_programs/cairo-1-contracts/felt252_dict_entry_init.casm");
    run_cairo_1_entrypoint(program_data.as_slice(), 0, &[], &[]);
}

#[test]
#[cfg_attr(target_arch = "wasm32", wasm_bindgen_test)]
fn felt252_dict_entry_update() {
    let program_data =
        include_bytes!("../../../cairo_programs/cairo-1-contracts/felt252_dict_entry_update.casm");
    run_cairo_1_entrypoint(
        program_data.as_slice(),
        0,
        &[],
        &[64_usize.into(), 75_usize.into()],
    );
}

#[test]
#[cfg_attr(target_arch = "wasm32", wasm_bindgen_test)]
fn widelmul128_test() {
    let program_data = include_bytes!("../../../cairo_programs/cairo-1-contracts/widemul128.casm");
    run_cairo_1_entrypoint(
        program_data.as_slice(),
        0,
        // numbers to multiply:
        &[4_usize.into(), 2_usize.into()],
        // it returns: `a * b == 2**128 * res_high + res_low`
        // that property should be 1 (true) if
        // the implementation is correct and
        // false otherwise.
        &[1_usize.into()],
    );
}

#[test]
#[cfg_attr(target_arch = "wasm32", wasm_bindgen_test)]
fn field_sqrt_test() {
    let program_data = include_bytes!("../../../cairo_programs/cairo-1-contracts/field_sqrt.casm");
    run_cairo_1_entrypoint(program_data.as_slice(), 0, &[], &[10.into()]);
}
#[test]
#[cfg_attr(target_arch = "wasm32", wasm_bindgen_test)]
fn uint512_div_mod_test() {
    let program_data =
        include_bytes!("../../../cairo_programs/cairo-1-contracts/uint512_div_mod.casm");
    run_cairo_1_entrypoint(
        program_data.as_slice(),
        0,
        &[],
        // that property should be 1 (true) if
        // the implementation is correct and
        // false otherwise.
        &[],
    );
}

// ================
//   Tests run cairo 1 entrypoint with RunResources
// ================

#[test]
#[cfg_attr(target_arch = "wasm32", wasm_bindgen_test)]
fn fibonacci_with_run_resources_ok() {
    let program_data = include_bytes!("../../../cairo_programs/cairo-1-contracts/fib.casm");
    let contract_class: CasmContractClass = serde_json::from_slice(program_data).unwrap();
    // Program takes 621 steps
    let mut hint_processor =
        Cairo1HintProcessor::new(&contract_class.hints, RunResources::new(621), false);
    assert_matches!(
        run_cairo_1_entrypoint_with_run_resources(
            serde_json::from_slice(program_data.as_slice()).unwrap(),
            0,
            &mut hint_processor,
            &[1_usize.into(), 1_usize.into(), 20_usize.into()],
        ),
        Ok(x) if x == [10946_usize.into()]
    );

    assert_eq!(hint_processor.run_resources(), &RunResources::new(0));
}

#[test]
#[cfg_attr(target_arch = "wasm32", wasm_bindgen_test)]
fn fibonacci_with_run_resources_2_ok() {
    let program_data = include_bytes!("../../../cairo_programs/cairo-1-contracts/fib.casm");
    let contract_class: CasmContractClass = serde_json::from_slice(program_data).unwrap();
    // Program takes 621 steps
    let mut hint_processor =
        Cairo1HintProcessor::new(&contract_class.hints, RunResources::new(1000), false);
    assert_matches!(
        run_cairo_1_entrypoint_with_run_resources(
            contract_class,
            0,
            &mut hint_processor,
            &[1_usize.into(), 1_usize.into(), 20_usize.into()],
        ),
        Ok(x) if x == [10946_usize.into()]
    );
    assert_eq!(
        hint_processor.run_resources(),
        &RunResources::new(1000 - 621)
    );
}

#[test]
#[cfg_attr(target_arch = "wasm32", wasm_bindgen_test)]
fn fibonacci_with_run_resources_error() {
    let program_data = include_bytes!("../../../cairo_programs/cairo-1-contracts/fib.casm");
    let contract_class: CasmContractClass = serde_json::from_slice(program_data).unwrap();
    // Program takes 621 steps
    let mut hint_processor =
        Cairo1HintProcessor::new(&contract_class.hints, RunResources::new(100), false);
    assert!(run_cairo_1_entrypoint_with_run_resources(
        contract_class,
        0,
        &mut hint_processor,
        &[1_usize.into(), 1_usize.into(), 20_usize.into()],
    )
    .is_err());
    assert_eq!(hint_processor.run_resources(), &RunResources::new(0));
}
