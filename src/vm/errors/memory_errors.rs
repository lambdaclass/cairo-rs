use thiserror::Error;

use crate::types::relocatable::{MaybeRelocatable, Relocatable};

#[derive(Debug, PartialEq, Error)]
pub enum MemoryError {
    #[error("Can't insert into segment #{0}; memory only has {1} segment")]
    UnallocatedSegment(usize, usize),
    #[error("Memory addresses must be relocatable")]
    AddressNotRelocatable,
    #[error("Range-check validation failed, number is out of valid range")]
    NumOutOfBounds,
    #[error("Range-check validation failed, encountered non-int value")]
    FoundNonInt,
    #[error("Inconsistent memory assignment at address {0:?}. {1:?} != {2:?}")]
    InconsistentMemory(MaybeRelocatable, MaybeRelocatable, MaybeRelocatable),
    #[error("compute_effective_sizes should be called before relocate_segments")]
    EffectiveSizesNotCalled,
    #[error("Inconsistent Relocation")]
    Relocation,
    #[error("Could not cast arguments")]
    WriteArg,
    #[error("Memory addresses mustn't be in a TemporarySegment, segment: {0}")]
    AddressInTemporarySegment(isize),
    #[error("Memory addresses must be in a TemporarySegment, segment: {0}")]
    AddressNotInTemporarySegment(isize),
    #[error("Non-zero offset found where zero is required, offset: {0}")]
    TemporarySegmentWithoutRealocationAddreess(isize),
    #[error("The TemporarySegment: {0} does't have a realocation address")]
    NonZeroOffset(usize),
    #[error("Attempt to overwrite a relocation rule, segment: {0}")]
    DuplicatedRelocation(isize),
    #[error("accessed_addresses is None.")]
    MissingAccessedAddresses,
    #[error("Segment effective sizes haven't been calculated.")]
    MissingSegmentUsedSizes,
    #[error("Segment at index {0} either doesn't exist or is not finalized.")]
    SegmentNotFinalized(usize),
    #[error("Invalid memory value at address {0:?}: {1:?}")]
    InvalidMemoryValue(Relocatable, MaybeRelocatable),
    #[error("Found a memory gap when calling get_continuous_range")]
    GetRangeMemoryGap,
    #[error("Error calculating builtin memory units")]
    ErrorCalculatingMemoryUnits,
    #[error("Number of steps is insufficient in the builtin.")]
    InsufficientAllocatedCells,
    #[error("Missing memory cells for builtin {0}")]
    MissingMemoryCells(&'static str),
    #[error("Missing memory cells for builtin {0}: {1:?}")]
    MissingMemoryCellsWithOffsets(&'static str, Vec<usize>),
}
