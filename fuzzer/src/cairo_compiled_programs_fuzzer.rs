#![no_main]
use cairo_vm::cairo_run::{self, CairoRunConfig, EncodeTraceError};
use cairo_vm::hint_processor::builtin_hint_processor::builtin_hint_processor_definition::BuiltinHintProcessor;
use libfuzzer_sys::fuzz_target;
use std::fs;
use std::process::Command;
use std::sync::atomic::{AtomicUsize, Ordering};

// Global counter for fuzz iteration
static FUZZ_ITERATION_COUNT: AtomicUsize = AtomicUsize::new(0);

fuzz_target!(|data: (u128, u8, u8, u128, u128)| {
    // Define fuzzer iteration with id purposes
    let iteration_count = FUZZ_ITERATION_COUNT.fetch_add(1, Ordering::SeqCst);

    // Define default configuration
    let cairo_run_config = CairoRunConfig::default();
    let mut hint_executor = BuiltinHintProcessor::new_empty();

    // Create and run the programs
    program_array_sum(data.0.to_string(), &cairo_run_config, &mut hint_executor);
    program_unsafe_keccak(data.0.to_string(), &cairo_run_config, &mut hint_executor);
    program_bitwise(data.1, data.2, &cairo_run_config, &mut hint_executor);
    program_poseidon(
        data.1,
        data.2,
        data.0,
        &cairo_run_config,
        &mut hint_executor,
    );
    program_range_check(
        data.1,
        data.0,
        data.3,
        &cairo_run_config,
        &mut hint_executor,
    );
    program_ec_op(data.1, &cairo_run_config, &mut hint_executor);
    program_pedersen(data.0, data.3, &cairo_run_config, &mut hint_executor);
    program_ecdsa(
        data.0,
        data.3,
        data.4,
        data.1,
        &cairo_run_config,
        &mut hint_executor,
    );
});

fn program_array_sum(
    array: String,
    cairo_run_config: &CairoRunConfig,
    hint_executor: &mut BuiltinHintProcessor,
) {
    let mut populated_array = array
        .chars()
        .enumerate()
        .map(|(index, num)| format!("assert [ptr + {}] = {};  \n", index, num))
        .collect::<Vec<_>>()
        .join("            ")
        .repeat(array.len());

    let file_content = format!(
        "
        %builtins output

        from starkware.cairo.common.alloc import alloc
        from starkware.cairo.common.serialize import serialize_word
        
        // Computes the sum of the memory elements at addresses:
        //   arr + 0, arr + 1, ..., arr + (size - 1).
        func array_sum(arr: felt*, size) -> (sum: felt) {{
            if (size == 0) {{
                return (sum=0);
            }}
        
            // size is not zero.
            let (sum_of_rest) = array_sum(arr=arr + 1, size=size - 1);
            return (sum=[arr] + sum_of_rest);
        }}
        
        func main{{output_ptr: felt*}}() {{
            const ARRAY_SIZE = {};
        
            // Allocate an array.
            let (ptr) = alloc();
        
            // Populate some values in the array.
            {}
        
            // Call array_sum to compute the sum of the elements.
            let (sum) = array_sum(arr=ptr, size=ARRAY_SIZE);
        
            // Write the sum to the program output.
            serialize_word(sum);
        
            return ();
    }}
    ",
        array.len(),
        populated_array
    );

    // Create programs names and program
    let cairo_path_array_sum = format!("cairo_programs/array_sum_{:?}.cairo", FUZZ_ITERATION_COUNT);
    let json_path_array_sum = format!("cairo_programs/array_sum_{:?}.json", FUZZ_ITERATION_COUNT);
    let _ = fs::write(&cairo_path_array_sum, file_content.as_bytes());

    compile_program(&cairo_path_array_sum, &json_path_array_sum);

    let program_content_array_sum = std::fs::read(&json_path_array_sum).unwrap();

    // Run the program with default configurations
    cairo_run::cairo_run(&program_content_array_sum, cairo_run_config, hint_executor);

    // Remove files to save memory
    delete_files(&cairo_path_array_sum, &json_path_array_sum);
}

fn program_unsafe_keccak(
    array: String,
    cairo_run_config: &CairoRunConfig,
    hint_executor: &mut BuiltinHintProcessor,
) {
    let mut populated_array = array
        .chars()
        .enumerate()
        .map(|(index, num)| {
            format!(
                "assert data[{}] = {}; \n",
                index,
                num.to_string().repeat(array.len() - (index + 1))
            )
        })
        .collect::<Vec<_>>()
        .join("            ");

    let file_content = format!(
        "
    %builtins output

    from starkware.cairo.common.alloc import alloc
    from starkware.cairo.common.serialize import serialize_word
    from starkware.cairo.common.keccak import unsafe_keccak

    func main{{output_ptr: felt*}}() {{
        alloc_locals;

        let (data: felt*) = alloc();

        {}

        let (low: felt, high: felt) = unsafe_keccak(data, {});

        serialize_word(low);
        serialize_word(high);

        return ();
    }}
    ",
        populated_array,
        array.len()
    );

    // Create programs names and program
    let cairo_path_unsafe_keccak = format!(
        "cairo_programs/unsafe_keccak_{:?}.cairo",
        FUZZ_ITERATION_COUNT
    );
    let json_path_unsafe_keccak = format!(
        "cairo_programs/unsafe_keccak_{:?}.json",
        FUZZ_ITERATION_COUNT
    );
    let _ = fs::write(&cairo_path_unsafe_keccak, file_content.as_bytes());

    compile_program(&cairo_path_unsafe_keccak, &json_path_unsafe_keccak);

    let program_content_unsafe_keccak = std::fs::read(&json_path_unsafe_keccak).unwrap();

    // Run the program with default configurations
    cairo_run::cairo_run(
        &program_content_unsafe_keccak,
        &cairo_run_config,
        hint_executor,
    );

    // Remove files to save memory
    delete_files(&cairo_path_unsafe_keccak, &json_path_unsafe_keccak);
}

fn program_bitwise(
    num1: u8,
    num2: u8,
    cairo_run_config: &CairoRunConfig,
    hint_executor: &mut BuiltinHintProcessor,
) {
    let and = num1 & num2;
    let xor = num1 ^ num2;
    let or = num1 | num2;

    let file_content = format!("
    %builtins bitwise
    from starkware.cairo.common.bitwise import bitwise_and, bitwise_xor, bitwise_or, bitwise_operations
    from starkware.cairo.common.cairo_builtins import BitwiseBuiltin

    func main{{bitwise_ptr: BitwiseBuiltin*}}() {{
        let (and_a) = bitwise_and({}, {});  
        assert and_a = {}; 
        let (xor_a) = bitwise_xor(12, 10);
        assert xor_a = {};
        let (or_a) = bitwise_or(12, 10);
        assert or_a = {};

        let (and_b, xor_b, or_b) = bitwise_operations({}, {});
        assert and_b = {};
        assert xor_b = {};
        assert or_b = {};
        return ();
    }}

    ", num1, num2, and, xor, or, num1, num2, and, xor, or);

    // Create programs names and program
    let cairo_path_bitwise = format!("cairo_programs/bitwise-{:?}.cairo", FUZZ_ITERATION_COUNT);
    let json_path_bitwise = format!("cairo_programs/bitwise-{:?}.json", FUZZ_ITERATION_COUNT);
    let _ = fs::write(&cairo_path_bitwise, file_content.as_bytes());

    compile_program(&cairo_path_bitwise, &json_path_bitwise);

    let program_content_bitwise = std::fs::read(&json_path_bitwise).unwrap();

    // Run the program with default configurations
    cairo_run::cairo_run(&program_content_bitwise, &cairo_run_config, hint_executor);

    // Remove files to save memory
    delete_files(&cairo_path_bitwise, &json_path_bitwise);
}

fn program_poseidon(
    num1: u8,
    num2: u8,
    num3: u128,
    cairo_run_config: &CairoRunConfig,
    hint_executor: &mut BuiltinHintProcessor,
) {
    let file_content = format!(
        "
    %builtins poseidon
    from starkware.cairo.common.cairo_builtins import PoseidonBuiltin
    from starkware.cairo.common.poseidon_state import PoseidonBuiltinState
    from starkware.cairo.common.builtin_poseidon.poseidon import (
        poseidon_hash,
        poseidon_hash_single,
        poseidon_hash_many,
    )
    from starkware.cairo.common.alloc import alloc

    func main{{poseidon_ptr: PoseidonBuiltin*}}() {{
        // Hash one
        let (x) = poseidon_hash_single(
            {}
        );
        // Hash two
        let (y) = poseidon_hash({}, {});
        // Hash three
        let felts: felt* = alloc();
        assert felts[0] = {};
        assert felts[1] = {};
        assert felts[2] = {};
        let (z) = poseidon_hash_many(3, felts);
        return ();
    }}

    ",
        num3, num1, num2, num1, num2, num3
    );

    // Create programs names and program
    let cairo_path_poseidon = format!("cairo_programs/poseidon_{:?}.cairo", FUZZ_ITERATION_COUNT);
    let json_path_poseidon = format!("cairo_programs/poseidon_{:?}.json", FUZZ_ITERATION_COUNT);
    let _ = fs::write(&cairo_path_poseidon, file_content.as_bytes());

    compile_program(&cairo_path_poseidon, &json_path_poseidon);

    let program_content_poseidon = std::fs::read(&json_path_poseidon).unwrap();

    // Run the program with default configurations
    cairo_run::cairo_run(&program_content_poseidon, cairo_run_config, hint_executor);

    // Remove files to save memory
    delete_files(&cairo_path_poseidon, &json_path_poseidon);
}

fn program_range_check(
    num1: u8,
    num2: u128,
    num3: u128,
    cairo_run_config: &CairoRunConfig,
    hint_executor: &mut BuiltinHintProcessor,
) {
    let file_content = format!(
        "
    %builtins range_check

    from starkware.cairo.common.math import assert_250_bit
    from starkware.cairo.common.alloc import alloc
    
    func assert_250_bit_element_array{{range_check_ptr: felt}}(
        array: felt*, array_length: felt, iterator: felt
    ) {{
        if (iterator == array_length) {{
            return ();
        }}
        assert_250_bit(array[iterator]);
        return assert_250_bit_element_array(array, array_length, iterator + 1);
    }}
    
    func fill_array(array: felt*, base: felt, step: felt, array_length: felt, iterator: felt) {{
        if (iterator == array_length) {{
            return ();
        }}
        assert array[iterator] = base + step * iterator;
        return fill_array(array, base, step, array_length, iterator + 1);
    }}
    
    func main{{range_check_ptr: felt}}() {{
        alloc_locals;
        tempvar array_length = {};
        let (array: felt*) = alloc();
        fill_array(array, {}, {}, array_length, 0);
        assert_250_bit_element_array(array, array_length, 0);
        return ();
    }}
    
    ",
        num1, num2, num3
    );

    // Create programs names and program
    let cairo_path_range_check = format!(
        "cairo_programs/range_check_{:?}.cairo",
        FUZZ_ITERATION_COUNT
    );
    let json_path_range_check =
        format!("cairo_programs/range_check_{:?}.json", FUZZ_ITERATION_COUNT);
    let _ = fs::write(&cairo_path_range_check, file_content.as_bytes());

    compile_program(&cairo_path_range_check, &json_path_range_check);

    let program_content_range_check = std::fs::read(&json_path_range_check).unwrap();

    // Run the program with default configurations
    cairo_run::cairo_run(
        &program_content_range_check,
        cairo_run_config,
        hint_executor,
    );

    // Remove files to save memory
    delete_files(&cairo_path_range_check, &json_path_range_check);
}

fn program_ec_op(
    num1: u8,
    cairo_run_config: &CairoRunConfig,
    hint_executor: &mut BuiltinHintProcessor,
) {
    let file_content = format!(
        "
    %builtins ec_op

    from starkware.cairo.common.cairo_builtins import EcOpBuiltin
    from starkware.cairo.common.ec_point import EcPoint
    from starkware.cairo.common.ec import recover_y

    func main{{ec_op_ptr: EcOpBuiltin*}}() {{
        let x = {:#02x};
        let r: EcPoint = recover_y(x);
        assert r.x = {:#02x};
        return ();
    }}
    
    ",
        num1, num1
    );

    // Create programs names and program
    let cairo_path_ec_op = format!("cairo_programs/ec_op_{:?}.cairo", FUZZ_ITERATION_COUNT);
    let json_path_ec_op = format!("cairo_programs/ec_op_{:?}.json", FUZZ_ITERATION_COUNT);
    let _ = fs::write(&cairo_path_ec_op, file_content.as_bytes());

    compile_program(&cairo_path_ec_op, &json_path_ec_op);

    let program_content_ec_op = std::fs::read(&json_path_ec_op).unwrap();

    // Run the program with default configurations
    cairo_run::cairo_run(&program_content_ec_op, cairo_run_config, hint_executor);

    // Remove files to save memory
    delete_files(&cairo_path_ec_op, &json_path_ec_op);
}

fn program_pedersen(
    num1: u128,
    num2: u128,
    cairo_run_config: &CairoRunConfig,
    hint_executor: &mut BuiltinHintProcessor,
) {
    let file_content = format!(
        "
    %builtins pedersen

    from starkware.cairo.common.cairo_builtins import HashBuiltin
    from starkware.cairo.common.hash import hash2
    
    func get_hash(hash_ptr: HashBuiltin*, num_a: felt, num_b: felt) -> (
        hash_ptr: HashBuiltin*, r: felt
    ) {{
        with hash_ptr {{
            let (result) = hash2(num_a, num_b);
        }}
        return (hash_ptr=hash_ptr, r=result);
    }}
    
    func builtins_wrapper{{
        pedersen_ptr: HashBuiltin*,
    }}(num_a: felt, num_b: felt) {{
        let (pedersen_ptr, result: felt) = get_hash(pedersen_ptr, num_a, num_b);
    
        return ();
    }}
    
    func builtins_wrapper_iter{{
        pedersen_ptr: HashBuiltin*,
    }}(num_a: felt, num_b: felt, n_iterations: felt) {{
        builtins_wrapper(num_a, num_b);
        if (n_iterations != 0) {{
            builtins_wrapper_iter(num_a, num_b, n_iterations - 1);
            tempvar pedersen_ptr = pedersen_ptr;
        }} else {{
            tempvar pedersen_ptr = pedersen_ptr;
        }}
    
        return ();
    }}
    
    func main{{
        pedersen_ptr: HashBuiltin*,
    }}() {{
        let num_a = {};
        let num_b = {};
        builtins_wrapper_iter(num_a, num_b, 50000);
    
        return ();
    }}
    ",
        num1, num2
    );

    // Create programs names and program
    let cairo_path_pedersen = format!("cairo_programs/pedersen_{:?}.cairo", FUZZ_ITERATION_COUNT);
    let json_path_pedersen = format!("cairo_programs/pedersen_{:?}.json", FUZZ_ITERATION_COUNT);
    let _ = fs::write(&cairo_path_pedersen, file_content.as_bytes());

    compile_program(&cairo_path_pedersen, &json_path_pedersen);

    let program_content_pedersen = std::fs::read(&json_path_pedersen).unwrap();

    // Run the program with default configurations
    cairo_run::cairo_run(&program_content_pedersen, cairo_run_config, hint_executor);

    // Remove files to save memory
    delete_files(&cairo_path_pedersen, &json_path_pedersen);
}

fn program_ecdsa(
    num1: u128,
    num2: u128,
    num3: u128,
    num4: u8,
    cairo_run_config: &CairoRunConfig,
    hint_executor: &mut BuiltinHintProcessor,
) {
    let file_content = format!(
        "
    %builtins ecdsa
    from starkware.cairo.common.serialize import serialize_word
    from starkware.cairo.common.cairo_builtins import SignatureBuiltin
    from starkware.cairo.common.signature import verify_ecdsa_signature
    
    func main{{ecdsa_ptr: SignatureBuiltin*}}() {{
        verify_ecdsa_signature(
            {},
            {},
            {},
            {},
        );
        return ();
    }}
    
    ",
        num4, num1, num2, num3
    );

    // Create programs names and program
    let cairo_path_ecdsa = format!("cairo_programs/ecdsa_{:?}.cairo", FUZZ_ITERATION_COUNT);
    let json_path_ecdsa = format!("cairo_programs/ecdsa_{:?}.json", FUZZ_ITERATION_COUNT);
    let _ = fs::write(&cairo_path_ecdsa, file_content.as_bytes());

    compile_program(&cairo_path_ecdsa, &json_path_ecdsa);

    let program_content_ecdsa = std::fs::read(&json_path_ecdsa).unwrap();

    // Run the program with default configurations
    cairo_run::cairo_run(&program_content_ecdsa, cairo_run_config, hint_executor);

    // Remove files to save memory
    delete_files(&cairo_path_ecdsa, &json_path_ecdsa);
}

fn compile_program(cairo_path: &str, json_path: &str) {
    let output = Command::new("cairo-compile")
        .arg(cairo_path)
        .arg("--output")
        .arg(json_path)
        .output()
        .expect("failed to execute process");
}

fn delete_files(cairo_path: &str, json_path: &str) {
    fs::remove_file(cairo_path);
    fs::remove_file(json_path);
}
