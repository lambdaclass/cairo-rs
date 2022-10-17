use num_bigint::BigInt;
use num_integer::Integer;
use std::any::Any;

use crate::types::relocatable::{MaybeRelocatable, Relocatable};
use crate::vm::errors::memory_errors::MemoryError;
use crate::vm::vm_memory::memory::Memory;

pub struct MemorySegmentManager {
    pub num_segments: usize,
    pub segment_used_sizes: Option<Vec<usize>>,
}

impl MemorySegmentManager {
    ///Adds a new segment and returns its starting location as a RelocatableValue.
    pub fn add(&mut self, memory: &mut Memory) -> Relocatable {
        let segment_index = self.num_segments;
        self.num_segments += 1;
        memory.data.push(Vec::new());
        Relocatable {
            segment_index,
            offset: 0,
        }
    }
    ///Writes data into the memory at address ptr and returns the first address after the data.
    pub fn load_data(
        &mut self,
        memory: &mut Memory,
        ptr: &MaybeRelocatable,
        data: Vec<MaybeRelocatable>,
    ) -> Result<MaybeRelocatable, MemoryError> {
        for (num, value) in data.iter().enumerate() {
            memory.insert(&ptr.add_usize_mod(num, None), value)?;
        }
        Ok(ptr.add_usize_mod(data.len(), None))
    }

    pub fn new() -> MemorySegmentManager {
        MemorySegmentManager {
            num_segments: 0,
            segment_used_sizes: None,
        }
    }

    ///Calculates the size (number of non-none elements) of each memory segment
    pub fn compute_effective_sizes(&mut self, memory: &Memory) {
        if self.segment_used_sizes != None {
            return;
        }
        let mut segment_used_sizes = Vec::new();
        for segment in memory.data.iter() {
            segment_used_sizes.push(segment.len());
        }
        self.segment_used_sizes = Some(segment_used_sizes);
    }

    ///Returns a vector that contains the first relocated address of each memory segment
    pub fn relocate_segments(&self) -> Result<Vec<usize>, MemoryError> {
        let first_addr = 1;
        let mut relocation_table = vec![first_addr];
        match &self.segment_used_sizes {
            Some(segment_used_sizes) => {
                for (i, size) in segment_used_sizes.iter().enumerate() {
                    relocation_table.push(relocation_table[i] + size);
                }
            }
            None => return Err(MemoryError::EffectiveSizesNotCalled),
        }
        //The last value corresponds to the total amount of elements across all segments, which isnt needed for relocation.
        relocation_table.pop();
        Ok(relocation_table)
    }

    pub fn gen_arg_vec_bigint(
        &self,
        arg: &[BigInt],
        prime: Option<&BigInt>,
    ) -> Vec<MaybeRelocatable> {
        if let Some(prime) = prime {
            return arg
                .iter()
                .map(|x| MaybeRelocatable::from(x.mod_floor(prime)))
                .collect();
        } else {
            arg.iter()
                .map(|x| MaybeRelocatable::from(x.clone()))
                .collect()
        }
    }

    pub fn write_arg(
        &mut self,
        memory: &mut Memory,
        ptr: &Relocatable,
        arg: &dyn Any,
        prime: Option<&BigInt>,
    ) -> Result<MaybeRelocatable, MemoryError> {
        if let Some(vector) = arg.downcast_ref::<Vec<BigInt>>() {
            let data = self.gen_arg_vec_bigint(vector, prime);
            self.load_data(
                memory,
                &MaybeRelocatable::from((ptr.segment_index, ptr.offset)),
                data,
            )
        } else {
            Err(MemoryError::WriteArg)
        }
    }
}

impl Default for MemorySegmentManager {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{bigint, relocatable, utils::test_utils::*};
    use num_bigint::BigInt;
    use std::vec;

    #[test]
    fn add_segment_no_size() {
        let mut segments = MemorySegmentManager::new();
        let mut memory = Memory::new();
        let base = segments.add(&mut memory);
        assert_eq!(base, relocatable!(0, 0));
        assert_eq!(segments.num_segments, 1);
    }

    #[test]
    fn add_segment_no_size_test_two_segments() {
        let mut segments = MemorySegmentManager::new();
        let mut memory = Memory::new();
        let mut _base = segments.add(&mut memory);
        _base = segments.add(&mut memory);
        assert_eq!(
            _base,
            Relocatable {
                segment_index: 1,
                offset: 0
            }
        );
        assert_eq!(segments.num_segments, 2);
    }

    #[test]
    fn load_data_empty() {
        let data = Vec::new();
        let ptr = MaybeRelocatable::from((0, 3));
        let mut segments = MemorySegmentManager::new();
        let mut memory = Memory::new();
        let current_ptr = segments.load_data(&mut memory, &ptr, data).unwrap();
        assert_eq!(current_ptr, MaybeRelocatable::from((0, 3)));
    }

    #[test]
    fn load_data_one_element() {
        let data = vec![MaybeRelocatable::from(bigint!(4))];
        let ptr = MaybeRelocatable::from((0, 0));
        let mut segments = MemorySegmentManager::new();
        let mut memory = Memory::new();
        segments.add(&mut memory);
        let current_ptr = segments.load_data(&mut memory, &ptr, data).unwrap();
        assert_eq!(current_ptr, MaybeRelocatable::from((0, 1)));
        assert_eq!(
            memory.get(&ptr).unwrap().unwrap().as_ref(),
            &MaybeRelocatable::from(bigint!(4))
        );
    }

    #[test]
    fn load_data_three_elements() {
        let data = vec![
            MaybeRelocatable::from(bigint!(4)),
            MaybeRelocatable::from(bigint!(5)),
            MaybeRelocatable::from(bigint!(6)),
        ];
        let ptr = MaybeRelocatable::from((0, 0));
        let mut segments = MemorySegmentManager::new();
        let mut memory = Memory::new();
        segments.add(&mut memory);
        let current_ptr = segments.load_data(&mut memory, &ptr, data).unwrap();
        assert_eq!(current_ptr, MaybeRelocatable::from((0, 3)));

        assert_eq!(
            memory.get(&ptr).unwrap().unwrap().as_ref(),
            &MaybeRelocatable::from(bigint!(4))
        );
        assert_eq!(
            memory
                .get(&MaybeRelocatable::from((0, 1)))
                .unwrap()
                .unwrap()
                .as_ref(),
            &MaybeRelocatable::from(bigint!(5))
        );
        assert_eq!(
            memory
                .get(&MaybeRelocatable::from((0, 2)))
                .unwrap()
                .unwrap()
                .as_ref(),
            &MaybeRelocatable::from(bigint!(6))
        );
    }
    #[test]
    fn compute_effective_sizes_for_one_segment_memory() {
        let mut segments = MemorySegmentManager::new();
        let memory = memory![((0, 0), 1), ((0, 1), 1), ((0, 2), 1)];
        segments.compute_effective_sizes(&memory);
        assert_eq!(Some(vec![3]), segments.segment_used_sizes);
    }

    #[test]
    fn compute_effective_sizes_for_one_segment_memory_with_gap() {
        let mut segments = MemorySegmentManager::new();
        let mut memory = Memory::new();
        segments.add(&mut memory);
        memory
            .insert(
                &MaybeRelocatable::from((0, 6)),
                &MaybeRelocatable::from(bigint!(1)),
            )
            .unwrap();
        segments.compute_effective_sizes(&memory);
        assert_eq!(Some(vec![7]), segments.segment_used_sizes);
    }

    #[test]
    fn compute_effective_sizes_for_one_segment_memory_with_gaps() {
        let mut segments = MemorySegmentManager::new();
        let memory = memory![((0, 3), 1), ((0, 4), 1), ((0, 7), 1), ((0, 9), 1)];
        segments.compute_effective_sizes(&memory);
        assert_eq!(Some(vec![10]), segments.segment_used_sizes);
    }

    #[test]
    fn compute_effective_sizes_for_three_segment_memory() {
        let mut segments = MemorySegmentManager::new();
        let memory = memory![
            ((0, 0), 1),
            ((0, 1), 1),
            ((0, 2), 1),
            ((1, 0), 1),
            ((1, 1), 1),
            ((1, 2), 1),
            ((2, 0), 1),
            ((2, 1), 1),
            ((2, 2), 1)
        ];
        segments.compute_effective_sizes(&memory);
        assert_eq!(Some(vec![3, 3, 3]), segments.segment_used_sizes);
    }

    #[test]
    fn compute_effective_sizes_for_three_segment_memory_with_gaps() {
        let mut segments = MemorySegmentManager::new();
        let memory = memory![
            ((0, 2), 1),
            ((0, 5), 1),
            ((0, 7), 1),
            ((1, 1), 1),
            ((2, 2), 1),
            ((2, 4), 1),
            ((2, 7), 1)
        ];
        segments.compute_effective_sizes(&memory);
        assert_eq!(Some(vec![8, 2, 8]), segments.segment_used_sizes);
    }

    #[test]
    fn relocate_segments_one_segment() {
        let mut segments = MemorySegmentManager::new();
        segments.segment_used_sizes = Some(vec![3]);
        assert_eq!(
            segments
                .relocate_segments()
                .expect("Couldn't relocate after compute effective sizes"),
            vec![1]
        )
    }

    #[test]
    fn relocate_segments_five_segment() {
        let mut segments = MemorySegmentManager::new();
        segments.segment_used_sizes = Some(vec![3, 3, 56, 78, 8]);
        assert_eq!(
            segments
                .relocate_segments()
                .expect("Couldn't relocate after compute effective sizes"),
            vec![1, 4, 7, 63, 141]
        )
    }

    #[test]
    fn write_arg_with_apply_modulo() {
        let data = vec![bigint!(11), bigint!(12), bigint!(13)];
        let ptr = Relocatable::from((1, 0));
        let mut segments = MemorySegmentManager::new();
        let mut memory = Memory::new();
        for _ in 0..2 {
            segments.add(&mut memory);
        }

        let exec = segments.write_arg(&mut memory, &ptr, &data, Some(&bigint!(5)));

        assert_eq!(exec, Ok(MaybeRelocatable::from((1, 3))));
        assert_eq!(
            memory.data[1],
            vec![
                Some(MaybeRelocatable::from(bigint!(1))),
                Some(MaybeRelocatable::from(bigint!(2))),
                Some(MaybeRelocatable::from(bigint!(3))),
            ]
        );
    }

    #[test]
    fn write_arg_with_no_apply_modulo() {
        let data = vec![bigint!(1), bigint!(2), bigint!(3)];
        let ptr = Relocatable::from((0, 0));
        let mut segments = MemorySegmentManager::new();
        let mut memory = Memory::new();
        segments.add(&mut memory);
        let exec = segments.write_arg(&mut memory, &ptr, &data, None);

        assert_eq!(exec, Ok(MaybeRelocatable::from((0, 3))));
        assert_eq!(
            memory.data[0],
            vec![
                Some(MaybeRelocatable::from(bigint!(1))),
                Some(MaybeRelocatable::from(bigint!(2))),
                Some(MaybeRelocatable::from(bigint!(3))),
            ]
        );
    }

    #[test]
    fn segment_default() {
        let segment_mng_new = MemorySegmentManager::new();
        let segment_mng_def: MemorySegmentManager = Default::default();
        assert_eq!(segment_mng_new.num_segments, segment_mng_def.num_segments);
        assert_eq!(
            segment_mng_new.segment_used_sizes,
            segment_mng_def.segment_used_sizes
        );
    }
}
