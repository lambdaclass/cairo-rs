use starknet_crypto::pedersen_hash;

use crate::Felt252;

use crate::stdlib::vec::Vec;
use crate::types::builtin_name::BuiltinName;
use crate::types::relocatable::MaybeRelocatable;
use crate::vm::runners::cairo_pie::StrippedProgram;

type HashFunction = fn(&Felt252, &Felt252) -> Felt252;

#[derive(thiserror_no_std::Error, Debug)]
pub enum HashChainError {
    #[error("Data array must contain at least one element.")]
    EmptyData,
}

#[derive(thiserror_no_std::Error, Debug)]
pub enum ProgramHashError {
    #[error(transparent)]
    HashChain(#[from] HashChainError),

    #[error(
        "Invalid program builtin: builtin name too long to be converted to field element: {0}"
    )]
    InvalidProgramBuiltin(&'static str),

    #[error("Invalid program data: data contains relocatable(s)")]
    InvalidProgramData,
}

/// Computes a hash chain over the data, in the following order:
///     h(data[0], h(data[1], h(..., h(data[n-2], data[n-1])))).
/// [cairo_lang reference](https://github.com/starkware-libs/cairo-lang/blob/efa9648f57568aad8f8a13fbf027d2de7c63c2c0/src/starkware/cairo/common/hash_chain.py#L6)

fn compute_hash_chain<'a, I>(data: I, hash_func: HashFunction) -> Result<Felt252, HashChainError>
where
    I: Iterator<Item = &'a Felt252> + DoubleEndedIterator,
{
    match data.copied().rev().reduce(|x, y| hash_func(&y, &x)) {
        Some(result) => Ok(result),
        None => Err(HashChainError::EmptyData),
    }
}

/// Creates an instance of `Felt252` from a builtin name.
///
/// Converts the builtin name to bytes then attempts to create a field element from
/// these bytes. This function will fail if the builtin name is over 31 characters.
fn builtin_name_to_field_element(builtin_name: &BuiltinName) -> Result<Felt252, ProgramHashError> {
    // The Python implementation uses the builtin name without suffix
    Ok(Felt252::from_bytes_be_slice(
        builtin_name.to_str().as_bytes(),
    ))
}

/// Converts a `MaybeRelocatable` into a `Felt252` value.
///
/// Returns `InvalidProgramData` if `maybe_relocatable` is not an integer
fn maybe_relocatable_to_field_element(
    maybe_relocatable: &MaybeRelocatable,
) -> Result<Felt252, ProgramHashError> {
    maybe_relocatable
        .get_int_ref()
        .copied()
        .ok_or(ProgramHashError::InvalidProgramData)
}

/// Computes the Pedersen hash of a program.
/// [(cairo_lang reference)](https://github.com/starkware-libs/cairo-lang/blob/efa9648f57568aad8f8a13fbf027d2de7c63c2c0/src/starkware/cairo/bootloaders/hash_program.py#L11)
pub fn compute_program_hash_chain(
    program: &StrippedProgram,
    bootloader_version: usize,
) -> Result<Felt252, ProgramHashError> {
    let program_main = program.main;
    let program_main = Felt252::from(program_main);

    // Convert builtin names to field elements
    let builtin_list: Result<Vec<Felt252>, _> = program
        .builtins
        .iter()
        .map(builtin_name_to_field_element)
        .collect();
    let builtin_list = builtin_list?;

    let program_header = vec![
        Felt252::from(bootloader_version),
        program_main,
        Felt252::from(program.builtins.len()),
    ];

    let program_data: Result<Vec<_>, _> = program
        .data
        .iter()
        .map(maybe_relocatable_to_field_element)
        .collect();
    let program_data = program_data?;

    let data_chain_len = program_header.len() + builtin_list.len() + program_data.len();
    let data_chain_len_vec = vec![Felt252::from(data_chain_len)];

    // Prepare a chain of iterators to feed to the hash function
    let data_chain = [
        &data_chain_len_vec,
        &program_header,
        &builtin_list,
        &program_data,
    ];

    let hash = compute_hash_chain(data_chain.iter().flat_map(|&v| v.iter()), pedersen_hash)?;
    Ok(hash)
}

#[cfg(test)]
mod tests {
    #[cfg(feature = "std")]
    use {crate::types::program::Program, rstest::rstest, std::path::PathBuf};

    use starknet_crypto::pedersen_hash;

    use super::*;

    #[test]
    fn test_compute_hash_chain() {
        let data: Vec<Felt252> = vec![
            Felt252::from(1u64),
            Felt252::from(2u64),
            Felt252::from(3u64),
        ];
        let expected_hash = pedersen_hash(
            &Felt252::from(1u64),
            &pedersen_hash(&Felt252::from(2u64), &Felt252::from(3u64)),
        );
        let computed_hash = compute_hash_chain(data.iter(), pedersen_hash)
            .expect("Hash computation failed unexpectedly");

        assert_eq!(computed_hash, expected_hash);
    }

    #[cfg(feature = "std")]
    #[rstest]
    // Expected hashes generated with `cairo-hash-program`
    #[case::fibonacci(
        "../cairo_programs/fibonacci.json",
        "0x43b17e9592f33142246af4c06cd2b574b460dd1f718d76b51341175a62b220f"
    )]
    #[case::field_arithmetic(
        "../cairo_programs/field_arithmetic.json",
        "0x1031772ca86e618b058101af9c9a3277bac90712b750bcea1cc69d6c7cad8a7"
    )]
    #[case::keccak_copy_inputs(
        "../cairo_programs/keccak_copy_inputs.json",
        "0x49484fdc8e7a85061f9f21b7e21fe276d8a88c8e96681101a2518809e686c6c"
    )]
    fn test_compute_program_hash_chain(
        #[case] program_path: PathBuf,
        #[case] expected_program_hash: String,
    ) {
        let program =
            Program::from_file(program_path.as_path(), Some("main"))
                .expect("Could not load program. Did you compile the sample programs? Run `make test` in the root directory.");
        let stripped_program = program.get_stripped_program().unwrap();
        let bootloader_version = 0;

        let program_hash = compute_program_hash_chain(&stripped_program, bootloader_version)
            .expect("Failed to compute program hash.");

        let program_hash_hex = format!("{:#x}", program_hash);

        assert_eq!(program_hash_hex, expected_program_hash);
    }
}
