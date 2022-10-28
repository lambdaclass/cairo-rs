use crate::types::relocatable::{MaybeRelocatable, Relocatable};
use crate::vm::errors::memory_errors::MemoryError;
use crate::vm::errors::runner_errors::RunnerError;
use crate::vm::vm_core::VirtualMachine;
use crate::vm::vm_memory::memory::Memory;
use crate::vm::vm_memory::memory_segments::MemorySegmentManager;
use std::any::Any;

mod bitwise;
mod ec_op;
mod hash;
mod output;
mod range_check;

pub use bitwise::BitwiseBuiltinRunner;
pub use ec_op::EcOpBuiltinRunner;
pub use hash::HashBuiltinRunner;
pub use output::OutputBuiltinRunner;
pub use range_check::RangeCheckBuiltinRunner;

pub trait BuiltinRunner {
    ///Creates the necessary segments for the builtin in the MemorySegmentManager and stores the first address on the builtin's base
    fn initialize_segments(&mut self, segments: &mut MemorySegmentManager, memory: &mut Memory);
    fn initial_stack(&self) -> Vec<MaybeRelocatable>;
    ///Returns the builtin's base
    fn base(&self) -> Relocatable;
    fn add_validation_rule(&self, memory: &mut Memory) -> Result<(), RunnerError>;
    fn deduce_memory_cell(
        &mut self,
        address: &Relocatable,
        memory: &Memory,
    ) -> Result<Option<MaybeRelocatable>, RunnerError>;
    fn as_any(&self) -> &dyn Any;

    fn get_memory_accesses(&self, vm: &VirtualMachine) -> Result<Vec<Relocatable>, MemoryError>;
    fn get_used_cells(&self, vm: &VirtualMachine) -> Result<usize, MemoryError>;
    fn get_used_instances(&self, vm: &VirtualMachine) -> Result<usize, MemoryError>;
}
