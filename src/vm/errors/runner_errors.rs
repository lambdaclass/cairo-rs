use std::collections::HashSet;

use super::memory_errors::MemoryError;
use crate::types::relocatable::MaybeRelocatable;
use num_bigint::BigInt;
use thiserror::Error;

#[derive(Debug, PartialEq, Error)]
pub enum RunnerError {
    #[error("Can't initialize state without an execution base")]
    NoExecBase,
    #[error("Can't initialize the function entrypoint without an execution base")]
    NoExecBaseForEntrypoint,
    #[error("Initialization failure: No program base")]
    NoProgBase,
    #[error("Missing main()")]
    MissingMain,
    #[error("Uninitialized base for builtin")]
    UninitializedBase,
    #[error("Base for builtin is not finished")]
    BaseNotFinished,
    #[error("Failed to write program output")]
    WriteFail,
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
    #[error("Memory addresses must be relocatable")]
    NonRelocatableAddress,
    #[error("Runner base mustn't be in a TemporarySegment, segment: {0}")]
    RunnerInTemporarySegment(isize),
    #[error("Failed to convert string to FieldElement")]
    FailedStringConversion,
    #[error("Expected integer at address {0:?}")]
    ExpectedInteger(MaybeRelocatable),
    #[error("Failed to retrieve value from address {0:?}")]
    MemoryGet(MaybeRelocatable),
    #[error(transparent)]
    FailedMemoryGet(MemoryError),
    #[error("EcOpBuiltin: m should be at most {0}")]
    EcOpBuiltinScalarLimit(BigInt),
    #[error("Given builtins are not in appropiate order")]
    DisorderedBuiltins,
    #[error("Expected integer at address {0:?} to be smaller than 2^{1}, Got {2}")]
    IntegerBiggerThanPowerOfTwo(MaybeRelocatable, u32, BigInt),
    #[error(
        "Cannot apply EC operation: computation reched two points with the same x coordinate. \n
    Attempting to compute P + m * Q where:\n
    P = {0:?} \n
    m = {1}\n
    Q = {2:?}."
    )]
    EcOpSameXCoordinate((BigInt, BigInt), BigInt, (BigInt, BigInt)),
    #[error("EcOpBuiltin: point {0:?} is not on the curve")]
    PointNotOnCurve((usize, usize)),
    #[error("Builtin(s) {0:?} not present in layout {1}")]
    NoBuiltinForInstance(HashSet<String>, String),
    #[error("Invalid layout {0}")]
    InvalidLayoutName(String),
    #[error("Run has already ended.")]
    RunAlreadyFinished,
    #[error("Run must be ended before calling finalize_segments.")]
    FinalizeNoEndRun,
    #[error("Builtin {0} not included.")]
    BuiltinNotIncluded(String),
    #[error("Builtin segment name collision on '{0}'")]
    BuiltinSegmentNameCollision(&'static str),
    #[error("Error while finalizing segments: {0}")]
    FinalizeSegements(MemoryError),
    #[error("Finalize_segments called but proof_mode is not enabled")]
    FinalizeSegmentsNoProofMode,
    #[error("Final stack error")]
    FinalStack,
    #[error("Invalid stop pointer for {0} ")]
    InvalidStopPointer(String),
    #[error("Running in proof-mode but no __start__ label found, try compiling with proof-mode")]
    NoProgramStart,
    #[error("Running in proof-mode but no __end__ label found, try compiling with proof-mode")]
    NoProgramEnd,
    #[error("Missing builtin: {0}")]
    MissingBuiltin(String),
    #[error("Cannot add the return values to the public memory after segment finalization.")]
    FailedAddingReturnValues,
    #[error("Missing execution public memory")]
    NoExecPublicMemory,
}
