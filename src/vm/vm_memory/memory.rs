use crate::{
    types::relocatable::{MaybeRelocatable, Relocatable},
    utils::from_relocatable_to_indexes,
    vm::errors::{memory_errors::MemoryError, vm_errors::VirtualMachineError},
};
use felt::Felt;
use num_traits::ToPrimitive;
use std::{
    borrow::Cow,
    collections::{HashMap, HashSet},
    fmt::{Display, Formatter},
};
pub struct ValidationRule(
    #[allow(clippy::type_complexity)]
    pub  Box<dyn Fn(&Memory, Relocatable) -> Result<Vec<Relocatable>, MemoryError>>,
);

pub struct Memory {
    pub data: Vec<Vec<Option<MaybeRelocatable>>>,
    pub temp_data: Vec<Vec<Option<MaybeRelocatable>>>,
    // relocation_rules's keys map to temp_data's indices and therefore begin at
    // zero; that is, segment_index = -1 maps to key 0, -2 to key 1...
    pub(crate) relocation_rules: HashMap<usize, Relocatable>,
    pub validated_addresses: HashSet<Relocatable>,
    validation_rules: HashMap<usize, ValidationRule>,
}

impl Memory {
    pub fn new() -> Memory {
        Memory {
            data: Vec::<Vec<Option<MaybeRelocatable>>>::new(),
            temp_data: Vec::<Vec<Option<MaybeRelocatable>>>::new(),
            relocation_rules: HashMap::new(),
            validated_addresses: HashSet::<Relocatable>::new(),
            validation_rules: HashMap::new(),
        }
    }
    ///Inserts an MaybeRelocatable value into an address given by a MaybeRelocatable::Relocatable
    /// Will panic if the segment index given by the address corresponds to a non-allocated segment
    /// If the address isnt contiguous with previously inserted data, memory gaps will be represented by inserting None values
    pub fn insert<'a, K: 'a, V: 'a>(&mut self, key: &'a K, val: &'a V) -> Result<(), MemoryError>
    where
        Relocatable: TryFrom<&'a K>,
        MaybeRelocatable: From<&'a V>,
    {
        let relocatable: Relocatable = key
            .try_into()
            .map_err(|_| MemoryError::AddressNotRelocatable)?;
        let val = MaybeRelocatable::from(val);
        let (value_index, value_offset) = from_relocatable_to_indexes(relocatable);

        let data = if relocatable.segment_index.is_negative() {
            &mut self.temp_data
        } else {
            &mut self.data
        };

        let data_len = data.len();
        let segment = data
            .get_mut(value_index)
            .ok_or(MemoryError::UnallocatedSegment(value_index, data_len))?;

        //Check if the element is inserted next to the last one on the segment
        //Forgoing this check would allow data to be inserted in a different index
        if segment.len() <= value_offset {
            segment.resize(value_offset + 1, None);
        }
        // At this point there's *something* in there

        match segment[value_offset] {
            None => segment[value_offset] = Some(val),
            Some(ref current_value) => {
                if current_value != &val {
                    //Existing memory cannot be changed
                    return Err(MemoryError::InconsistentMemory(
                        relocatable.into(),
                        current_value.to_owned(),
                        val,
                    ));
                }
            }
        };
        self.validate_memory_cell(relocatable)
    }

    /// Retrieve a value from memory (either normal or temporary) and apply relocation rules
    pub(crate) fn get<'a, 'b: 'a, K: 'a>(
        &'b self,
        key: &'a K,
    ) -> Result<Option<Cow<MaybeRelocatable>>, MemoryError>
    where
        Relocatable: TryFrom<&'a K>,
    {
        let relocatable: Relocatable = key
            .try_into()
            .map_err(|_| MemoryError::AddressNotRelocatable)?;

        let data = if relocatable.segment_index.is_negative() {
            &self.temp_data
        } else {
            &self.data
        };
        let (i, j) = from_relocatable_to_indexes(relocatable);
        if data.len() > i && data[i].len() > j {
            if let Some(ref element) = data[i][j] {
                return Ok(Some(self.relocate_value(element)));
            }
        }

        Ok(None)
    }

    // Version of Memory.relocate_value() that doesn't require a self reference
    fn relocate_address(
        addr: Relocatable,
        relocation_rules: &HashMap<usize, Relocatable>,
    ) -> MaybeRelocatable {
        let segment_idx = addr.segment_index;
        if segment_idx >= 0 {
            return addr.into();
        }

        // Adjust the segment index to begin at zero, as per the struct field's
        match relocation_rules.get(&(-(segment_idx + 1) as usize)) {
            Some(x) => (x + addr.offset).into(),
            None => addr.into(),
        }
    }

    /// Relocates the memory according to the relocation rules and clears `self.relocaction_rules`.
    pub fn relocate_memory(&mut self) -> Result<(), MemoryError> {
        if self.relocation_rules.is_empty() || self.temp_data.is_empty() {
            return Ok(());
        }
        // Relocate temporary addresses in memory
        for segment in self.data.iter_mut().chain(self.temp_data.iter_mut()) {
            for value in segment.iter_mut() {
                match value {
                    Some(MaybeRelocatable::RelocatableValue(addr)) if addr.segment_index < 0 => {
                        *value = Some(Memory::relocate_address(*addr, &self.relocation_rules));
                    }
                    _ => {}
                }
            }
        }
        // Move relocated temporary memory into the real memory
        for index in (0..self.temp_data.len()).rev() {
            if let Some(base_addr) = self.relocation_rules.get(&index) {
                let data_segment = self.temp_data.remove(index);
                // Insert the to-be relocated segment into the real memory
                let mut addr = *base_addr;
                if let Some(s) = self.data.get_mut(addr.segment_index as usize) {
                    s.reserve_exact(data_segment.len())
                }
                for elem in data_segment {
                    if let Some(value) = elem {
                        // Rely on Memory::insert to catch memory inconsistencies
                        self.insert(&addr, &value)?;
                    }
                    addr = addr + 1;
                }
            }
        }
        self.relocation_rules.clear();
        Ok(())
    }

    /// Add a new relocation rule.
    ///
    /// Will return an error if any of the following conditions are not met:
    ///   - Source address's segment must be negative (temporary).
    ///   - Source address's offset must be zero.
    ///   - There shouldn't already be relocation at the source segment.
    pub(crate) fn add_relocation_rule(
        &mut self,
        src_ptr: Relocatable,
        dst_ptr: Relocatable,
    ) -> Result<(), MemoryError> {
        if src_ptr.segment_index >= 0 {
            return Err(MemoryError::AddressNotInTemporarySegment(
                src_ptr.segment_index,
            ));
        }
        if src_ptr.offset != 0 {
            return Err(MemoryError::NonZeroOffset(src_ptr.offset));
        }

        // Adjust the segment index to begin at zero, as per the struct field's
        // comment.
        let segment_index = -(src_ptr.segment_index + 1) as usize;
        if self.relocation_rules.contains_key(&segment_index) {
            return Err(MemoryError::DuplicatedRelocation(src_ptr.segment_index));
        }

        self.relocation_rules.insert(segment_index, dst_ptr);
        Ok(())
    }

    //Gets the value from memory address.
    //If the value is an MaybeRelocatable::Int(Bigint) return &Bigint
    //else raises Err
    pub fn get_integer(&self, key: Relocatable) -> Result<Cow<Felt>, VirtualMachineError> {
        match self.get(&key).map_err(VirtualMachineError::MemoryError)? {
            Some(Cow::Borrowed(MaybeRelocatable::Int(int))) => Ok(Cow::Borrowed(int)),
            Some(Cow::Owned(MaybeRelocatable::Int(int))) => Ok(Cow::Owned(int)),
            _ => Err(VirtualMachineError::ExpectedInteger(
                MaybeRelocatable::from(key),
            )),
        }
    }

    pub fn get_relocatable(&self, key: Relocatable) -> Result<Relocatable, VirtualMachineError> {
        match self.get(&key).map_err(VirtualMachineError::MemoryError)? {
            Some(Cow::Borrowed(MaybeRelocatable::RelocatableValue(rel))) => Ok(*rel),
            Some(Cow::Owned(MaybeRelocatable::RelocatableValue(rel))) => Ok(rel),
            _ => Err(VirtualMachineError::ExpectedRelocatable(
                MaybeRelocatable::from(key),
            )),
        }
    }

    pub fn insert_value<T: Into<MaybeRelocatable>>(
        &mut self,
        key: Relocatable,
        val: T,
    ) -> Result<(), VirtualMachineError> {
        self.insert(&key, &val.into())
            .map_err(VirtualMachineError::MemoryError)
    }

    pub fn add_validation_rule(&mut self, segment_index: usize, rule: ValidationRule) {
        self.validation_rules.insert(segment_index, rule);
    }

    fn validate_memory_cell(&mut self, addr: Relocatable) -> Result<(), MemoryError> {
        if !self.validated_addresses.contains(&addr) {
            if let Some(rule) = addr
                .segment_index
                .to_usize()
                .and_then(|x| self.validation_rules.get(&x))
            {
                self.validated_addresses.extend(rule.0(self, addr)?);
            }
        }
        Ok(())
    }
    ///Applies validation_rules to the current memory
    pub fn validate_existing_memory(&mut self) -> Result<(), MemoryError> {
        for (index, rule) in &self.validation_rules {
            if *index < self.data.len() {
                for offset in 0..self.data[*index].len() {
                    let addr = Relocatable::from((*index as isize, offset));
                    if !self.validated_addresses.contains(&addr) {
                        self.validated_addresses.extend(rule.0(self, addr)?);
                    }
                }
            }
        }
        Ok(())
    }

    pub fn get_range(
        &self,
        addr: &MaybeRelocatable,
        size: usize,
    ) -> Result<Vec<Option<Cow<MaybeRelocatable>>>, MemoryError> {
        let mut values = Vec::new();

        for i in 0..size {
            values.push(self.get(&addr.add_usize(i))?);
        }

        Ok(values)
    }

    pub fn get_continuous_range(
        &self,
        addr: &MaybeRelocatable,
        size: usize,
    ) -> Result<Vec<MaybeRelocatable>, MemoryError> {
        let mut values = Vec::with_capacity(size);

        for i in 0..size {
            values.push(match self.get(&addr.add_usize(i))? {
                Some(elem) => elem.into_owned(),
                None => return Err(MemoryError::GetRangeMemoryGap),
            });
        }

        Ok(values)
    }

    pub fn get_integer_range(
        &self,
        addr: Relocatable,
        size: usize,
    ) -> Result<Vec<Cow<Felt>>, VirtualMachineError> {
        let mut values = Vec::new();

        for i in 0..size {
            values.push(self.get_integer(addr + i)?);
        }

        Ok(values)
    }
}

impl Display for Memory {
    fn fmt(&self, f: &mut Formatter) -> std::fmt::Result {
        for (i, segment) in self.temp_data.iter().enumerate() {
            for (j, cell) in segment.iter().enumerate() {
                if let Some(cell) = cell {
                    let temp_segment = i + 1;
                    writeln!(f, "(-{temp_segment},{j}) : {cell}")?;
                }
            }
        }
        for (i, segment) in self.data.iter().enumerate() {
            for (j, cell) in segment.iter().enumerate() {
                if let Some(cell) = cell {
                    writeln!(f, "({i},{j}) : {cell}")?;
                }
            }
        }
        writeln!(f, "}}")
    }
}

pub(crate) trait RelocateValue<'a, Input: 'a, Output: 'a> {
    fn relocate_value(&self, value: Input) -> Output;
}

impl RelocateValue<'_, Relocatable, Relocatable> for Memory {
    fn relocate_value(&self, addr: Relocatable) -> Relocatable {
        let segment_idx = addr.segment_index;
        if segment_idx >= 0 {
            return addr;
        }

        // Adjust the segment index to begin at zero, as per the struct field's
        // comment.
        match self.relocation_rules.get(&(-(segment_idx + 1) as usize)) {
            Some(x) => x + addr.offset,
            None => addr,
        }
    }
}

impl<'a> RelocateValue<'a, &'a Felt, &'a Felt> for Memory {
    fn relocate_value(&self, value: &'a Felt) -> &'a Felt {
        value
    }
}

impl<'a> RelocateValue<'a, &'a MaybeRelocatable, Cow<'a, MaybeRelocatable>> for Memory {
    fn relocate_value(&self, value: &'a MaybeRelocatable) -> Cow<'a, MaybeRelocatable> {
        match value {
            MaybeRelocatable::Int(_) => Cow::Borrowed(value),
            MaybeRelocatable::RelocatableValue(addr) => {
                Cow::Owned(self.relocate_value(*addr).into())
            }
        }
    }
}

impl Default for Memory {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod memory_tests {
    use super::*;
    use crate::{
        types::instance_definitions::ecdsa_instance_def::EcdsaInstanceDef,
        utils::test_utils::{mayberelocatable, memory},
        vm::{
            runners::builtin_runner::{RangeCheckBuiltinRunner, SignatureBuiltinRunner},
            vm_memory::memory_segments::MemorySegmentManager,
        },
    };
    use assert_matches::assert_matches;
    use felt::felt_str;

    use crate::vm::errors::memory_errors::MemoryError;

    use crate::utils::test_utils::memory_from_memory;
    use crate::utils::test_utils::memory_inner;

    pub fn memory_from(
        key_val_list: Vec<(MaybeRelocatable, MaybeRelocatable)>,
        num_segements: usize,
    ) -> Result<Memory, MemoryError> {
        let mut memory = Memory::new();
        for _ in 0..num_segements {
            memory.data.push(Vec::new());
        }
        for (key, val) in key_val_list.iter() {
            memory.insert(key, val)?;
        }
        Ok(memory)
    }

    #[test]
    fn insert_and_get_succesful() {
        let key = MaybeRelocatable::from((0, 0));
        let val = MaybeRelocatable::from(Felt::new(5));
        let mut memory = Memory::new();
        memory.data.push(Vec::new());
        memory.insert(&key, &val).unwrap();
        assert_eq!(
            memory.get(&key).unwrap().unwrap().as_ref(),
            &MaybeRelocatable::from(Felt::new(5))
        );
    }

    #[test]
    fn get_valuef_from_temp_segment() {
        let mut memory = Memory::new();
        memory.temp_data = vec![vec![None, None, Some(mayberelocatable!(8))]];
        assert_eq!(
            memory
                .get(&mayberelocatable!(-1, 2))
                .unwrap()
                .unwrap()
                .as_ref(),
            &mayberelocatable!(8),
        );
    }

    #[test]
    fn insert_value_in_temp_segment() {
        let key = MaybeRelocatable::from((-1, 3));
        let val = MaybeRelocatable::from(Felt::new(8));
        let mut memory = Memory::new();
        memory.temp_data.push(Vec::new());
        memory.insert(&key, &val).unwrap();
        assert_eq!(
            memory.temp_data[0][3],
            Some(MaybeRelocatable::from(Felt::new(8)))
        );
    }

    #[test]
    fn insert_and_get_from_temp_segment_succesful() {
        let key = MaybeRelocatable::from((-1, 0));
        let val = MaybeRelocatable::from(Felt::new(5));
        let mut memory = Memory::new();
        memory.temp_data.push(Vec::new());
        memory.insert(&key, &val).unwrap();
        assert_eq!(
            memory.get(&key).unwrap().unwrap().as_ref(),
            &MaybeRelocatable::from(Felt::new(5)),
        );
    }

    #[test]
    fn insert_and_get_from_temp_segment_failed() {
        let key = mayberelocatable!(-1, 1);
        let mut memory = Memory::new();
        memory.temp_data = vec![vec![None, Some(mayberelocatable!(8))]];
        assert_eq!(
            memory.insert(&key, &mayberelocatable!(5)),
            Err(MemoryError::InconsistentMemory(
                mayberelocatable!(-1, 1),
                mayberelocatable!(8),
                mayberelocatable!(5)
            ))
        );
    }

    #[test]
    fn get_non_allocated_memory() {
        let key = MaybeRelocatable::from((0, 0));
        let memory = Memory::new();
        assert_eq!(memory.get(&key).unwrap(), None);
    }

    #[test]
    fn get_non_existant_element() {
        let key = MaybeRelocatable::from((0, 0));
        let memory = Memory::new();
        assert_eq!(memory.get(&key).unwrap(), None);
    }

    #[test]
    fn get_non_relocatable_key() {
        let key = MaybeRelocatable::from(Felt::new(0));
        let memory = Memory::new();
        let error = memory.get(&key);
        assert_eq!(error, Err(MemoryError::AddressNotRelocatable));
        assert_eq!(
            error.unwrap_err().to_string(),
            "Memory addresses must be relocatable"
        );
    }

    #[test]
    fn insert_non_allocated_memory() {
        let key = MaybeRelocatable::from((0, 0));
        let val = MaybeRelocatable::from(Felt::new(5));
        let mut memory = Memory::new();
        let error = memory.insert(&key, &val);
        assert_eq!(error, Err(MemoryError::UnallocatedSegment(0, 0)));
    }

    #[test]
    fn insert_inconsistent_memory() {
        let key = MaybeRelocatable::from((0, 0));
        let val_a = MaybeRelocatable::from(Felt::new(5));
        let val_b = MaybeRelocatable::from(Felt::new(6));
        let mut memory = Memory::new();
        memory.data.push(Vec::new());
        memory
            .insert(&key, &val_a)
            .expect("Unexpected memory insert fail");
        let error = memory.insert(&key, &val_b);
        assert_eq!(
            error,
            Err(MemoryError::InconsistentMemory(key, val_a, val_b))
        );
    }

    #[test]
    fn insert_address_not_relocatable() {
        let key = MaybeRelocatable::from(Felt::new(5));
        let val = MaybeRelocatable::from(Felt::new(5));
        let mut memory = Memory::new();
        let error = memory.insert(&key, &val);
        assert_eq!(error, Err(MemoryError::AddressNotRelocatable));
        assert_eq!(
            error.unwrap_err().to_string(),
            "Memory addresses must be relocatable"
        );
    }

    #[test]
    fn insert_non_contiguous_element() {
        let key_a = MaybeRelocatable::from((0, 0));
        let key_b = MaybeRelocatable::from((0, 2));
        let val = MaybeRelocatable::from(Felt::new(5));
        let mut memory = Memory::new();
        memory.data.push(Vec::new());
        memory.insert(&key_a, &val).unwrap();
        memory.insert(&key_b, &val).unwrap();
        assert_eq!(memory.get(&key_b).unwrap().unwrap().as_ref(), &val);
    }

    #[test]
    fn insert_non_contiguous_element_memory_gaps_none() {
        let key_a = MaybeRelocatable::from((0, 0));
        let key_b = MaybeRelocatable::from((0, 5));
        let val = MaybeRelocatable::from(Felt::new(5));
        let mut memory = Memory::new();
        memory.data.push(Vec::new());
        memory.insert(&key_a, &val).unwrap();
        memory.insert(&key_b, &val).unwrap();
        assert_eq!(memory.get(&key_b).unwrap().unwrap().as_ref(), &val);
        assert_eq!(memory.get(&MaybeRelocatable::from((0, 1))).unwrap(), None);
        assert_eq!(memory.get(&MaybeRelocatable::from((0, 2))).unwrap(), None);
        assert_eq!(memory.get(&MaybeRelocatable::from((0, 3))).unwrap(), None);
        assert_eq!(memory.get(&MaybeRelocatable::from((0, 4))).unwrap(), None);
    }

    #[test]
    fn from_array_test() {
        let mem = memory_from(
            vec![(
                MaybeRelocatable::from((1, 0)),
                MaybeRelocatable::from(Felt::new(5)),
            )],
            2,
        )
        .unwrap();
        assert_matches!(mem.get(&MaybeRelocatable::from((1, 0))), Ok(Some(inner)) if inner.clone().into_owned() == MaybeRelocatable::Int(Felt::new(5)));
    }

    #[test]
    fn validate_existing_memory_for_range_check_within_bounds() {
        let mut builtin = RangeCheckBuiltinRunner::new(8, 8, true);
        let mut segments = MemorySegmentManager::new();
        builtin.initialize_segments(&mut segments);
        builtin.add_validation_rule(&mut segments.memory);
        for _ in 0..3 {
            segments.add();
        }

        segments
            .memory
            .insert(
                &MaybeRelocatable::from((0, 0)),
                &MaybeRelocatable::from(Felt::new(45)),
            )
            .unwrap();
        segments.memory.validate_existing_memory().unwrap();
        assert!(segments
            .memory
            .validated_addresses
            .contains(&Relocatable::from((0, 0))));
    }

    #[test]
    fn validate_existing_memory_for_range_check_outside_bounds() {
        let mut builtin = RangeCheckBuiltinRunner::new(8, 8, true);
        let mut segments = MemorySegmentManager::new();
        segments.add();
        builtin.initialize_segments(&mut segments);
        segments
            .memory
            .insert(
                &MaybeRelocatable::from((1, 0)),
                &MaybeRelocatable::from(Felt::new(-10)),
            )
            .unwrap();
        builtin.add_validation_rule(&mut segments.memory);
        let error = segments.memory.validate_existing_memory();
        assert_eq!(error, Err(MemoryError::NumOutOfBounds));
        assert_eq!(
            error.unwrap_err().to_string(),
            "Range-check validation failed, number is out of valid range"
        );
    }

    #[test]
    fn validate_existing_memory_for_invalid_signature() {
        let mut builtin = SignatureBuiltinRunner::new(&EcdsaInstanceDef::default(), true);
        let mut segments = MemorySegmentManager::new();
        builtin.initialize_segments(&mut segments);
        segments.memory = memory![
            (
                (0, 0),
                (
                    "874739451078007766457464989774322083649278607533249481151382481072868806602",
                    10
                )
            ),
            (
                (0, 1),
                (
                    "-1472574760335685482768423018116732869320670550222259018541069375211356613248",
                    10
                )
            )
        ];
        builtin.add_validation_rule(&mut segments.memory);
        let error = segments.memory.validate_existing_memory();
        assert_eq!(error, Err(MemoryError::SignatureNotFound((0, 0).into())));
    }

    #[test]
    fn validate_existing_memory_for_valid_signature() {
        let mut builtin = SignatureBuiltinRunner::new(&EcdsaInstanceDef::default(), true);

        let signature_r = felt_str!(
            "1839793652349538280924927302501143912227271479439798783640887258675143576352"
        );
        let signature_s = felt_str!(
            "1819432147005223164874083361865404672584671743718628757598322238853218813979"
        );

        builtin
            .add_signature(Relocatable::from((1, 0)), &(signature_r, signature_s))
            .unwrap();

        let mut segments = MemorySegmentManager::new();

        segments.memory = memory![
            (
                (1, 0),
                (
                    "874739451078007766457464989774322083649278607533249481151382481072868806602",
                    10
                )
            ),
            ((1, 1), 2)
        ];

        builtin.initialize_segments(&mut segments);

        builtin.add_validation_rule(&mut segments.memory);

        let result = segments.memory.validate_existing_memory();

        assert_eq!(result, Ok(()))
    }

    #[test]
    fn validate_existing_memory_for_range_check_relocatable_value() {
        let mut builtin = RangeCheckBuiltinRunner::new(8, 8, true);
        let mut segments = MemorySegmentManager::new();
        builtin.initialize_segments(&mut segments);
        segments.memory = memory![((0, 7), (0, 4))];
        builtin.add_validation_rule(&mut segments.memory);
        let error = segments.memory.validate_existing_memory();
        assert_eq!(error, Err(MemoryError::FoundNonInt));
        assert_eq!(
            error.unwrap_err().to_string(),
            "Range-check validation failed, encountered non-int value"
        );
    }

    #[test]
    fn validate_existing_memory_for_range_check_out_of_bounds_diff_segment() {
        let mut builtin = RangeCheckBuiltinRunner::new(8, 8, true);
        let mut segments = MemorySegmentManager::new();
        segments.memory = Memory::new();
        segments.add();
        builtin.initialize_segments(&mut segments);
        segments
            .memory
            .insert(
                &MaybeRelocatable::from((0, 0)),
                &MaybeRelocatable::from(Felt::new(-45)),
            )
            .unwrap();
        builtin.add_validation_rule(&mut segments.memory);
        assert_eq!(segments.memory.validate_existing_memory(), Ok(()));
    }

    #[test]
    fn get_integer_valid() {
        let memory = memory![((0, 0), 10)];
        assert_eq!(
            memory
                .get_integer(Relocatable::from((0, 0)))
                .unwrap()
                .as_ref(),
            &Felt::new(10)
        );
    }

    #[test]
    fn get_integer_invalid_expected_integer() {
        let mut segments = MemorySegmentManager::new();
        segments.add();
        segments
            .memory
            .insert(
                &MaybeRelocatable::from((0, 0)),
                &MaybeRelocatable::from((0, 10)),
            )
            .unwrap();
        assert_matches!(
            segments.memory.get_integer(Relocatable::from((0, 0))),
            Err(VirtualMachineError::ExpectedInteger(
                e
            )) if e == MaybeRelocatable::from((0, 0))
        );
    }

    #[test]
    fn default_memory() {
        let mem: Memory = Default::default();
        assert_eq!(mem.data.len(), 0);
    }

    #[test]
    fn insert_and_get_temporary_succesful() {
        let mut memory = Memory::new();
        memory.temp_data.push(Vec::new());

        let key = MaybeRelocatable::from((-1, 0));
        let val = MaybeRelocatable::from(Felt::new(5));
        memory.insert(&key, &val).unwrap();

        assert_eq!(memory.get(&key).unwrap().unwrap().as_ref(), &val);
    }

    #[test]
    fn add_relocation_rule() {
        let mut memory = Memory::new();

        assert_eq!(
            memory.add_relocation_rule((-1, 0).into(), (1, 2).into()),
            Ok(()),
        );
        assert_eq!(
            memory.add_relocation_rule((-2, 0).into(), (-1, 1).into()),
            Ok(()),
        );
        assert_eq!(
            memory.add_relocation_rule((5, 0).into(), (0, 0).into()),
            Err(MemoryError::AddressNotInTemporarySegment(5)),
        );
        assert_eq!(
            memory.add_relocation_rule((-3, 6).into(), (0, 0).into()),
            Err(MemoryError::NonZeroOffset(6)),
        );
        assert_eq!(
            memory.add_relocation_rule((-1, 0).into(), (0, 0).into()),
            Err(MemoryError::DuplicatedRelocation(-1)),
        );
    }

    #[test]
    fn relocate_value_bigint() {
        let mut memory = Memory::new();
        memory
            .add_relocation_rule((-1, 0).into(), (2, 0).into())
            .unwrap();
        memory
            .add_relocation_rule((-2, 0).into(), (2, 2).into())
            .unwrap();

        // Test when value is Some(BigInt):
        assert_eq!(
            memory.relocate_value(&MaybeRelocatable::Int(Felt::new(0))),
            Cow::Owned(MaybeRelocatable::Int(Felt::new(0))),
        );
    }

    #[test]
    fn relocate_value_mayberelocatable() {
        let mut memory = Memory::new();
        memory
            .add_relocation_rule((-1, 0).into(), (2, 0).into())
            .unwrap();
        memory
            .add_relocation_rule((-2, 0).into(), (2, 2).into())
            .unwrap();

        // Test when value is Some(MaybeRelocatable) with segment_index >= 0:
        assert_eq!(
            memory.relocate_value(&MaybeRelocatable::RelocatableValue((0, 0).into())),
            Cow::Owned(MaybeRelocatable::RelocatableValue((0, 0).into())),
        );
        assert_eq!(
            memory.relocate_value(&MaybeRelocatable::RelocatableValue((5, 0).into())),
            Cow::Owned(MaybeRelocatable::RelocatableValue((5, 0).into())),
        );
    }

    #[test]
    fn relocate_value_mayberelocatable_temporary_segment_no_rules() {
        let mut memory = Memory::new();
        memory
            .add_relocation_rule((-1, 0).into(), (2, 0).into())
            .unwrap();
        memory
            .add_relocation_rule((-2, 0).into(), (2, 2).into())
            .unwrap();

        // Test when value is Some(MaybeRelocatable) with segment_index < 0 and
        // there are no applicable relocation rules:
        assert_eq!(
            memory.relocate_value(&MaybeRelocatable::RelocatableValue((-5, 0).into())),
            Cow::Owned(MaybeRelocatable::RelocatableValue((-5, 0).into())),
        );
    }

    #[test]
    fn relocate_value_mayberelocatable_temporary_segment_rules() {
        let mut memory = Memory::new();
        memory
            .add_relocation_rule((-1, 0).into(), (2, 0).into())
            .unwrap();
        memory
            .add_relocation_rule((-2, 0).into(), (2, 2).into())
            .unwrap();

        // Test when value is Some(MaybeRelocatable) with segment_index < 0 and
        // there are applicable relocation rules:
        assert_eq!(
            memory.relocate_value(&MaybeRelocatable::RelocatableValue((-1, 0).into())),
            Cow::Owned(MaybeRelocatable::RelocatableValue((2, 0).into())),
        );
        assert_eq!(
            memory.relocate_value(&MaybeRelocatable::RelocatableValue((-2, 0).into())),
            Cow::Owned(MaybeRelocatable::RelocatableValue((2, 2).into())),
        );
        assert_eq!(
            memory.relocate_value(&MaybeRelocatable::RelocatableValue((-1, 5).into())),
            Cow::Owned(MaybeRelocatable::RelocatableValue((2, 5).into())),
        );
        assert_eq!(
            memory.relocate_value(&MaybeRelocatable::RelocatableValue((-2, 5).into())),
            Cow::Owned(MaybeRelocatable::RelocatableValue((2, 7).into())),
        );
    }
    #[test]
    fn get_range_for_continuous_memory() {
        let memory = memory![((1, 0), 2), ((1, 1), 3), ((1, 2), 4)];

        let value1 = MaybeRelocatable::from(Felt::new(2));
        let value2 = MaybeRelocatable::from(Felt::new(3));
        let value3 = MaybeRelocatable::from(Felt::new(4));

        let expected_vec = vec![
            Some(Cow::Borrowed(&value1)),
            Some(Cow::Borrowed(&value2)),
            Some(Cow::Borrowed(&value3)),
        ];
        assert_eq!(
            memory.get_range(&MaybeRelocatable::from((1, 0)), 3),
            Ok(expected_vec)
        );
    }

    #[test]
    fn get_range_for_non_continuous_memory() {
        let memory = memory![((1, 0), 2), ((1, 1), 3), ((1, 3), 4)];

        let value1 = MaybeRelocatable::from(Felt::new(2));
        let value2 = MaybeRelocatable::from(Felt::new(3));
        let value3 = MaybeRelocatable::from(Felt::new(4));

        let expected_vec = vec![
            Some(Cow::Borrowed(&value1)),
            Some(Cow::Borrowed(&value2)),
            None,
            Some(Cow::Borrowed(&value3)),
        ];
        assert_eq!(
            memory.get_range(&MaybeRelocatable::from((1, 0)), 4),
            Ok(expected_vec)
        );
    }

    #[test]
    fn get_continuous_range_for_continuous_memory() {
        let memory = memory![((1, 0), 2), ((1, 1), 3), ((1, 2), 4)];

        let value1 = MaybeRelocatable::from(Felt::new(2));
        let value2 = MaybeRelocatable::from(Felt::new(3));
        let value3 = MaybeRelocatable::from(Felt::new(4));

        let expected_vec = vec![value1, value2, value3];
        assert_eq!(
            memory.get_continuous_range(&MaybeRelocatable::from((1, 0)), 3),
            Ok(expected_vec)
        );
    }

    #[test]
    fn get_continuous_range_for_non_continuous_memory() {
        let memory = memory![((1, 0), 2), ((1, 1), 3), ((1, 3), 4)];

        assert_eq!(
            memory.get_continuous_range(&MaybeRelocatable::from((1, 0)), 3),
            Err(MemoryError::GetRangeMemoryGap)
        );
    }

    /// Test that relocate_memory() works when there are no relocation rules.
    #[test]
    fn relocate_memory_empty_relocation_rules() {
        let mut memory = memory![((0, 0), 1), ((0, 1), 2), ((0, 2), 3)];

        assert_eq!(memory.relocate_memory(), Ok(()));
        assert_eq!(
            memory.data,
            vec![vec![
                mayberelocatable!(1).into(),
                mayberelocatable!(2).into(),
                mayberelocatable!(3).into(),
            ]],
        );
    }

    #[test]
    fn relocate_memory_new_segment_with_gap() {
        let mut memory = memory![
            ((0, 0), 1),
            ((0, 1), (-1, 0)),
            ((0, 2), 3),
            ((1, 0), (-1, 1)),
            ((1, 1), 5),
            ((1, 2), (-1, 2))
        ];
        memory.temp_data = vec![vec![
            mayberelocatable!(7).into(),
            mayberelocatable!(8).into(),
            mayberelocatable!(9).into(),
        ]];
        memory
            .add_relocation_rule((-1, 0).into(), (2, 1).into())
            .unwrap();
        memory.data.push(vec![]);

        assert_eq!(memory.relocate_memory(), Ok(()));
        assert_eq!(
            memory.data,
            vec![
                vec![
                    mayberelocatable!(1).into(),
                    mayberelocatable!(2, 1).into(),
                    mayberelocatable!(3).into(),
                ],
                vec![
                    mayberelocatable!(2, 2).into(),
                    mayberelocatable!(5).into(),
                    mayberelocatable!(2, 3).into(),
                ],
                vec![
                    None,
                    mayberelocatable!(7).into(),
                    mayberelocatable!(8).into(),
                    mayberelocatable!(9).into(),
                ]
            ],
        );
        assert!(memory.temp_data.is_empty());
    }

    #[test]
    fn relocate_memory_new_segment() {
        let mut memory = memory![
            ((0, 0), 1),
            ((0, 1), (-1, 0)),
            ((0, 2), 3),
            ((1, 0), (-1, 1)),
            ((1, 1), 5),
            ((1, 2), (-1, 2))
        ];
        memory.temp_data = vec![vec![
            mayberelocatable!(7).into(),
            mayberelocatable!(8).into(),
            mayberelocatable!(9).into(),
        ]];
        memory
            .add_relocation_rule((-1, 0).into(), (2, 0).into())
            .unwrap();
        memory.data.push(vec![]);

        assert_eq!(memory.relocate_memory(), Ok(()));
        assert_eq!(
            memory.data,
            vec![
                vec![
                    mayberelocatable!(1).into(),
                    mayberelocatable!(2, 0).into(),
                    mayberelocatable!(3).into(),
                ],
                vec![
                    mayberelocatable!(2, 1).into(),
                    mayberelocatable!(5).into(),
                    mayberelocatable!(2, 2).into(),
                ],
                vec![
                    mayberelocatable!(7).into(),
                    mayberelocatable!(8).into(),
                    mayberelocatable!(9).into(),
                ]
            ],
        );
        assert!(memory.temp_data.is_empty());
    }

    #[test]
    fn relocate_memory_new_segment_unallocated() {
        let mut memory = memory![
            ((0, 0), 1),
            ((0, 1), (-1, 0)),
            ((0, 2), 3),
            ((1, 0), (-1, 1)),
            ((1, 1), 5),
            ((1, 2), (-1, 2))
        ];
        memory.temp_data = vec![vec![
            mayberelocatable!(7).into(),
            mayberelocatable!(8).into(),
            mayberelocatable!(9).into(),
        ]];
        memory
            .add_relocation_rule((-1, 0).into(), (2, 0).into())
            .unwrap();

        assert_eq!(
            memory.relocate_memory(),
            Err(MemoryError::UnallocatedSegment(2, 2))
        );
    }

    #[test]
    fn relocate_memory_into_existing_segment() {
        let mut memory = memory![
            ((0, 0), 1),
            ((0, 1), (-1, 0)),
            ((0, 2), 3),
            ((1, 0), (-1, 1)),
            ((1, 1), 5),
            ((1, 2), (-1, 2))
        ];
        memory.temp_data = vec![vec![
            mayberelocatable!(7).into(),
            mayberelocatable!(8).into(),
            mayberelocatable!(9).into(),
        ]];
        memory
            .add_relocation_rule((-1, 0).into(), (1, 3).into())
            .unwrap();

        assert_eq!(memory.relocate_memory(), Ok(()));
        assert_eq!(
            memory.data,
            vec![
                vec![
                    mayberelocatable!(1).into(),
                    mayberelocatable!(1, 3).into(),
                    mayberelocatable!(3).into(),
                ],
                vec![
                    mayberelocatable!(1, 4).into(),
                    mayberelocatable!(5).into(),
                    mayberelocatable!(1, 5).into(),
                    mayberelocatable!(7).into(),
                    mayberelocatable!(8).into(),
                    mayberelocatable!(9).into(),
                ],
            ],
        );
        assert!(memory.temp_data.is_empty());
    }

    #[test]
    fn relocate_memory_into_existing_segment_inconsistent_memory() {
        let mut memory = memory![
            ((0, 0), 1),
            ((0, 1), (-1, 0)),
            ((0, 2), 3),
            ((1, 0), (-1, 1)),
            ((1, 1), 5),
            ((1, 2), (-1, 2))
        ];
        memory.temp_data = vec![vec![
            mayberelocatable!(7).into(),
            mayberelocatable!(8).into(),
            mayberelocatable!(9).into(),
        ]];
        memory
            .add_relocation_rule((-1, 0).into(), (1, 0).into())
            .unwrap();

        assert_eq!(
            memory.relocate_memory(),
            Err(MemoryError::InconsistentMemory(
                (1, 0).into(),
                (1, 1).into(),
                7.into(),
            ))
        );
    }

    #[test]
    fn relocate_memory_new_segment_2_temporary_segments_one_relocated() {
        let mut memory = memory![
            ((0, 0), 1),
            ((0, 1), (-1, 0)),
            ((0, 2), 3),
            ((1, 0), (-1, 1)),
            ((1, 1), 5),
            ((1, 2), (-1, 2))
        ];
        memory.temp_data = vec![
            vec![
                mayberelocatable!(7).into(),
                mayberelocatable!(8).into(),
                mayberelocatable!(9).into(),
            ],
            vec![mayberelocatable!(10).into(), mayberelocatable!(11).into()],
        ];
        memory
            .add_relocation_rule((-1, 0).into(), (2, 0).into())
            .unwrap();
        memory.data.push(vec![]);

        assert_eq!(memory.relocate_memory(), Ok(()));
        assert_eq!(
            memory.data,
            vec![
                vec![
                    mayberelocatable!(1).into(),
                    mayberelocatable!(2, 0).into(),
                    mayberelocatable!(3).into(),
                ],
                vec![
                    mayberelocatable!(2, 1).into(),
                    mayberelocatable!(5).into(),
                    mayberelocatable!(2, 2).into(),
                ],
                vec![
                    mayberelocatable!(7).into(),
                    mayberelocatable!(8).into(),
                    mayberelocatable!(9).into(),
                ]
            ],
        );
        assert_eq!(
            memory.temp_data,
            vec![vec![
                mayberelocatable!(10).into(),
                mayberelocatable!(11).into(),
            ]]
        );
    }

    #[test]
    fn relocate_memory_new_segment_2_temporary_segments_relocated() {
        let mut memory = memory![
            ((0, 0), 1),
            ((0, 1), (-1, 0)),
            ((0, 2), 3),
            ((1, 0), (-1, 1)),
            ((1, 1), 5),
            ((1, 2), (-1, 2))
        ];
        memory.temp_data = vec![
            vec![
                mayberelocatable!(7).into(),
                mayberelocatable!(8).into(),
                mayberelocatable!(9).into(),
            ],
            vec![mayberelocatable!(10).into(), mayberelocatable!(11).into()],
        ];
        memory.data.push(vec![]);
        memory
            .add_relocation_rule((-1, 0).into(), (2, 0).into())
            .unwrap();
        memory.data.push(vec![]);
        memory
            .add_relocation_rule((-2, 0).into(), (3, 0).into())
            .unwrap();

        assert_eq!(memory.relocate_memory(), Ok(()));
        assert_eq!(
            memory.data,
            vec![
                vec![
                    mayberelocatable!(1).into(),
                    mayberelocatable!(2, 0).into(),
                    mayberelocatable!(3).into(),
                ],
                vec![
                    mayberelocatable!(2, 1).into(),
                    mayberelocatable!(5).into(),
                    mayberelocatable!(2, 2).into(),
                ],
                vec![
                    mayberelocatable!(7).into(),
                    mayberelocatable!(8).into(),
                    mayberelocatable!(9).into(),
                ],
                vec![mayberelocatable!(10).into(), mayberelocatable!(11).into(),]
            ],
        );
        assert!(memory.temp_data.is_empty());
    }

    #[test]
    fn test_memory_display() {
        let mut memory = memory![
            ((0, 0), 1),
            ((0, 1), (-1, 0)),
            ((0, 2), 3),
            ((1, 0), (-1, 1)),
            ((1, 1), 5),
            ((1, 2), (-1, 2))
        ];

        memory.temp_data = vec![vec![
            mayberelocatable!(-1, 0).into(),
            mayberelocatable!(8).into(),
            mayberelocatable!(9).into(),
        ]];

        assert_eq!(
            format!("{}", memory),
            "(-1,0) : -1:0\n(-1,1) : 8\n(-1,2) : 9\n(0,0) : 1\n(0,1) : -1:0\n(0,2) : 3\n(1,0) : -1:1\n(1,1) : 5\n(1,2) : -1:2\n}\n");
    }

    #[test]
    fn relocate_memory_into_existing_segment_temporary_values_in_temporary_memory() {
        let mut memory = memory![
            ((0, 0), 1),
            ((0, 1), (-1, 0)),
            ((0, 2), 3),
            ((1, 0), (-1, 1)),
            ((1, 1), 5),
            ((1, 2), (-1, 2))
        ];
        memory.temp_data = vec![vec![
            mayberelocatable!(-1, 0).into(),
            mayberelocatable!(8).into(),
            mayberelocatable!(9).into(),
        ]];
        memory
            .add_relocation_rule((-1, 0).into(), (1, 3).into())
            .unwrap();

        assert_eq!(memory.relocate_memory(), Ok(()));
        assert_eq!(
            memory.data,
            vec![
                vec![
                    mayberelocatable!(1).into(),
                    mayberelocatable!(1, 3).into(),
                    mayberelocatable!(3).into(),
                ],
                vec![
                    mayberelocatable!(1, 4).into(),
                    mayberelocatable!(5).into(),
                    mayberelocatable!(1, 5).into(),
                    mayberelocatable!(1, 3).into(),
                    mayberelocatable!(8).into(),
                    mayberelocatable!(9).into(),
                ],
            ],
        );
        assert!(memory.temp_data.is_empty());
    }

    #[test]
    fn relocate_address_with_rules() {
        let mut memory = Memory::new();
        memory
            .add_relocation_rule((-1, 0).into(), (2, 0).into())
            .unwrap();
        memory
            .add_relocation_rule((-2, 0).into(), (2, 2).into())
            .unwrap();

        assert_eq!(
            Memory::relocate_address((-1, 0).into(), &memory.relocation_rules),
            MaybeRelocatable::RelocatableValue((2, 0).into()),
        );
        assert_eq!(
            Memory::relocate_address((-2, 1).into(), &memory.relocation_rules),
            MaybeRelocatable::RelocatableValue((2, 3).into()),
        );
    }

    #[test]
    fn relocate_address_no_rules() {
        let memory = Memory::new();
        assert_eq!(
            Memory::relocate_address((-1, 0).into(), &memory.relocation_rules),
            MaybeRelocatable::RelocatableValue((-1, 0).into()),
        );
        assert_eq!(
            Memory::relocate_address((-2, 1).into(), &memory.relocation_rules),
            MaybeRelocatable::RelocatableValue((-2, 1).into()),
        );
    }

    #[test]
    fn relocate_address_real_addr() {
        let memory = Memory::new();
        assert_eq!(
            Memory::relocate_address((1, 0).into(), &memory.relocation_rules),
            MaybeRelocatable::RelocatableValue((1, 0).into()),
        );
        assert_eq!(
            Memory::relocate_address((1, 1).into(), &memory.relocation_rules),
            MaybeRelocatable::RelocatableValue((1, 1).into()),
        );
    }
}
