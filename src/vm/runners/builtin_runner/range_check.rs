use crate::bigint;
use crate::math_utils::safe_div_usize;
use crate::types::instance_definitions::range_check_instance_def::CELLS_PER_RANGE_CHECK;
use crate::types::relocatable::{MaybeRelocatable, Relocatable};
use crate::vm::errors::memory_errors::MemoryError;
use crate::vm::errors::runner_errors::RunnerError;
use crate::vm::vm_core::VirtualMachine;
use crate::vm::vm_memory::memory::{Memory, ValidationRule};
use crate::vm::vm_memory::memory_segments::MemorySegmentManager;
use num_bigint::BigInt;
use num_integer::Integer;
use num_traits::{One, ToPrimitive, Zero};
use std::borrow::Cow;
use std::cmp::{max, min};
use std::ops::Shl;

#[derive(Debug)]
pub struct RangeCheckBuiltinRunner {
    ratio: u32,
    base: isize,
    stop_ptr: Option<usize>,
    pub(crate) cells_per_instance: u32,
    pub(crate) n_input_cells: u32,
    inner_rc_bound: usize,
    pub _bound: BigInt,
    pub(crate) _included: bool,
    n_parts: u32,
    instances_per_component: u32,
}

impl RangeCheckBuiltinRunner {
    pub fn new(ratio: u32, n_parts: u32, included: bool) -> RangeCheckBuiltinRunner {
        let inner_rc_bound = 1usize << 16;
        RangeCheckBuiltinRunner {
            ratio,
            base: 0,
            stop_ptr: None,
            cells_per_instance: CELLS_PER_RANGE_CHECK,
            n_input_cells: CELLS_PER_RANGE_CHECK,
            inner_rc_bound,
            _bound: bigint!(inner_rc_bound).pow(n_parts),
            _included: included,
            n_parts,
            instances_per_component: 1,
        }
    }

    pub fn initialize_segments(
        &mut self,
        segments: &mut MemorySegmentManager,
        memory: &mut Memory,
    ) {
        self.base = segments.add(memory).segment_index
    }

    pub fn initial_stack(&self) -> Vec<MaybeRelocatable> {
        if self._included {
            vec![MaybeRelocatable::from((self.base, 0))]
        } else {
            vec![]
        }
    }

    pub fn base(&self) -> isize {
        self.base
    }

    pub fn ratio(&self) -> u32 {
        self.ratio
    }

    pub fn add_validation_rule(&self, memory: &mut Memory) -> Result<(), RunnerError> {
        let rule: ValidationRule = ValidationRule(Box::new(
            |memory: &Memory,
             address: &MaybeRelocatable|
             -> Result<MaybeRelocatable, MemoryError> {
                match memory.get(address)? {
                    Some(Cow::Owned(MaybeRelocatable::Int(ref num)))
                    | Some(Cow::Borrowed(MaybeRelocatable::Int(ref num))) => {
                        if &BigInt::zero() <= num && num < &BigInt::one().shl(128u8) {
                            Ok(address.to_owned())
                        } else {
                            Err(MemoryError::NumOutOfBounds)
                        }
                    }
                    _ => Err(MemoryError::FoundNonInt),
                }
            },
        ));

        let segment_index: usize = self
            .base
            .try_into()
            .map_err(|_| RunnerError::RunnerInTemporarySegment(self.base))?;

        memory.add_validation_rule(segment_index, rule);

        Ok(())
    }

    pub fn deduce_memory_cell(
        &mut self,
        _address: &Relocatable,
        _memory: &Memory,
    ) -> Result<Option<MaybeRelocatable>, RunnerError> {
        Ok(None)
    }

    pub fn get_allocated_memory_units(&self, vm: &VirtualMachine) -> Result<usize, MemoryError> {
        let value = safe_div_usize(vm.current_step, self.ratio as usize)
            .map_err(|_| MemoryError::ErrorCalculatingMemoryUnits)?;
        match (self.cells_per_instance as usize * value).to_usize() {
            Some(result) => Ok(result),
            _ => Err(MemoryError::ErrorCalculatingMemoryUnits),
        }
    }

    pub fn get_memory_segment_addresses(&self) -> (&'static str, (isize, Option<usize>)) {
        ("range_check", (self.base, self.stop_ptr))
    }

    pub fn get_used_cells(&self, vm: &VirtualMachine) -> Result<usize, MemoryError> {
        let base = self.base();
        vm.segments
            .get_segment_used_size(
                base.try_into()
                    .map_err(|_| MemoryError::AddressInTemporarySegment(base))?,
            )
            .ok_or(MemoryError::MissingSegmentUsedSizes)
    }

    pub fn get_used_cells_and_allocated_size(
        &self,
        vm: &VirtualMachine,
    ) -> Result<(usize, usize), MemoryError> {
        let ratio = self.ratio as usize;
        let cells_per_instance = self.cells_per_instance;
        let min_step = ratio * self.instances_per_component as usize;
        if vm.current_step < min_step {
            Err(MemoryError::InsufficientAllocatedCells)
        } else {
            let used = self.get_used_cells(vm)?;
            let size = cells_per_instance as usize
                * safe_div_usize(vm.current_step, ratio as usize)
                    .map_err(|_| MemoryError::InsufficientAllocatedCells)?;
            Ok((used, size))
        }
    }

    pub fn get_range_check_usage(&self, memory: &Memory) -> Option<(usize, usize)> {
        let mut rc_bounds: Option<(usize, usize)> = None;
        let range_check_segment = memory.data.get(self.base as usize)?;
        let inner_rc_bound = bigint!(self.inner_rc_bound);
        for value in range_check_segment {
            //Split val into n_parts parts.
            for _ in 0..self.n_parts {
                let part_val = value
                    .as_ref()?
                    .get_int_ref()
                    .ok()?
                    .mod_floor(&inner_rc_bound)
                    .to_usize()?;
                rc_bounds = Some(match rc_bounds {
                    None => (part_val, part_val),
                    Some((rc_min, rc_max)) => {
                        let rc_min = min(rc_min, part_val);
                        let rc_max = max(rc_max, part_val);

                        (rc_min, rc_max)
                    }
                });
            }
        }
        rc_bounds
    }

    pub fn get_used_instances(&self, vm: &VirtualMachine) -> Result<usize, MemoryError> {
        self.get_used_cells(vm)
    }

    pub fn final_stack(
        &mut self,
        vm: &VirtualMachine,
        pointer: Relocatable,
    ) -> Result<Relocatable, RunnerError> {
        if self._included {
            if let Ok(stop_pointer) = vm
                .get_relocatable(&(pointer.sub(1)).map_err(|_| RunnerError::FinalStack)?)
                .as_deref()
            {
                self.stop_ptr = Some(stop_pointer.offset);
                let num_instances = self
                    .get_used_instances(vm)
                    .map_err(|_| RunnerError::FinalStack)?;
                let used_cells = num_instances * self.cells_per_instance as usize;
                if self.stop_ptr != Some(self.base() as usize + used_cells) {
                    return Err(RunnerError::InvalidStopPointer("range_check".to_string()));
                }
                pointer.sub(1).map_err(|_| RunnerError::FinalStack)
            } else {
                Err(RunnerError::FinalStack)
            }
        } else {
            self.stop_ptr = std::option::Option::Some(self.base() as usize);
            Ok(pointer)
        }
    }

    /// Returns the number of range check units used by the builtin.
    pub fn get_used_perm_range_check_units(
        &self,
        vm: &VirtualMachine,
    ) -> Result<usize, MemoryError> {
        let (used_cells, _) = self.get_used_cells_and_allocated_size(vm)?;
        Ok(used_cells * self.n_parts as usize)
    }
}

#[cfg(test)]
mod tests {

    use super::*;
    use crate::hint_processor::builtin_hint_processor::builtin_hint_processor_definition::BuiltinHintProcessor;
    use crate::types::program::Program;
    use crate::vm::runners::cairo_runner::CairoRunner;
    use crate::{bigint, utils::test_utils::*};
    use crate::{vm::runners::builtin_runner::BuiltinRunner, vm::vm_core::VirtualMachine};
    use num_bigint::Sign;

    #[test]
    fn get_used_instances() {
        let builtin = RangeCheckBuiltinRunner::new(10, 12, true);

        let mut vm = vm!();

        vm.memory = memory![
            ((0, 0), (0, 0)),
            ((0, 1), (0, 1)),
            ((2, 0), (0, 0)),
            ((2, 1), (0, 0))
        ];

        vm.segments.segment_used_sizes = Some(vec![1]);

        assert_eq!(builtin.get_used_instances(&vm), Ok(1));
    }

    #[test]
    fn final_stack() {
        let mut builtin = RangeCheckBuiltinRunner::new(10, 12, true);

        let mut vm = vm!();

        vm.memory = memory![
            ((0, 0), (0, 0)),
            ((0, 1), (0, 1)),
            ((2, 0), (0, 0)),
            ((2, 1), (0, 0))
        ];

        vm.segments.segment_used_sizes = Some(vec![0]);

        let pointer = Relocatable::from((2, 2));

        assert_eq!(
            builtin.final_stack(&vm, pointer),
            Ok(Relocatable::from((2, 1)))
        );
    }

    #[test]
    fn final_stack_error_stop_pointer() {
        let mut builtin = RangeCheckBuiltinRunner::new(10, 12, true);

        let mut vm = vm!();

        vm.memory = memory![
            ((0, 0), (0, 0)),
            ((0, 1), (0, 1)),
            ((2, 0), (0, 0)),
            ((2, 1), (0, 0))
        ];

        vm.segments.segment_used_sizes = Some(vec![999]);

        let pointer = Relocatable::from((2, 2));

        assert_eq!(
            builtin.final_stack(&vm, pointer),
            Err(RunnerError::InvalidStopPointer("range_check".to_string()))
        );
    }

    #[test]
    fn final_stack_error_when_not_included() {
        let mut builtin = RangeCheckBuiltinRunner::new(10, 12, false);

        let mut vm = vm!();

        vm.memory = memory![
            ((0, 0), (0, 0)),
            ((0, 1), (0, 1)),
            ((2, 0), (0, 0)),
            ((2, 1), (0, 0))
        ];

        vm.segments.segment_used_sizes = Some(vec![0]);

        let pointer = Relocatable::from((2, 2));

        assert_eq!(
            builtin.final_stack(&vm, pointer),
            Ok(Relocatable::from((2, 2)))
        );
    }

    #[test]
    fn final_stack_error_non_relocatable() {
        let mut builtin = RangeCheckBuiltinRunner::new(10, 12, true);

        let mut vm = vm!();

        vm.memory = memory![
            ((0, 0), (0, 0)),
            ((0, 1), (0, 1)),
            ((2, 0), (0, 0)),
            ((2, 1), 2)
        ];

        vm.segments.segment_used_sizes = Some(vec![0]);

        let pointer = Relocatable::from((2, 2));

        assert_eq!(
            builtin.final_stack(&vm, pointer),
            Err(RunnerError::FinalStack)
        );
    }

    #[test]
    fn get_used_cells_and_allocated_size_test() {
        let builtin = RangeCheckBuiltinRunner::new(10, 12, true);

        let mut vm = vm!();

        vm.segments.segment_used_sizes = Some(vec![0]);

        let program = program!(
            builtins = vec![String::from("pedersen")],
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
                    b"3618502788666131213697322783095070105623107215331596699973092056135872020470",
                    10
                )),
                (2345108766317314046_i64)
            ),
            main = Some(8),
        );

        let mut cairo_runner = cairo_runner!(program);

        let hint_processor = BuiltinHintProcessor::new_empty();

        let address = cairo_runner.initialize(&mut vm).unwrap();

        cairo_runner
            .run_until_pc(address, &mut vm, &hint_processor)
            .unwrap();

        assert_eq!(builtin.get_used_cells_and_allocated_size(&vm), Ok((0, 1)));
    }

    #[test]
    fn get_allocated_memory_units() {
        let builtin = RangeCheckBuiltinRunner::new(10, 12, true);

        let mut vm = vm!();

        let program = program!(
            builtins = vec![String::from("pedersen")],
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
                    b"3618502788666131213697322783095070105623107215331596699973092056135872020470",
                    10
                )),
                (2345108766317314046_i64)
            ),
            main = Some(8),
        );

        let mut cairo_runner = cairo_runner!(program);

        let hint_processor = BuiltinHintProcessor::new_empty();

        let address = cairo_runner.initialize(&mut vm).unwrap();

        cairo_runner
            .run_until_pc(address, &mut vm, &hint_processor)
            .unwrap();

        assert_eq!(builtin.get_allocated_memory_units(&vm), Ok(1));
    }

    #[test]
    fn initialize_segments_for_range_check() {
        let mut builtin = RangeCheckBuiltinRunner::new(8, 8, true);
        let mut segments = MemorySegmentManager::new();
        let mut memory = Memory::new();
        builtin.initialize_segments(&mut segments, &mut memory);
        assert_eq!(builtin.base, 0);
    }

    #[test]
    fn get_initial_stack_for_range_check_with_base() {
        let mut builtin = RangeCheckBuiltinRunner::new(8, 8, true);
        builtin.base = 1;
        let initial_stack = builtin.initial_stack();
        assert_eq!(
            initial_stack[0].clone(),
            MaybeRelocatable::RelocatableValue((builtin.base(), 0).into())
        );
        assert_eq!(initial_stack.len(), 1);
    }

    #[test]
    fn get_memory_segment_addresses() {
        let builtin = RangeCheckBuiltinRunner::new(8, 8, true);

        assert_eq!(
            builtin.get_memory_segment_addresses(),
            ("range_check", (0, None)),
        );
    }

    #[test]
    fn get_memory_accesses_missing_segment_used_sizes() {
        let builtin = BuiltinRunner::RangeCheck(RangeCheckBuiltinRunner::new(256, 8, true));
        let vm = vm!();

        assert_eq!(
            builtin.get_memory_accesses(&vm),
            Err(MemoryError::MissingSegmentUsedSizes),
        );
    }

    #[test]
    fn get_memory_accesses_empty() {
        let builtin = BuiltinRunner::RangeCheck(RangeCheckBuiltinRunner::new(256, 8, true));
        let mut vm = vm!();

        vm.segments.segment_used_sizes = Some(vec![0]);
        assert_eq!(builtin.get_memory_accesses(&vm), Ok(vec![]));
    }

    #[test]
    fn get_memory_accesses() {
        let builtin = BuiltinRunner::RangeCheck(RangeCheckBuiltinRunner::new(256, 8, true));
        let mut vm = vm!();

        vm.segments.segment_used_sizes = Some(vec![4]);
        assert_eq!(
            builtin.get_memory_accesses(&vm),
            Ok(vec![
                (builtin.base(), 0).into(),
                (builtin.base(), 1).into(),
                (builtin.base(), 2).into(),
                (builtin.base(), 3).into(),
            ]),
        );
    }

    #[test]
    fn get_used_cells_missing_segment_used_sizes() {
        let builtin = BuiltinRunner::RangeCheck(RangeCheckBuiltinRunner::new(256, 8, true));
        let vm = vm!();

        assert_eq!(
            builtin.get_used_cells(&vm),
            Err(MemoryError::MissingSegmentUsedSizes)
        );
    }

    #[test]
    fn get_used_cells_empty() {
        let builtin = BuiltinRunner::RangeCheck(RangeCheckBuiltinRunner::new(256, 8, true));
        let mut vm = vm!();

        vm.segments.segment_used_sizes = Some(vec![0]);
        assert_eq!(builtin.get_used_cells(&vm), Ok(0));
    }

    #[test]
    fn get_used_cells() {
        let builtin = BuiltinRunner::RangeCheck(RangeCheckBuiltinRunner::new(256, 8, true));
        let mut vm = vm!();

        vm.segments.segment_used_sizes = Some(vec![4]);
        assert_eq!(builtin.get_used_cells(&vm), Ok(4));
    }

    #[test]
    fn get_range_check_usage_succesful_a() {
        let builtin = RangeCheckBuiltinRunner::new(8, 8, true);
        let memory = memory![((0, 0), 1), ((0, 1), 2), ((0, 2), 3), ((0, 3), 4)];
        assert_eq!(builtin.get_range_check_usage(&memory), Some((1, 4)));
    }

    #[test]
    fn get_range_check_usage_succesful_b() {
        let builtin = RangeCheckBuiltinRunner::new(8, 8, true);
        let memory = memory![
            ((0, 0), 1465218365),
            ((0, 1), 2134570341),
            ((0, 2), 31349610736_i64),
            ((0, 3), 413468326585859_i64)
        ];
        assert_eq!(builtin.get_range_check_usage(&memory), Some((6384, 62821)));
    }

    #[test]
    fn get_range_check_usage_succesful_c() {
        let builtin = RangeCheckBuiltinRunner::new(8, 8, true);
        let memory = memory![
            ((0, 0), 634834751465218365_i64),
            ((0, 1), 42876922134570341_i64),
            ((0, 2), 23469831349610736_i64),
            ((0, 3), 23468413468326585859_i128),
            ((0, 4), 75346043276073460326_i128),
            ((0, 5), 87234598724867609478353436890268_i128)
        ];
        assert_eq!(builtin.get_range_check_usage(&memory), Some((10480, 42341)));
    }

    #[test]
    fn get_range_check_empty_memory() {
        let builtin = RangeCheckBuiltinRunner::new(8, 8, true);
        let memory = Memory::new();
        assert_eq!(builtin.get_range_check_usage(&memory), None);
    }

    /// Test that the method get_used_perm_range_check_units works as intended.
    #[test]
    fn get_used_perm_range_check_units() {
        let builtin_runner = RangeCheckBuiltinRunner::new(8, 8, true);
        let mut vm = vm!();

        vm.current_step = 8;
        vm.segments.segment_used_sizes = Some(vec![5]);
        assert_eq!(builtin_runner.get_used_perm_range_check_units(&vm), Ok(40));
    }
}
