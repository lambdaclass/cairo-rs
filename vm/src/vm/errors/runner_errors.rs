// The `(*.0).0` syntax of thiserror falsely triggers this clippy warning
#![allow(clippy::explicit_auto_deref)]

use crate::stdlib::{collections::HashSet, prelude::*};
use crate::types::builtin_name::BuiltinName;
use crate::types::layout_name::LayoutName;
use thiserror_no_std::Error;

use super::{memory_errors::MemoryError, trace_errors::TraceError};
use crate::types::{errors::math_errors::MathError, relocatable::Relocatable};
use crate::Felt252;

#[derive(Debug, PartialEq, Error)]
pub enum RunnerError {
    #[error("Initialization failure: No execution base")]
    NoExecBase,
    #[error("Initialization failure: No program base")]
    NoProgBase,
    #[error("Missing main()")]
    MissingMain,
    #[error("Found None PC during VM initialization")]
    NoPC,
    #[error("Found None AP during VM initialization")]
    NoAP,
    #[error("Found None FP during VM initialization")]
    NoFP,
    #[error("Memory validation failed during VM initialization: {0}")]
    MemoryValidationError(MemoryError),
    #[error("Memory loading failed during state initialization: {0}")]
    MemoryInitializationError(MemoryError),
    #[error("Failed to convert string to FieldElement")]
    FailedStringConversion,
    #[error("EcOpBuiltin: m should be at most {0}")]
    EcOpBuiltinScalarLimit(Box<Felt252>),
    #[error("Given builtins are not in appropiate order")]
    DisorderedBuiltins,
    #[error("Expected integer at address {:?} to be smaller than 2^{}, Got {}", (*.0).0, (*.0).1, (*.0).2)]
    IntegerBiggerThanPowerOfTwo(Box<(Relocatable, u32, Felt252)>),
    #[error("{0}")]
    EcOpSameXCoordinate(Box<str>),
    #[error("EcOpBuiltin: point {0:?} is not on the curve")]
    PointNotOnCurve(Box<(Felt252, Felt252)>),
    #[error("Builtin(s) {:?} not present in layout {}", (*.0).0, (*.0).1)]
    NoBuiltinForInstance(Box<(HashSet<BuiltinName>, LayoutName)>),
    #[error("end_run called twice.")]
    EndRunCalledTwice,
    #[error("end_run must be called before finalize_segments.")]
    FinalizeNoEndRun,
    #[error("end_run must be called before read_return_values.")]
    ReadReturnValuesNoEndRun,
    #[error("Error while finalizing segments: {0}")]
    FinalizeSegements(MemoryError),
    #[error("finalize_segments called but proof_mode is not enabled")]
    FinalizeSegmentsNoProofMode,
    #[error("Invalid stop pointer for {}: Stop pointer has value {} but builtin segment is {}", (*.0).0, (*.0).1, (*.0).2)]
    InvalidStopPointerIndex(Box<(BuiltinName, Relocatable, usize)>),
    #[error("Invalid stop pointer for {}. Expected: {}, found: {}", (*.0).0, (*.0).1, (*.0).2)]
    InvalidStopPointer(Box<(BuiltinName, Relocatable, Relocatable)>),
    #[error("No stop pointer found for builtin {0}")]
    NoStopPointer(Box<BuiltinName>),
    #[error("Running in proof-mode but no __start__ label found, try compiling with proof-mode")]
    NoProgramStart,
    #[error("Running in proof-mode but no __end__ label found, try compiling with proof-mode")]
    NoProgramEnd,
    #[error("Could not convert slice to array")]
    SliceToArrayError,
    #[error("Cannot add the return values to the public memory after segment finalization.")]
    FailedAddingReturnValues,
    #[error("Missing execution public memory")]
    NoExecPublicMemory,
    #[error("Coulnd't parse prime from felt lib")]
    CouldntParsePrime,
    #[error("Could not convert vec with Maybe Relocatables into u64 array")]
    MaybeRelocVecToU64ArrayError,
    #[error("Expected Integer value, got Relocatable instead")]
    FoundNonInt,
    #[error(transparent)]
    Memory(#[from] MemoryError),
    #[error(transparent)]
    Math(#[from] MathError),
    #[error("keccak_builtin: Failed to get first input address")]
    KeccakNoFirstInput,
    #[error("{}: Expected integer at address {}", (*.0).0, (*.0).1)]
    BuiltinExpectedInteger(Box<(BuiltinName, Relocatable)>),
    #[error("keccak_builtin: Failed to convert input cells to u64 values")]
    KeccakInputCellsNotU64,
    #[error("Unexpected ret_fp_segment size")]
    UnexpectedRetFpSegmentSize,
    #[error("Unexpected ret_pc_segment size")]
    UnexpectedRetPcSegmentSize,
    #[error("Expected program base offset to be zero")]
    ProgramBaseOffsetNotZero,
    #[error("Expected execution base offset to be zero")]
    ExecBaseOffsetNotZero,
    #[error("Expected ret_fp offset to be zero")]
    RetFpOffsetNotZero,
    #[error("Expected ret_pc offset to be zero")]
    RetPcOffsetNotZero,
    #[error("Can't build a StrippedProgram from a Program without main")]
    StrippedProgramNoMain,
    #[error(transparent)]
    Trace(#[from] TraceError),
    #[error("EcOp builtin: Invalid Point")]
    InvalidPoint,
    #[error("Page ({0}) is not on the expected segment {1}")]
    PageNotOnSegment(Relocatable, usize),
    #[error("Expected integer at address {} to be smaller than 2^{}. Got: {}.", (*.0).0, (*.0).1, (*.0).2)]
    WordExceedsModBuiltinWordBitLen(Box<(Relocatable, u32, Felt252)>),
    #[error("{}: Expected n >= 1. Got: {}.", (*.0).0, (*.0).1)]
    ModBuiltinNLessThanOne(Box<(BuiltinName, usize)>),
    #[error("{}: Missing value at address {}.", (*.0).0, (*.0).1)]
    ModBuiltinMissingValue(Box<(BuiltinName, Relocatable)>),
    #[error("{}: n must be <= {}", (*.0).0, (*.0).1)]
    FillMemoryMaxExceeded(Box<(BuiltinName, usize)>),
    #[error("{0}: write_n_words value must be 0 after loop")]
    WriteNWordsValueNotZero(BuiltinName),
    #[error("add_mod and mul_mod builtins must have the same n_words and word_bit_len.")]
    ModBuiltinsMismatchedInstanceDef,
    #[error("At least one of add_mod and mul_mod must be given.")]
    FillMemoryNoBuiltinSet,
    #[error("Could not fill the values table, add_mod_index={0}, mul_mod_index={1}")]
    FillMemoryCoudNotFillTable(usize, usize),
    #[error("{}: {}", (*.0).0, (*.0).1)]
    ModBuiltinSecurityCheck(Box<(BuiltinName, String)>),
    #[error("{0} is missing")]
    MissingBuiltin(BuiltinName),
    #[error("The stop pointer of the missing builtin {0} must be 0")]
    MissingBuiltinStopPtrNotZero(BuiltinName),
    #[error("The number of steps in the Cairo PIE's execution resources does not match the number of steps in the RunResources")]
    PieNStepsVsRunResourcesNStepsMismatch,
    #[error("A Cairo PIE can not be ran in proof_mode")]
    CairoPieProofMode,
    #[error("{0}: Invalid additional data")]
    InvalidAdditionalData(BuiltinName),
    #[error("dynamic layout params is missing")]
    MissingDynamicLayoutParams,
    #[error("dynamic layout {0} ratio should be 0 when disabled")]
    BadDynamicLayoutBuiltinRatio(BuiltinName),
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    // Test to catch possible enum size regressions
    fn test_runner_error_size() {
        let size = crate::stdlib::mem::size_of::<RunnerError>();
        assert!(size <= 32, "{size}")
    }
}
