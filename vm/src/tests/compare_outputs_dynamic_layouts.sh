#!/usr/bin/env bash
#
# Compares programs with different dynamic layouts against cairo-lang


# Build temporary dynamic layout params files
TEMP_FOLDER=$(mktemp -d)
cat <<EOF > "$TEMP_FOLDER/all_cairo.json"
{
    "rc_units": 4,
    "log_diluted_units_per_step": 4,
    "cpu_component_step": 8,
    "memory_units_per_step": 8,
    "uses_pedersen_builtin": true,
    "pedersen_ratio": 256,
    "uses_range_check_builtin": true,
    "range_check_ratio": 8,
    "uses_ecdsa_builtin": true,
    "ecdsa_ratio": 2048,
    "uses_bitwise_builtin": true,
    "bitwise_ratio": 16,
    "uses_ec_op_builtin": true,
    "ec_op_ratio": 1024,
    "uses_keccak_builtin": true,
    "keccak_ratio": 2048,
    "uses_poseidon_builtin": true,
    "poseidon_ratio": 256,
    "uses_range_check96_builtin": true,
    "range_check96_ratio": 8,
    "range_check96_ratio_den": 1,
    "uses_add_mod_builtin": true,
    "add_mod_ratio": 128,
    "add_mod_ratio_den": 1,
    "uses_mul_mod_builtin": true,
    "mul_mod_ratio": 256,
    "mul_mod_ratio_den": 1
}
EOF
cat <<EOF > "$TEMP_FOLDER/double_all_cairo.json"
{
    "rc_units": 8,
    "log_diluted_units_per_step": 8,
    "cpu_component_step": 16,
    "memory_units_per_step": 16,
    "uses_pedersen_builtin": true,
    "pedersen_ratio": 512,
    "uses_range_check_builtin": true,
    "range_check_ratio": 16,
    "uses_ecdsa_builtin": true,
    "ecdsa_ratio": 4096,
    "uses_bitwise_builtin": true,
    "bitwise_ratio": 32,
    "uses_ec_op_builtin": true,
    "ec_op_ratio": 2048,
    "uses_keccak_builtin": true,
    "keccak_ratio": 4096,
    "uses_poseidon_builtin": true,
    "poseidon_ratio": 512,
    "uses_range_check96_builtin": true,
    "range_check96_ratio": 16,
    "range_check96_ratio_den": 1,
    "uses_add_mod_builtin": true,
    "add_mod_ratio": 256,
    "add_mod_ratio_den": 1,
    "uses_mul_mod_builtin": true,
    "mul_mod_ratio": 512,
    "mul_mod_ratio_den": 1
}
EOF

cat <<EOF > "$TEMP_FOLDER/fractional_units_per_step.json"
{
    "rc_units": 4,
    "log_diluted_units_per_step": -2,
    "cpu_component_step": 8,
    "memory_units_per_step": 8,
    "uses_pedersen_builtin": false,
    "pedersen_ratio": 0,
    "uses_range_check_builtin": false,
    "range_check_ratio": 0,
    "uses_ecdsa_builtin": false,
    "ecdsa_ratio": 0,
    "uses_bitwise_builtin": false,
    "bitwise_ratio": 0,
    "uses_ec_op_builtin": false,
    "ec_op_ratio": 0,
    "uses_keccak_builtin": false,
    "keccak_ratio": 0,
    "uses_poseidon_builtin": false,
    "poseidon_ratio": 0,
    "uses_range_check96_builtin": false,
    "range_check96_ratio": 0,
    "range_check96_ratio_den": 1,
    "uses_add_mod_builtin": false,
    "add_mod_ratio": 0,
    "add_mod_ratio_den": 1,
    "uses_mul_mod_builtin": false,
    "mul_mod_ratio": 0,
    "mul_mod_ratio_den": 1
}
EOF

cat <<EOF > "$TEMP_FOLDER/ratio_den.json"
{
    "rc_units": 4,
    "log_diluted_units_per_step": 4,
    "cpu_component_step": 8,
    "memory_units_per_step": 512,
    "uses_pedersen_builtin": false,
    "pedersen_ratio": 0,
    "uses_range_check_builtin": false,
    "range_check_ratio": 0,
    "uses_ecdsa_builtin": false,
    "ecdsa_ratio": 0,
    "uses_bitwise_builtin": false,
    "bitwise_ratio": 0,
    "uses_ec_op_builtin": false,
    "ec_op_ratio": 0,
    "uses_keccak_builtin": false,
    "keccak_ratio": 0,
    "uses_poseidon_builtin": false,
    "poseidon_ratio": 0,
    "uses_range_check96_builtin": true,
    "range_check96_ratio": 1,
    "range_check96_ratio_den": 2,
    "uses_add_mod_builtin": true,
    "add_mod_ratio": 1,
    "add_mod_ratio_den": 2,
    "uses_mul_mod_builtin": true,
    "mul_mod_ratio": 1,
    "mul_mod_ratio_den": 2
}
EOF

# Build cases to execute
CASES=(
    "cairo_programs/proof_programs/factorial.json;all_cairo"
    "cairo_programs/proof_programs/factorial.json;double_all_cairo"
    "cairo_programs/proof_programs/fibonacci.json;all_cairo"
    "cairo_programs/proof_programs/fibonacci.json;double_all_cairo"
    "cairo_programs/proof_programs/bigint.json;all_cairo"
    "cairo_programs/proof_programs/bigint.json;double_all_cairo"
    "cairo_programs/proof_programs/dict.json;all_cairo"
    "cairo_programs/proof_programs/dict.json;double_all_cairo"
    "cairo_programs/proof_programs/sha256.json;all_cairo"
    "cairo_programs/proof_programs/sha256.json;double_all_cairo"
    "cairo_programs/proof_programs/keccak.json;all_cairo"
    "cairo_programs/proof_programs/keccak.json;double_all_cairo"
    # Mod builtin feature
    "cairo_programs/mod_builtin_feature/proof/mod_builtin.json;all_cairo"
    "cairo_programs/mod_builtin_feature/proof/mod_builtin_failure.json;all_cairo"
    "cairo_programs/mod_builtin_feature/proof/apply_poly.json;all_cairo"
    # Fractional units per step
    "cairo_programs/proof_programs/factorial.json;fractional_units_per_step"
    "cairo_programs/proof_programs/fibonacci.json;fractional_units_per_step"
    # Ratio den
    "cairo_programs/mod_builtin_feature/proof/mod_builtin.json;ratio_den"
    "cairo_programs/mod_builtin_feature/proof/mod_builtin_failure.json;ratio_den"
    "cairo_programs/mod_builtin_feature/proof/apply_poly.json;ratio_den"
)

# Build pie cases to execute
PIE_CASES=(
    "cairo_programs/fibonacci.rs.pie.zip;all_cairo"
    "cairo_programs/fibonacci.rs.pie.zip;double_all_cairo"
    "cairo_programs/factorial.rs.pie.zip;all_cairo"
    "cairo_programs/factorial.rs.pie.zip;double_all_cairo"
    "cairo_programs/bigint.rs.pie.zip;all_cairo"
    "cairo_programs/bigint.rs.pie.zip;double_all_cairo"
    "cairo_programs/dict.rs.pie.zip;all_cairo"
    "cairo_programs/dict.rs.pie.zip;double_all_cairo"
    "cairo_programs/sha256.rs.pie.zip;all_cairo"
    "cairo_programs/sha256.rs.pie.zip;double_all_cairo"
    "cairo_programs/keccak.rs.pie.zip;all_cairo"
    "cairo_programs/keccak.rs.pie.zip;double_all_cairo"
)

passed_tests=0
failed_tests=0
exit_code=0

for case in "${CASES[@]}"; do
    IFS=";" read -r program layout <<< "$case"
    
    full_program="$program"
    full_layout="$TEMP_FOLDER/$layout.json"

    # Run cairo-vm
    echo "Running cairo-vm with case: $case"
    cargo run -p cairo-vm-cli --features mod_builtin --release -- "$full_program" \
        --layout "dynamic" --cairo_layout_params_file "$full_layout" --proof_mode \
        --trace_file program_rs.trace --memory_file program_rs.memory --air_public_input program_rs.air_public_input --air_private_input program_rs.air_private_input

    # Run cairo-lang
    echo "Running cairo-lang with case: $case"
    cairo-run --program "$full_program" \
        --layout "dynamic" --cairo_layout_params_file "$full_layout" --proof_mode \
        --trace_file program_py.trace --memory_file program_py.memory --air_public_input program_py.air_public_input --air_private_input program_py.air_private_input

    # Compare trace
    echo "Running trace comparison for case: $case"
    if ! diff -q program_rs.trace program_py.trace; then
        echo "Trace differs for case: $case"
        exit_code=1
        failed_tests=$((failed_tests + 1))
    else
        passed_tests=$((passed_tests + 1))
    fi

    # Compare memory
    echo "Running memory comparison for case: $case"
    if ! ./vm/src/tests/memory_comparator.py program_rs.memory program_py.memory; then
        echo "Memory differs for case: $case"
        exit_code=1
        failed_tests=$((failed_tests + 1))
    else
        passed_tests=$((passed_tests + 1))
    fi

    # Compare air public input
    echo "Running air public input  comparison for case: $case"
    if ! ./vm/src/tests/air_public_input_comparator.py program_rs.air_public_input program_py.air_public_input; then
        echo "Air public input differs for case: $case"
        exit_code=1
        failed_tests=$((failed_tests + 1))
    else
        passed_tests=$((passed_tests + 1))
    fi

    # Compare air private input
    echo "Running air private input  comparison for case: $case"
    if ! ./vm/src/tests/air_private_input_comparator.py program_rs.air_private_input program_py.air_private_input; then
        echo "Air private input differs for case: $case"
        exit_code=1
        failed_tests=$((failed_tests + 1))
    else
        passed_tests=$((passed_tests + 1))
    fi

    # Clean files generated by the script
    echo "Cleaning files"
    rm program_rs.*
    rm program_py.*
done


for case in "${PIE_CASES[@]}"; do
    IFS=";" read -r program layout <<< "$case"

    full_program="$program"
    full_layout="$TEMP_FOLDER/$layout.json"

    # Run cairo-vm
    echo "Running cairo-vm with case: $case"
    cargo run -p cairo-vm-cli --features mod_builtin --release -- "$full_program" \
        --layout "dynamic" --cairo_layout_params_file "$full_layout" --run_from_cairo_pie  \
        --trace_file program_rs.trace --memory_file program_rs.memory

    # Run cairo-lang
    echo "Running cairo-lang with case: $case"
    cairo-run --run_from_cairo_pie "$full_program" \
        --layout "dynamic" --cairo_layout_params_file "$full_layout" \
        --trace_file program_py.trace --memory_file program_py.memory

    # Compare trace
    echo "Running trace comparison for case: $case"
    if ! diff -q program_rs.trace program_py.trace; then
        echo "Trace differs for case: $case"
        exit_code=1
        failed_tests=$((failed_tests + 1))
    else
        passed_tests=$((passed_tests + 1))
    fi

    # Compare memory
    echo "Running memory comparison for case: $case"
    if ! ./vm/src/tests/memory_comparator.py program_rs.memory program_py.memory; then
        echo "Memory differs for case: $case"
        exit_code=1
        failed_tests=$((failed_tests + 1))
    else
        passed_tests=$((passed_tests + 1))
    fi

    # Clean files generated by the script
    echo "Cleaning files"
    rm program_rs.*
    rm program_py.*
done

if test $failed_tests != 0; then
    echo "Comparisons: $failed_tests failed, $passed_tests passed, $((failed_tests + passed_tests)) total"
elif test $passed_tests = 0; then
    echo "No tests ran!"
    exit_code=2
else
    echo "All $passed_tests tests passed; no discrepancies found"
fi

exit "${exit_code}"
