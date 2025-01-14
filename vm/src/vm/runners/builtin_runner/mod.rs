use crate::air_private_input::PrivateInput;
use crate::math_utils::safe_div_usize;
use crate::stdlib::prelude::*;
use crate::types::builtin_name::BuiltinName;
use crate::types::instance_definitions::bitwise_instance_def::{
    CELLS_PER_BITWISE, INPUT_CELLS_PER_BITWISE,
};
use crate::types::instance_definitions::builtins_instance_def::BUILTIN_INSTANCES_PER_COMPONENT;
use crate::types::instance_definitions::ec_op_instance_def::{
    CELLS_PER_EC_OP, INPUT_CELLS_PER_EC_OP,
};
use crate::types::instance_definitions::ecdsa_instance_def::CELLS_PER_SIGNATURE;
use crate::types::instance_definitions::keccak_instance_def::{
    CELLS_PER_KECCAK, INPUT_CELLS_PER_KECCAK, KECCAK_INSTANCES_PER_COMPONENT,
};
use crate::types::instance_definitions::mod_instance_def::CELLS_PER_MOD;
use crate::types::instance_definitions::pedersen_instance_def::{
    CELLS_PER_HASH, INPUT_CELLS_PER_HASH,
};
use crate::types::instance_definitions::poseidon_instance_def::{
    CELLS_PER_POSEIDON, INPUT_CELLS_PER_POSEIDON,
};
use crate::types::instance_definitions::range_check_instance_def::CELLS_PER_RANGE_CHECK;
use crate::types::relocatable::{MaybeRelocatable, Relocatable};
use crate::vm::errors::memory_errors::{self, InsufficientAllocatedCellsError, MemoryError};
use crate::vm::errors::runner_errors::RunnerError;
use crate::vm::errors::vm_errors::VirtualMachineError;
use crate::vm::vm_core::VirtualMachine;
use crate::vm::vm_memory::memory::Memory;
use crate::vm::vm_memory::memory_segments::MemorySegmentManager;

mod bitwise;
mod ec_op;
mod hash;
mod keccak;
mod modulo;
mod output;
mod poseidon;
mod range_check;
mod segment_arena;
mod signature;

pub use self::keccak::KeccakBuiltinRunner;
pub(crate) use self::range_check::{RC_N_PARTS_96, RC_N_PARTS_STANDARD};
use self::segment_arena::ARENA_BUILTIN_SIZE;
pub use bitwise::BitwiseBuiltinRunner;
pub use ec_op::EcOpBuiltinRunner;
pub use hash::HashBuiltinRunner;
pub use modulo::ModBuiltinRunner;
use num_integer::{div_ceil, div_floor};
pub use output::{OutputBuiltinRunner, OutputBuiltinState};
pub use poseidon::PoseidonBuiltinRunner;
pub use range_check::RangeCheckBuiltinRunner;
pub use segment_arena::SegmentArenaBuiltinRunner;
pub use signature::SignatureBuiltinRunner;

use super::cairo_pie::BuiltinAdditionalData;

/* NB: this enum is no accident: we may need (and cairo-vm-py *does* need)
 * structs containing this to be `Send`. The only two ways to achieve that
 * are either storing a `dyn Trait` inside an `Arc<Mutex<&dyn Trait>>` or
 * making the type itself `Send`. We opted for not complicating the user nor
 * moving the guarantees to runtime by using an `enum` rather than a `Trait`.
 * This works under the assumption that we don't expect downstream users to
 * extend Cairo by adding new builtin runners.
 */
#[derive(Debug, Clone)]
pub enum BuiltinRunner {
    Bitwise(BitwiseBuiltinRunner),
    EcOp(EcOpBuiltinRunner),
    Hash(HashBuiltinRunner),
    Output(OutputBuiltinRunner),
    RangeCheck(RangeCheckBuiltinRunner<RC_N_PARTS_STANDARD>),
    RangeCheck96(RangeCheckBuiltinRunner<RC_N_PARTS_96>),
    Keccak(KeccakBuiltinRunner),
    Signature(SignatureBuiltinRunner),
    Poseidon(PoseidonBuiltinRunner),
    SegmentArena(SegmentArenaBuiltinRunner),
    Mod(ModBuiltinRunner),
}

impl BuiltinRunner {
    ///Creates the necessary segments for the builtin in the MemorySegmentManager and stores the first address on the builtin's base
    pub fn initialize_segments(&mut self, segments: &mut MemorySegmentManager) {
        match *self {
            BuiltinRunner::Bitwise(ref mut bitwise) => bitwise.initialize_segments(segments),
            BuiltinRunner::EcOp(ref mut ec) => ec.initialize_segments(segments),
            BuiltinRunner::Hash(ref mut hash) => hash.initialize_segments(segments),
            BuiltinRunner::Output(ref mut output) => output.initialize_segments(segments),
            BuiltinRunner::RangeCheck(ref mut range_check) => {
                range_check.initialize_segments(segments)
            }
            BuiltinRunner::RangeCheck96(ref mut range_check) => {
                range_check.initialize_segments(segments)
            }
            BuiltinRunner::Keccak(ref mut keccak) => keccak.initialize_segments(segments),
            BuiltinRunner::Signature(ref mut signature) => signature.initialize_segments(segments),
            BuiltinRunner::Poseidon(ref mut poseidon) => poseidon.initialize_segments(segments),
            BuiltinRunner::SegmentArena(ref mut segment_arena) => {
                segment_arena.initialize_segments(segments)
            }
            BuiltinRunner::Mod(ref mut modulo) => modulo.initialize_segments(segments),
        }
    }

    pub fn initial_stack(&self) -> Vec<MaybeRelocatable> {
        match *self {
            BuiltinRunner::Bitwise(ref bitwise) => bitwise.initial_stack(),
            BuiltinRunner::EcOp(ref ec) => ec.initial_stack(),
            BuiltinRunner::Hash(ref hash) => hash.initial_stack(),
            BuiltinRunner::Output(ref output) => output.initial_stack(),
            BuiltinRunner::RangeCheck(ref range_check) => range_check.initial_stack(),
            BuiltinRunner::RangeCheck96(ref range_check) => range_check.initial_stack(),
            BuiltinRunner::Keccak(ref keccak) => keccak.initial_stack(),
            BuiltinRunner::Signature(ref signature) => signature.initial_stack(),
            BuiltinRunner::Poseidon(ref poseidon) => poseidon.initial_stack(),
            BuiltinRunner::SegmentArena(ref segment_arena) => segment_arena.initial_stack(),
            BuiltinRunner::Mod(ref modulo) => modulo.initial_stack(),
        }
    }

    ///Returns the builtin's final stack
    pub fn final_stack(
        &mut self,
        segments: &MemorySegmentManager,
        pointer: Relocatable,
    ) -> Result<Relocatable, RunnerError> {
        if let BuiltinRunner::Output(output) = self {
            return output.final_stack(segments, pointer);
        }
        if self.included() {
            let stop_pointer_addr =
                (pointer - 1).map_err(|_| RunnerError::NoStopPointer(Box::new(self.name())))?;
            let stop_pointer = segments
                .memory
                .get_relocatable(stop_pointer_addr)
                .map_err(|_| RunnerError::NoStopPointer(Box::new(self.name())))?;
            if self.base() as isize != stop_pointer.segment_index {
                return Err(RunnerError::InvalidStopPointerIndex(Box::new((
                    self.name(),
                    stop_pointer,
                    self.base(),
                ))));
            }
            let stop_ptr = stop_pointer.offset;
            let mut num_instances = self.get_used_instances(segments)?;
            if matches!(self, BuiltinRunner::SegmentArena(_)) {
                // SegmentArena builtin starts with one instance pre-loaded
                // This is reflected in the builtin base's offset, but as we compare `stop_ptr.offset` agains `used`
                // instead of comparing `stop_ptr` against `base + used` we need to account for the base offset (aka the pre-loaded instance) here
                num_instances += 1;
            }
            let used = num_instances * self.cells_per_instance() as usize;
            if stop_ptr != used {
                return Err(RunnerError::InvalidStopPointer(Box::new((
                    self.name(),
                    Relocatable::from((self.base() as isize, used)),
                    Relocatable::from((self.base() as isize, stop_ptr)),
                ))));
            }
            self.set_stop_ptr(stop_ptr);
            Ok(stop_pointer_addr)
        } else {
            self.set_stop_ptr(0);
            Ok(pointer)
        }
    }

    ///Returns the builtin's allocated memory units
    pub fn get_allocated_memory_units(
        &self,
        vm: &VirtualMachine,
    ) -> Result<usize, memory_errors::MemoryError> {
        Ok(self.get_allocated_instances(vm)? * self.cells_per_instance() as usize)
    }

    ///Returns the builtin's allocated instances
    pub fn get_allocated_instances(
        &self,
        vm: &VirtualMachine,
    ) -> Result<usize, memory_errors::MemoryError> {
        match *self {
            BuiltinRunner::Output(_) | BuiltinRunner::SegmentArena(_) => Ok(0),
            _ => {
                match self.ratio() {
                    None => {
                        // Dynamic layout has the exact number of instances it needs (up to a power of 2).
                        let instances: usize =
                            self.get_used_cells(&vm.segments)? / self.cells_per_instance() as usize;
                        let needed_components = instances / self.instances_per_component() as usize;

                        let components = if needed_components > 0 {
                            needed_components.next_power_of_two()
                        } else {
                            0
                        };
                        Ok(self.instances_per_component() as usize * components)
                    }
                    // Dynamic layout allows for builtins with ratio 0
                    Some(0) => Ok(0),
                    Some(ratio) => {
                        // if we disable trace padding, we don't need to check the min_step,
                        // since we won't have the guarantee of number of steps being padded to
                        // accommodate each builtin size.
                        if !vm.disable_trace_padding {
                            let min_step_num = (ratio * self.instances_per_component()) as usize;
                            let min_step = if let Some(ratio_den) = self.ratio_den() {
                                div_ceil(min_step_num, ratio_den as usize)
                            } else {
                                min_step_num
                            };

                            if vm.current_step < min_step {
                                return Err(InsufficientAllocatedCellsError::MinStepNotReached(
                                    Box::new((min_step, self.name())),
                                )
                                .into());
                            };
                        }
                        let ratio_den = self.ratio_den().unwrap_or(1);
                        let numerator = vm.current_step * ratio_den as usize;

                        let allocated_instances = if vm.disable_trace_padding {
                            // if no trace padding is used, we can't guarantee that the number of
                            // instances is a multiple of the ratio, so we round up so the size will
                            // be enough (more than used instances). This is fine, since in this
                            // case we don't send this into a proof under the assumption of
                            // n_steps being padded to accommodate each builtin size.
                            div_ceil(numerator, ratio as usize)
                        } else {
                            safe_div_usize(numerator, ratio as usize)
                                .map_err(|_| MemoryError::ErrorCalculatingMemoryUnits)?
                        };

                        Ok(allocated_instances)
                    }
                }
            }
        }
    }

    /// Returns if the builtin is included in the program builtins
    fn included(&self) -> bool {
        match *self {
            BuiltinRunner::Bitwise(ref bitwise) => bitwise.included,
            BuiltinRunner::EcOp(ref ec) => ec.included,
            BuiltinRunner::Hash(ref hash) => hash.included,
            BuiltinRunner::Output(ref output) => output.included,
            BuiltinRunner::RangeCheck(ref range_check) => range_check.included,
            BuiltinRunner::RangeCheck96(ref range_check) => range_check.included,
            BuiltinRunner::Keccak(ref keccak) => keccak.included,
            BuiltinRunner::Signature(ref signature) => signature.included,
            BuiltinRunner::Poseidon(ref poseidon) => poseidon.included,
            BuiltinRunner::SegmentArena(ref segment_arena) => segment_arena.included,
            BuiltinRunner::Mod(ref modulo) => modulo.included,
        }
    }

    ///Returns the builtin's base
    pub fn base(&self) -> usize {
        match *self {
            BuiltinRunner::Bitwise(ref bitwise) => bitwise.base(),
            BuiltinRunner::EcOp(ref ec) => ec.base(),
            BuiltinRunner::Hash(ref hash) => hash.base(),
            BuiltinRunner::Output(ref output) => output.base(),
            BuiltinRunner::RangeCheck(ref range_check) => range_check.base(),
            BuiltinRunner::RangeCheck96(ref range_check) => range_check.base(),
            BuiltinRunner::Keccak(ref keccak) => keccak.base(),
            BuiltinRunner::Signature(ref signature) => signature.base(),
            BuiltinRunner::Poseidon(ref poseidon) => poseidon.base(),
            //Warning, returns only the segment index, base offset will be 3
            BuiltinRunner::SegmentArena(ref segment_arena) => segment_arena.base(),
            BuiltinRunner::Mod(ref modulo) => modulo.base(),
        }
    }

    pub fn ratio(&self) -> Option<u32> {
        match self {
            BuiltinRunner::Bitwise(bitwise) => bitwise.ratio(),
            BuiltinRunner::EcOp(ec) => ec.ratio(),
            BuiltinRunner::Hash(hash) => hash.ratio(),
            BuiltinRunner::Output(_) | BuiltinRunner::SegmentArena(_) => None,
            BuiltinRunner::RangeCheck(range_check) => range_check.ratio(),
            BuiltinRunner::RangeCheck96(range_check) => range_check.ratio(),
            BuiltinRunner::Keccak(keccak) => keccak.ratio(),
            BuiltinRunner::Signature(ref signature) => signature.ratio(),
            BuiltinRunner::Poseidon(poseidon) => poseidon.ratio(),
            BuiltinRunner::Mod(ref modulo) => modulo.ratio(),
        }
    }

    pub fn ratio_den(&self) -> Option<u32> {
        match self {
            BuiltinRunner::RangeCheck(range_check) => range_check.ratio_den(),
            BuiltinRunner::RangeCheck96(range_check) => range_check.ratio_den(),
            BuiltinRunner::Mod(modulo) => modulo.ratio_den(),
            _ => None,
        }
    }

    pub fn add_validation_rule(&self, memory: &mut Memory) {
        match *self {
            BuiltinRunner::RangeCheck(ref range_check) => range_check.add_validation_rule(memory),
            BuiltinRunner::RangeCheck96(ref range_check) => range_check.add_validation_rule(memory),
            BuiltinRunner::Signature(ref signature) => signature.add_validation_rule(memory),
            BuiltinRunner::Poseidon(ref poseidon) => poseidon.add_validation_rule(memory),
            _ => {}
        }
    }

    pub fn deduce_memory_cell(
        &self,
        address: Relocatable,
        memory: &Memory,
    ) -> Result<Option<MaybeRelocatable>, RunnerError> {
        match *self {
            BuiltinRunner::Bitwise(ref bitwise) => bitwise.deduce_memory_cell(address, memory),
            BuiltinRunner::EcOp(ref ec) => ec.deduce_memory_cell(address, memory),
            BuiltinRunner::Hash(ref hash) => hash.deduce_memory_cell(address, memory),
            BuiltinRunner::Keccak(ref keccak) => keccak.deduce_memory_cell(address, memory),
            BuiltinRunner::Poseidon(ref poseidon) => poseidon.deduce_memory_cell(address, memory),
            _ => Ok(None),
        }
    }

    pub fn get_memory_segment_addresses(&self) -> (usize, Option<usize>) {
        (self.base(), self.stop_ptr())
    }

    pub fn get_used_cells(&self, segments: &MemorySegmentManager) -> Result<usize, MemoryError> {
        match self {
            BuiltinRunner::Bitwise(ref bitwise) => bitwise.get_used_cells(segments),
            BuiltinRunner::EcOp(ref ec) => ec.get_used_cells(segments),
            BuiltinRunner::Hash(ref hash) => hash.get_used_cells(segments),
            BuiltinRunner::Output(ref output) => output.get_used_cells(segments),
            BuiltinRunner::RangeCheck(ref range_check) => range_check.get_used_cells(segments),
            BuiltinRunner::RangeCheck96(ref range_check) => range_check.get_used_cells(segments),
            BuiltinRunner::Keccak(ref keccak) => keccak.get_used_cells(segments),
            BuiltinRunner::Signature(ref signature) => signature.get_used_cells(segments),
            BuiltinRunner::Poseidon(ref poseidon) => poseidon.get_used_cells(segments),
            BuiltinRunner::SegmentArena(ref segment_arena) => {
                segment_arena.get_used_cells(segments)
            }
            BuiltinRunner::Mod(ref modulo) => modulo.get_used_cells(segments),
        }
    }

    pub fn get_used_instances(
        &self,
        segments: &MemorySegmentManager,
    ) -> Result<usize, MemoryError> {
        match self {
            BuiltinRunner::Bitwise(ref bitwise) => bitwise.get_used_instances(segments),
            BuiltinRunner::EcOp(ref ec) => ec.get_used_instances(segments),
            BuiltinRunner::Hash(ref hash) => hash.get_used_instances(segments),
            BuiltinRunner::Output(ref output) => output.get_used_instances(segments),
            BuiltinRunner::RangeCheck(ref range_check) => range_check.get_used_instances(segments),
            BuiltinRunner::RangeCheck96(ref range_check) => {
                range_check.get_used_instances(segments)
            }
            BuiltinRunner::Keccak(ref keccak) => keccak.get_used_instances(segments),
            BuiltinRunner::Signature(ref signature) => signature.get_used_instances(segments),
            BuiltinRunner::Poseidon(ref poseidon) => poseidon.get_used_instances(segments),
            BuiltinRunner::SegmentArena(ref segment_arena) => {
                segment_arena.get_used_instances(segments)
            }
            BuiltinRunner::Mod(modulo) => modulo.get_used_instances(segments),
        }
    }

    pub fn get_range_check_usage(&self, memory: &Memory) -> Option<(usize, usize)> {
        match self {
            BuiltinRunner::RangeCheck(ref range_check) => range_check.get_range_check_usage(memory),
            BuiltinRunner::RangeCheck96(ref range_check) => {
                range_check.get_range_check_usage(memory)
            }
            _ => None,
        }
    }

    /// Returns the number of range check units used by the builtin.
    pub fn get_used_perm_range_check_units(
        &self,
        vm: &VirtualMachine,
    ) -> Result<usize, MemoryError> {
        match self {
            BuiltinRunner::RangeCheck(range_check) => {
                let (used_cells, _) = self.get_used_cells_and_allocated_size(vm)?;
                Ok(used_cells * range_check.n_parts() as usize)
            }
            BuiltinRunner::RangeCheck96(range_check) => {
                let (used_cells, _) = self.get_used_cells_and_allocated_size(vm)?;
                Ok(used_cells * range_check.n_parts() as usize)
            }
            _ => Ok(0),
        }
    }

    pub fn get_used_diluted_check_units(&self, diluted_spacing: u32, diluted_n_bits: u32) -> usize {
        match self {
            BuiltinRunner::Bitwise(ref bitwise) => {
                bitwise.get_used_diluted_check_units(diluted_spacing, diluted_n_bits)
            }
            BuiltinRunner::Keccak(ref keccak) => {
                keccak.get_used_diluted_check_units(diluted_n_bits)
            }
            _ => 0,
        }
    }

    fn cells_per_instance(&self) -> u32 {
        match self {
            BuiltinRunner::Bitwise(_) => CELLS_PER_BITWISE,
            BuiltinRunner::EcOp(_) => CELLS_PER_EC_OP,
            BuiltinRunner::Hash(_) => CELLS_PER_HASH,
            BuiltinRunner::RangeCheck(_) | BuiltinRunner::RangeCheck96(_) => CELLS_PER_RANGE_CHECK,
            BuiltinRunner::Output(_) => 0,
            BuiltinRunner::Keccak(_) => CELLS_PER_KECCAK,
            BuiltinRunner::Signature(_) => CELLS_PER_SIGNATURE,
            BuiltinRunner::Poseidon(_) => CELLS_PER_POSEIDON,
            BuiltinRunner::SegmentArena(_) => ARENA_BUILTIN_SIZE,
            BuiltinRunner::Mod(_) => CELLS_PER_MOD,
        }
    }

    fn n_input_cells(&self) -> u32 {
        match self {
            BuiltinRunner::Bitwise(_) => INPUT_CELLS_PER_BITWISE,
            BuiltinRunner::EcOp(_) => INPUT_CELLS_PER_EC_OP,
            BuiltinRunner::Hash(_) => INPUT_CELLS_PER_HASH,
            BuiltinRunner::RangeCheck(_) | BuiltinRunner::RangeCheck96(_) => CELLS_PER_RANGE_CHECK,
            BuiltinRunner::Output(_) => 0,
            BuiltinRunner::Keccak(_) => INPUT_CELLS_PER_KECCAK,
            BuiltinRunner::Signature(_) => CELLS_PER_SIGNATURE,
            BuiltinRunner::Poseidon(_) => INPUT_CELLS_PER_POSEIDON,
            BuiltinRunner::SegmentArena(_) => ARENA_BUILTIN_SIZE,
            BuiltinRunner::Mod(_) => CELLS_PER_MOD,
        }
    }

    fn instances_per_component(&self) -> u32 {
        match self {
            BuiltinRunner::Keccak(_) => KECCAK_INSTANCES_PER_COMPONENT,
            _ => BUILTIN_INSTANCES_PER_COMPONENT,
        }
    }

    pub fn name(&self) -> BuiltinName {
        match self {
            BuiltinRunner::Bitwise(_) => BuiltinName::bitwise,
            BuiltinRunner::EcOp(_) => BuiltinName::ec_op,
            BuiltinRunner::Hash(_) => BuiltinName::pedersen,
            BuiltinRunner::RangeCheck(_) => BuiltinName::range_check,
            BuiltinRunner::RangeCheck96(_) => BuiltinName::range_check96,
            BuiltinRunner::Output(_) => BuiltinName::output,
            BuiltinRunner::Keccak(_) => BuiltinName::keccak,
            BuiltinRunner::Signature(_) => BuiltinName::ecdsa,
            BuiltinRunner::Poseidon(_) => BuiltinName::poseidon,
            BuiltinRunner::SegmentArena(_) => BuiltinName::segment_arena,
            BuiltinRunner::Mod(b) => b.name(),
        }
    }

    pub fn run_security_checks(&self, vm: &VirtualMachine) -> Result<(), VirtualMachineError> {
        if let BuiltinRunner::Output(_) | BuiltinRunner::SegmentArena(_) = self {
            return Ok(());
        }
        if let BuiltinRunner::Mod(modulo) = self {
            modulo.run_additional_security_checks(vm)?;
        }
        let cells_per_instance = self.cells_per_instance() as usize;
        let n_input_cells = self.n_input_cells() as usize;
        let builtin_segment_index = self.base();
        // If the builtin's segment is empty, there are no security checks to run
        let builtin_segment = match vm.segments.memory.data.get(builtin_segment_index) {
            Some(segment) if !segment.is_empty() => segment,
            _ => return Ok(()),
        };
        // The builtin segment's size - 1 is the maximum offset within the segment's addresses
        // Assumption: The last element is not a None value
        // It is safe to asume this for normal program execution
        // If there are trailing None values at the end, the following security checks will fail
        let offset_max = builtin_segment.len().saturating_sub(1);
        // offset_len is the amount of non-None values in the segment
        let offset_len = builtin_segment.iter().filter(|x| x.is_some()).count();
        let n = match offset_len {
            0 => 0,
            _ => div_floor(offset_max, cells_per_instance) + 1,
        };
        // Verify that n is not too large to make sure the expected_offsets set that is constructed
        // below is not too large.
        if n > div_floor(offset_len, n_input_cells) {
            return Err(MemoryError::MissingMemoryCells(Box::new(self.name())).into());
        }
        // Check that the two inputs (x and y) of each instance are set.
        let mut missing_offsets = Vec::with_capacity(n);
        // Check for missing expected offsets (either their address is no present, or their value is None)
        for i in 0..n {
            for j in 0..n_input_cells {
                let offset = cells_per_instance * i + j;
                if builtin_segment
                    .get(offset)
                    .filter(|x| x.is_some())
                    .is_none()
                {
                    missing_offsets.push(offset)
                }
            }
        }
        if !missing_offsets.is_empty() {
            return Err(MemoryError::MissingMemoryCellsWithOffsets(Box::new((
                self.name(),
                missing_offsets,
            )))
            .into());
        }
        // Verify auto deduction rules for the unasigned output cells
        // Assigned output cells are checked as part of the call to verify_auto_deductions().
        for i in 0..n {
            for j in n_input_cells..cells_per_instance {
                let offset = cells_per_instance * i + j;
                if builtin_segment
                    .get(offset)
                    .filter(|x| x.is_some())
                    .is_none()
                {
                    vm.verify_auto_deductions_for_addr(
                        Relocatable::from((builtin_segment_index as isize, offset)),
                        self,
                    )?;
                }
            }
        }
        Ok(())
    }

    pub fn get_used_cells_and_allocated_size(
        &self,
        vm: &VirtualMachine,
    ) -> Result<(usize, usize), MemoryError> {
        match self {
            BuiltinRunner::Output(_) | BuiltinRunner::SegmentArena(_) => {
                let used = self.get_used_cells(&vm.segments)?;
                Ok((used, used))
            }
            _ => {
                let used = self.get_used_cells(&vm.segments)?;
                let size = self.get_allocated_memory_units(vm)?;
                if used > size {
                    return Err(InsufficientAllocatedCellsError::BuiltinCells(Box::new((
                        self.name(),
                        used,
                        size,
                    )))
                    .into());
                }
                Ok((used, size))
            }
        }
    }

    /// Returns data stored internally by builtins needed to re-execute from a cairo pie
    pub fn get_additional_data(&self) -> BuiltinAdditionalData {
        match self {
            BuiltinRunner::Hash(builtin) => builtin.get_additional_data(),
            BuiltinRunner::Output(builtin) => builtin.get_additional_data(),
            BuiltinRunner::Signature(builtin) => builtin.get_additional_data(),
            _ => BuiltinAdditionalData::None,
        }
    }

    /// Extends the builtin's internal data with the internal data obtained from a previous cairo execution
    /// Used solely when running from a cairo pie
    pub fn extend_additional_data(
        &mut self,
        additional_data: &BuiltinAdditionalData,
    ) -> Result<(), RunnerError> {
        match self {
            BuiltinRunner::Hash(builtin) => builtin.extend_additional_data(additional_data),
            BuiltinRunner::Output(builtin) => builtin.extend_additional_data(additional_data),
            BuiltinRunner::Signature(builtin) => builtin.extend_additional_data(additional_data),
            _ => Ok(()),
        }
    }

    // Returns information about the builtin that should be added to the AIR private input.
    pub fn air_private_input(&self, segments: &MemorySegmentManager) -> Vec<PrivateInput> {
        match self {
            BuiltinRunner::RangeCheck(builtin) => builtin.air_private_input(&segments.memory),
            BuiltinRunner::RangeCheck96(builtin) => builtin.air_private_input(&segments.memory),
            BuiltinRunner::Bitwise(builtin) => builtin.air_private_input(&segments.memory),
            BuiltinRunner::Hash(builtin) => builtin.air_private_input(&segments.memory),
            BuiltinRunner::EcOp(builtin) => builtin.air_private_input(&segments.memory),
            BuiltinRunner::Poseidon(builtin) => builtin.air_private_input(&segments.memory),
            BuiltinRunner::Signature(builtin) => builtin.air_private_input(&segments.memory),
            BuiltinRunner::Keccak(builtin) => builtin.air_private_input(&segments.memory),
            BuiltinRunner::Mod(builtin) => builtin.air_private_input(segments),
            _ => vec![],
        }
    }

    pub(crate) fn set_stop_ptr(&mut self, stop_ptr: usize) {
        match self {
            BuiltinRunner::Bitwise(ref mut bitwise) => bitwise.stop_ptr = Some(stop_ptr),
            BuiltinRunner::EcOp(ref mut ec) => ec.stop_ptr = Some(stop_ptr),
            BuiltinRunner::Hash(ref mut hash) => hash.stop_ptr = Some(stop_ptr),
            BuiltinRunner::Output(ref mut output) => output.stop_ptr = Some(stop_ptr),
            BuiltinRunner::RangeCheck(ref mut range_check) => range_check.stop_ptr = Some(stop_ptr),
            BuiltinRunner::RangeCheck96(ref mut range_check) => {
                range_check.stop_ptr = Some(stop_ptr)
            }
            BuiltinRunner::Keccak(ref mut keccak) => keccak.stop_ptr = Some(stop_ptr),
            BuiltinRunner::Signature(ref mut signature) => signature.stop_ptr = Some(stop_ptr),
            BuiltinRunner::Poseidon(ref mut poseidon) => poseidon.stop_ptr = Some(stop_ptr),
            BuiltinRunner::SegmentArena(ref mut segment_arena) => {
                segment_arena.stop_ptr = Some(stop_ptr)
            }
            BuiltinRunner::Mod(modulo) => modulo.stop_ptr = Some(stop_ptr),
        }
    }

    pub(crate) fn stop_ptr(&self) -> Option<usize> {
        match self {
            BuiltinRunner::Bitwise(ref bitwise) => bitwise.stop_ptr,
            BuiltinRunner::EcOp(ref ec) => ec.stop_ptr,
            BuiltinRunner::Hash(ref hash) => hash.stop_ptr,
            BuiltinRunner::Output(ref output) => output.stop_ptr,
            BuiltinRunner::RangeCheck(ref range_check) => range_check.stop_ptr,
            BuiltinRunner::RangeCheck96(ref range_check) => range_check.stop_ptr,
            BuiltinRunner::Keccak(ref keccak) => keccak.stop_ptr,
            BuiltinRunner::Signature(ref signature) => signature.stop_ptr,
            BuiltinRunner::Poseidon(ref poseidon) => poseidon.stop_ptr,
            BuiltinRunner::SegmentArena(ref segment_arena) => segment_arena.stop_ptr,
            BuiltinRunner::Mod(ref modulo) => modulo.stop_ptr,
        }
    }
}

impl From<KeccakBuiltinRunner> for BuiltinRunner {
    fn from(runner: KeccakBuiltinRunner) -> Self {
        BuiltinRunner::Keccak(runner)
    }
}

impl From<BitwiseBuiltinRunner> for BuiltinRunner {
    fn from(runner: BitwiseBuiltinRunner) -> Self {
        BuiltinRunner::Bitwise(runner)
    }
}

impl From<EcOpBuiltinRunner> for BuiltinRunner {
    fn from(runner: EcOpBuiltinRunner) -> Self {
        BuiltinRunner::EcOp(runner)
    }
}

impl From<HashBuiltinRunner> for BuiltinRunner {
    fn from(runner: HashBuiltinRunner) -> Self {
        BuiltinRunner::Hash(runner)
    }
}

impl From<OutputBuiltinRunner> for BuiltinRunner {
    fn from(runner: OutputBuiltinRunner) -> Self {
        BuiltinRunner::Output(runner)
    }
}

impl From<RangeCheckBuiltinRunner<RC_N_PARTS_STANDARD>> for BuiltinRunner {
    fn from(runner: RangeCheckBuiltinRunner<RC_N_PARTS_STANDARD>) -> Self {
        BuiltinRunner::RangeCheck(runner)
    }
}

impl From<RangeCheckBuiltinRunner<RC_N_PARTS_96>> for BuiltinRunner {
    fn from(runner: RangeCheckBuiltinRunner<RC_N_PARTS_96>) -> Self {
        BuiltinRunner::RangeCheck96(runner)
    }
}

impl From<SignatureBuiltinRunner> for BuiltinRunner {
    fn from(runner: SignatureBuiltinRunner) -> Self {
        BuiltinRunner::Signature(runner)
    }
}

impl From<PoseidonBuiltinRunner> for BuiltinRunner {
    fn from(runner: PoseidonBuiltinRunner) -> Self {
        BuiltinRunner::Poseidon(runner)
    }
}

impl From<SegmentArenaBuiltinRunner> for BuiltinRunner {
    fn from(runner: SegmentArenaBuiltinRunner) -> Self {
        BuiltinRunner::SegmentArena(runner)
    }
}

impl From<ModBuiltinRunner> for BuiltinRunner {
    fn from(runner: ModBuiltinRunner) -> Self {
        BuiltinRunner::Mod(runner)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cairo_run::{cairo_run, CairoRunConfig};
    use crate::hint_processor::builtin_hint_processor::builtin_hint_processor_definition::BuiltinHintProcessor;
    use crate::relocatable;
    use crate::types::builtin_name::BuiltinName;
    use crate::types::instance_definitions::mod_instance_def::ModInstanceDef;
    use crate::types::instance_definitions::LowRatio;
    use crate::types::layout_name::LayoutName;
    use crate::types::program::Program;
    use crate::utils::test_utils::*;
    use crate::vm::errors::memory_errors::InsufficientAllocatedCellsError;
    use crate::vm::vm_memory::memory::MemoryCell;
    use assert_matches::assert_matches;

    #[cfg(target_arch = "wasm32")]
    use wasm_bindgen_test::*;

    #[test]
    #[cfg_attr(target_arch = "wasm32", wasm_bindgen_test)]
    fn get_n_input_cells_bitwise() {
        let bitwise = BitwiseBuiltinRunner::new(Some(10), true);
        let builtin: BuiltinRunner = bitwise.clone().into();
        assert_eq!(INPUT_CELLS_PER_BITWISE, builtin.n_input_cells())
    }

    #[test]
    #[cfg_attr(target_arch = "wasm32", wasm_bindgen_test)]
    fn get_n_input_cells_hash() {
        let hash = HashBuiltinRunner::new(Some(10), true);
        let builtin: BuiltinRunner = hash.clone().into();
        assert_eq!(INPUT_CELLS_PER_HASH, builtin.n_input_cells())
    }

    #[test]
    #[cfg_attr(target_arch = "wasm32", wasm_bindgen_test)]
    fn get_n_input_cells_ec_op() {
        let ec_op = EcOpBuiltinRunner::new(Some(256), true);
        let builtin: BuiltinRunner = ec_op.clone().into();
        assert_eq!(INPUT_CELLS_PER_EC_OP, builtin.n_input_cells())
    }

    #[test]
    #[cfg_attr(target_arch = "wasm32", wasm_bindgen_test)]
    fn get_n_input_cells_ecdsa() {
        let signature = SignatureBuiltinRunner::new(Some(10), true);
        let builtin: BuiltinRunner = signature.clone().into();
        assert_eq!(CELLS_PER_SIGNATURE, builtin.n_input_cells())
    }

    #[test]
    #[cfg_attr(target_arch = "wasm32", wasm_bindgen_test)]
    fn get_n_input_cells_output() {
        let output = OutputBuiltinRunner::new(true);
        let builtin: BuiltinRunner = output.into();
        assert_eq!(0, builtin.n_input_cells())
    }

    #[test]
    #[cfg_attr(target_arch = "wasm32", wasm_bindgen_test)]
    fn get_cells_per_instance_bitwise() {
        let bitwise = BitwiseBuiltinRunner::new(Some(10), true);
        let builtin: BuiltinRunner = bitwise.clone().into();
        assert_eq!(CELLS_PER_BITWISE, builtin.cells_per_instance())
    }

    #[test]
    #[cfg_attr(target_arch = "wasm32", wasm_bindgen_test)]
    fn get_cells_per_instance_hash() {
        let hash = HashBuiltinRunner::new(Some(10), true);
        let builtin: BuiltinRunner = hash.clone().into();
        assert_eq!(CELLS_PER_HASH, builtin.cells_per_instance())
    }

    #[test]
    #[cfg_attr(target_arch = "wasm32", wasm_bindgen_test)]
    fn get_cells_per_instance_ec_op() {
        let ec_op = EcOpBuiltinRunner::new(Some(256), true);
        let builtin: BuiltinRunner = ec_op.clone().into();
        assert_eq!(CELLS_PER_EC_OP, builtin.cells_per_instance())
    }

    #[test]
    #[cfg_attr(target_arch = "wasm32", wasm_bindgen_test)]
    fn get_cells_per_instance_ecdsa() {
        let signature = SignatureBuiltinRunner::new(Some(10), true);
        let builtin: BuiltinRunner = signature.clone().into();
        assert_eq!(CELLS_PER_SIGNATURE, builtin.cells_per_instance())
    }

    #[test]
    #[cfg_attr(target_arch = "wasm32", wasm_bindgen_test)]
    fn get_cells_per_instance_output() {
        let output = OutputBuiltinRunner::new(true);
        let builtin: BuiltinRunner = output.into();
        assert_eq!(0, builtin.cells_per_instance())
    }

    #[test]
    #[cfg_attr(target_arch = "wasm32", wasm_bindgen_test)]
    fn get_name_bitwise() {
        let bitwise = BitwiseBuiltinRunner::new(Some(10), true);
        let builtin: BuiltinRunner = bitwise.into();
        assert_eq!(BuiltinName::bitwise, builtin.name())
    }

    #[test]
    #[cfg_attr(target_arch = "wasm32", wasm_bindgen_test)]
    fn get_name_hash() {
        let hash = HashBuiltinRunner::new(Some(10), true);
        let builtin: BuiltinRunner = hash.into();
        assert_eq!(BuiltinName::pedersen, builtin.name())
    }

    #[test]
    #[cfg_attr(target_arch = "wasm32", wasm_bindgen_test)]
    fn get_name_range_check() {
        let range_check = RangeCheckBuiltinRunner::<RC_N_PARTS_STANDARD>::new(Some(10), true);
        let builtin: BuiltinRunner = range_check.into();
        assert_eq!(BuiltinName::range_check, builtin.name())
    }

    #[test]
    #[cfg_attr(target_arch = "wasm32", wasm_bindgen_test)]
    fn get_name_ec_op() {
        let ec_op = EcOpBuiltinRunner::new(Some(256), true);
        let builtin: BuiltinRunner = ec_op.into();
        assert_eq!(BuiltinName::ec_op, builtin.name())
    }

    #[test]
    #[cfg_attr(target_arch = "wasm32", wasm_bindgen_test)]
    fn get_name_ecdsa() {
        let signature = SignatureBuiltinRunner::new(Some(10), true);
        let builtin: BuiltinRunner = signature.into();
        assert_eq!(BuiltinName::ecdsa, builtin.name())
    }

    #[test]
    #[cfg_attr(target_arch = "wasm32", wasm_bindgen_test)]
    fn get_name_output() {
        let output = OutputBuiltinRunner::new(true);
        let builtin: BuiltinRunner = output.into();
        assert_eq!(BuiltinName::output, builtin.name())
    }

    #[test]
    #[cfg_attr(target_arch = "wasm32", wasm_bindgen_test)]
    fn get_allocated_memory_units_bitwise_with_items() {
        let builtin = BuiltinRunner::Bitwise(BitwiseBuiltinRunner::new(Some(10), true));

        let program = program!(
            builtins = vec![BuiltinName::bitwise],
            data = vec_data!(
                (4612671182993129469_i64),
                (5189976364521848832_i64),
                (18446744073709551615_i128),
                (5199546496550207487_i64),
                (4612389712311386111_i64),
                (5198983563776393216_i64),
                (2),
                (2345108766317314046_i64),
                (5191102247248822272_i64),
                (5189976364521848832_i64),
                (7),
                (1226245742482522112_i64),
                ((
                    "3618502788666131213697322783095070105623107215331596699973092056135872020470",
                    10
                )),
                (2345108766317314046_i64)
            ),
            main = Some(8),
        );

        let mut cairo_runner = cairo_runner!(program);

        let mut hint_processor = BuiltinHintProcessor::new_empty();

        let address = cairo_runner.initialize(false).unwrap();

        cairo_runner
            .run_until_pc(address, &mut hint_processor)
            .unwrap();

        assert_eq!(builtin.get_allocated_memory_units(&cairo_runner.vm), Ok(5));
    }

    #[test]
    #[cfg_attr(target_arch = "wasm32", wasm_bindgen_test)]
    fn compare_proof_mode_with_and_without_disable_trace_padding() {
        let program_data =
            include_bytes!("../../../../../cairo_programs/proof_programs/pedersen_test.json");

        let config_false = CairoRunConfig {
            disable_trace_padding: false,
            proof_mode: true,
            layout: LayoutName::all_cairo,
            ..Default::default()
        };
        let mut hint_processor_false = BuiltinHintProcessor::new_empty();
        let runner_false =
            cairo_run(program_data, &config_false, &mut hint_processor_false).unwrap();
        let last_step_false = runner_false.vm.current_step;

        assert!(last_step_false.is_power_of_two());

        let config_true = CairoRunConfig {
            disable_trace_padding: true,
            proof_mode: true,
            layout: LayoutName::all_cairo,
            ..Default::default()
        };
        let mut hint_processor_true = BuiltinHintProcessor::new_empty();
        let runner_true = cairo_run(program_data, &config_true, &mut hint_processor_true).unwrap();
        let last_step_true = runner_true.vm.current_step;

        // Ensure the last step is not a power of two - true for this specific program, not always.
        assert!(!last_step_true.is_power_of_two());

        assert!(last_step_true < last_step_false);

        let builtin_runners_false = &runner_false.vm.builtin_runners;
        let builtin_runners_true = &runner_true.vm.builtin_runners;
        assert_eq!(builtin_runners_false.len(), builtin_runners_true.len());
        // Compare allocated instances for each pair of builtin runners
        for (builtin_runner_false, builtin_runner_true) in builtin_runners_false
            .iter()
            .zip(builtin_runners_true.iter())
        {
            let allocated_false = builtin_runner_false
                .get_allocated_instances(&runner_false.vm)
                .unwrap_or(0);
            let allocated_true = builtin_runner_true
                .get_allocated_instances(&runner_true.vm)
                .unwrap_or(0);

            assert!(allocated_false >= allocated_true);
        }
    }

    #[test]
    #[cfg_attr(target_arch = "wasm32", wasm_bindgen_test)]
    fn get_allocated_memory_units_ec_op_with_items() {
        let builtin = BuiltinRunner::EcOp(EcOpBuiltinRunner::new(Some(10), true));

        let program = program!(
            builtins = vec![BuiltinName::ec_op],
            data = vec_data!(
                (4612671182993129469_i64),
                (5189976364521848832_i64),
                (18446744073709551615_i128),
                (5199546496550207487_i64),
                (4612389712311386111_i64),
                (5198983563776393216_i64),
                (2),
                (2345108766317314046_i64),
                (5191102247248822272_i64),
                (5189976364521848832_i64),
                (7),
                (1226245742482522112_i64),
                ((
                    "3618502788666131213697322783095070105623107215331596699973092056135872020470",
                    10
                )),
                (2345108766317314046_i64)
            ),
            main = Some(8),
        );

        let mut cairo_runner = cairo_runner!(program);

        let mut hint_processor = BuiltinHintProcessor::new_empty();

        let address = cairo_runner.initialize(false).unwrap();

        cairo_runner
            .run_until_pc(address, &mut hint_processor)
            .unwrap();

        assert_eq!(builtin.get_allocated_memory_units(&cairo_runner.vm), Ok(7));
    }

    #[test]
    #[cfg_attr(target_arch = "wasm32", wasm_bindgen_test)]
    fn get_allocated_memory_units_hash_with_items() {
        let builtin = BuiltinRunner::Hash(HashBuiltinRunner::new(Some(10), true));

        let program = program!(
            builtins = vec![BuiltinName::pedersen],
            data = vec_data!(
                (4612671182993129469_i64),
                (5189976364521848832_i64),
                (18446744073709551615_i128),
                (5199546496550207487_i64),
                (4612389712311386111_i64),
                (5198983563776393216_i64),
                (2),
                (2345108766317314046_i64),
                (5191102247248822272_i64),
                (5189976364521848832_i64),
                (7),
                (1226245742482522112_i64),
                ((
                    "3618502788666131213697322783095070105623107215331596699973092056135872020470",
                    10
                )),
                (2345108766317314046_i64)
            ),
            main = Some(8),
        );

        let mut cairo_runner = cairo_runner!(program);

        let mut hint_processor = BuiltinHintProcessor::new_empty();

        let address = cairo_runner.initialize(false).unwrap();

        cairo_runner
            .run_until_pc(address, &mut hint_processor)
            .unwrap();

        assert_eq!(builtin.get_allocated_memory_units(&cairo_runner.vm), Ok(3));
    }

    #[test]
    #[cfg_attr(target_arch = "wasm32", wasm_bindgen_test)]
    fn get_allocated_memory_units_range_check_with_items() {
        let builtin = BuiltinRunner::RangeCheck(
            RangeCheckBuiltinRunner::<RC_N_PARTS_STANDARD>::new(Some(10), true),
        );

        let program = program!(
            builtins = vec![BuiltinName::range_check],
            data = vec_data!(
                (4612671182993129469_i64),
                (5189976364521848832_i64),
                (18446744073709551615_i128),
                (5199546496550207487_i64),
                (4612389712311386111_i64),
                (5198983563776393216_i64),
                (2),
                (2345108766317314046_i64),
                (5191102247248822272_i64),
                (5189976364521848832_i64),
                (7),
                (1226245742482522112_i64),
                ((
                    "3618502788666131213697322783095070105623107215331596699973092056135872020470",
                    10
                )),
                (2345108766317314046_i64)
            ),
            main = Some(8),
        );

        let mut cairo_runner = cairo_runner!(program);

        let mut hint_processor = BuiltinHintProcessor::new_empty();

        let address = cairo_runner.initialize(false).unwrap();

        cairo_runner
            .run_until_pc(address, &mut hint_processor)
            .unwrap();

        assert_eq!(builtin.get_allocated_memory_units(&cairo_runner.vm), Ok(1));
    }

    #[test]
    #[cfg_attr(target_arch = "wasm32", wasm_bindgen_test)]
    fn get_allocated_memory_units_keccak_with_items() {
        let builtin = BuiltinRunner::Keccak(KeccakBuiltinRunner::new(Some(10), true));

        let mut vm = vm!();
        vm.current_step = 160;
        assert_eq!(builtin.get_allocated_memory_units(&vm), Ok(256));
    }

    #[test]
    fn get_allocated_memory_units_keccak_min_steps_not_reached() {
        let builtin = BuiltinRunner::Keccak(KeccakBuiltinRunner::new(Some(10), true));

        let mut vm = vm!();
        vm.current_step = 10;
        assert_eq!(
            builtin.get_allocated_memory_units(&vm),
            Err(MemoryError::InsufficientAllocatedCells(
                InsufficientAllocatedCellsError::MinStepNotReached(Box::new((
                    160,
                    BuiltinName::keccak
                )))
            ))
        );
    }

    #[test]
    #[cfg_attr(target_arch = "wasm32", wasm_bindgen_test)]
    fn get_allocated_memory_units_output() {
        let builtin = BuiltinRunner::Output(OutputBuiltinRunner::new(true));
        let vm = vm!();

        // In this case, the function always return Ok(0)
        assert_eq!(builtin.get_allocated_memory_units(&vm), Ok(0));
    }

    #[test]
    #[cfg_attr(target_arch = "wasm32", wasm_bindgen_test)]
    fn get_allocated_memory_units_range_check() {
        let builtin = BuiltinRunner::RangeCheck(
            RangeCheckBuiltinRunner::<RC_N_PARTS_STANDARD>::new(Some(8), true),
        );
        let mut vm = vm!();
        vm.current_step = 8;
        assert_eq!(builtin.get_allocated_memory_units(&vm), Ok(1));
    }

    #[test]
    #[cfg_attr(target_arch = "wasm32", wasm_bindgen_test)]
    fn get_allocated_memory_units_hash() {
        let builtin = BuiltinRunner::Hash(HashBuiltinRunner::new(Some(1), true));
        let mut vm = vm!();
        vm.current_step = 1;
        assert_eq!(builtin.get_allocated_memory_units(&vm), Ok(3));
    }

    #[test]
    #[cfg_attr(target_arch = "wasm32", wasm_bindgen_test)]
    fn get_allocated_memory_units_bitwise() {
        let builtin = BuiltinRunner::Bitwise(BitwiseBuiltinRunner::new(Some(256), true));
        let mut vm = vm!();
        vm.current_step = 256;
        assert_eq!(builtin.get_allocated_memory_units(&vm), Ok(5));
    }

    #[test]
    #[cfg_attr(target_arch = "wasm32", wasm_bindgen_test)]
    fn get_allocated_memory_units_ec_op() {
        let builtin = BuiltinRunner::EcOp(EcOpBuiltinRunner::new(Some(256), true));
        let mut vm = vm!();
        vm.current_step = 256;
        assert_eq!(builtin.get_allocated_memory_units(&vm), Ok(7));
    }

    #[test]
    #[cfg_attr(target_arch = "wasm32", wasm_bindgen_test)]
    fn get_allocated_memory_units_keccak() {
        let builtin = BuiltinRunner::Keccak(KeccakBuiltinRunner::new(Some(2048), true));
        let mut vm = vm!();
        vm.current_step = 32768;
        assert_eq!(builtin.get_allocated_memory_units(&vm), Ok(256));
    }

    #[test]
    #[cfg_attr(target_arch = "wasm32", wasm_bindgen_test)]
    fn get_allocated_memory_units_zero_ratio() {
        let builtin = BuiltinRunner::Keccak(KeccakBuiltinRunner::new(Some(0), true));
        let vm = vm!();
        assert_eq!(builtin.get_allocated_memory_units(&vm), Ok(0));
    }

    #[test]
    #[cfg_attr(target_arch = "wasm32", wasm_bindgen_test)]
    fn get_allocated_memory_units_none_ratio() {
        let mut builtin = BuiltinRunner::Keccak(KeccakBuiltinRunner::new(None, true));
        let mut vm = vm!();

        builtin.initialize_segments(&mut vm.segments);
        vm.compute_segments_effective_sizes();

        assert_eq!(builtin.get_allocated_memory_units(&vm), Ok(0));
    }

    #[test]
    #[cfg_attr(target_arch = "wasm32", wasm_bindgen_test)]
    fn get_range_check_usage_range_check() {
        let builtin = BuiltinRunner::RangeCheck(
            RangeCheckBuiltinRunner::<RC_N_PARTS_STANDARD>::new(Some(8), true),
        );
        let memory = memory![((0, 0), 1), ((0, 1), 2), ((0, 2), 3), ((0, 3), 4)];
        assert_eq!(builtin.get_range_check_usage(&memory), Some((0, 4)));
    }

    #[test]
    #[cfg_attr(target_arch = "wasm32", wasm_bindgen_test)]
    fn get_range_check_usage_output() {
        let builtin = BuiltinRunner::Output(OutputBuiltinRunner::new(true));
        let memory = memory![((0, 0), 1), ((0, 1), 2), ((0, 2), 3), ((0, 3), 4)];
        assert_eq!(builtin.get_range_check_usage(&memory), None);
    }

    #[test]
    #[cfg_attr(target_arch = "wasm32", wasm_bindgen_test)]
    fn get_range_check_usage_hash() {
        let builtin = BuiltinRunner::Hash(HashBuiltinRunner::new(Some(256), true));
        let memory = memory![((0, 0), 1), ((0, 1), 2), ((0, 2), 3), ((0, 3), 4)];
        assert_eq!(builtin.get_range_check_usage(&memory), None);
    }

    #[test]
    #[cfg_attr(target_arch = "wasm32", wasm_bindgen_test)]
    fn get_range_check_usage_ec_op() {
        let builtin = BuiltinRunner::EcOp(EcOpBuiltinRunner::new(Some(256), true));
        let memory = memory![((0, 0), 1), ((0, 1), 2), ((0, 2), 3), ((0, 3), 4)];
        assert_eq!(builtin.get_range_check_usage(&memory), None);
    }

    #[test]
    #[cfg_attr(target_arch = "wasm32", wasm_bindgen_test)]
    fn get_range_check_usage_bitwise() {
        let builtin = BuiltinRunner::Bitwise(BitwiseBuiltinRunner::new(Some(256), true));
        let memory = memory![((0, 0), 1), ((0, 1), 2), ((0, 2), 3), ((0, 3), 4)];
        assert_eq!(builtin.get_range_check_usage(&memory), None);
    }

    #[test]
    #[cfg_attr(target_arch = "wasm32", wasm_bindgen_test)]
    fn get_used_diluted_check_units_bitwise() {
        let builtin = BuiltinRunner::Bitwise(BitwiseBuiltinRunner::new(Some(256), true));
        assert_eq!(builtin.get_used_diluted_check_units(270, 7), 1255);
    }

    #[test]
    #[cfg_attr(target_arch = "wasm32", wasm_bindgen_test)]
    fn get_used_diluted_check_units_keccak_zero_case() {
        let builtin = BuiltinRunner::Keccak(KeccakBuiltinRunner::new(Some(2048), true));
        assert_eq!(builtin.get_used_diluted_check_units(270, 7), 0);
    }

    #[test]
    #[cfg_attr(target_arch = "wasm32", wasm_bindgen_test)]
    fn get_used_diluted_check_units_keccak_non_zero_case() {
        let builtin = BuiltinRunner::Keccak(KeccakBuiltinRunner::new(Some(2048), true));
        assert_eq!(builtin.get_used_diluted_check_units(0, 8), 32768);
    }

    #[test]
    #[cfg_attr(target_arch = "wasm32", wasm_bindgen_test)]
    fn get_used_diluted_check_units_ec_op() {
        let builtin = BuiltinRunner::EcOp(EcOpBuiltinRunner::new(Some(10), true));
        assert_eq!(builtin.get_used_diluted_check_units(270, 7), 0);
    }

    #[test]
    #[cfg_attr(target_arch = "wasm32", wasm_bindgen_test)]
    fn get_used_diluted_check_units_hash() {
        let builtin = BuiltinRunner::Hash(HashBuiltinRunner::new(Some(1), true));
        assert_eq!(builtin.get_used_diluted_check_units(270, 7), 0);
    }

    #[test]
    #[cfg_attr(target_arch = "wasm32", wasm_bindgen_test)]
    fn get_used_diluted_check_units_range_check() {
        let builtin = BuiltinRunner::RangeCheck(
            RangeCheckBuiltinRunner::<RC_N_PARTS_STANDARD>::new(Some(8), true),
        );
        assert_eq!(builtin.get_used_diluted_check_units(270, 7), 0);
    }

    #[test]
    #[cfg_attr(target_arch = "wasm32", wasm_bindgen_test)]
    fn get_used_diluted_check_units_output() {
        let builtin = BuiltinRunner::Output(OutputBuiltinRunner::new(true));
        assert_eq!(builtin.get_used_diluted_check_units(270, 7), 0);
    }

    #[test]
    #[cfg_attr(target_arch = "wasm32", wasm_bindgen_test)]
    fn get_memory_segment_addresses_test() {
        let bitwise_builtin: BuiltinRunner = BitwiseBuiltinRunner::new(Some(256), true).into();
        assert_eq!(bitwise_builtin.get_memory_segment_addresses(), (0, None),);
        let ec_op_builtin: BuiltinRunner = EcOpBuiltinRunner::new(Some(256), true).into();
        assert_eq!(ec_op_builtin.get_memory_segment_addresses(), (0, None),);
        let hash_builtin: BuiltinRunner = HashBuiltinRunner::new(Some(8), true).into();
        assert_eq!(hash_builtin.get_memory_segment_addresses(), (0, None),);
        let output_builtin: BuiltinRunner = OutputBuiltinRunner::new(true).into();
        assert_eq!(output_builtin.get_memory_segment_addresses(), (0, None),);
        let range_check_builtin: BuiltinRunner = BuiltinRunner::RangeCheck(
            RangeCheckBuiltinRunner::<RC_N_PARTS_STANDARD>::new(Some(8), true),
        );
        assert_eq!(
            range_check_builtin.get_memory_segment_addresses(),
            (0, None),
        );
    }

    #[test]
    #[cfg_attr(target_arch = "wasm32", wasm_bindgen_test)]
    fn run_security_checks_for_output() {
        let builtin = BuiltinRunner::Output(OutputBuiltinRunner::new(true));
        let vm = vm!();

        assert_matches!(builtin.run_security_checks(&vm), Ok(()));
    }

    #[test]
    #[cfg_attr(target_arch = "wasm32", wasm_bindgen_test)]
    fn run_security_checks_empty_memory() {
        let builtin = BuiltinRunner::Bitwise(BitwiseBuiltinRunner::new(Some(256), true));
        let vm = vm!();
        // Unused builtin shouldn't fail security checks
        assert_matches!(builtin.run_security_checks(&vm), Ok(()));
    }

    #[test]
    #[cfg_attr(target_arch = "wasm32", wasm_bindgen_test)]
    fn run_security_checks_empty_offsets() {
        let builtin = BuiltinRunner::Bitwise(BitwiseBuiltinRunner::new(Some(256), true));
        let mut vm = vm!();

        vm.segments.memory.data = vec![vec![]];

        assert_matches!(builtin.run_security_checks(&vm), Ok(()));
    }

    #[test]
    #[cfg_attr(target_arch = "wasm32", wasm_bindgen_test)]
    fn run_security_checks_bitwise_missing_memory_cells_with_offsets() {
        let builtin = BuiltinRunner::Bitwise(BitwiseBuiltinRunner::new(Some(256), true));
        let mut vm = vm!();
        vm.segments.memory = memory![
            ((0, 1), (0, 1)),
            ((0, 2), (0, 2)),
            ((0, 3), (0, 3)),
            ((0, 4), (0, 4))
        ];

        assert_matches!(
            builtin.run_security_checks(&vm),
            Err(VirtualMachineError::Memory(
                MemoryError::MissingMemoryCellsWithOffsets(bx)
            )) if *bx == (BuiltinName::bitwise, vec![0])
        );
    }

    #[test]
    #[cfg_attr(target_arch = "wasm32", wasm_bindgen_test)]
    fn run_security_checks_bitwise_missing_memory_cells() {
        let bitwise_builtin = BitwiseBuiltinRunner::new(Some(256), true);

        let builtin: BuiltinRunner = bitwise_builtin.into();

        let mut vm = vm!();

        vm.segments.memory = memory![((0, 4), (0, 5))];

        assert_matches!(
            builtin.run_security_checks(&vm),
            Err(VirtualMachineError::Memory(
                MemoryError::MissingMemoryCells(bx)
            )) if *bx == BuiltinName::bitwise
        );
    }

    #[test]
    #[cfg_attr(target_arch = "wasm32", wasm_bindgen_test)]
    fn run_security_checks_hash_missing_memory_cells_with_offsets() {
        let builtin: BuiltinRunner = HashBuiltinRunner::new(Some(8), true).into();
        let mut vm = vm!();

        vm.segments.memory = memory![
            ((0, 1), (0, 1)),
            ((0, 2), (0, 2)),
            ((0, 3), (0, 3)),
            ((0, 4), (0, 4)),
            ((0, 5), (0, 5))
        ];
        assert_matches!(
            builtin.run_security_checks(&vm),
            Err(VirtualMachineError::Memory(
                MemoryError::MissingMemoryCellsWithOffsets(bx)
            )) if *bx == (BuiltinName::pedersen, vec![0])
        );
    }

    #[test]
    #[cfg_attr(target_arch = "wasm32", wasm_bindgen_test)]
    fn run_security_checks_hash_missing_memory_cells() {
        let hash_builtin = HashBuiltinRunner::new(Some(8), true);

        let builtin: BuiltinRunner = hash_builtin.into();

        let mut vm = vm!();

        vm.segments.memory = memory![((0, 0), (0, 0))];

        assert_matches!(
            builtin.run_security_checks(&vm),
            Err(VirtualMachineError::Memory(
                MemoryError::MissingMemoryCells(bx)
            )) if *bx == BuiltinName::pedersen
        );
    }

    #[test]
    #[cfg_attr(target_arch = "wasm32", wasm_bindgen_test)]
    fn run_security_checks_range_check_missing_memory_cells_with_offsets() {
        let range_check_builtin =
            RangeCheckBuiltinRunner::<RC_N_PARTS_STANDARD>::new(Some(8), true);
        let builtin: BuiltinRunner = range_check_builtin.into();
        let mut vm = vm!();

        vm.segments.memory = memory![
            ((0, 1), 100),
            ((0, 2), 2),
            ((0, 3), 3),
            ((0, 5), 5),
            ((0, 6), 17),
            ((0, 7), 22)
        ];

        assert_matches!(
            builtin.run_security_checks(&vm),
            Err(VirtualMachineError::Memory(
                MemoryError::MissingMemoryCells(bx)
            )) if *bx == BuiltinName::range_check
        );
    }

    #[test]
    #[cfg_attr(target_arch = "wasm32", wasm_bindgen_test)]
    fn run_security_checks_range_check_missing_memory_cells() {
        let builtin: BuiltinRunner = BuiltinRunner::RangeCheck(RangeCheckBuiltinRunner::<
            RC_N_PARTS_STANDARD,
        >::new(Some(8), true));
        let mut vm = vm!();

        vm.segments.memory = memory![((0, 1), 1)];

        assert_matches!(
            builtin.run_security_checks(&vm),
            Err(VirtualMachineError::Memory(
                MemoryError::MissingMemoryCells(bx)
            )) if *bx == BuiltinName::range_check
        );
    }

    #[test]
    #[cfg_attr(target_arch = "wasm32", wasm_bindgen_test)]
    fn run_security_checks_range_check_empty() {
        let range_check_builtin =
            RangeCheckBuiltinRunner::<RC_N_PARTS_STANDARD>::new(Some(8), true);

        let builtin: BuiltinRunner = range_check_builtin.into();

        let mut vm = vm!();

        vm.segments.memory.data = vec![vec![MemoryCell::NONE, MemoryCell::NONE, MemoryCell::NONE]];

        assert_matches!(builtin.run_security_checks(&vm), Ok(()));
    }

    #[test]
    #[cfg_attr(target_arch = "wasm32", wasm_bindgen_test)]
    fn run_security_checks_validate_auto_deductions() {
        let builtin: BuiltinRunner = BitwiseBuiltinRunner::new(Some(256), true).into();

        let mut vm = vm!();
        vm.segments
            .memory
            .validated_addresses
            .extend(&[relocatable!(0, 2)]);

        vm.segments.memory = memory![
            ((0, 0), (0, 0)),
            ((0, 1), (0, 1)),
            ((0, 2), (0, 2)),
            ((0, 3), (0, 3)),
            ((0, 4), (0, 4))
        ];

        assert_matches!(builtin.run_security_checks(&vm), Ok(()));
    }

    #[test]
    #[cfg_attr(target_arch = "wasm32", wasm_bindgen_test)]
    fn run_security_ec_op_check_memory_empty() {
        let ec_op_builtin = EcOpBuiltinRunner::new(Some(256), true);

        let builtin: BuiltinRunner = ec_op_builtin.into();

        let mut vm = vm!();
        // The values stored in memory are not relevant for this test
        vm.segments.memory.data = vec![vec![]];

        assert_matches!(builtin.run_security_checks(&vm), Ok(()));
    }

    #[test]
    #[cfg_attr(target_arch = "wasm32", wasm_bindgen_test)]
    fn run_security_ec_op_check_memory_1_element() {
        let ec_op_builtin = EcOpBuiltinRunner::new(Some(256), true);

        let builtin: BuiltinRunner = ec_op_builtin.into();

        let mut vm = vm!();
        // The values stored in memory are not relevant for this test
        vm.segments.memory = memory![((0, 0), 0)];
        assert_matches!(
            builtin.run_security_checks(&vm),
            Err(VirtualMachineError::Memory(
                MemoryError::MissingMemoryCells(bx)
            )) if *bx == BuiltinName::ec_op
        );
    }

    #[test]
    #[cfg_attr(target_arch = "wasm32", wasm_bindgen_test)]
    fn run_security_ec_op_check_memory_3_elements() {
        let ec_op_builtin = EcOpBuiltinRunner::new(Some(256), true);

        let builtin: BuiltinRunner = ec_op_builtin.into();

        let mut vm = vm!();
        // The values stored in memory are not relevant for this test
        vm.segments.memory = memory![((0, 0), 0), ((0, 1), 0), ((0, 2), 0)];

        assert_matches!(
            builtin.run_security_checks(&vm),
            Err(VirtualMachineError::Memory(
                MemoryError::MissingMemoryCells(bx)
            )) if *bx == BuiltinName::ec_op
        );
    }

    #[test]
    #[cfg_attr(target_arch = "wasm32", wasm_bindgen_test)]
    fn run_security_ec_op_missing_memory_cells_with_offsets() {
        let builtin: BuiltinRunner = EcOpBuiltinRunner::new(Some(256), true).into();
        let mut vm = vm!();
        vm.segments.memory = memory![
            ((0, 1), (0, 1)),
            ((0, 2), (0, 2)),
            ((0, 3), (0, 3)),
            ((0, 4), (0, 4)),
            ((0, 5), (0, 5)),
            ((0, 6), (0, 6))
        ];

        assert_matches!(
            builtin.run_security_checks(&vm),
            Err(VirtualMachineError::Memory(
                MemoryError::MissingMemoryCellsWithOffsets(bx)
            )) if *bx == (BuiltinName::ec_op, vec![0])
        );
    }

    #[test]
    #[cfg_attr(target_arch = "wasm32", wasm_bindgen_test)]
    fn run_security_ec_op_check_memory_gap() {
        let ec_op_builtin = EcOpBuiltinRunner::new(Some(256), true);

        let builtin: BuiltinRunner = ec_op_builtin.into();

        let mut vm = vm!();
        // The values stored in memory are not relevant for this test
        vm.segments.memory = memory![
            ((0, 0), 0),
            ((0, 1), 1),
            ((0, 2), 2),
            ((0, 3), 3),
            ((0, 4), 4),
            ((0, 5), 5),
            ((0, 6), 6),
            ((0, 8), 8),
            ((0, 9), 9),
            ((0, 10), 10),
            ((0, 11), 11)
        ];

        assert_matches!(
            builtin.run_security_checks(&vm),
            Err(VirtualMachineError::Memory(
                MemoryError::MissingMemoryCellsWithOffsets(bx)
            )) if *bx == (BuiltinName::ec_op, vec![7])
        );
    }

    /// Test that get_used_perm_range_check_units() returns zero when the
    /// builtin is a BitwiseBuiltinRunner.
    #[test]
    #[cfg_attr(target_arch = "wasm32", wasm_bindgen_test)]
    fn get_used_perm_range_check_units_bitwise() {
        let builtin_runner: BuiltinRunner = BitwiseBuiltinRunner::new(Some(256), true).into();
        let mut vm = vm!();

        vm.current_step = 8;
        vm.segments.segment_used_sizes = Some(vec![5]);
        assert_eq!(builtin_runner.get_used_perm_range_check_units(&vm), Ok(0));
    }

    /// Test that get_used_perm_range_check_units() returns zero when the
    /// builtin is an EcOpBuiltinRunner.
    #[test]
    #[cfg_attr(target_arch = "wasm32", wasm_bindgen_test)]
    fn get_used_perm_range_check_units_ec_op() {
        let builtin_runner: BuiltinRunner = EcOpBuiltinRunner::new(Some(256), true).into();
        let mut vm = vm!();

        vm.current_step = 8;
        vm.segments.segment_used_sizes = Some(vec![5]);
        assert_eq!(builtin_runner.get_used_perm_range_check_units(&vm), Ok(0));
    }

    /// Test that get_used_perm_range_check_units() returns zero when the
    /// builtin is a HashBuiltinRunner.
    #[test]
    #[cfg_attr(target_arch = "wasm32", wasm_bindgen_test)]
    fn get_used_perm_range_check_units_hash() {
        let builtin_runner: BuiltinRunner = HashBuiltinRunner::new(Some(8), true).into();
        let mut vm = vm!();

        vm.current_step = 8;
        vm.segments.segment_used_sizes = Some(vec![5]);
        assert_eq!(builtin_runner.get_used_perm_range_check_units(&vm), Ok(0));
    }

    /// Test that get_used_perm_range_check_units() returns zero when the
    /// builtin is an OutputBuiltinRunner.
    #[test]
    #[cfg_attr(target_arch = "wasm32", wasm_bindgen_test)]
    fn get_used_perm_range_check_units_output() {
        let builtin_runner: BuiltinRunner = OutputBuiltinRunner::new(true).into();
        let mut vm = vm!();

        vm.current_step = 8;
        vm.segments.segment_used_sizes = Some(vec![5]);
        assert_eq!(builtin_runner.get_used_perm_range_check_units(&vm), Ok(0));
    }

    /// Test that get_used_perm_range_check_units() calls the corresponding
    /// method when the builtin is a RangeCheckBuiltinRunner.
    #[test]
    #[cfg_attr(target_arch = "wasm32", wasm_bindgen_test)]
    fn get_used_perm_range_check_units_range_check() {
        let builtin_runner: BuiltinRunner =
            RangeCheckBuiltinRunner::<RC_N_PARTS_STANDARD>::new(Some(8), true).into();
        let mut vm = vm!();

        vm.current_step = 8;
        vm.segments.segment_used_sizes = Some(vec![1]);
        assert_eq!(builtin_runner.get_used_perm_range_check_units(&vm), Ok(8));
    }

    #[test]
    #[cfg_attr(target_arch = "wasm32", wasm_bindgen_test)]
    fn get_ratio_tests() {
        let bitwise_builtin: BuiltinRunner = BitwiseBuiltinRunner::new(Some(256), true).into();
        assert_eq!(bitwise_builtin.ratio(), (Some(256)),);
        let ec_op_builtin: BuiltinRunner = EcOpBuiltinRunner::new(Some(256), true).into();
        assert_eq!(ec_op_builtin.ratio(), (Some(256)),);
        let hash_builtin: BuiltinRunner = HashBuiltinRunner::new(Some(8), true).into();
        assert_eq!(hash_builtin.ratio(), (Some(8)),);
        let output_builtin: BuiltinRunner = OutputBuiltinRunner::new(true).into();
        assert_eq!(output_builtin.ratio(), None,);
        let range_check_builtin: BuiltinRunner = BuiltinRunner::RangeCheck(
            RangeCheckBuiltinRunner::<RC_N_PARTS_STANDARD>::new(Some(8), true),
        );
        assert_eq!(range_check_builtin.ratio(), (Some(8)),);
        let keccak_builtin: BuiltinRunner = KeccakBuiltinRunner::new(Some(2048), true).into();
        assert_eq!(keccak_builtin.ratio(), (Some(2048)),);
    }

    #[test]
    #[cfg_attr(target_arch = "wasm32", wasm_bindgen_test)]
    fn get_ratio_den_tests() {
        let rangecheck_builtin: BuiltinRunner =
            RangeCheckBuiltinRunner::<RC_N_PARTS_STANDARD>::new_with_low_ratio(
                Some(LowRatio::new(1, 2)),
                true,
            )
            .into();
        assert_eq!(rangecheck_builtin.ratio_den(), (Some(2)),);

        let rangecheck96_builtin: BuiltinRunner =
            RangeCheckBuiltinRunner::<RC_N_PARTS_96>::new_with_low_ratio(
                Some(LowRatio::new(1, 4)),
                true,
            )
            .into();
        assert_eq!(rangecheck96_builtin.ratio_den(), (Some(4)),);

        let mod_builtin: BuiltinRunner =
            ModBuiltinRunner::new_add_mod(&ModInstanceDef::new(Some(5), 3, 3), true).into();
        assert_eq!(mod_builtin.ratio_den(), (Some(1)),);
    }

    #[test]
    #[cfg_attr(target_arch = "wasm32", wasm_bindgen_test)]
    fn bitwise_get_used_instances_test() {
        let mut vm = vm!();
        vm.segments.segment_used_sizes = Some(vec![4]);

        let bitwise_builtin: BuiltinRunner = BitwiseBuiltinRunner::new(Some(256), true).into();
        assert_eq!(bitwise_builtin.get_used_instances(&vm.segments), Ok(1));
    }

    #[test]
    #[cfg_attr(target_arch = "wasm32", wasm_bindgen_test)]
    fn ec_op_get_used_instances_test() {
        let mut vm = vm!();
        vm.segments.segment_used_sizes = Some(vec![4]);

        let ec_op_builtin: BuiltinRunner = EcOpBuiltinRunner::new(Some(256), true).into();
        assert_eq!(ec_op_builtin.get_used_instances(&vm.segments), Ok(1));
    }

    #[test]
    #[cfg_attr(target_arch = "wasm32", wasm_bindgen_test)]
    fn hash_get_used_instances_test() {
        let mut vm = vm!();
        vm.segments.segment_used_sizes = Some(vec![4]);

        let hash_builtin: BuiltinRunner = HashBuiltinRunner::new(Some(8), true).into();
        assert_eq!(hash_builtin.get_used_instances(&vm.segments), Ok(2));
    }

    #[test]
    #[cfg_attr(target_arch = "wasm32", wasm_bindgen_test)]
    fn output_get_used_instances_test() {
        let mut vm = vm!();
        vm.segments.segment_used_sizes = Some(vec![4]);

        let output_builtin: BuiltinRunner = OutputBuiltinRunner::new(true).into();
        assert_eq!(output_builtin.get_used_instances(&vm.segments), Ok(4));
    }
    #[test]
    #[cfg_attr(target_arch = "wasm32", wasm_bindgen_test)]
    fn range_check_get_used_instances_test() {
        let mut vm = vm!();
        vm.segments.segment_used_sizes = Some(vec![4]);

        let range_check_builtin: BuiltinRunner = BuiltinRunner::RangeCheck(
            RangeCheckBuiltinRunner::<RC_N_PARTS_STANDARD>::new(Some(8), true),
        );
        assert_eq!(range_check_builtin.get_used_instances(&vm.segments), Ok(4));
    }

    #[test]
    #[cfg_attr(target_arch = "wasm32", wasm_bindgen_test)]
    fn runners_final_stack() {
        let mut builtins = vec![
            BuiltinRunner::Bitwise(BitwiseBuiltinRunner::new(Some(256), false)),
            BuiltinRunner::EcOp(EcOpBuiltinRunner::new(Some(256), false)),
            BuiltinRunner::Hash(HashBuiltinRunner::new(Some(1), false)),
            BuiltinRunner::Output(OutputBuiltinRunner::new(false)),
            BuiltinRunner::RangeCheck(RangeCheckBuiltinRunner::<RC_N_PARTS_STANDARD>::new(
                Some(8),
                false,
            )),
            BuiltinRunner::Keccak(KeccakBuiltinRunner::new(Some(2048), false)),
            BuiltinRunner::Signature(SignatureBuiltinRunner::new(Some(512), false)),
        ];
        let vm = vm!();

        for br in builtins.iter_mut() {
            assert_eq!(br.final_stack(&vm.segments, vm.get_ap()), Ok(vm.get_ap()));
        }
    }

    #[test]
    #[cfg_attr(target_arch = "wasm32", wasm_bindgen_test)]
    fn runners_set_stop_ptr() {
        let builtins = vec![
            BuiltinRunner::Bitwise(BitwiseBuiltinRunner::new(Some(256), false)),
            BuiltinRunner::EcOp(EcOpBuiltinRunner::new(Some(256), false)),
            BuiltinRunner::Hash(HashBuiltinRunner::new(Some(1), false)),
            BuiltinRunner::Output(OutputBuiltinRunner::new(false)),
            BuiltinRunner::RangeCheck(RangeCheckBuiltinRunner::<RC_N_PARTS_STANDARD>::new(
                Some(8),
                false,
            )),
            BuiltinRunner::Keccak(KeccakBuiltinRunner::new(Some(2048), false)),
            BuiltinRunner::Signature(SignatureBuiltinRunner::new(Some(512), false)),
            BuiltinRunner::Poseidon(PoseidonBuiltinRunner::new(Some(32), false)),
            BuiltinRunner::SegmentArena(SegmentArenaBuiltinRunner::new(false)),
        ];

        let ptr = 3;

        for mut br in builtins {
            br.set_stop_ptr(ptr);
            let (_, stop_ptr) = br.get_memory_segment_addresses();
            assert_eq!(stop_ptr, Some(ptr));
        }
    }

    #[test]
    fn get_additonal_data_none() {
        let builtin: BuiltinRunner = PoseidonBuiltinRunner::new(None, true).into();
        assert_eq!(builtin.get_additional_data(), BuiltinAdditionalData::None)
    }
}
