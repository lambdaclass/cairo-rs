// The `(*.0).0` syntax of thiserror falsely triggers this clippy warning
#![allow(clippy::explicit_auto_deref)]

use crate::stdlib::prelude::*;

use thiserror_no_std::Error;

use crate::Felt252;
use num_bigint::{BigInt, BigUint};

use crate::types::{
    errors::math_errors::MathError,
    relocatable::{MaybeRelocatable, Relocatable},
};

use super::{
    exec_scope_errors::ExecScopeError, memory_errors::MemoryError, vm_errors::VirtualMachineError,
};

// For more info on #[error] syntax, see https://docs.rs/thiserror/latest/thiserror/#details
#[derive(Debug, Error)]
pub enum HintError {
    #[error(transparent)]
    FromScopeError(#[from] ExecScopeError),
    #[error(transparent)]
    Internal(#[from] VirtualMachineError),
    #[error(transparent)]
    Memory(#[from] MemoryError),
    #[error(transparent)]
    Math(#[from] MathError),
    #[error("HintProcessor failed retrieve the compiled data necessary for hint execution")]
    WrongHintData,
    #[error("Unknown identifier {0}")]
    UnknownIdentifier(Box<str>),
    #[error("Expected ids.{} to be an Integer value", (*.0))]
    IdentifierNotInteger(Box<str>),
    #[error("Expected ids.{} to be a Relocatable value", (*.0))]
    IdentifierNotRelocatable(Box<str>),
    #[error("ids.{} has no member {} or it is of incorrect type", (*.0).0, (*.0).1)]
    IdentifierHasNoMember(Box<(String, String)>),
    #[error("Unknown identifier")]
    UnknownIdentifierInternal,
    #[error("Wrong identifier type")]
    WrongIdentifierTypeInternal,
    #[error("Hint Error: {0}")]
    CustomHint(Box<str>),
    #[error("Missing constant: {0}")]
    MissingConstant(Box<&'static str>),
    #[error("Fail to get constants for hint execution")]
    FailedToGetConstant,
    #[error("Arc too big, {} must be <= {} and {} <= {}", (*.0).0, (*.0).1, (*.0).2, (*.0).3)]
    ArcTooBig(Box<(Felt252, Felt252, Felt252, Felt252)>),
    #[error("Excluded is supposed to be 2, got {0}")]
    ExcludedNot2(Box<Felt252>),
    #[error("Value: {0} is outside of the range [0, 2**250)")]
    ValueOutside250BitRange(Box<Felt252>),
    #[error("Failed to get scope variables")]
    ScopeError,
    #[error("Variable {0} not present in current execution scope")]
    VariableNotInScopeError(Box<str>),
    #[error("DictManagerError: Tried to create tracker for a dictionary on segment: {0} when there is already a tracker for a dictionary on this segment")]
    CantCreateDictionaryOnTakenSegment(isize),
    #[error("Dict Error: No dict tracker found for segment {0}")]
    NoDictTracker(isize),
    #[error("Dict Error: No value found for key: {0}")]
    NoValueForKey(Box<MaybeRelocatable>),
    #[error("find_element(): No value found for key: {0}")]
    NoValueForKeyFindElement(Box<Felt252>),
    #[error("Assertion failed, a = {} % PRIME is not less than b = {} % PRIME", (*.0).0, (*.0).1)]
    AssertLtFelt252(Box<(Felt252, Felt252)>),
    #[error("find_element() can only be used with n_elms <= {}.\nGot: n_elms = {}", (*.0).0, (*.0).1)]
    FindElemMaxSize(Box<(Felt252, Felt252)>),
    #[error(
        "Invalid index found in find_element_index. Index: {}.\nExpected key: {}, found_key {}", (*.0).0, (*.0).1, (*.0).2
    )]
    InvalidIndex(Box<(Felt252, Felt252, Felt252)>),
    #[error("Found Key is None")]
    KeyNotFound,
    #[error("AP tracking data is None; could not apply correction to address")]
    NoneApTrackingData,
    #[error("Tracking groups should be the same, got {} and {}", (*.0).0, (*.0).0)]
    InvalidTrackingGroup(Box<(usize, usize)>),
    #[error("Expected relocatable for ap, got {0}")]
    InvalidApValue(Box<MaybeRelocatable>),
    #[error("Dict Error: Tried to create a dict without an initial dict")]
    NoInitialDict,
    #[error("squash_dict_inner fail: couldnt find key {0} in accesses_indices")]
    NoKeyInAccessIndices(Box<Felt252>),
    #[error("squash_dict_inner fail: local accessed_indices is empty")]
    EmptyAccessIndices,
    #[error("squash_dict_inner fail: local current_accessed_indices is empty")]
    EmptyCurrentAccessIndices,
    #[error("squash_dict_inner fail: local current_accessed_indices not empty, loop ended with remaining unaccounted elements")]
    CurrentAccessIndicesNotEmpty,
    #[error("Dict Error: Got the wrong value for dict_update, expected value: {}, got: {} for key: {}", (*.0).0, (*.0).1, (*.0).2)]
    WrongPrevValue(Box<(MaybeRelocatable, MaybeRelocatable, MaybeRelocatable)>),
    #[error("squash_dict_inner fail: Number of used accesses:{} doesnt match the lengh: {} of the access_indices at key: {}", (*.0).0, (*.0).1, (*.0).2)]
    NumUsedAccessesAssertFail(Box<(Felt252, usize, Felt252)>),
    #[error("squash_dict_inner fail: local keys is not empty")]
    KeysNotEmpty,
    #[error("squash_dict_inner fail: No keys left but remaining_accesses > 0")]
    EmptyKeys,
    #[error("squash_dict fail: Accesses array size must be divisible by DictAccess.SIZE")]
    PtrDiffNotDivisibleByDictAccessSize,
    #[error("squash_dict() can only be used with n_accesses<={}. ' \nGot: n_accesses={}", (*.0).0, (*.0).1)]
    SquashDictMaxSizeExceeded(Box<(Felt252, Felt252)>),
    #[error("squash_dict fail: n_accesses: {0} is too big to be converted into an iterator")]
    NAccessesTooBig(Box<Felt252>),
    #[error("Couldn't convert BigInt to usize")]
    BigintToUsizeFail,
    #[error("usort() can only be used with input_len<={}. Got: input_len={}.", (*.0).0, (*.0).1)]
    UsortOutOfRange(Box<(u64, Felt252)>),
    #[error("unexpected usort fail: positions_dict or key value pair not found")]
    UnexpectedPositionsDictFail,
    #[error("unexpected verify multiplicity fail: positions not found")]
    PositionsNotFound,
    #[error("unexpected verify multiplicity fail: positions length != 0")]
    PositionsLengthNotZero,
    #[error("unexpected verify multiplicity fail: couldn't pop positions")]
    CouldntPopPositions,
    #[error("unexpected verify multiplicity fail: last_pos not found")]
    LastPosNotFound,
    #[error("Set's starting point {} is bigger it's ending point {}", (*.0).0, (*.0).1)]
    InvalidSetRange(Box<(Relocatable, Relocatable)>),
    #[error("Failed to construct a fixed size array of size: {0}")]
    FixedSizeArrayFail(usize),
    #[error("{0}")]
    AssertionFailed(Box<str>),
    #[error("Wrong dict pointer supplied. Got {}, expected {}.", (*.0).0, (*.0).1)]
    MismatchedDictPtr(Box<(Relocatable, Relocatable)>),
    #[error("Integer must be postive or zero, got: {0}")]
    SecpSplitNegative(Box<BigInt>),
    #[error("Integer: {0} out of range")]
    SecpSplitOutOfRange(Box<BigUint>),
    #[error("verify_zero: Invalid input {0}")]
    SecpVerifyZero(Box<BigInt>),
    #[error("unsafe_keccak() can only be used with length<={}. Got: length={}", (*.0).0, (*.0).1)]
    KeccakMaxSize(Box<(Felt252, Felt252)>),
    #[error("Invalid word size: {0}")]
    InvalidWordSize(Box<Felt252>),
    #[error("Invalid input length, Got: length={0}")]
    InvalidKeccakInputLength(Box<Felt252>),
    #[error("assert_not_equal failed: {} =  {}", (*.0).0, (*.0).1)]
    AssertNotEqualFail(Box<(MaybeRelocatable, MaybeRelocatable)>),
    #[error("split_int(): value is out of range")]
    SplitIntNotZero,
    #[error("split_int(): Limb {0} is out of range.")]
    SplitIntLimbOutOfRange(Box<Felt252>),
    #[error("Expected size to be in the range from [0, 100), got: {0}")]
    InvalidKeccakStateSizeFelt252s(Box<Felt252>),
    #[error("Expected size to be in range from [0, 10), got: {0}")]
    InvalidBlockSize(Box<Felt252>),
    #[error("Couldn't convert BigInt to u32")]
    BigintToU32Fail,
    #[error("BigInt to BigUint failed, BigInt is negative")]
    BigIntToBigUintFail,
    #[error("BigUint to BigInt failed")]
    BigUintToBigIntFail,
    #[error("Assertion failed, 0 <= ids.a % PRIME < range_check_builtin.bound \n a = {0} is out of range")]
    ValueOutOfRange(Box<Felt252>),
    #[error("Assertion failed, 0 <= ids.a % PRIME < range_check_builtin.bound \n a = {0} is out of range")]
    AssertNNValueOutOfRange(Box<Felt252>),
    #[error("Assertion failed, {} % {} is equal to 0", (*.0).0, (*.0).1)]
    AssertNotZero(Box<(Felt252, String)>),
    #[error("Div out of range: 0 < {} <= {}", (*.0).0, (*.0).1)]
    OutOfValidRange(Box<(Felt252, Felt252)>),
    #[error("Value: {0} is outside valid range")]
    ValueOutsideValidRange(Box<Felt252>),
    #[error("Assertion failed, {}, is not less or equal to {}", (*.0).0, (*.0).1)]
    NonLeFelt252(Box<(Felt252, Felt252)>),
    #[error("Unknown Hint: {0}")]
    UnknownHint(Box<str>),
    #[error("Signature hint must point to the signature builtin segment, not {0}.")]
    AddSignatureWrongEcdsaPtr(Box<Relocatable>),
    #[error("Signature hint must point to the public key cell, not {0}.")]
    AddSignatureNotAPublicKey(Box<Relocatable>),
    #[error("random_ec_point: Could not find a point on the curve.")]
    RandomEcPointNotOnCurve,
    #[error("Invalid value for len. Got: {0}.")]
    InvalidLenValue(Box<Felt252>),
    #[error("recover_y: {0} does not represent the x coordinate of a point on the curve.")]
    RecoverYPointNotOnCurve(Box<Felt252>),
    #[error("Invalid value for {}. Got: {}. Expected: {}", (*.0).0, (*.0).1, (*.0).2)]
    InvalidValue(Box<(&'static str, Felt252, Felt252)>),
    #[error("Attempt to subtract with overflow: ids.m - 1")]
    NPairBitsTooLowM,
    #[error("{0}")]
    SyscallError(Box<str>),
    #[error("excess_balance_func: Failed to fetch {0} dictionary. It is either missing or has unexpected data types")]
    ExcessBalanceFailedToFecthDict(Box<str>),
    #[error("excess_balance_func: Key not found in {0} dictionary")]
    ExcessBalanceKeyError(Box<str>),
    #[error("excess_balance_func: Failed to calculate {0}")]
    ExcessBalanceCalculationFailed(Box<str>),
    #[error("generate_nibbles fail: tried to read from an empty list of nibbles")]
    EmptyNibbles,
    #[error("circuit evalution: {0}")]
    CircuitEvaluationFailed(Box<str>),
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn test_multiple_members_variant_message_format() {
        let a = Felt252::from(42);
        let b = Felt252::from(53);
        let string = "test";

        let error_msg = HintError::InvalidValue(Box::new((string, a, b))).to_string();

        let expected_msg = format!("Invalid value for {string}. Got: {a}. Expected: {b}");
        assert_eq!(error_msg, expected_msg)
    }

    #[test]
    fn test_single_felt_variant_message_format() {
        let x = Felt252::from(15131);

        let error_msg = HintError::InvalidKeccakStateSizeFelt252s(Box::new(x)).to_string();

        let expected_msg = format!("Expected size to be in the range from [0, 100), got: {x}");
        assert_eq!(error_msg, expected_msg)
    }

    #[test]
    fn test_hint_error_size() {
        let size = crate::stdlib::mem::size_of::<HintError>();
        assert!(size <= 32, "{size}")
    }
}
