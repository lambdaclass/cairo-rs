// The `(*.0).0` syntax of thiserror falsely triggers this clippy warning
#![allow(clippy::explicit_auto_deref)]

use crate::stdlib::boxed::Box;
use crate::Felt252;
use num_bigint::{BigInt, BigUint};
use thiserror_no_std::Error;

use crate::types::relocatable::{MaybeRelocatable, Relocatable};

#[derive(Debug, Error, PartialEq)]
pub enum MathError {
    // Math functions
    #[error("{} is not divisible by {}", (*.0).0, (*.0).1)]
    SafeDivFail(Box<(Felt252, Felt252)>),
    #[error("{} is not divisible by {}", (*.0).0, (*.0).1)]
    SafeDivFailBigInt(Box<(BigInt, BigInt)>),
    #[error("{0} is not divisible by {1}")]
    SafeDivFailU32(u32, u32),
    #[error("{} is not divisible by {}", (*.0).0, (*.0).1)]
    SafeDivFailUsize(Box<(usize, usize)>),
    #[error("Attempted to divide by zero")]
    DividedByZero,
    #[error("Failed to calculate the square root of: {0})")]
    FailedToGetSqrt(Box<BigUint>),
    #[error("is_quad_residue: p must be > 0")]
    IsQuadResidueZeroPrime,
    // Relocatable Operations
    #[error("Cant convert felt: {0} to Relocatable")]
    Felt252ToRelocatable(Box<Felt252>),
    #[error("Operation failed: {} - {}, offsets cant be negative", (*.0).0, (*.0).1)]
    RelocatableSubFelt252NegOffset(Box<(Relocatable, Felt252)>),
    #[error("Operation failed: {} - {}, offsets cant be negative", (*.0).0, (*.0).1)]
    RelocatableSubUsizeNegOffset(Box<(Relocatable, usize)>),
    #[error("Operation failed: {} + {}, maximum offset value exceeded", (*.0).0, (*.0).1)]
    RelocatableAddFelt252OffsetExceeded(Box<(Relocatable, Felt252)>),
    #[error("Operation failed: {} + {}, maximum offset value exceeded", (*.0).0, (*.0).1)]
    RelocatableAddUsizeOffsetExceeded(Box<(Relocatable, usize)>),
    #[error("Operation failed: {} + {}, can't add two relocatable values", (*.0).0, (*.0).1)]
    RelocatableAdd(Box<(Relocatable, Relocatable)>),
    #[error("Operation failed: {} - {}, can't subtract two relocatable values with different segment indexes", (*.0).0, (*.0).1)]
    RelocatableSubDiffIndex(Box<(Relocatable, Relocatable)>),
    #[error(
        "Operation failed: {}.divmod({}, divmod can only be performed between two integer values", (*.0).0, (*.0).1
    )]
    DivModWrongType(Box<(MaybeRelocatable, MaybeRelocatable)>),
    #[error("Operation failed {} - {}, can't subtract a relocatable value from an integer", (*.0).0, (*.0).1)]
    SubRelocatableFromInt(Box<(Felt252, Relocatable)>),
    // Type conversions
    #[error("Conversion to i32 failed for Felt252 {0}")]
    Felt252ToI32Conversion(Box<Felt252>),
    #[error("Conversion to u32 failed for Felt252 {0}")]
    Felt252ToU32Conversion(Box<Felt252>),
    #[error("Conversion to usize failed for Felt252 {0}")]
    Felt252ToUsizeConversion(Box<Felt252>),
    #[error("Conversion to u64 failed for Felt252 {0}")]
    Felt252ToU64Conversion(Box<Felt252>),
    #[error("Byte conversion error")]
    ByteConversionError,
    #[error(
        "Operation failed: divmod({}, {}, {}), igcdex({}, {}) != 1 ", (*.0).0, (*.0).1, (*.0).2, (*.0).1, (*.0).2
    )]
    DivModIgcdexNotZero(Box<(BigInt, BigInt, BigInt)>),
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    // Test to catch possible enum size regressions
    fn test_math_error_size() {
        let size = crate::stdlib::mem::size_of::<MathError>();
        assert!(size <= 16, "{size}")
    }
}
