use crate::stdlib::prelude::*;
use crate::utils::PRIME_STR;
use thiserror_no_std::Error;

#[derive(Debug, Error)]
pub enum ProgramError {
    #[cfg(feature = "std")]
    #[error(transparent)]
    IO(#[from] std::io::Error),
    #[error(transparent)]
    Parse(#[from] serde_json::Error),
    #[error("The \"{0}\" operation is not supported")]
    OperationNotSupported(String),
    #[error("Entrypoint {0} not found")]
    EntrypointNotFound(String),
    #[error("Constant {0} has no value")]
    ConstWithoutValue(String),
    #[error("Expected prime {PRIME_STR}, got {0}")]
    PrimeDiffers(String),
    #[error("Can't build a StrippedProgram from a Program without main")]
    StrippedProgramNoMain,
    #[error("Hint PC ({0}) is greater or equal to program length ({1})")]
    InvalidHintPc(usize, usize),
}

#[cfg(test)]
mod tests {
    use super::*;

    #[cfg(target_arch = "wasm32")]
    use wasm_bindgen_test::*;

    #[test]
    #[cfg_attr(target_arch = "wasm32", wasm_bindgen_test)]
    fn format_entrypoint_not_found_error() {
        let error = ProgramError::EntrypointNotFound(String::from("my_function"));
        let formatted_error = format!("{error}");
        assert_eq!(formatted_error, "Entrypoint my_function not found");
    }
}
