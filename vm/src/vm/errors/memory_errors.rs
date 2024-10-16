// The `(*.0).0` syntax of thiserror falsely triggers this clippy warning
#![allow(clippy::explicit_auto_deref)]

use crate::stdlib::prelude::*;
use crate::types::builtin_name::BuiltinName;

use thiserror_no_std::Error;

use crate::Felt252;

use crate::types::{
    errors::math_errors::MathError,
    relocatable::{MaybeRelocatable, Relocatable},
};

#[derive(Debug, PartialEq, Error)]
pub enum MemoryError {
    #[error(transparent)]
    Math(#[from] MathError),
    #[error(transparent)]
    InsufficientAllocatedCells(#[from] InsufficientAllocatedCellsError),
    #[error("Can't insert into segment #{}; memory only has {} segment", (*.0).0, (*.0).1)]
    UnallocatedSegment(Box<(usize, usize)>),
    #[error("Memory addresses must be relocatable")]
    AddressNotRelocatable,
    #[error("Range-check validation failed, number {} is out of valid range [0, {}]", (*.0).0, (*.0).1)]
    RangeCheckNumOutOfBounds(Box<(Felt252, Felt252)>),
    #[error("Range-check validation failed, encountered non-int value at address {0}")]
    RangeCheckFoundNonInt(Box<Relocatable>),
    #[error("Inconsistent memory assignment at address {:?}. {:?} != {:?}", (*.0).0, (*.0).1, (*.0).2)]
    InconsistentMemory(Box<(Relocatable, MaybeRelocatable, MaybeRelocatable)>),
    #[error("Inconsistent Relocation")]
    Relocation,
    #[error("Could not cast arguments")]
    WriteArg,
    #[error("Memory addresses mustn't be in a TemporarySegment, segment: {0}")]
    AddressInTemporarySegment(isize),
    #[error("Memory addresses must be in a TemporarySegment, segment: {0}")]
    AddressNotInTemporarySegment(isize),
    #[error("Temporary segment found while relocating (flattening), segment: {0}")]
    TemporarySegmentInRelocation(isize),
    #[error("The TemporarySegment: {0} doesn't have a relocation address")]
    NonZeroOffset(usize),
    #[error("Attempt to overwrite a relocation rule, segment: {0}")]
    DuplicatedRelocation(isize),
    #[error("Segment effective sizes haven't been calculated.")]
    MissingSegmentUsedSizes,
    #[error("Found a memory gap when calling get_continuous_range with base:{} and size: {}", (*.0).0, (*.0).1)]
    GetRangeMemoryGap(Box<(Relocatable, usize)>),
    #[error("Error calculating builtin memory units")]
    ErrorCalculatingMemoryUnits,
    #[error("Missing memory cells for {0}")]
    MissingMemoryCells(Box<BuiltinName>),
    #[error("Missing memory cells for {}: {:?}", (*.0).0, (*.0).1)]
    MissingMemoryCellsWithOffsets(Box<(BuiltinName, Vec<usize>)>),
    #[error("ErrorInitializing Verifying Key from public key: {0:?}")]
    InitializingVerifyingKey(Box<Vec<u8>>),
    #[error(
        "Signature {}, is invalid, with respect to the public key {}, 
    and the message hash {}.", (*.0).0, (*.0).1, (*.0).2
    )]
    InvalidSignature(Box<(String, Felt252, Felt252)>),
    #[error(
        "Signature hint is missing for ECDSA builtin at address {0}.
    Add it using 'ecdsa_builtin.add_signature'."
    )]
    SignatureNotFound(Box<Relocatable>),
    #[error("Could not create pubkey from: {0:?}")]
    ErrorParsingPubKey(Box<str>),
    #[error("Could not retrieve message from: {0:?}")]
    ErrorRetrievingMessage(Box<str>),
    #[error("Error verifying given signature")]
    ErrorVerifyingSignature,
    #[error("ECDSA builtin: Expected public key at address {0} to be an integer")]
    PubKeyNonInt(Box<Relocatable>),
    #[error("ECDSA builtin: Expected message hash at address {0} to be an integer")]
    MsgNonInt(Box<Relocatable>),
    #[error("Failed to fetch {} return values, ap is only {}", (*.0).0, (*.0).1)]
    FailedToGetReturnValues(Box<(usize, Relocatable)>),
    #[error("Segment {} has {} amount of accessed addresses but its size is only {}.", (*.0).0, (*.0).1, (*.0).2)]
    SegmentHasMoreAccessedAddressesThanSize(Box<(usize, usize, usize)>),
    #[error("gen_arg: found argument of invalid type.")]
    GenArgInvalidType,
    // Memory.get() errors
    #[error("Expected integer at address {0}")]
    ExpectedInteger(Box<Relocatable>),
    #[error("Expected relocatable at address {0}")]
    ExpectedRelocatable(Box<Relocatable>),
    #[error("Unknown memory cell at address {0}")]
    UnknownMemoryCell(Box<Relocatable>),
    // SegmentArenaBuiltin
    #[error("segment_arena_builtin: assert used >= INITIAL_SEGMENT_SIZE")]
    InvalidUsedSizeSegmentArena,
    #[error("Vector capacity exceeded")]
    VecCapacityExceeded,
    #[error("Memory wasn't relocated")]
    UnrelocatedMemory,
    #[error("Malformed public memory")]
    MalformedPublicMemory,
}

#[derive(Debug, PartialEq, Eq, Error)]
pub enum InsufficientAllocatedCellsError {
    #[error("Number of steps must be at least {} for the {}.", (*.0).0, (*.0).1)]
    MinStepNotReached(Box<(usize, BuiltinName)>),
    #[error("The {} used {} cells but the capacity is {}.", (*.0).0, (*.0).1, (*.0).2)]
    BuiltinCells(Box<(BuiltinName, usize, usize)>),
    #[error("There are only {} cells to fill the range checks holes, but potentially {} are required.", (*.0).0, (*.0).1)]
    RangeCheckUnits(Box<(usize, usize)>),
    #[error("There are only {} cells to fill the diluted check holes, but potentially {} are required.", (*.0).0, (*.0).1)]
    DilutedCells(Box<(usize, usize)>),
    #[error("There are only {} cells to fill the memory address holes, but {} are required.", (*.0).0, (*.0).1)]
    MemoryAddresses(Box<(u32, usize)>),
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    // Test to catch possible enum size regressions
    fn test_memory_error_size() {
        let size = crate::stdlib::mem::size_of::<MemoryError>();
        assert!(size <= 24, "{size}")
    }

    #[test]
    // Test to catch possible enum size regressions
    fn test_insufficient_allocated_cells_error_size() {
        let size = crate::stdlib::mem::size_of::<InsufficientAllocatedCellsError>();
        assert!(size <= 16, "{size}")
    }
}
