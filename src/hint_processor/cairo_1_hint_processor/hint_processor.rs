use super::dict_manager::DictManagerExecScope;
use super::hint_processor_utils::*;
use crate::any_box;
use crate::felt::{felt_str, Felt252};
use crate::hint_processor::cairo_1_hint_processor::dict_manager::DictSquashExecScope;
use crate::hint_processor::hint_processor_definition::HintReference;
use crate::types::relocatable::Relocatable;

use crate::stdlib::collections::HashMap;
use crate::stdlib::prelude::*;
use crate::{
    hint_processor::hint_processor_definition::HintProcessor,
    types::exec_scope::ExecutionScopes,
    vm::errors::vm_errors::VirtualMachineError,
    vm::{errors::hint_errors::HintError, vm_core::VirtualMachine},
};
use ark_ff::fields::{Fp256, MontBackend, MontConfig};
use ark_ff::{Field, PrimeField};
use ark_std::UniformRand;
use cairo_lang_casm::{
    hints::{CoreHint, Hint},
    operand::{CellRef, ResOperand},
};
use core::any::Any;
use core::ops::Mul;
use num_bigint::BigUint;
use num_integer::Integer;
use num_traits::{cast::ToPrimitive, Zero};

/// Execution scope for constant memory allocation.
struct MemoryExecScope {
    /// The first free address in the segment.
    next_address: Relocatable,
}

#[derive(MontConfig)]
#[modulus = "3618502788666131213697322783095070105623107215331596699973092056135872020481"]
#[generator = "3"]

/// Returns the Beta value of the Starkware elliptic curve.
struct FqConfig;
type Fq = Fp256<MontBackend<FqConfig, 4>>;

fn get_beta() -> Felt252 {
    felt_str!("3141592653589793238462643383279502884197169399375105820974944592307816406665")
}

/// HintProcessor for Cairo 1 compiler hints.
pub struct Cairo1HintProcessor {
    hints: HashMap<usize, Vec<Hint>>,
}

impl Cairo1HintProcessor {
    pub fn new(hints: &[(usize, Vec<Hint>)]) -> Self {
        Self {
            hints: hints.iter().cloned().collect(),
        }
    }
    // Runs a single Hint
    pub fn execute(
        &self,
        vm: &mut VirtualMachine,
        exec_scopes: &mut ExecutionScopes,
        hint: &Hint,
    ) -> Result<(), HintError> {
        match hint {
            Hint::Core(CoreHint::AllocSegment { dst }) => self.alloc_segment(vm, dst),
            Hint::Core(CoreHint::TestLessThan { lhs, rhs, dst }) => {
                self.test_less_than(vm, lhs, rhs, dst)
            }
            Hint::Core(CoreHint::TestLessThanOrEqual { lhs, rhs, dst }) => {
                self.test_less_than_or_equal(vm, lhs, rhs, dst)
            }
            Hint::Core(CoreHint::Felt252DictRead {
                dict_ptr,
                key,
                value_dst,
            }) => self.dict_read(vm, exec_scopes, dict_ptr, key, value_dst),
            Hint::Core(CoreHint::SquareRoot { value, dst }) => self.square_root(vm, value, dst),
            Hint::Core(CoreHint::GetSegmentArenaIndex {
                dict_end_ptr,
                dict_index,
            }) => self.get_segment_arena_index(vm, exec_scopes, dict_end_ptr, dict_index),

            Hint::Core(CoreHint::DivMod {
                lhs,
                rhs,
                quotient,
                remainder,
            }) => self.div_mod(vm, lhs, rhs, quotient, remainder),
            Hint::Core(CoreHint::DebugPrint { start, end }) => self.debug_print(vm, start, end),

            Hint::Core(CoreHint::Uint256SquareRoot {
                value_low,
                value_high,
                sqrt0,
                sqrt1,
                remainder_low,
                remainder_high,
                sqrt_mul_2_minus_remainder_ge_u128,
            }) => self.uint256_square_root(
                vm,
                value_low,
                value_high,
                sqrt0,
                sqrt1,
                remainder_low,
                remainder_high,
                sqrt_mul_2_minus_remainder_ge_u128,
            ),

            Hint::Core(CoreHint::GetNextDictKey { next_key }) => {
                self.get_next_dict_key(vm, exec_scopes, next_key)
            }

            Hint::Core(CoreHint::Uint256DivMod {
                dividend_low,
                dividend_high,
                divisor_low,
                divisor_high,
                quotient0,
                quotient1,
                divisor0,
                divisor1,
                extra0,
                extra1,
                remainder_low,
                remainder_high,
            }) => self.uint256_div_mod(
                vm,
                dividend_low,
                dividend_high,
                divisor_low,
                divisor_high,
                quotient0,
                quotient1,
                divisor0,
                divisor1,
                extra0,
                extra1,
                remainder_low,
                remainder_high,
            ),
            Hint::Core(CoreHint::Felt252DictWrite {
                dict_ptr,
                key,
                value,
            }) => self.dict_write(exec_scopes, vm, dict_ptr, key, value),
            Hint::Core(CoreHint::AssertLeIsFirstArcExcluded {
                skip_exclude_a_flag,
            }) => self.assert_le_if_first_arc_exclueded(vm, skip_exclude_a_flag, exec_scopes),

            Hint::Core(CoreHint::AssertAllAccessesUsed { n_used_accesses }) => {
                self.assert_all_accesses_used(vm, exec_scopes, n_used_accesses)
            }

            Hint::Core(CoreHint::AssertLeIsSecondArcExcluded {
                skip_exclude_b_minus_a,
            }) => self.assert_le_is_second_excluded(vm, skip_exclude_b_minus_a, exec_scopes),

            Hint::Core(CoreHint::LinearSplit {
                value,
                scalar,
                max_x,
                x,
                y,
            }) => self.linear_split(vm, value, scalar, max_x, x, y),

            Hint::Core(CoreHint::AllocFelt252Dict { segment_arena_ptr }) => {
                self.alloc_felt_256_dict(vm, segment_arena_ptr, exec_scopes)
            }

            Hint::Core(CoreHint::AssertLeFindSmallArcs {
                range_check_ptr,
                a,
                b,
            }) => self.assert_le_find_small_arcs(vm, exec_scopes, range_check_ptr, a, b),

            Hint::Core(CoreHint::RandomEcPoint { x, y }) => self.random_ec_point(vm, x, y),

            Hint::Core(CoreHint::ShouldSkipSquashLoop { should_skip_loop }) => {
                self.should_skip_squash_loop(vm, exec_scopes, should_skip_loop)
            }
            Hint::Core(CoreHint::Felt252DictEntryInit { dict_ptr, key }) => {
                self.dict_entry_init(vm, exec_scopes, dict_ptr, key)
            }
            Hint::Core(CoreHint::Felt252DictEntryUpdate { dict_ptr, value }) => {
                self.felt_252_dict_entry_update(vm, exec_scopes, dict_ptr, value)
            }
            Hint::Core(CoreHint::GetCurrentAccessDelta { index_delta_minus1 }) => {
                self.get_current_access_delta(vm, exec_scopes, index_delta_minus1)
            }
            Hint::Core(CoreHint::InitSquashData {
                dict_accesses,
                n_accesses,
                big_keys,
                ..
            }) => self.init_squash_data(vm, exec_scopes, dict_accesses, n_accesses, big_keys),
            Hint::Core(CoreHint::AllocConstantSize { size, dst }) => {
                self.alloc_constant_size(vm, exec_scopes, size, dst)
            }
            Hint::Core(CoreHint::GetCurrentAccessIndex { range_check_ptr }) => {
                self.get_current_access_index(vm, exec_scopes, range_check_ptr)
            }
            Hint::Core(CoreHint::ShouldContinueSquashLoop { should_continue }) => {
                self.should_continue_squash_loop(vm, exec_scopes, should_continue)
            }
            Hint::Core(CoreHint::FieldSqrt { val, sqrt }) => self.field_sqrt(vm, val, sqrt),
            hint => Err(HintError::UnknownHint(hint.to_string())),
        }
    }

    fn alloc_segment(&self, vm: &mut VirtualMachine, dst: &CellRef) -> Result<(), HintError> {
        let segment = vm.add_memory_segment();
        vm.insert_value(cell_ref_to_relocatable(dst, vm)?, segment)
            .map_err(HintError::from)
    }

    fn test_less_than(
        &self,
        vm: &mut VirtualMachine,
        lhs: &ResOperand,
        rhs: &ResOperand,
        dst: &CellRef,
    ) -> Result<(), HintError> {
        let lhs_value = res_operand_get_val(vm, lhs)?;
        let rhs_value = res_operand_get_val(vm, rhs)?;
        let result = Felt252::from((lhs_value < rhs_value) as u8);

        vm.insert_value(cell_ref_to_relocatable(dst, vm)?, result)
            .map_err(HintError::from)
    }

    fn square_root(
        &self,
        vm: &mut VirtualMachine,
        value: &ResOperand,
        dst: &CellRef,
    ) -> Result<(), HintError> {
        let value = res_operand_get_val(vm, value)?;
        let result = value.sqrt();
        vm.insert_value(cell_ref_to_relocatable(dst, vm)?, result)
            .map_err(HintError::from)
    }

    fn test_less_than_or_equal(
        &self,
        vm: &mut VirtualMachine,
        lhs: &ResOperand,
        rhs: &ResOperand,
        dst: &CellRef,
    ) -> Result<(), HintError> {
        let lhs_value = res_operand_get_val(vm, lhs)?;
        let rhs_value = res_operand_get_val(vm, rhs)?;
        let result = Felt252::from((lhs_value <= rhs_value) as u8);

        vm.insert_value(cell_ref_to_relocatable(dst, vm)?, result)
            .map_err(HintError::from)
    }

    fn assert_le_find_small_arcs(
        &self,
        vm: &mut VirtualMachine,
        exec_scopes: &mut ExecutionScopes,
        range_check_ptr: &ResOperand,
        a: &ResOperand,
        b: &ResOperand,
    ) -> Result<(), HintError> {
        let a_val = res_operand_get_val(vm, a)?;
        let b_val = res_operand_get_val(vm, b)?;
        let mut lengths_and_indices = vec![
            (a_val.clone(), 0),
            (b_val.clone() - a_val, 1),
            (Felt252::from(-1) - b_val, 2),
        ];
        lengths_and_indices.sort();
        exec_scopes.assign_or_update_variable("excluded_arc", Box::new(lengths_and_indices[2].1));
        // ceil((PRIME / 3) / 2 ** 128).
        let prime_over_3_high = 3544607988759775765608368578435044694_u128;
        // ceil((PRIME / 2) / 2 ** 128).
        let prime_over_2_high = 5316911983139663648412552867652567041_u128;
        let (range_check_base, range_check_offset) = extract_buffer(range_check_ptr)?;
        let range_check_ptr = get_ptr(vm, range_check_base, &range_check_offset)?;
        vm.insert_value(
            range_check_ptr,
            Felt252::from(lengths_and_indices[0].0.to_biguint() % prime_over_3_high),
        )?;
        vm.insert_value(
            (range_check_ptr + 1)?,
            Felt252::from(lengths_and_indices[0].0.to_biguint() / prime_over_3_high),
        )?;
        vm.insert_value(
            (range_check_ptr + 2)?,
            Felt252::from(lengths_and_indices[1].0.to_biguint() % prime_over_2_high),
        )?;
        vm.insert_value(
            (range_check_ptr + 3)?,
            Felt252::from(lengths_and_indices[1].0.to_biguint() / prime_over_2_high),
        )
        .map_err(HintError::from)
    }

    fn dict_read(
        &self,
        vm: &mut VirtualMachine,
        exec_scopes: &mut ExecutionScopes,
        dict_ptr: &ResOperand,
        key: &ResOperand,
        value_dst: &CellRef,
    ) -> Result<(), HintError> {
        let (dict_base, dict_offset) = extract_buffer(dict_ptr)?;
        let dict_address = get_ptr(vm, dict_base, &dict_offset)?;
        let key = res_operand_get_val(vm, key)?;
        let dict_manager_exec_scope =
            exec_scopes.get_mut_ref::<DictManagerExecScope>("dict_manager_exec_scope")?;

        let value = dict_manager_exec_scope
            .get_from_tracker(dict_address, &key)
            .unwrap_or_else(|| DictManagerExecScope::DICT_DEFAULT_VALUE.into());

        vm.insert_value(cell_ref_to_relocatable(value_dst, vm)?, value)
            .map_err(HintError::from)
    }

    fn div_mod(
        &self,
        vm: &mut VirtualMachine,
        lhs: &ResOperand,
        rhs: &ResOperand,
        quotient: &CellRef,
        remainder: &CellRef,
    ) -> Result<(), HintError> {
        let lhs_value = res_operand_get_val(vm, lhs)?.to_biguint();
        let rhs_value = res_operand_get_val(vm, rhs)?.to_biguint();
        let quotient_value = Felt252::new(&lhs_value / &rhs_value);
        let remainder_value = Felt252::new(lhs_value % rhs_value);
        vm.insert_value(cell_ref_to_relocatable(quotient, vm)?, quotient_value)?;
        vm.insert_value(cell_ref_to_relocatable(remainder, vm)?, remainder_value)
            .map_err(HintError::from)
    }

    fn get_segment_arena_index(
        &self,
        vm: &mut VirtualMachine,
        exec_scopes: &mut ExecutionScopes,
        dict_end_ptr: &ResOperand,
        dict_index: &CellRef,
    ) -> Result<(), HintError> {
        let (dict_base, dict_offset) = extract_buffer(dict_end_ptr)?;
        let dict_address = get_ptr(vm, dict_base, &dict_offset)?;
        let dict_manager_exec_scope =
            exec_scopes.get_mut_ref::<DictManagerExecScope>("dict_manager_exec_scope")?;

        let dict_infos_index = dict_manager_exec_scope.get_dict_infos_index(dict_address)?;
        vm.insert_value(cell_ref_to_relocatable(dict_index, vm)?, dict_infos_index)
            .map_err(HintError::from)
    }

    #[allow(clippy::too_many_arguments)]
    fn uint256_div_mod(
        &self,
        vm: &mut VirtualMachine,
        dividend_low: &ResOperand,
        dividend_high: &ResOperand,
        divisor_low: &ResOperand,
        divisor_high: &ResOperand,
        quotient0: &CellRef,
        quotient1: &CellRef,
        divisor0: &CellRef,
        divisor1: &CellRef,
        extra0: &CellRef,
        extra1: &CellRef,
        remainder_low: &CellRef,
        remainder_high: &CellRef,
    ) -> Result<(), HintError> {
        let pow_2_128 = Felt252::from(u128::MAX) + 1u32;
        let pow_2_64 = Felt252::from(u64::MAX) + 1u32;
        let dividend_low = res_operand_get_val(vm, dividend_low)?;
        let dividend_high = res_operand_get_val(vm, dividend_high)?;
        let divisor_low = res_operand_get_val(vm, divisor_low)?;
        let divisor_high = res_operand_get_val(vm, divisor_high)?;
        let dividend = dividend_low + dividend_high.mul(pow_2_128.clone());
        let divisor = divisor_low + (&divisor_high * &pow_2_128);
        let quotient = &dividend / &divisor;
        let remainder = dividend % divisor.clone();

        // Guess quotient limbs.
        let (quotient, limb) = quotient.div_rem(&pow_2_64);
        vm.insert_value(cell_ref_to_relocatable(quotient0, vm)?, limb)?;
        let (quotient, limb) = quotient.div_rem(&pow_2_64);
        vm.insert_value(cell_ref_to_relocatable(quotient1, vm)?, limb)?;
        let (quotient, limb) = quotient.div_rem(&pow_2_64);
        if divisor_high.is_zero() {
            vm.insert_value(cell_ref_to_relocatable(extra0, vm)?, limb)?;
            vm.insert_value(cell_ref_to_relocatable(extra1, vm)?, quotient)?;
        }

        // Guess divisor limbs.
        let (divisor, limb) = divisor.div_rem(&pow_2_64);
        vm.insert_value(cell_ref_to_relocatable(divisor0, vm)?, limb)?;
        let (divisor, limb) = divisor.div_rem(&pow_2_64);
        vm.insert_value(cell_ref_to_relocatable(divisor1, vm)?, limb)?;
        let (divisor, limb) = divisor.div_rem(&pow_2_64);
        if !divisor_high.is_zero() {
            vm.insert_value(cell_ref_to_relocatable(extra0, vm)?, limb)?;
            vm.insert_value(cell_ref_to_relocatable(extra1, vm)?, divisor)?;
        }

        // Guess remainder limbs.
        vm.insert_value(
            cell_ref_to_relocatable(remainder_low, vm)?,
            remainder.clone() % pow_2_128.clone(),
        )?;
        vm.insert_value(
            cell_ref_to_relocatable(remainder_high, vm)?,
            remainder / pow_2_128,
        )?;
        Ok(())
    }

    fn assert_le_if_first_arc_exclueded(
        &self,
        vm: &mut VirtualMachine,
        skip_exclude_a_flag: &CellRef,
        exec_scopes: &mut ExecutionScopes,
    ) -> Result<(), HintError> {
        let excluded_arc: i32 = exec_scopes.get("excluded_arc")?;
        let val = Felt252::from((excluded_arc != 0) as u8);
        vm.insert_value(cell_ref_to_relocatable(skip_exclude_a_flag, vm)?, val)?;
        Ok(())
    }

    fn linear_split(
        &self,
        vm: &mut VirtualMachine,
        value: &ResOperand,
        scalar: &ResOperand,
        max_x: &ResOperand,
        x: &CellRef,
        y: &CellRef,
    ) -> Result<(), HintError> {
        let value = res_operand_get_val(vm, value)?;
        let scalar = res_operand_get_val(vm, scalar)?;
        let max_x = res_operand_get_val(vm, max_x)?;
        let x_value = (&value / &scalar).min(max_x);
        let y_value = value - &x_value * &scalar;

        vm.insert_value(cell_ref_to_relocatable(x, vm)?, x_value)
            .map_err(HintError::from)?;
        vm.insert_value(cell_ref_to_relocatable(y, vm)?, y_value)
            .map_err(HintError::from)?;

        Ok(())
    }

    fn random_ec_point(
        &self,
        vm: &mut VirtualMachine,
        x: &CellRef,
        y: &CellRef,
    ) -> Result<(), HintError> {
        let beta = Fq::from(get_beta().to_biguint());

        let mut rng = ark_std::test_rng();
        let (random_x, random_y_squared) = loop {
            let random_x = Fq::rand(&mut rng);
            let random_y_squared = random_x * random_x * random_x + random_x + beta;
            if random_y_squared.legendre().is_qr() {
                break (random_x, random_y_squared);
            }
        };

        let x_bigint: BigUint = random_x.into_bigint().into();
        let y_bigint: BigUint = random_y_squared
            .sqrt()
            .ok_or(HintError::CustomHint("Failed to compute sqrt".to_string()))?
            .into_bigint()
            .into();

        vm.insert_value(cell_ref_to_relocatable(x, vm)?, Felt252::from(x_bigint))?;
        vm.insert_value(cell_ref_to_relocatable(y, vm)?, Felt252::from(y_bigint))?;

        Ok(())
    }

    fn get_next_dict_key(
        &self,
        vm: &mut VirtualMachine,
        exec_scopes: &mut ExecutionScopes,
        next_key: &CellRef,
    ) -> Result<(), HintError> {
        let dict_squash_exec_scope: &mut DictSquashExecScope =
            exec_scopes.get_mut_ref("dict_squash_exec_scope")?;
        dict_squash_exec_scope.pop_current_key()?;
        if let Some(current_key) = dict_squash_exec_scope.current_key() {
            return vm
                .insert_value(cell_ref_to_relocatable(next_key, vm)?, current_key)
                .map_err(HintError::from);
        }
        Err(HintError::KeyNotFound)
    }

    fn alloc_felt_256_dict(
        &self,
        vm: &mut VirtualMachine,
        segment_arena_ptr: &ResOperand,
        exec_scopes: &mut ExecutionScopes,
    ) -> Result<(), HintError> {
        let (cell, base_offset) = extract_buffer(segment_arena_ptr)?;
        let dict_manager_address = get_ptr(vm, cell, &base_offset)?;

        let n_dicts = vm
            .get_integer((dict_manager_address - 2)?)?
            .into_owned()
            .to_usize()
            .ok_or(HintError::CustomHint(
                "Invalid number of dictionaries.".to_string(),
            ))?;

        let dict_infos_base = vm.get_relocatable((dict_manager_address - 3)?)?;

        let dict_manager_exec_scope =
            match exec_scopes.get_mut_ref::<DictManagerExecScope>("dict_manager_exec_scope") {
                Ok(dict_manager_exec_scope) => dict_manager_exec_scope,
                Err(_) => {
                    exec_scopes.assign_or_update_variable(
                        "dict_manager_exec_scope",
                        Box::<DictManagerExecScope>::default(),
                    );
                    exec_scopes.get_mut_ref::<DictManagerExecScope>("dict_manager_exec_scope")?
                }
            };
        let new_dict_segment = dict_manager_exec_scope.new_default_dict(vm)?;
        vm.insert_value((dict_infos_base + 3 * n_dicts)?, new_dict_segment)?;

        Ok(())
    }

    fn assert_le_is_second_excluded(
        &self,
        vm: &mut VirtualMachine,
        skip_exclude_b_minus_a: &CellRef,
        exec_scopes: &mut ExecutionScopes,
    ) -> Result<(), HintError> {
        let excluded_arc: i32 = exec_scopes.get("excluded_arc")?;
        let val = Felt252::from((excluded_arc != 1) as u8);

        vm.insert_value(cell_ref_to_relocatable(skip_exclude_b_minus_a, vm)?, val)?;

        Ok(())
    }

    fn dict_write(
        &self,
        exec_scopes: &mut ExecutionScopes,
        vm: &mut VirtualMachine,
        dict_ptr: &ResOperand,
        key: &ResOperand,
        value: &ResOperand,
    ) -> Result<(), HintError> {
        let (dict_base, dict_offset) = extract_buffer(dict_ptr)?;
        let dict_address = get_ptr(vm, dict_base, &dict_offset)?;
        let key = res_operand_get_val(vm, key)?;
        let value = res_operand_get_val(vm, value)?;
        let dict_manager_exec_scope =
            exec_scopes.get_mut_ref::<DictManagerExecScope>("dict_manager_exec_scope")?;

        let prev_value = dict_manager_exec_scope
            .get_from_tracker(dict_address, &key)
            .unwrap_or_else(|| DictManagerExecScope::DICT_DEFAULT_VALUE.into());

        vm.insert_value((dict_address + 1)?, prev_value)?;
        dict_manager_exec_scope.insert_to_tracker(dict_address, key, value);
        Ok(())
    }

    #[allow(clippy::too_many_arguments)]
    fn uint256_square_root(
        &self,
        vm: &mut VirtualMachine,
        value_low: &ResOperand,
        value_high: &ResOperand,
        sqrt0: &CellRef,
        sqrt1: &CellRef,
        remainder_low: &CellRef,
        remainder_high: &CellRef,
        sqrt_mul_2_minus_remainder_ge_u128: &CellRef,
    ) -> Result<(), HintError> {
        let pow_2_128 = Felt252::from(u128::MAX) + 1u32;
        let pow_2_64 = Felt252::from(u64::MAX) + 1u32;
        let value_low = res_operand_get_val(vm, value_low)?;
        let value_high = res_operand_get_val(vm, value_high)?;
        let value = value_low + value_high * &pow_2_128;
        let sqrt = value.sqrt();
        let remainder = value - &sqrt * &sqrt;
        let sqrt_mul_2_minus_remainder_ge_u128_val =
            &sqrt * &Felt252::from(2u32) - &remainder >= pow_2_128;

        let (sqrt1_val, sqrt0_val) = sqrt.div_rem(&pow_2_64);
        vm.insert_value(cell_ref_to_relocatable(sqrt0, vm)?, sqrt0_val)?;
        vm.insert_value(cell_ref_to_relocatable(sqrt1, vm)?, sqrt1_val)?;

        let (remainder_high_val, remainder_low_val) = remainder.div_rem(&pow_2_128);

        vm.insert_value(
            cell_ref_to_relocatable(remainder_low, vm)?,
            remainder_low_val,
        )?;
        vm.insert_value(
            cell_ref_to_relocatable(remainder_high, vm)?,
            remainder_high_val,
        )?;
        vm.insert_value(
            cell_ref_to_relocatable(sqrt_mul_2_minus_remainder_ge_u128, vm)?,
            usize::from(sqrt_mul_2_minus_remainder_ge_u128_val),
        )?;

        Ok(())
    }

    fn dict_entry_init(
        &self,
        vm: &mut VirtualMachine,
        exec_scopes: &mut ExecutionScopes,
        dict_ptr: &ResOperand,
        key: &ResOperand,
    ) -> Result<(), HintError> {
        let (dict_base, dict_offset) = extract_buffer(dict_ptr)?;
        let dict_address = get_ptr(vm, dict_base, &dict_offset)?;
        let key = res_operand_get_val(vm, key)?;
        let dict_manager_exec_scope =
            exec_scopes.get_mut_ref::<DictManagerExecScope>("dict_manager_exec_scope")?;

        let prev_value = dict_manager_exec_scope
            .get_from_tracker(dict_address, &key)
            .unwrap_or_else(|| DictManagerExecScope::DICT_DEFAULT_VALUE.into());

        vm.insert_value((dict_address + 1)?, prev_value)
            .map_err(HintError::from)
    }

    fn debug_print(
        &self,
        vm: &mut VirtualMachine,
        start: &ResOperand,
        end: &ResOperand,
    ) -> Result<(), HintError> {
        let mut curr = as_relocatable(vm, start)?;
        let end = as_relocatable(vm, end)?;

        #[cfg(not(target_arch = "wasm32"))]
        {
            while curr != end {
                let value = vm.get_integer(curr)?;
                if let Some(shortstring) = as_cairo_short_string(&value) {
                    println!("[DEBUG]\t{shortstring: <31}\t(raw: {value: <31})");
                } else {
                    println!("[DEBUG]\t{0: <31}\t(raw: {value: <31}) ", ' ');
                }
                curr += 1;
            }
            println!();
        }
        Ok(())
    }

    fn assert_all_accesses_used(
        &self,
        vm: &mut VirtualMachine,
        exec_scopes: &mut ExecutionScopes,
        n_used_accesses: &CellRef,
    ) -> Result<(), HintError> {
        let key = exec_scopes.get::<Felt252>("key")?;
        let n = get_cell_val(vm, n_used_accesses)?;

        let dict_squash_exec_scope: &mut DictSquashExecScope =
            exec_scopes.get_mut_ref("dict_squash_exec_scope")?;

        let access_indices_at_key = dict_squash_exec_scope
            .access_indices
            .get(&key.clone())
            .ok_or_else(|| HintError::NoKeyInAccessIndices(key.clone()))?;

        if n != Felt252::new(access_indices_at_key.len()) {
            return Err(HintError::NumUsedAccessesAssertFail(
                n,
                access_indices_at_key.len(),
                key,
            ));
        }

        Ok(())
    }

    fn should_skip_squash_loop(
        &self,
        vm: &mut VirtualMachine,
        exec_scopes: &mut ExecutionScopes,
        should_skip_loop: &CellRef,
    ) -> Result<(), HintError> {
        let dict_squash_exec_scope: &mut DictSquashExecScope =
            exec_scopes.get_mut_ref("dict_squash_exec_scope")?;

        let val = Felt252::from(
            (dict_squash_exec_scope
                .current_access_indices()
                .ok_or(HintError::CustomHint("no indices accessed".to_string()))?
                .len()
                > 1) as u8,
        );

        vm.insert_value(cell_ref_to_relocatable(should_skip_loop, vm)?, val)?;

        Ok(())
    }

    fn felt_252_dict_entry_update(
        &self,
        vm: &mut VirtualMachine,
        exec_scopes: &mut ExecutionScopes,
        dict_ptr: &ResOperand,
        value: &ResOperand,
    ) -> Result<(), HintError> {
        let (dict_base, dict_offset) = extract_buffer(dict_ptr)?;
        let dict_address = get_ptr(vm, dict_base, &dict_offset)?;
        let key = get_double_deref_val(vm, dict_base, &(dict_offset + Felt252::from(-3)))?;
        let value = res_operand_get_val(vm, value)?;
        let dict_manager_exec_scope = exec_scopes
            .get_mut_ref::<DictManagerExecScope>("dict_manager_exec_scope")
            .map_err(|_| {
                HintError::CustomHint(
                    "Trying to write to a dict while dict manager was not initialized.".to_string(),
                )
            })?;
        dict_manager_exec_scope.insert_to_tracker(dict_address, key, value);

        Ok(())
    }

    fn get_current_access_delta(
        &self,
        vm: &mut VirtualMachine,
        exec_scopes: &mut ExecutionScopes,
        index_delta_minus1: &CellRef,
    ) -> Result<(), HintError> {
        let dict_squash_exec_scope: &mut DictSquashExecScope =
            exec_scopes.get_mut_ref("dict_squash_exec_scope")?;
        let prev_access_index = dict_squash_exec_scope
            .pop_current_access_index()
            .ok_or(HintError::CustomHint("no accessed index".to_string()))?;
        let index_delta_minus_1_val = dict_squash_exec_scope
            .current_access_index()
            .ok_or(HintError::CustomHint("no index accessed".to_string()))?
            .clone()
            - prev_access_index
            - 1_u32;

        vm.insert_value(
            cell_ref_to_relocatable(index_delta_minus1, vm)?,
            index_delta_minus_1_val,
        )?;

        Ok(())
    }

    fn init_squash_data(
        &self,
        vm: &mut VirtualMachine,
        exec_scopes: &mut ExecutionScopes,
        dict_accesses: &ResOperand,
        n_accesses: &ResOperand,
        big_keys: &CellRef,
    ) -> Result<(), HintError> {
        let dict_access_size = 3;
        let rangecheck_bound = Felt252::from(u128::MAX) + 1u32;

        exec_scopes.assign_or_update_variable(
            "dict_squash_exec_scope",
            Box::<DictSquashExecScope>::default(),
        );
        let dict_squash_exec_scope =
            exec_scopes.get_mut_ref::<DictSquashExecScope>("dict_squash_exec_scope")?;
        let (dict_accesses_base, dict_accesses_offset) = extract_buffer(dict_accesses)?;
        let dict_accesses_address = get_ptr(vm, dict_accesses_base, &dict_accesses_offset)?;
        let n_accesses =
            res_operand_get_val(vm, n_accesses)?
                .to_usize()
                .ok_or(HintError::CustomHint(
                    "Number of accesses is too large or negative.".to_string(),
                ))?;

        for i in 0..n_accesses {
            let current_key = vm.get_integer((dict_accesses_address + i * dict_access_size)?)?;
            dict_squash_exec_scope
                .access_indices
                .entry(current_key.into_owned())
                .and_modify(|indices| indices.push(Felt252::from(i)))
                .or_insert_with(|| vec![Felt252::from(i)]);
        }
        // Reverse the accesses in order to pop them in order later.
        for (_, accesses) in dict_squash_exec_scope.access_indices.iter_mut() {
            accesses.reverse();
        }

        dict_squash_exec_scope.keys = dict_squash_exec_scope
            .access_indices
            .keys()
            .cloned()
            .collect();
        dict_squash_exec_scope.keys.sort_by(|a, b| b.cmp(a));
        // big_keys indicates if the keys are greater than rangecheck_bound. If they are not
        // a simple range check is used instead of assert_le_felt252.

        let val = Felt252::from((dict_squash_exec_scope.keys[0] < rangecheck_bound) as u8);

        vm.insert_value(cell_ref_to_relocatable(big_keys, vm)?, val)?;

        vm.insert_value(
            cell_ref_to_relocatable(big_keys, vm)?,
            dict_squash_exec_scope
                .current_key()
                .ok_or(HintError::CustomHint("No current key".to_string()))?,
        )?;

        Ok(())
    }

    fn alloc_constant_size(
        &self,
        vm: &mut VirtualMachine,
        exec_scopes: &mut ExecutionScopes,
        size: &ResOperand,
        dst: &CellRef,
    ) -> Result<(), HintError> {
        let object_size = res_operand_get_val(vm, size)?
            .to_usize()
            .expect("Object size too large.");
        let memory_exec_scope =
            match exec_scopes.get_mut_ref::<MemoryExecScope>("memory_exec_scope") {
                Ok(memory_exec_scope) => memory_exec_scope,
                Err(_) => {
                    exec_scopes.assign_or_update_variable(
                        "memory_exec_scope",
                        Box::new(MemoryExecScope {
                            next_address: vm.add_memory_segment(),
                        }),
                    );
                    exec_scopes.get_mut_ref::<MemoryExecScope>("memory_exec_scope")?
                }
            };

        vm.insert_value(
            cell_ref_to_relocatable(dst, vm)?,
            memory_exec_scope.next_address,
        )?;

        memory_exec_scope.next_address.offset += object_size;
        Ok(())
    }

    fn get_current_access_index(
        &self,
        vm: &mut VirtualMachine,
        exec_scopes: &mut ExecutionScopes,
        range_check_ptr: &ResOperand,
    ) -> Result<(), HintError> {
        let dict_squash_exec_scope: &mut DictSquashExecScope =
            exec_scopes.get_mut_ref("dict_squash_exec_scope")?;
        let (range_check_base, range_check_offset) = extract_buffer(range_check_ptr)?;
        let range_check_ptr = get_ptr(vm, range_check_base, &range_check_offset)?;
        let current_access_index =
            dict_squash_exec_scope
                .current_access_index()
                .ok_or(HintError::CustomHint(
                    "No current accessed index".to_string(),
                ))?;
        vm.insert_value(range_check_ptr, current_access_index)?;

        Ok(())
    }

    fn should_continue_squash_loop(
        &self,
        vm: &mut VirtualMachine,
        exec_scopes: &mut ExecutionScopes,
        dst: &CellRef,
    ) -> Result<(), HintError> {
        let dict_squash_exec_scope: &mut DictSquashExecScope =
            exec_scopes.get_mut_ref("dict_squash_exec_scope")?;
        let current_access_indices = dict_squash_exec_scope
            .current_access_indices()
            .ok_or(HintError::EmptyCurrentAccessIndices)?;

        let should_continue = Felt252::from((current_access_indices.len() > 1) as u8);

        vm.insert_value(cell_ref_to_relocatable(dst, vm), should_continue)
            .map_err(HintError::from)
    }

    fn field_sqrt(
        &self,
        vm: &mut VirtualMachine,
        val: &ResOperand,
        sqrt: &CellRef,
    ) -> Result<(), HintError> {
        let value = Fq::from(res_operand_get_val(vm, val)?.to_biguint());

        let three_fq = Fq::from(Felt252::new(3).to_biguint());
        let res = if value.legendre().is_qr() {
            value
        } else {
            value * three_fq
        };

        if let Some(root) = res.sqrt() {
            let root0: BigUint = root.into_bigint().into();
            let root1: BigUint = (-root).into_bigint().into();
            let root = Felt252::from(std::cmp::min(root0, root1));
            vm.insert_value(cell_ref_to_relocatable(sqrt, vm), root)
                .map_err(HintError::from)
        } else {
            Err(HintError::CustomHint(
                "Field element is not a square".to_string(),
            ))
        }
    }
}

impl HintProcessor for Cairo1HintProcessor {
    // Ignores all data except for the code that should contain
    fn compile_hint(
        &self,
        //Block of hint code as String
        hint_code: &str,
        //Ap Tracking Data corresponding to the Hint
        _ap_tracking_data: &crate::serde::deserialize_program::ApTracking,
        //Map from variable name to reference id number
        //(may contain other variables aside from those used by the hint)
        _reference_ids: &HashMap<String, usize>,
        //List of all references (key corresponds to element of the previous dictionary)
        _references: &HashMap<usize, HintReference>,
    ) -> Result<Box<dyn Any>, VirtualMachineError> {
        let data = hint_code.parse().ok().and_then(|x: usize| self.hints.get(&x).cloned()).ok_or(VirtualMachineError::CompileHintFail(format!("No hint found for pc {}. Cairo1HintProccesor can only be used when running CasmContractClass", hint_code)))?;
        Ok(any_box!(data))
    }

    // Executes all the hints for a given pc
    fn execute_hint(
        &mut self,
        //Proxy to VM, contains refrences to necessary data
        //+ MemoryProxy, which provides the necessary methods to manipulate memory
        vm: &mut VirtualMachine,
        //Proxy to ExecutionScopes, provides the necessary methods to manipulate the scopes and
        //access current scope variables
        exec_scopes: &mut ExecutionScopes,
        //Data structure that can be downcasted to the structure generated by compile_hint
        hint_data: &Box<dyn Any>,
        //Constant values extracted from the program specification.
        _constants: &HashMap<String, Felt252>,
    ) -> Result<(), HintError> {
        let hints: &Vec<Hint> = hint_data.downcast_ref().ok_or(HintError::WrongHintData)?;
        for hint in hints {
            self.execute(vm, exec_scopes, hint)?;
        }
        Ok(())
    }
}
