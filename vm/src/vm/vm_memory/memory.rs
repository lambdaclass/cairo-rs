use crate::stdlib::{borrow::Cow, collections::HashMap, fmt, prelude::*};

use crate::types::errors::math_errors::MathError;
use crate::vm::runners::cairo_pie::CairoPieMemory;
use crate::Felt252;
use crate::{
    types::relocatable::{MaybeRelocatable, Relocatable},
    utils::from_relocatable_to_indexes,
    vm::errors::memory_errors::MemoryError,
};
use bitvec::prelude as bv;
use core::cmp::Ordering;
use num_traits::ToPrimitive;

pub struct ValidationRule(
    #[allow(clippy::type_complexity)]
    pub  Box<dyn Fn(&Memory, Relocatable) -> Result<Vec<Relocatable>, MemoryError>>,
);

/// [`MemoryCell`] represents an optimized storage layout for the VM memory.
/// It's specified to have both size an alignment of 32 bytes to optimize cache access.
/// Typical cache sizes are 64 bytes, a few cases might be 128 bytes, meaning 32 bytes aligned to
/// 32 bytes boundaries will never get split into two separate lines, avoiding double stalls and
/// reducing false sharing and evictions.
/// The trade off is extra computation for conversion to our "in-flight" `MaybeRelocatable` and
/// `Felt252` as well as some extra copies. Empirically, this seems to be offset by the improved
/// locality of the bigger structure for Lambdaworks. There is a big hit from the conversions when
/// using the `BigUint` implementation, since those force allocations on the heap, but since that's
/// dropped in later versions anyway it's not a priority. For Lambdaworks the new copies are mostly
/// to the stack, which is typically already in the cache.
/// The layout uses the 4 MSB in the first `u64` as flags:
/// - BIT63: NONE flag, 1 when the cell is actually empty.
/// - BIT62: ACCESS flag, 1 when the cell has been accessed in a way observable to Cairo.
/// - BIT61: RELOCATABLE flag, 1 when the contained value is a `Relocatable`, 0 when it is a
///   `Felt252`.
///   `Felt252` values are stored in big-endian order to keep the flag bits free.
///   `Relocatable` values are stored as native endian, with the 3rd word storing the segment index
///   and the 4th word storing the offset.
#[derive(Copy, Clone, Eq, Ord, PartialEq, PartialOrd, Debug)]
#[repr(align(32))]
pub(crate) struct MemoryCell([u64; 4]);

impl MemoryCell {
    pub const NONE_MASK: u64 = 1 << 63;
    pub const ACCESS_MASK: u64 = 1 << 62;
    pub const RELOCATABLE_MASK: u64 = 1 << 61;
    pub const NONE: Self = Self([Self::NONE_MASK, 0, 0, 0]);

    pub fn new(value: MaybeRelocatable) -> Self {
        value.into()
    }

    pub fn is_none(&self) -> bool {
        self.0[0] & Self::NONE_MASK == Self::NONE_MASK
    }

    pub fn is_some(&self) -> bool {
        !self.is_none()
    }

    pub fn mark_accessed(&mut self) {
        self.0[0] |= Self::ACCESS_MASK;
    }

    pub fn is_accessed(&self) -> bool {
        self.0[0] & Self::ACCESS_MASK == Self::ACCESS_MASK
    }

    pub fn get_value(&self) -> Option<MaybeRelocatable> {
        self.is_some().then(|| (*self).into())
    }
}

impl From<MaybeRelocatable> for MemoryCell {
    fn from(value: MaybeRelocatable) -> Self {
        match value {
            MaybeRelocatable::Int(x) => Self(x.to_raw()),
            MaybeRelocatable::RelocatableValue(x) => Self([
                Self::RELOCATABLE_MASK,
                0,
                // NOTE: hack around signedness
                usize::from_ne_bytes(x.segment_index.to_ne_bytes()) as u64,
                x.offset as u64,
            ]),
        }
    }
}

impl From<MemoryCell> for MaybeRelocatable {
    fn from(cell: MemoryCell) -> Self {
        debug_assert!(cell.is_some());
        let flags = cell.0[0];
        match flags & MemoryCell::RELOCATABLE_MASK {
            MemoryCell::RELOCATABLE_MASK => Self::from((
                // NOTE: hack around signedness
                isize::from_ne_bytes((cell.0[2] as usize).to_ne_bytes()),
                cell.0[3] as usize,
            )),
            _ => {
                let mut value = cell.0;
                // Remove all flag bits
                value[0] &= 0x0fffffffffffffff;
                Self::Int(Felt252::from_raw(value))
            }
        }
    }
}

pub struct AddressSet(Vec<bv::BitVec>);

impl AddressSet {
    pub(crate) fn new() -> Self {
        Self(Vec::new())
    }

    pub(crate) fn contains(&self, addr: &Relocatable) -> bool {
        let segment = addr.segment_index;
        if segment.is_negative() {
            return false;
        }

        self.0
            .get(segment as usize)
            .and_then(|segment| segment.get(addr.offset))
            .map(|bit| *bit)
            .unwrap_or(false)
    }

    pub(crate) fn extend(&mut self, addresses: &[Relocatable]) {
        for addr in addresses {
            let segment = addr.segment_index;
            if segment.is_negative() {
                continue;
            }
            let segment = segment as usize;
            if segment >= self.0.len() {
                self.0.resize(segment + 1, bv::BitVec::new());
            }

            let offset = addr.offset;
            if offset >= self.0[segment].len() {
                self.0[segment].resize(offset + 1, false);
            }
            self.0[segment].replace(offset, true);
        }
    }
}

#[cfg(test)]
impl AddressSet {
    pub(crate) fn len(&self) -> usize {
        self.0
            .iter()
            .map(|segment| segment.iter().map(|bit| *bit as usize).sum::<usize>())
            .sum()
    }
}

pub struct Memory {
    pub(crate) data: Vec<Vec<MemoryCell>>,
    pub(crate) temp_data: Vec<Vec<MemoryCell>>,
    // relocation_rules's keys map to temp_data's indices and therefore begin at
    // zero; that is, segment_index = -1 maps to key 0, -2 to key 1...
    #[cfg(not(feature = "extensive_hints"))]
    pub(crate) relocation_rules: HashMap<usize, Relocatable>,
    #[cfg(feature = "extensive_hints")]
    pub(crate) relocation_rules: HashMap<usize, MaybeRelocatable>,
    pub validated_addresses: AddressSet,
    validation_rules: Vec<Option<ValidationRule>>,
}

impl Memory {
    pub fn new() -> Memory {
        Memory {
            data: Vec::new(),
            temp_data: Vec::new(),
            relocation_rules: HashMap::new(),
            validated_addresses: AddressSet::new(),
            validation_rules: Vec::with_capacity(7),
        }
    }

    /// Inserts a value into a memory address
    /// Will return an Error if the segment index given by the address corresponds to a non-allocated segment,
    /// or if the inserted value is inconsistent with the current value at the memory cell
    /// If the address isnt contiguous with previously inserted data, memory gaps will be represented by None values
    pub fn insert<V>(&mut self, key: Relocatable, val: V) -> Result<(), MemoryError>
    where
        MaybeRelocatable: From<V>,
    {
        let val = MaybeRelocatable::from(val);
        let (value_index, value_offset) = from_relocatable_to_indexes(key);

        let data = if key.segment_index.is_negative() {
            &mut self.temp_data
        } else {
            &mut self.data
        };

        let data_len = data.len();
        let segment = data
            .get_mut(value_index)
            .ok_or_else(|| MemoryError::UnallocatedSegment(Box::new((value_index, data_len))))?;

        //Check if the element is inserted next to the last one on the segment
        //Forgoing this check would allow data to be inserted in a different index
        let (len, capacity) = (segment.len(), segment.capacity());
        if len <= value_offset {
            let new_len = value_offset
                .checked_add(1)
                .ok_or(MemoryError::VecCapacityExceeded)?;
            segment
                .try_reserve(new_len.saturating_sub(capacity))
                .map_err(|_| MemoryError::VecCapacityExceeded)?;
            segment.resize(new_len, MemoryCell::NONE);
        }
        // At this point there's *something* in there

        match segment[value_offset].get_value() {
            None => segment[value_offset] = MemoryCell::new(val),
            Some(current_cell) => {
                if current_cell != val {
                    //Existing memory cannot be changed
                    return Err(MemoryError::InconsistentMemory(Box::new((
                        key,
                        current_cell,
                        val,
                    ))));
                }
            }
        };
        self.validate_memory_cell(key)
    }

    /// Retrieve a value from memory (either normal or temporary) and apply relocation rules
    pub(crate) fn get<'a, 'b: 'a, K: 'a>(&'b self, key: &'a K) -> Option<Cow<MaybeRelocatable>>
    where
        Relocatable: TryFrom<&'a K>,
    {
        let relocatable: Relocatable = key.try_into().ok()?;

        let data = if relocatable.segment_index.is_negative() {
            &self.temp_data
        } else {
            &self.data
        };
        let (i, j) = from_relocatable_to_indexes(relocatable);
        let value = data.get(i)?.get(j)?.get_value()?;
        Some(Cow::Owned(self.relocate_value(&value).ok()?.into_owned()))
    }

    // Version of Memory.relocate_value() that doesn't require a self reference
    #[cfg(not(feature = "extensive_hints"))]
    fn relocate_address(
        addr: Relocatable,
        relocation_rules: &HashMap<usize, Relocatable>,
    ) -> Result<MaybeRelocatable, MemoryError> {
        if addr.segment_index < 0 {
            // Adjust the segment index to begin at zero, as per the struct field's
            // comment.
            if let Some(x) = relocation_rules.get(&(-(addr.segment_index + 1) as usize)) {
                return Ok((*x + addr.offset)?.into());
            }
        }
        Ok(addr.into())
    }
    #[cfg(feature = "extensive_hints")]
    fn relocate_address(
        addr: Relocatable,
        relocation_rules: &HashMap<usize, MaybeRelocatable>,
    ) -> Result<MaybeRelocatable, MemoryError> {
        if addr.segment_index < 0 {
            // Adjust the segment index to begin at zero, as per the struct field's
            // comment.
            if let Some(x) = relocation_rules.get(&(-(addr.segment_index + 1) as usize)) {
                return Ok(match x {
                    MaybeRelocatable::RelocatableValue(r) => (*r + addr.offset)?.into(),
                    MaybeRelocatable::Int(i) => i.into(),
                });
            }
        }
        Ok(addr.into())
    }

    /// Relocates the memory according to the relocation rules and clears `self.relocaction_rules`.
    pub fn relocate_memory(&mut self) -> Result<(), MemoryError> {
        if self.relocation_rules.is_empty() || self.temp_data.is_empty() {
            return Ok(());
        }
        // Relocate temporary addresses in memory
        for segment in self.data.iter_mut().chain(self.temp_data.iter_mut()) {
            for cell in segment.iter_mut() {
                let value = cell.get_value();
                match value {
                    Some(MaybeRelocatable::RelocatableValue(addr)) if addr.segment_index < 0 => {
                        let mut new_cell = MemoryCell::new(Memory::relocate_address(
                            addr,
                            &self.relocation_rules,
                        )?);
                        if cell.is_accessed() {
                            new_cell.mark_accessed();
                        }
                        *cell = new_cell;
                    }
                    _ => {}
                }
            }
        }
        // Move relocated temporary memory into the real memory
        for index in (0..self.temp_data.len()).rev() {
            if let Some(base_addr) = self.relocation_rules.get(&index) {
                let data_segment = self.temp_data.remove(index);

                #[cfg(feature = "extensive_hints")]
                let base_addr = match base_addr {
                    MaybeRelocatable::RelocatableValue(addr) => addr,
                    MaybeRelocatable::Int(_) => {
                        continue;
                    }
                };

                // Insert the to-be relocated segment into the real memory
                let mut addr = *base_addr;
                if let Some(s) = self.data.get_mut(addr.segment_index as usize) {
                    s.reserve_exact(data_segment.len())
                }
                for cell in data_segment {
                    if let Some(v) = cell.get_value() {
                        // Rely on Memory::insert to catch memory inconsistencies
                        self.insert(addr, v)?;
                        // If the cell is accessed, mark the relocated one as accessed too
                        if cell.is_accessed() {
                            self.mark_as_accessed(addr)
                        }
                    }
                    addr = (addr + 1)?;
                }
            }
        }
        self.relocation_rules.clear();
        Ok(())
    }
    /// Add a new relocation rule.
    ///
    /// When using feature "extensive_hints" the destination is allowed to be an Integer (via
    /// MaybeRelocatable). Relocating memory to anything other than a `Relocatable` is generally
    /// not useful, but it does make the implementation consistent with the pythonic version.
    ///
    /// Will return an error if any of the following conditions are not met:
    ///   - Source address's segment must be negative (temporary).
    ///   - Source address's offset must be zero.
    ///   - There shouldn't already be relocation at the source segment.
    #[cfg(not(feature = "extensive_hints"))]
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
    #[cfg(feature = "extensive_hints")]
    pub(crate) fn add_relocation_rule(
        &mut self,
        src_ptr: Relocatable,
        dst: MaybeRelocatable,
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

        self.relocation_rules.insert(segment_index, dst);
        Ok(())
    }

    /// Gets the value from memory address as a Felt252 value.
    /// Returns an Error if the value at the memory address is missing or not a Felt252.
    pub fn get_integer(&self, key: Relocatable) -> Result<Cow<Felt252>, MemoryError> {
        match self
            .get(&key)
            .ok_or_else(|| MemoryError::UnknownMemoryCell(Box::new(key)))?
        {
            Cow::Borrowed(MaybeRelocatable::Int(int)) => Ok(Cow::Borrowed(int)),
            Cow::Owned(MaybeRelocatable::Int(int)) => Ok(Cow::Owned(int)),
            _ => Err(MemoryError::ExpectedInteger(Box::new(key))),
        }
    }

    /// Gets the value from memory address as a usize.
    /// Returns an Error if the value at the memory address is missing not a Felt252, or can't be converted to usize.
    pub fn get_usize(&self, key: Relocatable) -> Result<usize, MemoryError> {
        let felt = self.get_integer(key)?.into_owned();
        felt.to_usize()
            .ok_or_else(|| MemoryError::Math(MathError::Felt252ToUsizeConversion(Box::new(felt))))
    }

    /// Gets the value from memory address as a Relocatable value.
    /// Returns an Error if the value at the memory address is missing or not a Relocatable.
    pub fn get_relocatable(&self, key: Relocatable) -> Result<Relocatable, MemoryError> {
        match self
            .get(&key)
            .ok_or_else(|| MemoryError::UnknownMemoryCell(Box::new(key)))?
        {
            Cow::Borrowed(MaybeRelocatable::RelocatableValue(rel)) => Ok(*rel),
            Cow::Owned(MaybeRelocatable::RelocatableValue(rel)) => Ok(rel),
            _ => Err(MemoryError::ExpectedRelocatable(Box::new(key))),
        }
    }

    /// Inserts a value into memory
    /// Returns an error if the memory cell asignment is invalid
    pub fn insert_value<T: Into<MaybeRelocatable>>(
        &mut self,
        key: Relocatable,
        val: T,
    ) -> Result<(), MemoryError> {
        self.insert(key, &val.into())
    }

    pub fn add_validation_rule(&mut self, segment_index: usize, rule: ValidationRule) {
        if segment_index >= self.validation_rules.len() {
            // Fill gaps
            self.validation_rules
                .resize_with(segment_index + 1, || None);
        }
        self.validation_rules.insert(segment_index, Some(rule));
    }

    fn validate_memory_cell(&mut self, addr: Relocatable) -> Result<(), MemoryError> {
        if let Some(Some(rule)) = addr
            .segment_index
            .to_usize()
            .and_then(|x| self.validation_rules.get(x))
        {
            if !self.validated_addresses.contains(&addr) {
                self.validated_addresses
                    .extend(rule.0(self, addr)?.as_slice());
            }
        }
        Ok(())
    }

    ///Applies validation_rules to the current memory
    pub fn validate_existing_memory(&mut self) -> Result<(), MemoryError> {
        for (index, rule) in self.validation_rules.iter().enumerate() {
            if index >= self.data.len() {
                continue;
            }
            let Some(rule) = rule else {
                continue;
            };
            for offset in 0..self.data[index].len() {
                let addr = Relocatable::from((index as isize, offset));
                if !self.validated_addresses.contains(&addr) {
                    self.validated_addresses
                        .extend(rule.0(self, addr)?.as_slice());
                }
            }
        }
        Ok(())
    }

    /// Compares two ranges of values in memory of length `len`
    /// Returns the ordering and the first relative position at which they differ
    /// Special cases:
    /// - `lhs` exists in memory but `rhs` doesn't -> (Ordering::Greater, 0)
    /// - `rhs` exists in memory but `lhs` doesn't -> (Ordering::Less, 0)
    /// - None of `lhs` or `rhs` exist in memory -> (Ordering::Equal, 0)
    ///   Everything else behaves much like `memcmp` in C.
    ///   This is meant as an optimization for hints to avoid allocations.
    pub(crate) fn memcmp(
        &self,
        lhs: Relocatable,
        rhs: Relocatable,
        len: usize,
    ) -> (Ordering, usize) {
        let get_segment = |idx: isize| {
            if idx.is_negative() {
                self.temp_data.get(-(idx + 1) as usize)
            } else {
                self.data.get(idx as usize)
            }
        };
        match (
            get_segment(lhs.segment_index),
            get_segment(rhs.segment_index),
        ) {
            (None, None) => {
                return (Ordering::Equal, 0);
            }
            (Some(_), None) => {
                return (Ordering::Greater, 0);
            }
            (None, Some(_)) => {
                return (Ordering::Less, 0);
            }
            (Some(lhs_segment), Some(rhs_segment)) => {
                let (lhs_start, rhs_start) = (lhs.offset, rhs.offset);
                for i in 0..len {
                    let (lhs, rhs) = (
                        lhs_segment.get(lhs_start + i),
                        rhs_segment.get(rhs_start + i),
                    );
                    let ord = lhs.cmp(&rhs);
                    if ord == Ordering::Equal {
                        continue;
                    }
                    return (ord, i);
                }
            }
        };
        (Ordering::Equal, len)
    }

    /// Compares two ranges of values in memory of length `len`
    /// Returns the ordering and the first relative position at which they differ
    /// Special cases:
    /// - `lhs` exists in memory but `rhs` doesn't -> (Ordering::Greater, 0)
    /// - `rhs` exists in memory but `lhs` doesn't -> (Ordering::Less, 0)
    /// - None of `lhs` or `rhs` exist in memory -> (Ordering::Equal, 0)
    ///   Everything else behaves much like `memcmp` in C.
    ///   This is meant as an optimization for hints to avoid allocations.
    pub(crate) fn mem_eq(&self, lhs: Relocatable, rhs: Relocatable, len: usize) -> bool {
        if lhs == rhs {
            return true;
        }
        let get_segment = |idx: isize| {
            if idx.is_negative() {
                self.temp_data.get(-(idx + 1) as usize)
            } else {
                self.data.get(idx as usize)
            }
        };
        match (
            get_segment(lhs.segment_index).and_then(|s| s.get(lhs.offset..)),
            get_segment(rhs.segment_index).and_then(|s| s.get(rhs.offset..)),
        ) {
            (Some(lhs), Some(rhs)) => {
                let (lhs_len, rhs_len) = (lhs.len().min(len), rhs.len().min(len));
                if lhs_len != rhs_len {
                    return false;
                }
                lhs[..lhs_len] == rhs[..rhs_len]
            }
            (None, None) => true,
            _ => false,
        }
    }

    /// Gets a range of memory values from addr to addr + size
    /// The outputed range may contain gaps if the original memory has them
    pub fn get_range(&self, addr: Relocatable, size: usize) -> Vec<Option<Cow<MaybeRelocatable>>> {
        let mut values = Vec::new();

        for i in 0..size {
            values.push((addr + i).ok().and_then(|x| self.get(&x)));
        }

        values
    }

    /// Gets a range of memory values from addr to addr + size
    /// Fails if there if any of the values inside the range is missing (memory gap)
    pub fn get_continuous_range(
        &self,
        addr: Relocatable,
        size: usize,
    ) -> Result<Vec<MaybeRelocatable>, MemoryError> {
        let mut values = Vec::with_capacity(size);

        for i in 0..size {
            values.push(match self.get(&(addr + i)?) {
                Some(elem) => elem.into_owned(),
                None => return Err(MemoryError::GetRangeMemoryGap(Box::new((addr, size)))),
            });
        }

        Ok(values)
    }

    /// Gets a range of Felt252 memory values from addr to addr + size
    /// Fails if there if any of the values inside the range is missing (memory gap),
    /// or is not a Felt252
    pub fn get_integer_range(
        &self,
        addr: Relocatable,
        size: usize,
    ) -> Result<Vec<Cow<Felt252>>, MemoryError> {
        let mut values = Vec::new();

        for i in 0..size {
            values.push(self.get_integer((addr + i)?)?);
        }

        Ok(values)
    }

    pub fn mark_as_accessed(&mut self, addr: Relocatable) {
        let (i, j) = from_relocatable_to_indexes(addr);
        let data = if addr.segment_index < 0 {
            &mut self.temp_data
        } else {
            &mut self.data
        };
        let cell = data.get_mut(i).and_then(|x| x.get_mut(j));
        if let Some(cell) = cell {
            cell.mark_accessed()
        }
    }

    pub fn get_amount_of_accessed_addresses_for_segment(
        &self,
        segment_index: usize,
    ) -> Option<usize> {
        let segment = self.data.get(segment_index)?;
        Some(
            segment
                .iter()
                .filter(|x| x.is_some() && x.is_accessed())
                .count(),
        )
    }

    // Inserts a value into memory & inmediately marks it as accessed if insertion was succesful
    // Used by ModBuiltinRunner, as it accesses memory outside of it's segment when operating
    pub(crate) fn insert_as_accessed<V>(
        &mut self,
        key: Relocatable,
        val: V,
    ) -> Result<(), MemoryError>
    where
        MaybeRelocatable: From<V>,
    {
        self.insert(key, val)?;
        self.mark_as_accessed(key);
        Ok(())
    }
}

impl From<&Memory> for CairoPieMemory {
    fn from(mem: &Memory) -> CairoPieMemory {
        let mut pie_memory = Vec::default();
        for (i, segment) in mem.data.iter().enumerate() {
            for (j, cell) in segment.iter().enumerate() {
                if let Some(value) = cell.get_value() {
                    pie_memory.push(((i, j), value))
                }
            }
        }
        CairoPieMemory(pie_memory)
    }
}

impl fmt::Display for Memory {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        for (i, segment) in self.temp_data.iter().enumerate() {
            for (j, cell) in segment.iter().enumerate() {
                if let Some(elem) = cell.get_value() {
                    let temp_segment = i + 1;
                    writeln!(f, "(-{temp_segment},{j}) : {elem}")?;
                }
            }
        }
        for (i, segment) in self.data.iter().enumerate() {
            for (j, cell) in segment.iter().enumerate() {
                if let Some(elem) = cell.get_value() {
                    writeln!(f, "({i},{j}) : {elem}")?;
                }
            }
        }
        Ok(())
    }
}

/// Applies `relocation_rules` to a value
pub(crate) trait RelocateValue<'a, Input: 'a, Output: 'a> {
    fn relocate_value(&self, value: Input) -> Result<Output, MemoryError>;
}

#[cfg(not(feature = "extensive_hints"))]
impl RelocateValue<'_, Relocatable, Relocatable> for Memory {
    fn relocate_value(&self, addr: Relocatable) -> Result<Relocatable, MemoryError> {
        if addr.segment_index < 0 {
            // Adjust the segment index to begin at zero, as per the struct field's
            // comment.
            if let Some(x) = self
                .relocation_rules
                .get(&(-(addr.segment_index + 1) as usize))
            {
                return (*x + addr.offset).map_err(MemoryError::Math);
            }
        }
        Ok(addr)
    }
}
#[cfg(feature = "extensive_hints")]
impl RelocateValue<'_, Relocatable, MaybeRelocatable> for Memory {
    fn relocate_value(&self, addr: Relocatable) -> Result<MaybeRelocatable, MemoryError> {
        if addr.segment_index < 0 {
            // Adjust the segment index to begin at zero, as per the struct field's
            // comment.
            if let Some(x) = self
                .relocation_rules
                .get(&(-(addr.segment_index + 1) as usize))
            {
                return Ok(match x {
                    MaybeRelocatable::RelocatableValue(r) => {
                        (*r + addr.offset).map_err(MemoryError::Math)?.into()
                    }
                    MaybeRelocatable::Int(i) => i.into(),
                });
            }
        }
        Ok(addr.into())
    }
}

impl<'a> RelocateValue<'a, &'a Felt252, &'a Felt252> for Memory {
    fn relocate_value(&self, value: &'a Felt252) -> Result<&'a Felt252, MemoryError> {
        Ok(value)
    }
}

impl<'a> RelocateValue<'a, &'a MaybeRelocatable, Cow<'a, MaybeRelocatable>> for Memory {
    fn relocate_value(
        &self,
        value: &'a MaybeRelocatable,
    ) -> Result<Cow<'a, MaybeRelocatable>, MemoryError> {
        Ok(match value {
            MaybeRelocatable::Int(_) => Cow::Borrowed(value),
            MaybeRelocatable::RelocatableValue(addr) => {
                #[cfg(not(feature = "extensive_hints"))]
                let v = self.relocate_value(*addr)?.into();
                #[cfg(feature = "extensive_hints")]
                let v = self.relocate_value(*addr)?;

                Cow::Owned(v)
            }
        })
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
        felt_hex, relocatable,
        utils::test_utils::*,
        vm::{
            runners::builtin_runner::{
                RangeCheckBuiltinRunner, SignatureBuiltinRunner, RC_N_PARTS_STANDARD,
            },
            vm_memory::memory_segments::MemorySegmentManager,
        },
    };
    use assert_matches::assert_matches;

    use crate::vm::errors::memory_errors::MemoryError;

    use crate::utils::test_utils::memory_from_memory;
    use crate::utils::test_utils::memory_inner;

    #[cfg(target_arch = "wasm32")]
    use wasm_bindgen_test::*;

    #[test]
    #[cfg_attr(target_arch = "wasm32", wasm_bindgen_test)]
    fn insert_and_get_succesful() {
        let key = Relocatable::from((0, 0));
        let val = MaybeRelocatable::from(Felt252::from(5_u64));
        let mut memory = Memory::new();
        memory.data.push(Vec::new());
        memory.insert(key, &val).unwrap();
        assert_eq!(
            memory.get(&key).unwrap().as_ref(),
            &MaybeRelocatable::from(Felt252::from(5_u64))
        );
    }

    #[test]
    #[cfg_attr(target_arch = "wasm32", wasm_bindgen_test)]
    fn get_valuef_from_temp_segment() {
        let mut memory = Memory::new();
        memory.temp_data = vec![vec![
            MemoryCell::NONE,
            MemoryCell::NONE,
            MemoryCell::new(mayberelocatable!(8)),
        ]];
        assert_eq!(
            memory.get(&mayberelocatable!(-1, 2)).unwrap().as_ref(),
            &mayberelocatable!(8),
        );
    }

    #[test]
    #[cfg_attr(target_arch = "wasm32", wasm_bindgen_test)]
    fn insert_value_in_temp_segment() {
        let key = Relocatable::from((-1, 3));
        let val = MaybeRelocatable::from(Felt252::from(8_u64));
        let mut memory = Memory::new();
        memory.temp_data.push(Vec::new());
        memory.insert(key, &val).unwrap();
        assert_eq!(
            memory.temp_data[0][3],
            MemoryCell::new(MaybeRelocatable::from(Felt252::from(8_u64)))
        );
    }

    #[test]
    #[cfg_attr(target_arch = "wasm32", wasm_bindgen_test)]
    fn insert_and_get_from_temp_segment_succesful() {
        let key = Relocatable::from((-1, 0));
        let val = MaybeRelocatable::from(Felt252::from(5_u64));
        let mut memory = Memory::new();
        memory.temp_data.push(Vec::new());
        memory.insert(key, &val).unwrap();
        assert_eq!(
            memory.get(&key).unwrap().as_ref(),
            &MaybeRelocatable::from(Felt252::from(5_u64)),
        );
    }

    #[test]
    #[cfg_attr(target_arch = "wasm32", wasm_bindgen_test)]
    fn insert_and_get_from_temp_segment_failed() {
        let key = relocatable!(-1, 1);
        let mut memory = Memory::new();
        memory.temp_data = vec![vec![
            MemoryCell::NONE,
            MemoryCell::new(mayberelocatable!(8)),
        ]];
        assert_eq!(
            memory.insert(key, &mayberelocatable!(5)),
            Err(MemoryError::InconsistentMemory(Box::new((
                relocatable!(-1, 1),
                mayberelocatable!(8),
                mayberelocatable!(5)
            ))))
        );
    }

    #[test]
    #[cfg_attr(target_arch = "wasm32", wasm_bindgen_test)]
    fn get_non_allocated_memory() {
        let key = Relocatable::from((0, 0));
        let memory = Memory::new();
        assert_eq!(memory.get(&key), None);
    }

    #[test]
    #[cfg_attr(target_arch = "wasm32", wasm_bindgen_test)]
    fn get_non_existant_element() {
        let key = Relocatable::from((0, 0));
        let memory = Memory::new();
        assert_eq!(memory.get(&key), None);
    }

    #[test]
    #[cfg_attr(target_arch = "wasm32", wasm_bindgen_test)]
    fn insert_non_allocated_memory() {
        let key = Relocatable::from((0, 0));
        let val = MaybeRelocatable::from(Felt252::from(5_u64));
        let mut memory = Memory::new();
        let error = memory.insert(key, &val);
        assert_eq!(
            error,
            Err(MemoryError::UnallocatedSegment(Box::new((0, 0))))
        );
    }

    #[test]
    #[cfg_attr(target_arch = "wasm32", wasm_bindgen_test)]
    fn insert_inconsistent_memory() {
        let key = Relocatable::from((0, 0));
        let val_a = MaybeRelocatable::from(Felt252::from(5_u64));
        let val_b = MaybeRelocatable::from(Felt252::from(6_u64));
        let mut memory = Memory::new();
        memory.data.push(Vec::new());
        memory
            .insert(key, &val_a)
            .expect("Unexpected memory insert fail");
        let error = memory.insert(key, &val_b);
        assert_eq!(
            error,
            Err(MemoryError::InconsistentMemory(Box::new((
                key, val_a, val_b
            ))))
        );
    }

    #[test]
    #[cfg_attr(target_arch = "wasm32", wasm_bindgen_test)]
    fn insert_non_contiguous_element() {
        let key_a = Relocatable::from((0, 0));
        let key_b = Relocatable::from((0, 2));
        let val = MaybeRelocatable::from(Felt252::from(5_u64));
        let mut memory = Memory::new();
        memory.data.push(Vec::new());
        memory.insert(key_a, &val).unwrap();
        memory.insert(key_b, &val).unwrap();
        assert_eq!(memory.get(&key_b).unwrap().as_ref(), &val);
    }

    #[test]
    #[cfg_attr(target_arch = "wasm32", wasm_bindgen_test)]
    fn insert_non_contiguous_element_memory_gaps_none() {
        let key_a = Relocatable::from((0, 0));
        let key_b = Relocatable::from((0, 5));
        let val = MaybeRelocatable::from(Felt252::from(5_u64));
        let mut memory = Memory::new();
        memory.data.push(Vec::new());
        memory.insert(key_a, &val).unwrap();
        memory.insert(key_b, &val).unwrap();
        assert_eq!(memory.get(&key_b).unwrap().as_ref(), &val);
        assert_eq!(memory.get(&MaybeRelocatable::from((0, 1))), None);
        assert_eq!(memory.get(&MaybeRelocatable::from((0, 2))), None);
        assert_eq!(memory.get(&MaybeRelocatable::from((0, 3))), None);
        assert_eq!(memory.get(&MaybeRelocatable::from((0, 4))), None);
    }

    #[test]
    #[cfg_attr(target_arch = "wasm32", wasm_bindgen_test)]
    fn validate_existing_memory_for_range_check_within_bounds() {
        let mut builtin = RangeCheckBuiltinRunner::<RC_N_PARTS_STANDARD>::new(Some(8), true);
        let mut segments = MemorySegmentManager::new();
        builtin.initialize_segments(&mut segments);
        builtin.add_validation_rule(&mut segments.memory);
        for _ in 0..3 {
            segments.add();
        }

        segments
            .memory
            .insert(
                Relocatable::from((0, 0)),
                &MaybeRelocatable::from(Felt252::from(45_u64)),
            )
            .unwrap();
        segments.memory.validate_existing_memory().unwrap();
        assert!(segments
            .memory
            .validated_addresses
            .contains(&Relocatable::from((0, 0))));
    }

    #[test]
    #[cfg_attr(target_arch = "wasm32", wasm_bindgen_test)]
    fn validate_existing_memory_for_range_check_outside_bounds() {
        let mut builtin = RangeCheckBuiltinRunner::<RC_N_PARTS_STANDARD>::new(Some(8), true);
        let mut segments = MemorySegmentManager::new();
        segments.add();
        builtin.initialize_segments(&mut segments);
        segments
            .memory
            .insert(
                Relocatable::from((1, 0)),
                &MaybeRelocatable::from(Felt252::from(-10)),
            )
            .unwrap();
        builtin.add_validation_rule(&mut segments.memory);
        let error = segments.memory.validate_existing_memory();
        assert_eq!(
            error,
            Err(MemoryError::RangeCheckNumOutOfBounds(Box::new((
                Felt252::from(-10),
                Felt252::TWO.pow(128_u128)
            ))))
        );
    }

    #[test]
    #[cfg_attr(target_arch = "wasm32", wasm_bindgen_test)]
    fn validate_existing_memory_for_invalid_signature() {
        let mut builtin = SignatureBuiltinRunner::new(Some(512), true);
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
        assert_eq!(
            error,
            Err(MemoryError::SignatureNotFound(Box::new((0, 0).into())))
        );
    }

    #[test]
    #[cfg_attr(target_arch = "wasm32", wasm_bindgen_test)]
    fn validate_existing_memory_for_valid_signature() {
        let mut builtin = SignatureBuiltinRunner::new(Some(512), true);

        let signature_r =
            felt_hex!("0x411494b501a98abd8262b0da1351e17899a0c4ef23dd2f96fec5ba847310b20");
        let signature_s =
            felt_hex!("0x405c3191ab3883ef2b763af35bc5f5d15b3b4e99461d70e84c654a351a7c81b");

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
    #[cfg_attr(target_arch = "wasm32", wasm_bindgen_test)]
    fn validate_existing_memory_for_range_check_relocatable_value() {
        let mut builtin = RangeCheckBuiltinRunner::<RC_N_PARTS_STANDARD>::new(Some(8), true);
        let mut segments = MemorySegmentManager::new();
        builtin.initialize_segments(&mut segments);
        segments.memory = memory![((0, 0), (0, 4))];
        builtin.add_validation_rule(&mut segments.memory);
        let error = segments.memory.validate_existing_memory();
        assert_eq!(
            error,
            Err(MemoryError::RangeCheckFoundNonInt(Box::new(relocatable!(
                0, 0
            ))))
        );
    }

    #[test]
    #[cfg_attr(target_arch = "wasm32", wasm_bindgen_test)]
    fn validate_existing_memory_for_range_check_out_of_bounds_diff_segment() {
        let mut builtin = RangeCheckBuiltinRunner::<RC_N_PARTS_STANDARD>::new(Some(8), true);
        let mut segments = MemorySegmentManager::new();
        segments.memory = Memory::new();
        segments.add();
        builtin.initialize_segments(&mut segments);
        segments
            .memory
            .insert(
                Relocatable::from((0, 0)),
                &MaybeRelocatable::from(Felt252::from(-45_i128)),
            )
            .unwrap();
        builtin.add_validation_rule(&mut segments.memory);
        assert_eq!(segments.memory.validate_existing_memory(), Ok(()));
    }

    #[test]
    #[cfg_attr(target_arch = "wasm32", wasm_bindgen_test)]
    fn get_integer_valid() {
        let memory = memory![((0, 0), 10)];
        assert_eq!(
            memory
                .get_integer(Relocatable::from((0, 0)))
                .unwrap()
                .as_ref(),
            &Felt252::from(10)
        );
    }

    #[test]
    #[cfg_attr(target_arch = "wasm32", wasm_bindgen_test)]
    fn get_integer_invalid_expected_integer() {
        let mut segments = MemorySegmentManager::new();
        segments.add();
        segments
            .memory
            .insert(Relocatable::from((0, 0)), &MaybeRelocatable::from((0, 10)))
            .unwrap();
        assert_matches!(
            segments.memory.get_integer(Relocatable::from((0, 0))),
            Err(MemoryError::ExpectedInteger(
                bx
            )) if *bx == Relocatable::from((0, 0))
        );
    }

    #[test]
    #[cfg_attr(target_arch = "wasm32", wasm_bindgen_test)]
    fn default_memory() {
        let mem: Memory = Default::default();
        assert_eq!(mem.data.len(), 0);
    }

    #[test]
    #[cfg_attr(target_arch = "wasm32", wasm_bindgen_test)]
    fn insert_and_get_temporary_succesful() {
        let mut memory = Memory::new();
        memory.temp_data.push(Vec::new());

        let key = Relocatable::from((-1, 0));
        let val = MaybeRelocatable::from(Felt252::from(5));
        memory.insert(key, &val).unwrap();

        assert_eq!(memory.get(&key).unwrap().as_ref(), &val);
    }

    #[test]
    #[cfg_attr(target_arch = "wasm32", wasm_bindgen_test)]
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
    #[cfg_attr(target_arch = "wasm32", wasm_bindgen_test)]
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
            memory
                .relocate_value(&MaybeRelocatable::Int(Felt252::from(0)))
                .unwrap(),
            Cow::Owned(MaybeRelocatable::Int(Felt252::from(0))),
        );
    }

    #[test]
    #[cfg_attr(target_arch = "wasm32", wasm_bindgen_test)]
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
            memory
                .relocate_value(&MaybeRelocatable::RelocatableValue((0, 0).into()))
                .unwrap(),
            Cow::Owned(MaybeRelocatable::RelocatableValue((0, 0).into())),
        );
        assert_eq!(
            memory
                .relocate_value(&MaybeRelocatable::RelocatableValue((5, 0).into()))
                .unwrap(),
            Cow::Owned(MaybeRelocatable::RelocatableValue((5, 0).into())),
        );
    }

    #[test]
    #[cfg_attr(target_arch = "wasm32", wasm_bindgen_test)]
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
            memory
                .relocate_value(&MaybeRelocatable::RelocatableValue((-5, 0).into()))
                .unwrap(),
            Cow::Owned(MaybeRelocatable::RelocatableValue((-5, 0).into())),
        );
    }

    #[test]
    #[cfg_attr(target_arch = "wasm32", wasm_bindgen_test)]
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
            memory
                .relocate_value(&MaybeRelocatable::RelocatableValue((-1, 0).into()))
                .unwrap(),
            Cow::Owned(MaybeRelocatable::RelocatableValue((2, 0).into())),
        );
        assert_eq!(
            memory
                .relocate_value(&MaybeRelocatable::RelocatableValue((-2, 0).into()))
                .unwrap(),
            Cow::Owned(MaybeRelocatable::RelocatableValue((2, 2).into())),
        );
        assert_eq!(
            memory
                .relocate_value(&MaybeRelocatable::RelocatableValue((-1, 5).into()))
                .unwrap(),
            Cow::Owned(MaybeRelocatable::RelocatableValue((2, 5).into())),
        );
        assert_eq!(
            memory
                .relocate_value(&MaybeRelocatable::RelocatableValue((-2, 5).into()))
                .unwrap(),
            Cow::Owned(MaybeRelocatable::RelocatableValue((2, 7).into())),
        );
    }

    #[test]
    #[cfg_attr(target_arch = "wasm32", wasm_bindgen_test)]
    fn get_range_for_continuous_memory() {
        let memory = memory![((1, 0), 2), ((1, 1), 3), ((1, 2), 4)];

        let value1 = MaybeRelocatable::from(Felt252::from(2));
        let value2 = MaybeRelocatable::from(Felt252::from(3));
        let value3 = MaybeRelocatable::from(Felt252::from(4));

        let expected_vec = vec![
            Some(Cow::Borrowed(&value1)),
            Some(Cow::Borrowed(&value2)),
            Some(Cow::Borrowed(&value3)),
        ];
        assert_eq!(memory.get_range(Relocatable::from((1, 0)), 3), expected_vec);
    }

    #[test]
    #[cfg_attr(target_arch = "wasm32", wasm_bindgen_test)]
    fn get_range_for_non_continuous_memory() {
        let memory = memory![((1, 0), 2), ((1, 1), 3), ((1, 3), 4)];

        let value1 = MaybeRelocatable::from(Felt252::from(2));
        let value2 = MaybeRelocatable::from(Felt252::from(3));
        let value3 = MaybeRelocatable::from(Felt252::from(4));

        let expected_vec = vec![
            Some(Cow::Borrowed(&value1)),
            Some(Cow::Borrowed(&value2)),
            None,
            Some(Cow::Borrowed(&value3)),
        ];
        assert_eq!(memory.get_range(Relocatable::from((1, 0)), 4), expected_vec);
    }

    #[test]
    #[cfg_attr(target_arch = "wasm32", wasm_bindgen_test)]
    fn get_continuous_range_for_continuous_memory() {
        let memory = memory![((1, 0), 2), ((1, 1), 3), ((1, 2), 4)];

        let value1 = MaybeRelocatable::from(Felt252::from(2));
        let value2 = MaybeRelocatable::from(Felt252::from(3));
        let value3 = MaybeRelocatable::from(Felt252::from(4));

        let expected_vec = vec![value1, value2, value3];
        assert_eq!(
            memory.get_continuous_range(Relocatable::from((1, 0)), 3),
            Ok(expected_vec)
        );
    }

    #[test]
    #[cfg_attr(target_arch = "wasm32", wasm_bindgen_test)]
    fn get_continuous_range_for_non_continuous_memory() {
        let memory = memory![((1, 0), 2), ((1, 1), 3), ((1, 3), 4)];

        assert_eq!(
            memory.get_continuous_range(Relocatable::from((1, 0)), 3),
            Err(MemoryError::GetRangeMemoryGap(Box::new(((1, 0).into(), 3))))
        );
    }

    /// Test that relocate_memory() works when there are no relocation rules.
    #[test]
    #[cfg_attr(target_arch = "wasm32", wasm_bindgen_test)]
    fn relocate_memory_empty_relocation_rules() {
        let mut memory = memory![((0, 0), 1), ((0, 1), 2), ((0, 2), 3)];

        assert_eq!(memory.relocate_memory(), Ok(()));
        check_memory!(memory, ((0, 0), 1), ((0, 1), 2), ((0, 2), 3));
    }

    #[test]
    #[cfg_attr(target_arch = "wasm32", wasm_bindgen_test)]
    fn relocate_memory_new_segment_with_gap() {
        let mut memory = memory![
            ((0, 0), 1),
            ((0, 1), (-1, 0)),
            ((0, 2), 3),
            ((1, 0), (-1, 1)),
            ((1, 1), 5),
            ((1, 2), (-1, 2)),
            ((-1, 0), 7),
            ((-1, 1), 8),
            ((-1, 2), 9)
        ];
        memory
            .add_relocation_rule((-1, 0).into(), (2, 1).into())
            .unwrap();
        memory.data.push(vec![]);

        assert_eq!(memory.relocate_memory(), Ok(()));
        check_memory!(
            memory,
            ((0, 0), 1),
            ((0, 1), (2, 1)),
            ((0, 2), 3),
            ((1, 0), (2, 2)),
            ((1, 1), 5),
            ((1, 2), (2, 3)),
            ((2, 1), 7),
            ((2, 2), 8),
            ((2, 3), 9)
        );
        assert!(memory.temp_data.is_empty());
    }

    #[test]
    #[cfg_attr(target_arch = "wasm32", wasm_bindgen_test)]
    fn relocate_memory_new_segment() {
        let mut memory = memory![
            ((0, 0), 1),
            ((0, 1), (-1, 0)),
            ((0, 2), 3),
            ((1, 0), (-1, 1)),
            ((1, 1), 5),
            ((1, 2), (-1, 2)),
            ((-1, 0), 7),
            ((-1, 1), 8),
            ((-1, 2), 9)
        ];
        memory
            .add_relocation_rule((-1, 0).into(), (2, 0).into())
            .unwrap();
        memory.data.push(vec![]);

        assert_eq!(memory.relocate_memory(), Ok(()));

        check_memory!(
            memory,
            ((0, 0), 1),
            ((0, 1), (2, 0)),
            ((0, 2), 3),
            ((1, 0), (2, 1)),
            ((1, 1), 5),
            ((1, 2), (2, 2)),
            ((2, 0), 7),
            ((2, 1), 8),
            ((2, 2), 9)
        );
        assert!(memory.temp_data.is_empty());
    }

    #[test]
    #[cfg_attr(target_arch = "wasm32", wasm_bindgen_test)]
    fn relocate_memory_new_segment_unallocated() {
        let mut memory = memory![
            ((0, 0), 1),
            ((0, 1), (-1, 0)),
            ((0, 2), 3),
            ((1, 0), (-1, 1)),
            ((1, 1), 5),
            ((1, 2), (-1, 2)),
            ((-1, 0), 7),
            ((-1, 1), 8),
            ((-1, 2), 9)
        ];
        memory
            .add_relocation_rule((-1, 0).into(), (2, 0).into())
            .unwrap();

        assert_eq!(
            memory.relocate_memory(),
            Err(MemoryError::UnallocatedSegment(Box::new((2, 2))))
        );
    }

    #[test]
    #[cfg_attr(target_arch = "wasm32", wasm_bindgen_test)]
    fn relocate_memory_into_existing_segment() {
        let mut memory = memory![
            ((0, 0), 1),
            ((0, 1), (-1, 0)),
            ((0, 2), 3),
            ((1, 0), (-1, 1)),
            ((1, 1), 5),
            ((1, 2), (-1, 2)),
            ((-1, 0), 7),
            ((-1, 1), 8),
            ((-1, 2), 9)
        ];
        memory
            .add_relocation_rule((-1, 0).into(), (1, 3).into())
            .unwrap();

        assert_eq!(memory.relocate_memory(), Ok(()));

        check_memory!(
            memory,
            ((0, 0), 1),
            ((0, 1), (1, 3)),
            ((0, 2), 3),
            ((1, 0), (1, 4)),
            ((1, 1), 5),
            ((1, 2), (1, 5)),
            ((1, 3), 7),
            ((1, 4), 8),
            ((1, 5), 9)
        );
        assert!(memory.temp_data.is_empty());
    }

    #[test]
    #[cfg_attr(target_arch = "wasm32", wasm_bindgen_test)]
    fn relocate_memory_into_existing_segment_inconsistent_memory() {
        let mut memory = memory![
            ((0, 0), 1),
            ((0, 1), (-1, 0)),
            ((0, 2), 3),
            ((1, 0), (-1, 1)),
            ((1, 1), 5),
            ((1, 2), (-1, 2)),
            ((-1, 0), 7),
            ((-1, 1), 8),
            ((-1, 2), 9)
        ];
        memory
            .add_relocation_rule((-1, 0).into(), (1, 0).into())
            .unwrap();

        assert_eq!(
            memory.relocate_memory(),
            Err(MemoryError::InconsistentMemory(Box::new((
                (1, 0).into(),
                (1, 1).into(),
                7.into(),
            ))))
        );
    }

    #[test]
    #[cfg_attr(target_arch = "wasm32", wasm_bindgen_test)]
    fn relocate_memory_new_segment_2_temporary_segments_one_relocated() {
        let mut memory = memory![
            ((0, 0), 1),
            ((0, 1), (-1, 0)),
            ((0, 2), 3),
            ((1, 0), (-1, 1)),
            ((1, 1), 5),
            ((1, 2), (-1, 2)),
            ((-1, 0), 7),
            ((-1, 1), 8),
            ((-1, 2), 9),
            ((-2, 0), 10),
            ((-2, 1), 11)
        ];
        memory
            .add_relocation_rule((-1, 0).into(), (2, 0).into())
            .unwrap();
        memory.data.push(vec![]);

        assert_eq!(memory.relocate_memory(), Ok(()));
        check_memory!(
            memory,
            ((0, 0), 1),
            ((0, 1), (2, 0)),
            ((0, 2), 3),
            ((1, 0), (2, 1)),
            ((1, 1), 5),
            ((1, 2), (2, 2)),
            ((2, 0), 7),
            ((2, 1), 8),
            ((2, 2), 9),
            ((-1, 0), 10),
            ((-1, 1), 11)
        );
    }

    #[test]
    #[cfg_attr(target_arch = "wasm32", wasm_bindgen_test)]
    fn relocate_memory_new_segment_2_temporary_segments_relocated() {
        let mut memory = memory![
            ((0, 0), 1),
            ((0, 1), (-1, 0)),
            ((0, 2), 3),
            ((1, 0), (-1, 1)),
            ((1, 1), 5),
            ((1, 2), (-1, 2)),
            ((-1, 0), 7),
            ((-1, 1), 8),
            ((-1, 2), 9),
            ((-2, 0), 10),
            ((-2, 1), 11)
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

        check_memory!(
            memory,
            ((0, 0), 1),
            ((0, 1), (2, 0)),
            ((0, 2), 3),
            ((1, 0), (2, 1)),
            ((1, 1), 5),
            ((1, 2), (2, 2)),
            ((2, 0), 7),
            ((2, 1), 8),
            ((2, 2), 9),
            ((3, 0), 10),
            ((3, 1), 11)
        );
        assert!(memory.temp_data.is_empty());
    }

    #[test]
    #[cfg_attr(target_arch = "wasm32", wasm_bindgen_test)]
    fn test_memory_display() {
        let memory = memory![
            ((0, 0), 1),
            ((0, 1), (-1, 0)),
            ((0, 2), 3),
            ((1, 0), (-1, 1)),
            ((1, 1), 5),
            ((1, 2), (-1, 2)),
            ((-1, 0), (-1, 0)),
            ((-1, 1), 8),
            ((-1, 2), 9)
        ];

        assert_eq!(
            format!("{}", memory),
            "(-1,0) : -1:0\n(-1,1) : 8\n(-1,2) : 9\n(0,0) : 1\n(0,1) : -1:0\n(0,2) : 3\n(1,0) : -1:1\n(1,1) : 5\n(1,2) : -1:2\n");
    }

    #[test]
    #[cfg_attr(target_arch = "wasm32", wasm_bindgen_test)]
    fn relocate_memory_into_existing_segment_temporary_values_in_temporary_memory() {
        let mut memory = memory![
            ((0, 0), 1),
            ((0, 1), (-1, 0)),
            ((0, 2), 3),
            ((1, 0), (-1, 1)),
            ((1, 1), 5),
            ((1, 2), (-1, 2)),
            ((-1, 0), (-1, 0)),
            ((-1, 1), 8),
            ((-1, 2), 9)
        ];
        memory
            .add_relocation_rule((-1, 0).into(), (1, 3).into())
            .unwrap();

        assert_eq!(memory.relocate_memory(), Ok(()));
        check_memory!(
            memory,
            ((0, 0), 1),
            ((0, 1), (1, 3)),
            ((0, 2), 3),
            ((1, 0), (1, 4)),
            ((1, 1), 5),
            ((1, 2), (1, 5)),
            ((1, 3), (1, 3)),
            ((1, 4), 8),
            ((1, 5), 9)
        );
        assert!(memory.temp_data.is_empty());
    }

    #[test]
    #[cfg_attr(target_arch = "wasm32", wasm_bindgen_test)]
    fn relocate_address_with_rules() {
        let mut memory = Memory::new();
        memory
            .add_relocation_rule((-1, 0).into(), (2, 0).into())
            .unwrap();
        memory
            .add_relocation_rule((-2, 0).into(), (2, 2).into())
            .unwrap();

        assert_eq!(
            Memory::relocate_address((-1, 0).into(), &memory.relocation_rules).unwrap(),
            MaybeRelocatable::RelocatableValue((2, 0).into()),
        );
        assert_eq!(
            Memory::relocate_address((-2, 1).into(), &memory.relocation_rules).unwrap(),
            MaybeRelocatable::RelocatableValue((2, 3).into()),
        );
    }

    #[test]
    #[cfg_attr(target_arch = "wasm32", wasm_bindgen_test)]
    fn relocate_address_no_rules() {
        let memory = Memory::new();
        assert_eq!(
            Memory::relocate_address((-1, 0).into(), &memory.relocation_rules).unwrap(),
            MaybeRelocatable::RelocatableValue((-1, 0).into()),
        );
        assert_eq!(
            Memory::relocate_address((-2, 1).into(), &memory.relocation_rules).unwrap(),
            MaybeRelocatable::RelocatableValue((-2, 1).into()),
        );
    }

    #[test]
    #[cfg_attr(target_arch = "wasm32", wasm_bindgen_test)]
    fn relocate_address_real_addr() {
        let memory = Memory::new();
        assert_eq!(
            Memory::relocate_address((1, 0).into(), &memory.relocation_rules).unwrap(),
            MaybeRelocatable::RelocatableValue((1, 0).into()),
        );
        assert_eq!(
            Memory::relocate_address((1, 1).into(), &memory.relocation_rules).unwrap(),
            MaybeRelocatable::RelocatableValue((1, 1).into()),
        );
    }

    #[test]
    #[cfg_attr(target_arch = "wasm32", wasm_bindgen_test)]
    #[cfg(feature = "extensive_hints")]
    fn relocate_address_to_integer() {
        let mut memory = Memory::new();
        memory
            .add_relocation_rule((-1, 0).into(), 0.into())
            .unwrap();
        memory
            .add_relocation_rule((-2, 0).into(), 42.into())
            .unwrap();

        assert_eq!(
            Memory::relocate_address((-1, 0).into(), &memory.relocation_rules).unwrap(),
            MaybeRelocatable::Int(0.into()),
        );
        assert_eq!(
            Memory::relocate_address((-2, 0).into(), &memory.relocation_rules).unwrap(),
            MaybeRelocatable::Int(42.into()),
        );
    }

    #[test]
    #[cfg_attr(target_arch = "wasm32", wasm_bindgen_test)]
    #[cfg(feature = "extensive_hints")]
    fn relocate_address_integer_no_duplicates() {
        let mut memory = Memory::new();
        memory
            .add_relocation_rule((-1, 0).into(), 1.into())
            .unwrap();
        assert_eq!(
            memory.add_relocation_rule((-1, 0).into(), 42.into()),
            Err(MemoryError::DuplicatedRelocation(-1))
        );
        assert_eq!(
            memory.add_relocation_rule((-1, 0).into(), (2, 0).into()),
            Err(MemoryError::DuplicatedRelocation(-1))
        );

        assert_eq!(
            Memory::relocate_address((-1, 0).into(), &memory.relocation_rules).unwrap(),
            MaybeRelocatable::Int(1.into()),
        );

        memory
            .add_relocation_rule((-2, 0).into(), (3, 0).into())
            .unwrap();
        assert_eq!(
            memory.add_relocation_rule((-2, 0).into(), 1.into()),
            Err(MemoryError::DuplicatedRelocation(-2))
        );

        assert_eq!(
            Memory::relocate_address((-2, 0).into(), &memory.relocation_rules).unwrap(),
            MaybeRelocatable::RelocatableValue((3, 0).into()),
        );
    }

    #[test]
    fn mark_address_as_accessed() {
        let mut memory = memory![((0, 0), 0)];
        assert!(!memory.data[0][0].is_accessed());
        memory.mark_as_accessed(relocatable!(0, 0));
        assert!(memory.data[0][0].is_accessed());
    }

    #[test]
    fn get_amount_of_accessed_addresses_for_segment_valid() {
        let mut memory = memory![((0, 0), 0)];
        assert_eq!(
            memory.get_amount_of_accessed_addresses_for_segment(0),
            Some(0)
        );
        memory.mark_as_accessed(relocatable!(0, 0));
        assert_eq!(
            memory.get_amount_of_accessed_addresses_for_segment(0),
            Some(1)
        );
    }

    #[test]
    fn get_amount_of_accessed_addresses_for_segment_invalid_segment() {
        let memory = memory![((0, 0), 0)];
        assert_eq!(memory.get_amount_of_accessed_addresses_for_segment(1), None);
    }

    #[test]
    fn memory_cell_new_is_not_accessed() {
        let cell = MemoryCell::new(mayberelocatable!(1));
        assert!(!cell.is_accessed())
    }

    #[test]
    fn memory_cell_mark_accessed() {
        let mut cell = MemoryCell::new(mayberelocatable!(1));
        cell.mark_accessed();
        assert!(cell.is_accessed())
    }

    #[test]
    fn memory_cell_get_value() {
        let cell = MemoryCell::new(mayberelocatable!(1));
        assert_eq!(cell.get_value(), Some(mayberelocatable!(1)));
    }

    use core::cmp::Ordering::*;

    fn check_memcmp(
        lhs: (isize, usize),
        rhs: (isize, usize),
        len: usize,
        ord: Ordering,
        pos: usize,
    ) {
        let mem = memory![
            ((-2, 0), 1),
            ((-2, 1), (1, 1)),
            ((-2, 3), 0),
            ((-2, 4), 0),
            ((-1, 0), 1),
            ((-1, 1), (1, 1)),
            ((-1, 3), 0),
            ((-1, 4), 3),
            ((0, 0), 1),
            ((0, 1), (1, 1)),
            ((0, 3), 0),
            ((0, 4), 0),
            ((1, 0), 1),
            ((1, 1), (1, 1)),
            ((1, 3), 0),
            ((1, 4), 3)
        ];
        assert_eq!((ord, pos), mem.memcmp(lhs.into(), rhs.into(), len));
    }

    #[test]
    fn insert_alloc_fails_gracefully() {
        let mut mem = memory![((0, 0), 1)];
        let err = mem.insert((0, usize::MAX >> 1).into(), Felt252::ONE);
        assert_eq!(err, Err(MemoryError::VecCapacityExceeded));
    }

    #[test]
    fn insert_overflow_fails_gracefully() {
        let mut mem = memory![((0, 0), 1)];
        let err = mem.insert((0, usize::MAX).into(), Felt252::ONE);
        assert_eq!(err, Err(MemoryError::VecCapacityExceeded));
    }

    #[test]
    fn memcmp() {
        check_memcmp((0, 0), (0, 0), 3, Equal, 3);
        check_memcmp((0, 0), (1, 0), 3, Equal, 3);
        check_memcmp((0, 0), (1, 0), 5, Less, 4);
        check_memcmp((1, 0), (0, 0), 5, Greater, 4);
        check_memcmp((2, 2), (2, 5), 8, Equal, 0);
        check_memcmp((0, 0), (2, 5), 8, Greater, 0);
        check_memcmp((2, 5), (0, 0), 8, Less, 0);
        check_memcmp((-2, 0), (-2, 0), 3, Equal, 3);
        check_memcmp((-2, 0), (-1, 0), 3, Equal, 3);
        check_memcmp((-2, 0), (-1, 0), 5, Less, 4);
        check_memcmp((-1, 0), (-2, 0), 5, Greater, 4);
        check_memcmp((-3, 2), (-3, 5), 8, Equal, 0);
        check_memcmp((-2, 0), (-3, 5), 8, Greater, 0);
        check_memcmp((-3, 5), (-2, 0), 8, Less, 0);
    }

    #[test]
    fn cairo_pie_memory_from_memory() {
        let memory = memory![((8, 9), 3), ((1, 2), 5), ((7, 6), (1, 2))];

        assert_eq!(
            CairoPieMemory::from(&memory),
            CairoPieMemory(vec![
                ((1, 2), MaybeRelocatable::from(5)),
                ((7, 6), MaybeRelocatable::from((1, 2))),
                ((8, 9), MaybeRelocatable::from(3))
            ])
        )
    }
}
