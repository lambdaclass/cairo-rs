// The `(*.0).0` syntax of thiserror falsely triggers this clippy warning
#![allow(clippy::explicit_auto_deref)]

use crate::stdlib::prelude::*;
use crate::types::builtin_name::BuiltinName;

use thiserror_no_std::Error;

use crate::{
    types::{
        errors::math_errors::MathError,
        relocatable::{MaybeRelocatable, Relocatable},
    },
    vm::errors::{
        exec_scope_errors::ExecScopeError, hint_errors::HintError, memory_errors::MemoryError,
        runner_errors::RunnerError, trace_errors::TraceError,
    },
};

pub const HINT_ERROR_STR: &str = "Got an exception while executing a hint: ";

#[derive(Debug, Error)]
pub enum VirtualMachineError {
    #[error(transparent)]
    RunnerError(#[from] RunnerError),
    #[error(transparent)]
    Memory(#[from] MemoryError),
    #[error(transparent)]
    Math(#[from] MathError),
    #[error(transparent)]
    TracerError(#[from] TraceError),
    #[error(transparent)]
    MainScopeError(#[from] ExecScopeError),
    #[error(transparent)]
    Other(anyhow::Error),
    #[error("Instruction MSB should be 0")]
    InstructionNonZeroHighBit,
    #[error("Instruction should be an int")]
    InvalidInstructionEncoding,
    #[error("Invalid op1_register value: {0}")]
    InvalidOp1Reg(u64),
    #[error("In immediate mode, off2 should be 1")]
    ImmShouldBe1,
    #[error("op0 must be known in double dereference")]
    UnknownOp0,
    #[error("Invalid ap_update value: {0}")]
    InvalidApUpdate(u64),
    #[error("Invalid pc_update value: {0}")]
    InvalidPcUpdate(u64),
    #[error("Res.UNCONSTRAINED cannot be used with ApUpdate.ADD")]
    UnconstrainedResAdd,
    #[error("Res.UNCONSTRAINED cannot be used with PcUpdate.JUMP")]
    UnconstrainedResJump,
    #[error("Res.UNCONSTRAINED cannot be used with PcUpdate.JUMP_REL")]
    UnconstrainedResJumpRel,
    #[error("Res.UNCONSTRAINED cannot be used with Opcode.ASSERT_EQ")]
    UnconstrainedResAssertEq,
    #[error("A relocatable value as Res cannot be used with PcUpdate.JUMP_REL")]
    JumpRelNotInt,
    #[error(
        "Failed to compute Res.MUL: Could not complete computation of non pure values {} * {}", (*.0).0, (*.0).1
    )]
    ComputeResRelocatableMul(Box<(MaybeRelocatable, MaybeRelocatable)>),
    #[error("Couldn't compute operand {}. Unknown value for memory cell {}", (*.0).0, (*.0).1)]
    FailedToComputeOperands(Box<(String, Relocatable)>),
    #[error("An ASSERT_EQ instruction failed: {} != {}.", (*.0).0, (*.0).1)]
    DiffAssertValues(Box<(MaybeRelocatable, MaybeRelocatable)>),
    #[error("Call failed to write return-pc (inconsistent op0): {} != {}. Did you forget to increment ap?", (*.0).0, (*.0).1)]
    CantWriteReturnPc(Box<(MaybeRelocatable, MaybeRelocatable)>),
    #[error("Call failed to write return-fp (inconsistent dst): {} != {}. Did you forget to increment ap?", (*.0).0, (*.0).1)]
    CantWriteReturnFp(Box<(MaybeRelocatable, MaybeRelocatable)>),
    #[error("Couldn't get or load dst")]
    NoDst,
    #[error("Invalid res value: {0}")]
    InvalidRes(u64),
    #[error("Invalid opcode value: {0}")]
    InvalidOpcode(u64),
    #[error("Inconsistent auto-deduction for {}, expected {}, got {:?}", (*.0).0, (*.0).1, (*.0).2)]
    InconsistentAutoDeduction(Box<(BuiltinName, MaybeRelocatable, Option<MaybeRelocatable>)>),
    #[error("Expected output builtin to be present")]
    NoOutputBuiltin,
    #[error("Expected range_check builtin to be present")]
    NoRangeCheckBuiltin,
    #[error("Expected ecdsa builtin to be present")]
    NoSignatureBuiltin,
    #[error("Expected {0} to be present")]
    NoModBuiltin(BuiltinName),
    #[error("Failed to compare {} and {}, cant compare a relocatable to an integer value", (*.0).0, (*.0).1)]
    DiffTypeComparison(Box<(MaybeRelocatable, MaybeRelocatable)>),
    #[error("Failed to compare {} and  {}, cant compare two relocatable values of different segment indexes", (*.0).0, (*.0).1)]
    DiffIndexComp(Box<(Relocatable, Relocatable)>),
    #[error("Expected integer, found: {0:?}")]
    ExpectedIntAtRange(Box<Option<MaybeRelocatable>>),
    #[error("Could not convert slice to array")]
    SliceToArrayError,
    #[error("Failed to compile hint: {0}")]
    CompileHintFail(Box<str>),
    #[error("op1_addr is Op1Addr.IMM, but no immediate was given")]
    NoImm,
    #[error("Execution reached the end of the program. Requested remaining steps: {0}.")]
    EndOfProgram(usize),
    #[error("Could not reach the end of the program. RunResources has no remaining steps.")]
    UnfinishedExecution,
    #[error("Current run is not finished")]
    RunNotFinished,
    #[error("{HINT_ERROR_STR}{}", (*.0).1)]
    Hint(Box<(usize, HintError)>),
    #[error("Unexpected Failure")]
    Unexpected,
    #[error("Out of bounds access to builtin segment")]
    OutOfBoundsBuiltinSegmentAccess,
    #[error("Out of bounds access to program segment")]
    OutOfBoundsProgramSegmentAccess,
    #[error("Security Error: Invalid Memory Value: temporary address not relocated: {0}")]
    InvalidMemoryValueTemporaryAddress(Box<Relocatable>),
    #[error("accessed_addresses is None.")]
    MissingAccessedAddresses,
    #[error("Failed to write the output builtin content")]
    FailedToWriteOutput,
    #[error("Failed to find index {0} in the vm's relocation table")]
    RelocationNotFound(usize),
    #[error("{} batch size is not {}", (*.0).0, (*.0).1)]
    ModBuiltinBatchSize(Box<(BuiltinName, usize)>),
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    // Test to catch possible enum size regressions
    fn test_vm_error_size() {
        let size = crate::stdlib::mem::size_of::<VirtualMachineError>();
        assert!(size <= 32, "{size}")
    }
}
