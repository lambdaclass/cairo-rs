use crate::{
    air_private_input::{ModInput, ModInputInstance, ModInputMemoryVars, PrivateInput},
    math_utils::{div_mod_unsigned, safe_div_usize},
    stdlib::{
        borrow::Cow,
        collections::BTreeMap,
        prelude::{Box, Vec},
    },
    types::{
        builtin_name::BuiltinName,
        errors::math_errors::MathError,
        instance_definitions::mod_instance_def::{ModInstanceDef, CELLS_PER_MOD, N_WORDS},
        relocatable::{relocate_address, MaybeRelocatable, Relocatable},
    },
    vm::{
        errors::{
            memory_errors::MemoryError, runner_errors::RunnerError, vm_errors::VirtualMachineError,
        },
        vm_core::VirtualMachine,
        vm_memory::{memory::Memory, memory_segments::MemorySegmentManager},
    },
    Felt252,
};
use core::ops::Shl;
use num_bigint::BigUint;
use num_integer::div_ceil;
use num_integer::Integer;
use num_traits::One;
use num_traits::Zero;

//The maximum n value that the function fill_memory accepts.
const FILL_MEMORY_MAX: usize = 100000;

const VALUES_PTR_OFFSET: u32 = 4;
const OFFSETS_PTR_OFFSET: u32 = 5;
const N_OFFSET: u32 = 6;

#[derive(Debug, Clone)]
pub struct ModBuiltinRunner {
    builtin_type: ModBuiltinType,
    base: usize,
    pub(crate) stop_ptr: Option<usize>,
    instance_def: ModInstanceDef,
    pub(crate) included: bool,
    zero_segment_index: usize,
    zero_segment_size: usize,
    // Precomputed powers used for reading and writing values that are represented as n_words words of word_bit_len bits each.
    shift: BigUint,
    shift_powers: [BigUint; N_WORDS],
    k_bound: BigUint,
}

#[derive(Debug, Clone)]
pub enum ModBuiltinType {
    Mul,
    Add,
}

impl ModBuiltinType {
    pub(crate) fn operation_string(&self) -> &'static str {
        match self {
            ModBuiltinType::Mul => "*",
            ModBuiltinType::Add => "+",
        }
    }
}

#[derive(Debug, Default)]
struct Inputs {
    p: BigUint,
    p_values: [Felt252; N_WORDS],
    values_ptr: Relocatable,
    offsets_ptr: Relocatable,
    n: usize,
}

impl ModBuiltinRunner {
    pub(crate) fn new_add_mod(instance_def: &ModInstanceDef, included: bool) -> Self {
        Self::new(
            instance_def.clone(),
            included,
            ModBuiltinType::Add,
            Some(2u32.into()),
        )
    }

    pub(crate) fn new_mul_mod(instance_def: &ModInstanceDef, included: bool) -> Self {
        Self::new(instance_def.clone(), included, ModBuiltinType::Mul, None)
    }

    fn new(
        instance_def: ModInstanceDef,
        included: bool,
        builtin_type: ModBuiltinType,
        k_bound: Option<BigUint>,
    ) -> Self {
        let shift = BigUint::one().shl(instance_def.word_bit_len);
        let shift_powers = core::array::from_fn(|i| shift.pow(i as u32));
        let zero_segment_size = core::cmp::max(N_WORDS, instance_def.batch_size * 3);
        let int_lim = BigUint::from(2_u32).pow(N_WORDS as u32 * instance_def.word_bit_len);
        Self {
            builtin_type,
            base: 0,
            stop_ptr: None,
            instance_def,
            included,
            zero_segment_index: 0,
            zero_segment_size,
            shift,
            shift_powers,
            k_bound: k_bound.unwrap_or(int_lim),
        }
    }

    pub fn name(&self) -> BuiltinName {
        match self.builtin_type {
            ModBuiltinType::Mul => BuiltinName::mul_mod,
            ModBuiltinType::Add => BuiltinName::add_mod,
        }
    }

    pub fn initialize_segments(&mut self, segments: &mut MemorySegmentManager) {
        self.base = segments.add().segment_index as usize; // segments.add() always returns a positive index
    }

    pub fn initialize_zero_segment(&mut self, segments: &mut MemorySegmentManager) {
        self.zero_segment_index = segments.add_zero_segment(self.zero_segment_size);
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
        self.instance_def.ratio.map(|ratio| ratio.numerator)
    }

    pub fn ratio_den(&self) -> Option<u32> {
        self.instance_def.ratio.map(|ratio| ratio.denominator)
    }

    pub fn batch_size(&self) -> usize {
        self.instance_def.batch_size
    }

    pub fn get_used_cells(&self, segments: &MemorySegmentManager) -> Result<usize, MemoryError> {
        segments
            .get_segment_used_size(self.base)
            .ok_or(MemoryError::MissingSegmentUsedSizes)
    }

    pub fn get_used_instances(
        &self,
        segments: &MemorySegmentManager,
    ) -> Result<usize, MemoryError> {
        let used_cells = self.get_used_cells(segments)?;
        Ok(div_ceil(used_cells, CELLS_PER_MOD as usize))
    }

    pub(crate) fn air_private_input(&self, segments: &MemorySegmentManager) -> Vec<PrivateInput> {
        let segment_index = self.base as isize;
        let segment_size = segments
            .get_segment_used_size(self.base)
            .unwrap_or_default();
        let relocation_table = segments.relocate_segments().unwrap_or_default();
        let mut instances = Vec::<ModInputInstance>::new();
        for instance in 0..segment_size
            .checked_div(CELLS_PER_MOD as usize)
            .unwrap_or_default()
        {
            let instance_addr_offset = instance * CELLS_PER_MOD as usize;
            let values_ptr = segments
                .memory
                .get_relocatable(
                    (
                        segment_index,
                        instance_addr_offset + VALUES_PTR_OFFSET as usize,
                    )
                        .into(),
                )
                .unwrap_or_default();
            let offsets_ptr = segments
                .memory
                .get_relocatable(
                    (
                        segment_index,
                        instance_addr_offset + OFFSETS_PTR_OFFSET as usize,
                    )
                        .into(),
                )
                .unwrap_or_default();
            let n = segments
                .memory
                .get_usize((segment_index, instance_addr_offset + N_OFFSET as usize).into())
                .unwrap_or_default();
            let p_values: [Felt252; N_WORDS] = core::array::from_fn(|i| {
                segments
                    .memory
                    .get_integer((segment_index, instance_addr_offset + i).into())
                    .unwrap_or_default()
                    .into_owned()
            });
            let mut batch = BTreeMap::<usize, ModInputMemoryVars>::new();
            let fetch_offset_and_words = |var_index: usize,
                                          index_in_batch: usize|
             -> (usize, [Felt252; N_WORDS]) {
                let offset = segments
                    .memory
                    .get_usize((offsets_ptr + (3 * index_in_batch + var_index)).unwrap_or_default())
                    .unwrap_or_default();
                let words: [Felt252; N_WORDS] = core::array::from_fn(|i| {
                    segments
                        .memory
                        .get_integer((values_ptr + (offset + i)).unwrap_or_default())
                        .unwrap_or_default()
                        .into_owned()
                });
                (offset, words)
            };
            for index_in_batch in 0..self.batch_size() {
                let (a_offset, a_values) = fetch_offset_and_words(0, index_in_batch);
                let (b_offset, b_values) = fetch_offset_and_words(1, index_in_batch);
                let (c_offset, c_values) = fetch_offset_and_words(2, index_in_batch);
                batch.insert(
                    index_in_batch,
                    ModInputMemoryVars {
                        a_offset,
                        b_offset,
                        c_offset,
                        a0: a_values[0],
                        a1: a_values[1],
                        a2: a_values[2],
                        a3: a_values[3],
                        b0: b_values[0],
                        b1: b_values[1],
                        b2: b_values[2],
                        b3: b_values[3],
                        c0: c_values[0],
                        c1: c_values[1],
                        c2: c_values[2],
                        c3: c_values[3],
                    },
                );
            }
            instances.push(ModInputInstance {
                index: instance,
                p0: p_values[0],
                p1: p_values[1],
                p2: p_values[2],
                p3: p_values[3],
                values_ptr: relocate_address(values_ptr, &relocation_table).unwrap_or_default(),
                offsets_ptr: relocate_address(offsets_ptr, &relocation_table).unwrap_or_default(),
                n,
                batch,
            });
        }

        instances.sort_by_key(|input| input.index);

        vec![PrivateInput::Mod(ModInput {
            instances,
            zero_value_address: relocation_table
                .get(self.zero_segment_index)
                .cloned()
                .unwrap_or_default(),
        })]
    }

    // Reads N_WORDS from memory, starting at address=addr.
    // Returns the words and the value if all words are in memory.
    // Verifies that all words are integers and are bounded by 2**self.instance_def.word_bit_len.
    fn read_n_words_value(
        &self,
        memory: &Memory,
        addr: Relocatable,
    ) -> Result<([Felt252; N_WORDS], Option<BigUint>), RunnerError> {
        let mut words = Default::default();
        let mut value = BigUint::zero();
        for i in 0..N_WORDS {
            let addr_i = (addr + i)?;
            match memory.get(&addr_i).map(Cow::into_owned) {
                None => return Ok((words, None)),
                Some(MaybeRelocatable::RelocatableValue(_)) => {
                    return Err(MemoryError::ExpectedInteger(Box::new(addr_i)).into())
                }
                Some(MaybeRelocatable::Int(word)) => {
                    let biguint_word = word.to_biguint();
                    if biguint_word >= self.shift {
                        return Err(RunnerError::WordExceedsModBuiltinWordBitLen(Box::new((
                            addr_i,
                            self.instance_def.word_bit_len,
                            word,
                        ))));
                    }
                    words[i] = word;
                    value += biguint_word * &self.shift_powers[i];
                }
            }
        }
        Ok((words, Some(value)))
    }

    // Reads the inputs to the builtin (see Inputs) from the memory at address=addr.
    // Returns a struct with the inputs. Asserts that it exists in memory.
    // Returns also the value of p, not just its words.
    fn read_inputs(&self, memory: &Memory, addr: Relocatable) -> Result<Inputs, RunnerError> {
        let values_ptr = memory.get_relocatable((addr + VALUES_PTR_OFFSET)?)?;
        let offsets_ptr = memory.get_relocatable((addr + OFFSETS_PTR_OFFSET)?)?;
        let n = memory.get_usize((addr + N_OFFSET)?)?;
        if n < 1 {
            return Err(RunnerError::ModBuiltinNLessThanOne(Box::new((
                self.name(),
                n,
            ))));
        }
        let (p_values, p) = self.read_n_words_value(memory, addr)?;
        let p = p.ok_or_else(|| {
            RunnerError::ModBuiltinMissingValue(Box::new((
                self.name(),
                (addr + N_WORDS).unwrap_or_default(),
            )))
        })?;
        Ok(Inputs {
            p,
            p_values,
            values_ptr,
            offsets_ptr,
            n,
        })
    }

    // Reads the memory variables to the builtin (see MEMORY_VARS) from the memory given
    // the inputs (specifically, values_ptr and offsets_ptr).
    // Computes and returns the values of a, b, and c.
    fn read_memory_vars(
        &self,
        memory: &Memory,
        values_ptr: Relocatable,
        offsets_ptr: Relocatable,
        index_in_batch: usize,
    ) -> Result<(BigUint, BigUint, BigUint), RunnerError> {
        let compute_value = |index: usize| -> Result<BigUint, RunnerError> {
            let offset = memory.get_usize((offsets_ptr + (index + 3 * index_in_batch))?)?;
            let value_addr = (values_ptr + offset)?;
            let (_, value) = self.read_n_words_value(memory, value_addr)?;
            let value = value.ok_or_else(|| {
                RunnerError::ModBuiltinMissingValue(Box::new((
                    self.name(),
                    (value_addr + N_WORDS).unwrap_or_default(),
                )))
            })?;
            Ok(value)
        };

        let a = compute_value(0)?;
        let b = compute_value(1)?;
        let c = compute_value(2)?;
        Ok((a, b, c))
    }

    fn fill_inputs(
        &self,
        memory: &mut Memory,
        builtin_ptr: Relocatable,
        inputs: &Inputs,
    ) -> Result<(), RunnerError> {
        if inputs.n > FILL_MEMORY_MAX {
            return Err(RunnerError::FillMemoryMaxExceeded(Box::new((
                self.name(),
                FILL_MEMORY_MAX,
            ))));
        }
        let n_instances = safe_div_usize(inputs.n, self.instance_def.batch_size)?;
        for instance in 1..n_instances {
            let instance_ptr = (builtin_ptr + instance * CELLS_PER_MOD as usize)?;
            for i in 0..N_WORDS {
                memory.insert_as_accessed((instance_ptr + i)?, &inputs.p_values[i])?;
            }
            memory.insert_as_accessed((instance_ptr + VALUES_PTR_OFFSET)?, &inputs.values_ptr)?;
            memory.insert_as_accessed(
                (instance_ptr + OFFSETS_PTR_OFFSET)?,
                (inputs.offsets_ptr + (3 * instance * self.instance_def.batch_size))?,
            )?;
            memory.insert_as_accessed(
                (instance_ptr + N_OFFSET)?,
                inputs
                    .n
                    .saturating_sub(instance * self.instance_def.batch_size),
            )?;
        }
        Ok(())
    }

    // Copies the first offsets in the offsets table to its end, n_copies times.
    fn fill_offsets(
        &self,
        memory: &mut Memory,
        offsets_ptr: Relocatable,
        index: usize,
        n_copies: usize,
    ) -> Result<(), RunnerError> {
        if n_copies.is_zero() {
            return Ok(());
        }
        for i in 0..3_usize {
            let addr = (offsets_ptr + i)?;
            let offset = memory
                .get(&((offsets_ptr + i)?))
                .ok_or_else(|| MemoryError::UnknownMemoryCell(Box::new(addr)))?
                .into_owned();
            for copy_i in 0..n_copies {
                memory.insert_as_accessed((offsets_ptr + (3 * (index + copy_i) + i))?, &offset)?;
            }
        }
        Ok(())
    }

    // Given a value, writes its n_words to memory, starting at address=addr.
    fn write_n_words_value(
        &self,
        memory: &mut Memory,
        addr: Relocatable,
        value: BigUint,
    ) -> Result<(), RunnerError> {
        let mut value = value;
        for i in 0..N_WORDS {
            let word = value.mod_floor(&self.shift);
            memory.insert_as_accessed((addr + i)?, Felt252::from(word))?;
            value = value.div_floor(&self.shift)
        }
        if !value.is_zero() {
            return Err(RunnerError::WriteNWordsValueNotZero(self.name()));
        }
        Ok(())
    }

    // Fills a value in the values table, if exactly one value is missing.
    // Returns true on success or if all values are already known.
    //
    // The builtin type (add or mul) determines which operation to perform
    fn fill_value(
        &self,
        memory: &mut Memory,
        inputs: &Inputs,
        index: usize,
    ) -> Result<bool, RunnerError> {
        let mut addresses = Vec::new();
        let mut values = Vec::new();
        for i in 0..3 {
            let addr = (inputs.values_ptr
                + memory
                    .get_integer((inputs.offsets_ptr + (3 * index + i))?)?
                    .as_ref())?;
            addresses.push(addr);
            let (_, value) = self.read_n_words_value(memory, addr)?;
            values.push(value)
        }
        let (a, b, c) = (&values[0], &values[1], &values[2]);
        match (a, b, c) {
            // Deduce c from a and b and write it to memory.
            (Some(a), Some(b), None) => {
                let value = self.apply_operation(a, b, &inputs.p)?;
                self.write_n_words_value(memory, addresses[2], value)?;
                Ok(true)
            }
            // Deduce b from a and c and write it to memory.
            (Some(a), None, Some(c)) => {
                let value = self.deduce_operand(a, c, &inputs.p)?;
                self.write_n_words_value(memory, addresses[1], value)?;
                Ok(true)
            }
            // Deduce a from b and c and write it to memory.
            (None, Some(b), Some(c)) => {
                let value = self.deduce_operand(b, c, &inputs.p)?;
                self.write_n_words_value(memory, addresses[0], value)?;
                Ok(true)
            }
            // All values are already known.
            (Some(_), Some(_), Some(_)) => Ok(true),
            _ => Ok(false),
        }
    }

    /// NOTE: It is advisable to use VirtualMachine::mod_builtin_fill_memory instead of this method directly
    /// when implementing hints to avoid cloning the runners

    /// Fills the memory with inputs to the builtin instances based on the inputs to the
    /// first instance, pads the offsets table to fit the number of operations writen in the
    /// input to the first instance, and caculates missing values in the values table.

    /// For each builtin, the given tuple is of the form (builtin_ptr, builtin_runner, n),
    /// where n is the number of operations in the offsets table (i.e., the length of the
    /// offsets table is 3*n).

    /// The number of operations written to the input of the first instance n' should be at
    /// least n and a multiple of batch_size. Previous offsets are copied to the end of the
    /// offsets table to make its length 3n'.
    pub fn fill_memory(
        memory: &mut Memory,
        add_mod: Option<(Relocatable, &ModBuiltinRunner, usize)>,
        mul_mod: Option<(Relocatable, &ModBuiltinRunner, usize)>,
    ) -> Result<(), RunnerError> {
        if add_mod.is_none() && mul_mod.is_none() {
            return Err(RunnerError::FillMemoryNoBuiltinSet);
        }
        // Check that the instance definitions of the builtins are the same.
        if let (Some((_, add_mod, _)), Some((_, mul_mod, _))) = (add_mod, mul_mod) {
            if add_mod.instance_def.word_bit_len != mul_mod.instance_def.word_bit_len {
                return Err(RunnerError::ModBuiltinsMismatchedInstanceDef);
            }
        }
        // Fill the inputs to the builtins.
        let (add_mod_inputs, add_mod_n) =
            if let Some((add_mod_addr, add_mod, add_mod_index)) = add_mod {
                let add_mod_inputs = add_mod.read_inputs(memory, add_mod_addr)?;
                add_mod.fill_inputs(memory, add_mod_addr, &add_mod_inputs)?;
                add_mod.fill_offsets(
                    memory,
                    add_mod_inputs.offsets_ptr,
                    add_mod_index,
                    add_mod_inputs.n.saturating_sub(add_mod_index),
                )?;
                (add_mod_inputs, add_mod_index)
            } else {
                Default::default()
            };

        let (mul_mod_inputs, mul_mod_n) =
            if let Some((mul_mod_addr, mul_mod, mul_mod_index)) = mul_mod {
                let mul_mod_inputs = mul_mod.read_inputs(memory, mul_mod_addr)?;
                mul_mod.fill_inputs(memory, mul_mod_addr, &mul_mod_inputs)?;
                mul_mod.fill_offsets(
                    memory,
                    mul_mod_inputs.offsets_ptr,
                    mul_mod_index,
                    mul_mod_inputs.n.saturating_sub(mul_mod_index),
                )?;
                (mul_mod_inputs, mul_mod_index)
            } else {
                Default::default()
            };

        // Fill the values table.
        let mut add_mod_index = 0;
        let mut mul_mod_index = 0;

        while add_mod_index < add_mod_n || mul_mod_index < mul_mod_n {
            if add_mod_index < add_mod_n {
                if let Some((_, add_mod_runner, _)) = add_mod {
                    if add_mod_runner.fill_value(memory, &add_mod_inputs, add_mod_index)? {
                        add_mod_index += 1;
                        continue;
                    }
                }
            }

            if mul_mod_index < mul_mod_n {
                if let Some((_, mul_mod_runner, _)) = mul_mod {
                    if mul_mod_runner.fill_value(memory, &mul_mod_inputs, mul_mod_index)? {
                        mul_mod_index += 1;
                    }
                    continue;
                }
            }

            return Err(RunnerError::FillMemoryCoudNotFillTable(
                add_mod_index,
                mul_mod_index,
            ));
        }
        Ok(())
    }

    // Additional checks added to the standard builtin runner security checks
    pub(crate) fn run_additional_security_checks(
        &self,
        vm: &VirtualMachine,
    ) -> Result<(), VirtualMachineError> {
        let segment_size = vm
            .get_segment_used_size(self.base)
            .ok_or(MemoryError::MissingSegmentUsedSizes)?;
        let n_instances = div_ceil(segment_size, CELLS_PER_MOD as usize);
        let mut prev_inputs = Inputs::default();
        for instance in 0..n_instances {
            let inputs = self.read_inputs(
                &vm.segments.memory,
                (self.base as isize, instance * CELLS_PER_MOD as usize).into(),
            )?;
            if !instance.is_zero() && prev_inputs.n > self.instance_def.batch_size {
                for i in 0..N_WORDS {
                    if inputs.p_values[i] != prev_inputs.p_values[i] {
                        return Err(RunnerError::ModBuiltinSecurityCheck(Box::new((self.name(), format!("inputs.p_values[i] != prev_inputs.p_values[i]. Got: i={}, inputs.p_values[i]={}, prev_inputs.p_values[i]={}",
                    i, inputs.p_values[i], prev_inputs.p_values[i])))).into());
                    }
                }
                if inputs.values_ptr != prev_inputs.values_ptr {
                    return Err(RunnerError::ModBuiltinSecurityCheck(Box::new((self.name(), format!("inputs.values_ptr != prev_inputs.values_ptr. Got: inputs.values_ptr={}, prev_inputs.values_ptr={}",
                inputs.values_ptr, prev_inputs.values_ptr)))).into());
                }
                if inputs.offsets_ptr
                    != (prev_inputs.offsets_ptr + (3 * self.instance_def.batch_size))?
                {
                    return Err(RunnerError::ModBuiltinSecurityCheck(Box::new((self.name(), format!("inputs.offsets_ptr != prev_inputs.offsets_ptr + 3 * batch_size. Got: inputs.offsets_ptr={}, prev_inputs.offsets_ptr={}, batch_size={}",
                inputs.values_ptr, prev_inputs.values_ptr, self.instance_def.batch_size)))).into());
                }
                if inputs.n != prev_inputs.n.saturating_sub(self.instance_def.batch_size) {
                    return Err(RunnerError::ModBuiltinSecurityCheck(Box::new((self.name(), format!("inputs.n != prev_inputs.n - batch_size. Got: inputs.n={}, prev_inputs.n={}, batch_size={}",
                inputs.n, prev_inputs.n, self.instance_def.batch_size)))).into());
                }
            }
            for index_in_batch in 0..self.instance_def.batch_size {
                let (a, b, c) = self.read_memory_vars(
                    &vm.segments.memory,
                    inputs.values_ptr,
                    inputs.offsets_ptr,
                    index_in_batch,
                )?;
                let a_op_b = self.apply_operation(&a, &b, &inputs.p)?;
                if a_op_b.mod_floor(&inputs.p) != c.mod_floor(&inputs.p) {
                    // Build error string
                    let p = inputs.p;
                    let op = self.builtin_type.operation_string();
                    let error_string = format!("Expected a {op} b == c (mod p). Got: instance={instance}, batch={index_in_batch}, p={p}, a={a}, b={b}, c={c}.");
                    return Err(RunnerError::ModBuiltinSecurityCheck(Box::new((
                        self.name(),
                        error_string,
                    )))
                    .into());
                }
            }
            prev_inputs = inputs;
        }
        if !n_instances.is_zero() && prev_inputs.n != self.instance_def.batch_size {
            return Err(RunnerError::ModBuiltinSecurityCheck(Box::new((
                self.name(),
                format!(
                    "prev_inputs.n != batch_size Got: prev_inputs.n={}, batch_size={}",
                    prev_inputs.n, self.instance_def.batch_size
                ),
            )))
            .into());
        }
        Ok(())
    }

    #[cfg(test)]
    #[cfg(feature = "mod_builtin")]
    // Testing method used to test programs that use parameters which are not included in any layout
    // For example, programs with large batch size
    pub(crate) fn override_layout_params(&mut self, batch_size: usize, word_bit_len: u32) {
        self.instance_def.batch_size = batch_size;
        self.instance_def.word_bit_len = word_bit_len;
        self.shift = BigUint::one().shl(word_bit_len);
        self.shift_powers = core::array::from_fn(|i| self.shift.pow(i as u32));
        self.zero_segment_size = core::cmp::max(N_WORDS, batch_size * 3);
    }

    // Calculates the result of `lhs OP rhs`
    //
    // The builtin type (add or mul) determines the OP
    pub(crate) fn apply_operation(
        &self,
        lhs: &BigUint,
        rhs: &BigUint,
        prime: &BigUint,
    ) -> Result<BigUint, MathError> {
        let full_value = match self.builtin_type {
            ModBuiltinType::Mul => lhs * rhs,
            ModBuiltinType::Add => lhs + rhs,
        };

        let value = if full_value < &self.k_bound * prime {
            full_value.mod_floor(prime)
        } else {
            full_value - (&self.k_bound - 1u32) * prime
        };

        Ok(value)
    }

    // Given `known OP unknown = result (mod p)`, it deduces `unknown`
    //
    // The builtin type (add or mul) determines the OP
    pub(crate) fn deduce_operand(
        &self,
        known: &BigUint,
        result: &BigUint,
        prime: &BigUint,
    ) -> Result<BigUint, MathError> {
        let value = match self.builtin_type {
            ModBuiltinType::Add => {
                if known <= result {
                    result - known
                } else {
                    result + prime - known
                }
            }
            ModBuiltinType::Mul => div_mod_unsigned(result, known, prime)?,
        };
        Ok(value)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn apply_operation_add() {
        let builtin = ModBuiltinRunner::new_add_mod(&ModInstanceDef::new(Some(8), 8, 8), true);

        assert_eq!(
            builtin
                .apply_operation(
                    &BigUint::from(2u32),
                    &BigUint::from(3u32),
                    &BigUint::from(7u32)
                )
                .unwrap(),
            BigUint::from(5u32)
        );

        assert_eq!(
            builtin
                .apply_operation(
                    &BigUint::from(5u32),
                    &BigUint::from(5u32),
                    &BigUint::from(5u32)
                )
                .unwrap(),
            BigUint::from(5u32)
        );
    }

    #[test]
    fn apply_operation_mul() {
        let builtin = ModBuiltinRunner::new_mul_mod(&ModInstanceDef::new(Some(8), 8, 8), true);

        assert_eq!(
            builtin
                .apply_operation(
                    &BigUint::from(2u32),
                    &BigUint::from(3u32),
                    &BigUint::from(7u32)
                )
                .unwrap(),
            BigUint::from(6u32)
        );
    }

    #[test]
    fn deduce_operand_add() {
        let builtin = ModBuiltinRunner::new_add_mod(&ModInstanceDef::new(Some(8), 8, 8), true);

        assert_eq!(
            builtin
                .deduce_operand(
                    &BigUint::from(2u32),
                    &BigUint::from(5u32),
                    &BigUint::from(7u32)
                )
                .unwrap(),
            BigUint::from(3u32)
        );
        assert_eq!(
            builtin
                .deduce_operand(
                    &BigUint::from(5u32),
                    &BigUint::from(2u32),
                    &BigUint::from(7u32)
                )
                .unwrap(),
            BigUint::from(4u32)
        );
    }

    #[test]
    fn deduce_operand_mul() {
        let builtin = ModBuiltinRunner::new_mul_mod(&ModInstanceDef::new(Some(8), 8, 8), true);

        assert_eq!(
            builtin
                .deduce_operand(
                    &BigUint::from(2u32),
                    &BigUint::from(1u32),
                    &BigUint::from(7u32)
                )
                .unwrap(),
            BigUint::from(4u32)
        );
    }

    #[test]
    #[cfg(feature = "mod_builtin")]
    fn test_air_private_input_all_cairo() {
        use crate::{
            air_private_input::{ModInput, ModInputInstance, ModInputMemoryVars, PrivateInput},
            hint_processor::builtin_hint_processor::builtin_hint_processor_definition::BuiltinHintProcessor,
            types::layout_name::LayoutName,
            utils::test_utils::Program,
            vm::runners::cairo_runner::CairoRunner,
            Felt252,
        };

        let program_data = include_bytes!(
            "../../../../../cairo_programs/mod_builtin_feature/proof/mod_builtin.json"
        );

        let mut hint_processor = BuiltinHintProcessor::new_empty();
        let program = Program::from_bytes(program_data, Some("main")).unwrap();
        let mut runner =
            CairoRunner::new(&program, LayoutName::all_cairo, None, true, false).unwrap();

        let end = runner.initialize(false).unwrap();
        // Modify add_mod & mul_mod params

        runner.run_until_pc(end, &mut hint_processor).unwrap();
        runner.run_for_steps(1, &mut hint_processor).unwrap();
        runner.end_run(false, false, &mut hint_processor).unwrap();
        runner.read_return_values(false).unwrap();
        runner.finalize_segments().unwrap();

        // We compare against the execution of python cairo-run with the same layout
        let air_private_input = runner.get_air_private_input();
        assert_eq!(
            air_private_input.0.get(&BuiltinName::add_mod).unwrap()[0],
            PrivateInput::Mod(ModInput {
                instances: vec![
                    ModInputInstance {
                        index: 0,
                        p0: Felt252::ONE,
                        p1: Felt252::ONE,
                        p2: Felt252::ZERO,
                        p3: Felt252::ZERO,
                        values_ptr: 23023,
                        offsets_ptr: 23055,
                        n: 2,
                        batch: BTreeMap::from([(
                            0,
                            ModInputMemoryVars {
                                a_offset: 0,
                                a0: Felt252::ONE,
                                a1: Felt252::ZERO,
                                a2: Felt252::ZERO,
                                a3: Felt252::ZERO,
                                b_offset: 12,
                                b0: Felt252::ONE,
                                b1: Felt252::ONE,
                                b2: Felt252::ZERO,
                                b3: Felt252::ZERO,
                                c_offset: 4,
                                c0: Felt252::TWO,
                                c1: Felt252::ONE,
                                c2: Felt252::ZERO,
                                c3: Felt252::ZERO
                            }
                        ),])
                    },
                    ModInputInstance {
                        index: 1,
                        p0: Felt252::ONE,
                        p1: Felt252::ONE,
                        p2: Felt252::ZERO,
                        p3: Felt252::ZERO,
                        values_ptr: 23023,
                        offsets_ptr: 23058,
                        n: 1,
                        batch: BTreeMap::from([(
                            0,
                            ModInputMemoryVars {
                                a_offset: 16,
                                a0: Felt252::ZERO,
                                a1: Felt252::ZERO,
                                a2: Felt252::ZERO,
                                a3: Felt252::ZERO,
                                b_offset: 20,
                                b0: Felt252::TWO,
                                b1: Felt252::ZERO,
                                b2: Felt252::ZERO,
                                b3: Felt252::ZERO,
                                c_offset: 24,
                                c0: Felt252::TWO,
                                c1: Felt252::ZERO,
                                c2: Felt252::ZERO,
                                c3: Felt252::ZERO
                            }
                        ),])
                    }
                ],
                zero_value_address: 23019
            })
        );
        assert_eq!(
            air_private_input.0.get(&BuiltinName::mul_mod).unwrap()[0],
            PrivateInput::Mod(ModInput {
                instances: vec![
                    ModInputInstance {
                        index: 0,
                        p0: Felt252::ONE,
                        p1: Felt252::ONE,
                        p2: Felt252::ZERO,
                        p3: Felt252::ZERO,
                        values_ptr: 23023,
                        offsets_ptr: 23061,
                        n: 3,
                        batch: BTreeMap::from([(
                            0,
                            ModInputMemoryVars {
                                a_offset: 12,
                                a0: Felt252::ONE,
                                a1: Felt252::ONE,
                                a2: Felt252::ZERO,
                                a3: Felt252::ZERO,
                                b_offset: 8,
                                b0: Felt252::TWO,
                                b1: Felt252::ZERO,
                                b2: Felt252::ZERO,
                                b3: Felt252::ZERO,
                                c_offset: 16,
                                c0: Felt252::ZERO,
                                c1: Felt252::ZERO,
                                c2: Felt252::ZERO,
                                c3: Felt252::ZERO
                            }
                        ),])
                    },
                    ModInputInstance {
                        index: 1,
                        p0: Felt252::ONE,
                        p1: Felt252::ONE,
                        p2: Felt252::ZERO,
                        p3: Felt252::ZERO,
                        values_ptr: 23023,
                        offsets_ptr: 23064,
                        n: 2,
                        batch: BTreeMap::from([(
                            0,
                            ModInputMemoryVars {
                                a_offset: 0,
                                a0: Felt252::ONE,
                                a1: Felt252::ZERO,
                                a2: Felt252::ZERO,
                                a3: Felt252::ZERO,
                                b_offset: 8,
                                b0: Felt252::TWO,
                                b1: Felt252::ZERO,
                                b2: Felt252::ZERO,
                                b3: Felt252::ZERO,
                                c_offset: 20,
                                c0: Felt252::TWO,
                                c1: Felt252::ZERO,
                                c2: Felt252::ZERO,
                                c3: Felt252::ZERO
                            }
                        ),])
                    },
                    ModInputInstance {
                        index: 2,
                        p0: Felt252::ONE,
                        p1: Felt252::ONE,
                        p2: Felt252::ZERO,
                        p3: Felt252::ZERO,
                        values_ptr: 23023,
                        offsets_ptr: 23067,
                        n: 1,
                        batch: BTreeMap::from([(
                            0,
                            ModInputMemoryVars {
                                a_offset: 8,
                                a0: Felt252::TWO,
                                a1: Felt252::ZERO,
                                a2: Felt252::ZERO,
                                a3: Felt252::ZERO,
                                b_offset: 28,
                                b0: Felt252::ONE,
                                b1: Felt252::ZERO,
                                b2: Felt252::ZERO,
                                b3: Felt252::ZERO,
                                c_offset: 24,
                                c0: Felt252::TWO,
                                c1: Felt252::ZERO,
                                c2: Felt252::ZERO,
                                c3: Felt252::ZERO
                            }
                        ),])
                    }
                ],
                zero_value_address: 23019
            })
        )
    }
}
