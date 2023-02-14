use felt::Felt;
use thiserror::Error;

use crate::types::relocatable::{MaybeRelocatable, Relocatable};

#[derive(Debug, PartialEq, Eq, Error)]
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
    #[error("Temporary segment found while relocating (flattening), segment: {0}")]
    TemporarySegmentInRelocation(isize),
    #[error("The TemporarySegment: {0} doesn't have a relocation address")]
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
    #[error("ErrorInitializing Verifying Key from public key: {0:?}")]
    InitializingVerifyingKey(Vec<u8>),
    #[error(
        "Signature {0}, is invalid, with respect to the public key {1}, 
    and the message hash {2}."
    )]
    InvalidSignature(String, Felt, Felt),
    #[error(
        "Signature hint is missing for ECDSA builtin at address {0}.
    Add it using 'ecdsa_builtin.add_signature'."
    )]
    SignatureNotFound(Relocatable),
    #[error("Could not create pubkey from: {0:?}")]
    ErrorParsingPubKey(String),
    #[error("Could not retrieve message from: {0:?}")]
    ErrorRetrievingMessage(String),
    #[error("Error verifying given signature")]
    ErrorVerifyingSignature,
    #[error("Couldn't obtain a mutable accessed offset")]
    CantGetMutAccessedOffset,
    #[error("ECDSA builtin: Expected public key at address {0} to be an integer")]
    PubKeyNonInt(Relocatable),
    #[error("ECDSA builtin: Expected message hash at address {0} to be an integer")]
    MsgNonInt(Relocatable),
    #[error("Failed to convert String: {0} to FieldElement")]
    FailedStringToFieldElementConversion(String),
    #[error("Failed to fetch {0} return values, ap is only {1}")]
    FailedToGetReturnValues(usize, Relocatable),
}
