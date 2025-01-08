use crate::{
    air_private_input::{PrivateInput, PrivateInputValue},
    stdlib::{
        cmp::{max, min},
        prelude::*,
    },
    types::{builtin_name::BuiltinName, instance_definitions::LowRatio},
};

use crate::Felt252;
use crate::{
    types::relocatable::{MaybeRelocatable, Relocatable},
    vm::{
        errors::memory_errors::MemoryError,
        vm_memory::{
            memory::{Memory, ValidationRule},
            memory_segments::MemorySegmentManager,
        },
    },
};

use lazy_static::lazy_static;

const INNER_RC_BOUND_SHIFT: u64 = 16;
const INNER_RC_BOUND_MASK: u64 = u16::MAX as u64;

pub const RC_N_PARTS_STANDARD: u64 = 8;
pub const RC_N_PARTS_96: u64 = 6;

lazy_static! {
    pub static ref BOUND_STANDARD: Felt252 =
        Felt252::TWO.pow(INNER_RC_BOUND_SHIFT * RC_N_PARTS_STANDARD);
    pub static ref BOUND_96: Felt252 = Felt252::TWO.pow(INNER_RC_BOUND_SHIFT * RC_N_PARTS_96);
}

#[derive(Debug, Clone)]
pub struct RangeCheckBuiltinRunner<const N_PARTS: u64> {
    ratio: Option<LowRatio>,
    base: usize,
    pub(crate) stop_ptr: Option<usize>,
    pub(crate) included: bool,
}

impl<const N_PARTS: u64> RangeCheckBuiltinRunner<N_PARTS> {
    pub fn new(ratio: Option<u32>, included: bool) -> RangeCheckBuiltinRunner<N_PARTS> {
        RangeCheckBuiltinRunner {
            ratio: ratio.map(LowRatio::new_int),
            base: 0,
            stop_ptr: None,
            included,
        }
    }

    pub fn new_with_low_ratio(
        ratio: Option<LowRatio>,
        included: bool,
    ) -> RangeCheckBuiltinRunner<N_PARTS> {
        RangeCheckBuiltinRunner {
            ratio,
            base: 0,
            stop_ptr: None,
            included,
        }
    }

    pub fn initialize_segments(&mut self, segments: &mut MemorySegmentManager) {
        self.base = segments.add().segment_index as usize // segments.add() always returns a positive index
    }

    pub fn initial_stack(&self) -> Vec<MaybeRelocatable> {
        if self.included {
            vec![MaybeRelocatable::from((self.base as isize, 0))]
        } else {
            vec![]
        }
    }

    pub fn base(&self) -> usize {
        self.base
    }

    pub fn ratio(&self) -> Option<u32> {
        self.ratio.map(|ratio| ratio.numerator)
    }

    pub fn ratio_den(&self) -> Option<u32> {
        self.ratio.map(|ratio| ratio.denominator)
    }

    pub fn name(&self) -> BuiltinName {
        match N_PARTS {
            RC_N_PARTS_96 => BuiltinName::range_check96,
            _ => BuiltinName::range_check,
        }
    }

    pub fn n_parts(&self) -> u64 {
        N_PARTS
    }

    pub fn bound(&self) -> &'static Felt252 {
        match N_PARTS {
            RC_N_PARTS_96 => &BOUND_96,
            _ => &BOUND_STANDARD,
        }
    }

    pub fn add_validation_rule(&self, memory: &mut Memory) {
        let rule = ValidationRule(Box::new(
            |memory: &Memory, address: Relocatable| -> Result<Vec<Relocatable>, MemoryError> {
                let num = memory
                    .get_integer(address)
                    .map_err(|_| MemoryError::RangeCheckFoundNonInt(Box::new(address)))?;
                if num.bits() as u64 <= N_PARTS * INNER_RC_BOUND_SHIFT {
                    Ok(vec![address.to_owned()])
                } else {
                    Err(MemoryError::RangeCheckNumOutOfBounds(Box::new((
                        num.into_owned(),
                        Felt252::TWO.pow((N_PARTS * INNER_RC_BOUND_SHIFT) as u128),
                    ))))
                }
            },
        ));
        memory.add_validation_rule(self.base, rule);
    }

    pub fn get_used_cells(&self, segments: &MemorySegmentManager) -> Result<usize, MemoryError> {
        segments
            .get_segment_used_size(self.base)
            .ok_or(MemoryError::MissingSegmentUsedSizes)
    }

    pub fn get_range_check_usage(&self, memory: &Memory) -> Option<(usize, usize)> {
        let range_check_segment = memory.data.get(self.base)?;
        let mut rc_bounds =
            (!range_check_segment.is_empty()).then_some((usize::MAX, usize::MIN))?;

        // Split value into n_parts parts of less than _INNER_RC_BOUND size.
        for value in range_check_segment {
            rc_bounds = value
                .get_value()?
                .get_int_ref()?
                .to_le_digits()
                // TODO: maybe skip leading zeros
                .into_iter()
                .flat_map(|digit| {
                    (0..=3)
                        .rev()
                        .map(move |i| ((digit >> (i * INNER_RC_BOUND_SHIFT)) & INNER_RC_BOUND_MASK))
                })
                .take(N_PARTS as usize)
                .fold(rc_bounds, |mm, x| {
                    (min(mm.0, x as usize), max(mm.1, x as usize))
                });
        }
        Some(rc_bounds)
    }

    pub fn get_used_instances(
        &self,
        segments: &MemorySegmentManager,
    ) -> Result<usize, MemoryError> {
        self.get_used_cells(segments)
    }

    pub fn air_private_input(&self, memory: &Memory) -> Vec<PrivateInput> {
        let mut private_inputs = vec![];
        if let Some(segment) = memory.data.get(self.base) {
            for (index, cell) in segment.iter().enumerate() {
                if let Some(value) = cell.get_value().and_then(|value| value.get_int()) {
                    private_inputs.push(PrivateInput::Value(PrivateInputValue { index, value }))
                }
            }
        }
        private_inputs
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::relocatable;
    use crate::types::builtin_name::BuiltinName;
    use crate::vm::errors::runner_errors::RunnerError;
    use crate::vm::vm_memory::memory::Memory;
    use crate::{
        hint_processor::builtin_hint_processor::builtin_hint_processor_definition::BuiltinHintProcessor,
        types::program::Program, utils::test_utils::*, vm::runners::builtin_runner::BuiltinRunner,
    };

    #[cfg(target_arch = "wasm32")]
    use wasm_bindgen_test::*;

    #[test]
    #[cfg_attr(target_arch = "wasm32", wasm_bindgen_test)]
    fn get_used_instances() {
        let builtin = RangeCheckBuiltinRunner::<RC_N_PARTS_STANDARD>::new(Some(10), true);

        let mut vm = vm!();
        vm.segments.segment_used_sizes = Some(vec![1]);

        assert_eq!(builtin.get_used_instances(&vm.segments), Ok(1));
    }

    #[test]
    #[cfg_attr(target_arch = "wasm32", wasm_bindgen_test)]
    fn final_stack() {
        let mut builtin: BuiltinRunner =
            RangeCheckBuiltinRunner::<RC_N_PARTS_STANDARD>::new(Some(10), true).into();

        let mut vm = vm!();

        vm.segments = segments![
            ((0, 0), (0, 0)),
            ((0, 1), (0, 1)),
            ((2, 0), (0, 0)),
            ((2, 1), (0, 0))
        ];

        vm.segments.segment_used_sizes = Some(vec![0]);

        let pointer = Relocatable::from((2, 2));

        assert_eq!(
            builtin.final_stack(&vm.segments, pointer).unwrap(),
            Relocatable::from((2, 1))
        );
    }

    #[test]
    #[cfg_attr(target_arch = "wasm32", wasm_bindgen_test)]
    fn final_stack_error_stop_pointer() {
        let mut builtin: BuiltinRunner =
            RangeCheckBuiltinRunner::<RC_N_PARTS_STANDARD>::new(Some(10), true).into();

        let mut vm = vm!();

        vm.segments = segments![
            ((0, 0), (0, 0)),
            ((0, 1), (0, 1)),
            ((2, 0), (0, 0)),
            ((2, 1), (0, 0))
        ];

        vm.segments.segment_used_sizes = Some(vec![998]);

        let pointer = Relocatable::from((2, 2));

        assert_eq!(
            builtin.final_stack(&vm.segments, pointer),
            Err(RunnerError::InvalidStopPointer(Box::new((
                BuiltinName::range_check,
                relocatable!(0, 998),
                relocatable!(0, 0)
            ))))
        );
    }

    #[test]
    #[cfg_attr(target_arch = "wasm32", wasm_bindgen_test)]
    fn final_stack_error_when_notincluded() {
        let mut builtin: BuiltinRunner =
            RangeCheckBuiltinRunner::<RC_N_PARTS_STANDARD>::new(Some(10), false).into();

        let mut vm = vm!();

        vm.segments = segments![
            ((0, 0), (0, 0)),
            ((0, 1), (0, 1)),
            ((2, 0), (0, 0)),
            ((2, 1), (0, 0))
        ];

        vm.segments.segment_used_sizes = Some(vec![0]);

        let pointer = Relocatable::from((2, 2));

        assert_eq!(
            builtin.final_stack(&vm.segments, pointer).unwrap(),
            Relocatable::from((2, 2))
        );
    }

    #[test]
    #[cfg_attr(target_arch = "wasm32", wasm_bindgen_test)]
    fn final_stack_error_non_relocatable() {
        let mut builtin: BuiltinRunner =
            RangeCheckBuiltinRunner::<RC_N_PARTS_STANDARD>::new(Some(10), true).into();

        let mut vm = vm!();

        vm.segments = segments![
            ((0, 0), (0, 0)),
            ((0, 1), (0, 1)),
            ((2, 0), (0, 0)),
            ((2, 1), 2)
        ];

        vm.segments.segment_used_sizes = Some(vec![0]);

        let pointer = Relocatable::from((2, 2));

        assert_eq!(
            builtin.final_stack(&vm.segments, pointer),
            Err(RunnerError::NoStopPointer(Box::new(
                BuiltinName::range_check
            )))
        );
    }

    #[test]
    #[cfg_attr(target_arch = "wasm32", wasm_bindgen_test)]
    fn get_used_cells_and_allocated_size_test() {
        let builtin: BuiltinRunner =
            RangeCheckBuiltinRunner::<RC_N_PARTS_STANDARD>::new(Some(10), true).into();

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

        cairo_runner.vm.segments.segment_used_sizes = Some(vec![0]);

        let mut hint_processor = BuiltinHintProcessor::new_empty();

        let address = cairo_runner.initialize(false).unwrap();

        cairo_runner
            .run_until_pc(address, &mut hint_processor)
            .unwrap();

        assert_eq!(
            builtin.get_used_cells_and_allocated_size(&cairo_runner.vm),
            Ok((0, 1))
        );
    }

    #[test]
    #[cfg_attr(target_arch = "wasm32", wasm_bindgen_test)]
    fn get_allocated_memory_units() {
        let builtin: BuiltinRunner =
            RangeCheckBuiltinRunner::<RC_N_PARTS_STANDARD>::new(Some(10), true).into();

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
    fn initialize_segments_for_range_check() {
        let mut builtin = RangeCheckBuiltinRunner::<RC_N_PARTS_STANDARD>::new(Some(8), true);
        let mut segments = MemorySegmentManager::new();
        builtin.initialize_segments(&mut segments);
        assert_eq!(builtin.base, 0);
    }

    #[test]
    #[cfg_attr(target_arch = "wasm32", wasm_bindgen_test)]
    fn get_initial_stack_for_range_check_with_base() {
        let mut builtin = RangeCheckBuiltinRunner::<RC_N_PARTS_STANDARD>::new(Some(8), true);
        builtin.base = 1;
        let initial_stack = builtin.initial_stack();
        assert_eq!(
            initial_stack[0].clone(),
            MaybeRelocatable::RelocatableValue((builtin.base() as isize, 0).into())
        );
        assert_eq!(initial_stack.len(), 1);
    }

    #[test]
    #[cfg_attr(target_arch = "wasm32", wasm_bindgen_test)]
    fn test_base() {
        let builtin = RangeCheckBuiltinRunner::<RC_N_PARTS_STANDARD>::new(Some(8), true);
        assert_eq!(builtin.base(), 0);
    }

    #[test]
    #[cfg_attr(target_arch = "wasm32", wasm_bindgen_test)]
    fn test_ratio() {
        let builtin = RangeCheckBuiltinRunner::<RC_N_PARTS_STANDARD>::new(Some(8), true);
        assert_eq!(builtin.ratio(), Some(8));
    }

    #[test]
    #[cfg_attr(target_arch = "wasm32", wasm_bindgen_test)]
    fn get_used_cells_missing_segment_used_sizes() {
        let builtin = BuiltinRunner::RangeCheck(
            RangeCheckBuiltinRunner::<RC_N_PARTS_STANDARD>::new(Some(256), true),
        );
        let vm = vm!();

        assert_eq!(
            builtin.get_used_cells(&vm.segments),
            Err(MemoryError::MissingSegmentUsedSizes)
        );
    }

    #[test]
    #[cfg_attr(target_arch = "wasm32", wasm_bindgen_test)]
    fn get_used_cells_empty() {
        let builtin = BuiltinRunner::RangeCheck(
            RangeCheckBuiltinRunner::<RC_N_PARTS_STANDARD>::new(Some(256), true),
        );
        let mut vm = vm!();

        vm.segments.segment_used_sizes = Some(vec![0]);
        assert_eq!(builtin.get_used_cells(&vm.segments), Ok(0));
    }

    #[test]
    #[cfg_attr(target_arch = "wasm32", wasm_bindgen_test)]
    fn get_used_cells() {
        let builtin = BuiltinRunner::RangeCheck(
            RangeCheckBuiltinRunner::<RC_N_PARTS_STANDARD>::new(Some(256), true),
        );
        let mut vm = vm!();

        vm.segments.segment_used_sizes = Some(vec![4]);
        assert_eq!(builtin.get_used_cells(&vm.segments), Ok(4));
    }

    #[test]
    #[cfg_attr(target_arch = "wasm32", wasm_bindgen_test)]
    fn get_range_check_usage_succesful_a() {
        let builtin = RangeCheckBuiltinRunner::<RC_N_PARTS_STANDARD>::new(Some(8), true);
        let memory = memory![((0, 0), 1), ((0, 1), 2), ((0, 2), 3), ((0, 3), 4)];
        assert_eq!(builtin.get_range_check_usage(&memory), Some((0, 4)));
    }

    #[test]
    #[cfg_attr(target_arch = "wasm32", wasm_bindgen_test)]
    fn get_range_check_usage_succesful_b() {
        let builtin = RangeCheckBuiltinRunner::<RC_N_PARTS_STANDARD>::new(Some(8), true);
        let memory = memory![
            ((0, 0), 1465218365),
            ((0, 1), 2134570341),
            ((0, 2), 31349610736_i64),
            ((0, 3), 413468326585859_i64)
        ];
        assert_eq!(builtin.get_range_check_usage(&memory), Some((0, 62821)));
    }

    #[test]
    #[cfg_attr(target_arch = "wasm32", wasm_bindgen_test)]
    fn get_range_check_usage_succesful_c() {
        let builtin = RangeCheckBuiltinRunner::<RC_N_PARTS_STANDARD>::new(Some(8), true);
        let memory = memory![
            ((0, 0), 634834751465218365_i64),
            ((0, 1), 42876922134570341_i64),
            ((0, 2), 23469831349610736_i64),
            ((0, 3), 23468413468326585859_i128),
            ((0, 4), 75346043276073460326_i128),
            ((0, 5), 87234598724867609478353436890268_i128)
        ];
        assert_eq!(builtin.get_range_check_usage(&memory), Some((0, 61576)));
    }

    #[test]
    #[cfg_attr(target_arch = "wasm32", wasm_bindgen_test)]
    fn get_range_check_empty_memory() {
        let builtin = RangeCheckBuiltinRunner::<RC_N_PARTS_STANDARD>::new(Some(8), true);
        let memory = Memory::new();
        assert_eq!(builtin.get_range_check_usage(&memory), None);
    }

    /// Test that the method get_used_perm_range_check_units works as intended.
    #[test]
    #[cfg_attr(target_arch = "wasm32", wasm_bindgen_test)]
    fn get_used_perm_range_check_units() {
        let builtin_runner: BuiltinRunner =
            RangeCheckBuiltinRunner::<RC_N_PARTS_STANDARD>::new(Some(8), true).into();
        let mut vm = vm!();

        vm.current_step = 8;
        vm.segments.segment_used_sizes = Some(vec![1]);
        assert_eq!(builtin_runner.get_used_perm_range_check_units(&vm), Ok(8));
    }

    #[test]
    #[cfg_attr(target_arch = "wasm32", wasm_bindgen_test)]
    fn get_air_private_input() {
        let builtin: BuiltinRunner =
            RangeCheckBuiltinRunner::<RC_N_PARTS_STANDARD>::new(None, true).into();

        let segments = segments![((0, 0), 0), ((0, 1), 1), ((0, 2), 2)];
        assert_eq!(
            builtin.air_private_input(&segments),
            (vec![
                PrivateInput::Value(PrivateInputValue {
                    index: 0,
                    value: 0.into(),
                }),
                PrivateInput::Value(PrivateInputValue {
                    index: 1,
                    value: 1.into(),
                }),
                PrivateInput::Value(PrivateInputValue {
                    index: 2,
                    value: 2.into(),
                }),
            ]),
        );
    }
}
