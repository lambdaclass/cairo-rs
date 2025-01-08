use super::circuit;
use super::dict_manager::DictManagerExecScope;
use super::hint_processor_utils::*;
use crate::any_box;
use crate::hint_processor::cairo_1_hint_processor::dict_manager::DictSquashExecScope;
use crate::hint_processor::hint_processor_definition::HintReference;
use crate::stdlib::{boxed::Box, collections::HashMap, prelude::*};
use crate::types::relocatable::{MaybeRelocatable, Relocatable};
use crate::vm::runners::cairo_runner::ResourceTracker;
use crate::vm::runners::cairo_runner::RunResources;
use crate::Felt252;
use crate::{
    hint_processor::hint_processor_definition::HintProcessorLogic,
    types::{errors::math_errors::MathError, exec_scope::ExecutionScopes},
    vm::errors::vm_errors::VirtualMachineError,
    vm::{errors::hint_errors::HintError, vm_core::VirtualMachine},
};
use ark_ff::fields::{Fp256, MontBackend, MontConfig};
use ark_ff::{Field, PrimeField};
use ark_std::UniformRand;
use cairo_lang_casm::hints::{CoreHintBase, DeprecatedHint, StarknetHint};
use cairo_lang_casm::{
    hints::{CoreHint, Hint},
    operand::{CellRef, ResOperand},
};
use core::any::Any;
use core::ops::Shl;

use num_bigint::{BigInt, BigUint};
use num_integer::{ExtendedGcd, Integer};
use num_traits::{Signed, ToPrimitive};

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
    Felt252::from_dec_str(
        "3141592653589793238462643383279502884197169399375105820974944592307816406665",
    )
    .unwrap()
}

/// HintProcessor for Cairo 1 compiler hints.
pub struct Cairo1HintProcessor {
    hints: HashMap<usize, Vec<Hint>>,
    run_resources: RunResources,
    /// If set to true, uses a single segment for dictionaries to aid in segment arena validations
    /// WARNING: The program must call the "RelocateAllDictionaries" Cheatcode if the flag is enabled
    segment_arena_validations: bool,
}

impl Cairo1HintProcessor {
    pub fn new(
        hints: &[(usize, Vec<Hint>)],
        run_resources: RunResources,
        segment_arena_validations: bool,
    ) -> Self {
        Self {
            hints: hints.iter().cloned().collect(),
            run_resources,
            segment_arena_validations,
        }
    }
    // Most of the Hints implementations are derived from the `cairo-lang-runner` crate.
    // https://github.com/starkware-libs/cairo/blob/40a7b60687682238f7f71ef7c59c986cc5733915/crates/cairo-lang-runner/src/casm_run/mod.rs#L1681
    /// Runs a single Hint
    pub fn execute(
        &self,
        vm: &mut VirtualMachine,
        exec_scopes: &mut ExecutionScopes,
        hint: &Hint,
    ) -> Result<(), HintError> {
        match hint {
            Hint::Core(CoreHintBase::Core(CoreHint::AllocSegment { dst })) => {
                self.alloc_segment(vm, dst)
            }
            Hint::Core(CoreHintBase::Core(CoreHint::TestLessThan { lhs, rhs, dst })) => {
                self.test_less_than(vm, lhs, rhs, dst)
            }
            Hint::Core(CoreHintBase::Core(CoreHint::TestLessThanOrEqual { lhs, rhs, dst }))
            | Hint::Core(CoreHintBase::Core(CoreHint::TestLessThanOrEqualAddress {
                lhs,
                rhs,
                dst,
            })) => self.test_less_than_or_equal(vm, lhs, rhs, dst),
            Hint::Core(CoreHintBase::Deprecated(DeprecatedHint::Felt252DictRead {
                dict_ptr,
                key,
                value_dst,
            })) => self.dict_read(vm, exec_scopes, dict_ptr, key, value_dst),
            Hint::Core(CoreHintBase::Core(CoreHint::SquareRoot { value, dst })) => {
                self.square_root(vm, value, dst)
            }
            Hint::Core(CoreHintBase::Core(CoreHint::GetSegmentArenaIndex {
                dict_end_ptr,
                dict_index,
            })) => self.get_segment_arena_index(vm, exec_scopes, dict_end_ptr, dict_index),

            Hint::Core(CoreHintBase::Core(CoreHint::DivMod {
                lhs,
                rhs,
                quotient,
                remainder,
            })) => self.div_mod(vm, lhs, rhs, quotient, remainder),
            Hint::Core(CoreHintBase::Core(CoreHint::DebugPrint { start, end })) => {
                self.debug_print(vm, start, end)
            }

            Hint::Core(CoreHintBase::Core(CoreHint::Uint256SquareRoot {
                value_low,
                value_high,
                sqrt0,
                sqrt1,
                remainder_low,
                remainder_high,
                sqrt_mul_2_minus_remainder_ge_u128,
            })) => self.uint256_square_root(
                vm,
                value_low,
                value_high,
                sqrt0,
                sqrt1,
                remainder_low,
                remainder_high,
                sqrt_mul_2_minus_remainder_ge_u128,
            ),

            Hint::Core(CoreHintBase::Core(CoreHint::GetNextDictKey { next_key })) => {
                self.get_next_dict_key(vm, exec_scopes, next_key)
            }

            Hint::Core(CoreHintBase::Core(CoreHint::Uint256DivMod {
                dividend0,
                dividend1,
                divisor0,
                divisor1,
                quotient0,
                quotient1,

                remainder0,
                remainder1,
            })) => self.uint256_div_mod(
                vm, dividend0, dividend1, divisor0, divisor1, quotient0, quotient1, remainder0,
                remainder1,
            ),
            Hint::Core(CoreHintBase::Deprecated(DeprecatedHint::Felt252DictWrite {
                dict_ptr,
                key,
                value,
            })) => self.dict_write(exec_scopes, vm, dict_ptr, key, value),
            Hint::Core(CoreHintBase::Core(CoreHint::AssertLeIsFirstArcExcluded {
                skip_exclude_a_flag,
            })) => self.assert_le_if_first_arc_excluded(vm, skip_exclude_a_flag, exec_scopes),

            Hint::Core(CoreHintBase::Deprecated(DeprecatedHint::AssertAllAccessesUsed {
                n_used_accesses,
            })) => self.assert_all_accesses_used(vm, exec_scopes, n_used_accesses),

            Hint::Core(CoreHintBase::Core(CoreHint::AssertLeIsSecondArcExcluded {
                skip_exclude_b_minus_a,
            })) => self.assert_le_is_second_excluded(vm, skip_exclude_b_minus_a, exec_scopes),

            Hint::Core(CoreHintBase::Core(CoreHint::LinearSplit {
                value,
                scalar,
                max_x,
                x,
                y,
            })) => self.linear_split(vm, value, scalar, max_x, x, y),

            Hint::Core(CoreHintBase::Core(CoreHint::AllocFelt252Dict { segment_arena_ptr })) => {
                self.alloc_felt_252_dict(vm, segment_arena_ptr, exec_scopes)
            }

            Hint::Core(CoreHintBase::Core(CoreHint::AssertLeFindSmallArcs {
                range_check_ptr,
                a,
                b,
            })) => self.assert_le_find_small_arcs(vm, exec_scopes, range_check_ptr, a, b),

            Hint::Core(CoreHintBase::Core(CoreHint::RandomEcPoint { x, y })) => {
                self.random_ec_point(vm, x, y)
            }

            Hint::Core(CoreHintBase::Core(CoreHint::ShouldSkipSquashLoop { should_skip_loop })) => {
                self.should_skip_squash_loop(vm, exec_scopes, should_skip_loop)
            }
            Hint::Core(CoreHintBase::Core(CoreHint::Felt252DictEntryInit { dict_ptr, key })) => {
                self.dict_entry_init(vm, exec_scopes, dict_ptr, key)
            }
            Hint::Core(CoreHintBase::Core(CoreHint::Felt252DictEntryUpdate {
                dict_ptr,
                value,
            })) => self.felt_252_dict_entry_update(vm, exec_scopes, dict_ptr, value),
            Hint::Core(CoreHintBase::Core(CoreHint::GetCurrentAccessDelta {
                index_delta_minus1,
            })) => self.get_current_access_delta(vm, exec_scopes, index_delta_minus1),
            Hint::Core(CoreHintBase::Core(CoreHint::InitSquashData {
                dict_accesses,
                n_accesses,
                big_keys,
                first_key,
                ..
            })) => self.init_squash_data(
                vm,
                exec_scopes,
                dict_accesses,
                n_accesses,
                big_keys,
                first_key,
            ),
            Hint::Core(CoreHintBase::Core(CoreHint::AllocConstantSize { size, dst })) => {
                self.alloc_constant_size(vm, exec_scopes, size, dst)
            }
            Hint::Core(CoreHintBase::Core(CoreHint::GetCurrentAccessIndex { range_check_ptr })) => {
                self.get_current_access_index(vm, exec_scopes, range_check_ptr)
            }
            Hint::Core(CoreHintBase::Core(CoreHint::ShouldContinueSquashLoop {
                should_continue,
            })) => self.should_continue_squash_loop(vm, exec_scopes, should_continue),
            Hint::Core(CoreHintBase::Core(CoreHint::FieldSqrt { val, sqrt })) => {
                self.field_sqrt(vm, val, sqrt)
            }
            Hint::Core(CoreHintBase::Core(CoreHint::WideMul128 {
                lhs,
                rhs,
                high,
                low,
            })) => self.wide_mul_128(vm, lhs, rhs, high, low),

            Hint::Core(CoreHintBase::Core(CoreHint::Uint512DivModByUint256 {
                dividend0,
                dividend1,
                dividend2,
                dividend3,
                divisor0,
                divisor1,
                quotient0,
                quotient1,
                quotient2,
                quotient3,
                remainder0,
                remainder1,
            })) => self.uint512_div_mod(
                vm, dividend0, dividend1, dividend2, dividend3, divisor0, divisor1, quotient0,
                quotient1, quotient2, quotient3, remainder0, remainder1,
            ),
            Hint::Core(CoreHintBase::Core(CoreHint::U256InvModN {
                b0,
                b1,
                n0,
                n1,
                g0_or_no_inv,
                g1_option,
                s_or_r0,
                s_or_r1,
                t_or_k0,
                t_or_k1,
            })) => self.u256_inv_mod_n(
                vm,
                b0,
                b1,
                n0,
                n1,
                g0_or_no_inv,
                g1_option,
                s_or_r0,
                s_or_r1,
                t_or_k0,
                t_or_k1,
            ),
            Hint::Core(CoreHintBase::Core(CoreHint::EvalCircuit {
                n_add_mods,
                add_mod_builtin,
                n_mul_mods,
                mul_mod_builtin,
            })) => self.eval_circuit(vm, n_add_mods, add_mod_builtin, n_mul_mods, mul_mod_builtin),
            Hint::Starknet(StarknetHint::Cheatcode { selector, .. }) => {
                let selector = &selector.value.to_bytes_be().1;
                let selector = crate::stdlib::str::from_utf8(selector).map_err(|_| {
                    HintError::CustomHint(Box::from("failed to parse selector".to_string()))
                })?;
                match selector {
                    "RelocateAllDictionaries" => {
                        let dict_manager_exec_scope = exec_scopes
                            .get_mut_ref::<DictManagerExecScope>("dict_manager_exec_scope")?;
                        dict_manager_exec_scope.relocate_all_dictionaries(vm)
                    }
                    _ => Err(HintError::UnknownHint(selector.into())),
                }
            }

            hint => Err(HintError::UnknownHint(
                format!("{:?}", hint).into_boxed_str(),
            )),
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
        let value = res_operand_get_val(vm, value)?.to_biguint();
        let result = value.sqrt();
        vm.insert_value(cell_ref_to_relocatable(dst, vm)?, Felt252::from(&result))
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
        let mut lengths_and_indices = [
            (a_val, 0),
            (b_val - a_val, 1),
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
            Felt252::from(&(lengths_and_indices[0].0.to_biguint() % prime_over_3_high)),
        )?;
        vm.insert_value(
            (range_check_ptr + 1)?,
            Felt252::from(&(lengths_and_indices[0].0.to_biguint() / prime_over_3_high)),
        )?;
        vm.insert_value(
            (range_check_ptr + 2)?,
            Felt252::from(&(lengths_and_indices[1].0.to_biguint() % prime_over_2_high)),
        )?;
        vm.insert_value(
            (range_check_ptr + 3)?,
            Felt252::from(&(lengths_and_indices[1].0.to_biguint() / prime_over_2_high)),
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
        let quotient_value = Felt252::from(&(&lhs_value / &rhs_value));
        let remainder_value = Felt252::from(&(lhs_value % rhs_value));
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
            .map_err(HintError::from)?;
        // The hint is only for dictionary finalization, so can be called.
        dict_manager_exec_scope.finalize_segment(dict_address)
    }

    #[allow(clippy::too_many_arguments)]
    fn uint256_div_mod(
        &self,
        vm: &mut VirtualMachine,
        dividend0: &ResOperand,
        dividend1: &ResOperand,
        divisor0: &ResOperand,
        divisor1: &ResOperand,
        quotient0: &CellRef,
        quotient1: &CellRef,
        remainder0: &CellRef,
        remainder1: &CellRef,
    ) -> Result<(), HintError> {
        let pow_2_128 = BigUint::from(u128::MAX) + 1u32;
        let dividend0 = get_val(vm, dividend0)?.to_biguint();
        let dividend1 = get_val(vm, dividend1)?.to_biguint();
        let divisor0 = get_val(vm, divisor0)?.to_biguint();
        let divisor1 = get_val(vm, divisor1)?.to_biguint();
        let dividend: BigUint = dividend0 + dividend1.shl(128);
        let divisor = divisor0 + divisor1.shl(128);
        let (quotient, remainder) = dividend.div_rem(&divisor);
        let (limb1, limb0) = quotient.div_rem(&pow_2_128);
        vm.insert_value(
            cell_ref_to_relocatable(quotient0, vm)?,
            Felt252::from(&limb0),
        )?;
        vm.insert_value(
            cell_ref_to_relocatable(quotient1, vm)?,
            Felt252::from(&limb1),
        )?;
        let (limb1, limb0) = remainder.div_rem(&pow_2_128);
        vm.insert_value(
            cell_ref_to_relocatable(remainder0, vm)?,
            Felt252::from(&limb0),
        )?;
        vm.insert_value(
            cell_ref_to_relocatable(remainder1, vm)?,
            Felt252::from(&limb1),
        )?;

        Ok(())
    }

    fn assert_le_if_first_arc_excluded(
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
        let value = res_operand_get_val(vm, value)?.to_biguint();
        let scalar = res_operand_get_val(vm, scalar)?.to_biguint();
        let max_x = res_operand_get_val(vm, max_x)?.to_biguint();
        let x_value = (&value / &scalar).min(max_x);
        let y_value = value - &x_value * &scalar;

        vm.insert_value(cell_ref_to_relocatable(x, vm)?, Felt252::from(&x_value))
            .map_err(HintError::from)?;
        vm.insert_value(cell_ref_to_relocatable(y, vm)?, Felt252::from(&y_value))
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
            .ok_or_else(|| {
                HintError::CustomHint("Failed to compute sqrt".to_string().into_boxed_str())
            })?
            .into_bigint()
            .into();

        vm.insert_value(cell_ref_to_relocatable(x, vm)?, Felt252::from(&x_bigint))?;
        vm.insert_value(cell_ref_to_relocatable(y, vm)?, Felt252::from(&y_bigint))?;

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

    fn alloc_felt_252_dict(
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
            .ok_or_else(|| {
                HintError::CustomHint(
                    "Invalid number of dictionaries."
                        .to_string()
                        .into_boxed_str(),
                )
            })?;

        let dict_infos_base = vm.get_relocatable((dict_manager_address - 3)?)?;

        let dict_manager_exec_scope =
            match exec_scopes.get_mut_ref::<DictManagerExecScope>("dict_manager_exec_scope") {
                Ok(dict_manager_exec_scope) => dict_manager_exec_scope,
                Err(_) => {
                    exec_scopes.assign_or_update_variable(
                        "dict_manager_exec_scope",
                        Box::new(DictManagerExecScope::new(self.segment_arena_validations)),
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
        let value = if let ResOperand::Deref(cell) = value {
            get_mayberelocatable(vm, cell)?
        } else {
            MaybeRelocatable::Int(res_operand_get_val(vm, value)?)
        };
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
    fn uint512_div_mod(
        &self,
        vm: &mut VirtualMachine,
        dividend0: &ResOperand,
        dividend1: &ResOperand,
        dividend2: &ResOperand,
        dividend3: &ResOperand,
        divisor0: &ResOperand,
        divisor1: &ResOperand,
        quotient0: &CellRef,
        quotient1: &CellRef,
        quotient2: &CellRef,
        quotient3: &CellRef,
        remainder0: &CellRef,
        remainder1: &CellRef,
    ) -> Result<(), HintError> {
        let pow_2_128 = BigUint::from(u128::MAX) + 1u32;
        let dividend0 = res_operand_get_val(vm, dividend0)?.to_biguint();
        let dividend1 = res_operand_get_val(vm, dividend1)?.to_biguint();
        let dividend2 = res_operand_get_val(vm, dividend2)?.to_biguint();
        let dividend3 = res_operand_get_val(vm, dividend3)?.to_biguint();
        let divisor0 = res_operand_get_val(vm, divisor0)?.to_biguint();
        let divisor1 = res_operand_get_val(vm, divisor1)?.to_biguint();
        let dividend: BigUint =
            dividend0 + dividend1.shl(128) + dividend2.shl(256) + dividend3.shl(384);
        let divisor = divisor0 + divisor1.shl(128);
        let (quotient, remainder) = dividend.div_rem(&divisor);
        let (quotient, limb0) = quotient.div_rem(&pow_2_128);

        vm.insert_value(
            cell_ref_to_relocatable(quotient0, vm)?,
            Felt252::from(&limb0),
        )?;

        let (quotient, limb1) = quotient.div_rem(&pow_2_128);
        vm.insert_value(
            cell_ref_to_relocatable(quotient1, vm)?,
            Felt252::from(&limb1),
        )?;
        let (limb3, limb2) = quotient.div_rem(&pow_2_128);

        vm.insert_value(
            cell_ref_to_relocatable(quotient2, vm)?,
            Felt252::from(&limb2),
        )?;
        vm.insert_value(
            cell_ref_to_relocatable(quotient3, vm)?,
            Felt252::from(&limb3),
        )?;
        let (limb1, limb0) = remainder.div_rem(&pow_2_128);

        vm.insert_value(
            cell_ref_to_relocatable(remainder0, vm)?,
            Felt252::from(&limb0),
        )?;
        vm.insert_value(
            cell_ref_to_relocatable(remainder1, vm)?,
            Felt252::from(&limb1),
        )?;

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
        let pow_2_128 = BigUint::from(u128::MAX) + 1u32;
        let pow_2_64 = BigUint::from(u64::MAX) + 1u32;
        let value_low = res_operand_get_val(vm, value_low)?.to_biguint();
        let value_high = res_operand_get_val(vm, value_high)?.to_biguint();
        let value = value_low + value_high * pow_2_128.clone();
        let sqrt = value.sqrt();
        let remainder = value - sqrt.clone() * sqrt.clone();
        let sqrt_mul_2_minus_remainder_ge_u128_val =
            sqrt.clone() * 2u32 - remainder.clone() >= pow_2_128;

        // Guess sqrt limbs.
        let (sqrt1_val, sqrt0_val) = sqrt.div_rem(&pow_2_64);
        vm.insert_value(
            cell_ref_to_relocatable(sqrt0, vm)?,
            Felt252::from(&sqrt0_val),
        )?;
        vm.insert_value(
            cell_ref_to_relocatable(sqrt1, vm)?,
            Felt252::from(&sqrt1_val),
        )?;

        let (remainder_high_val, remainder_low_val) = remainder.div_rem(&pow_2_128);

        vm.insert_value(
            cell_ref_to_relocatable(remainder_low, vm)?,
            Felt252::from(&remainder_low_val),
        )?;
        vm.insert_value(
            cell_ref_to_relocatable(remainder_high, vm)?,
            Felt252::from(&remainder_high_val),
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

    #[allow(unused_variables)]
    fn debug_print(
        &self,
        vm: &mut VirtualMachine,
        start: &ResOperand,
        end: &ResOperand,
    ) -> Result<(), HintError> {
        #[cfg(feature = "std")]
        {
            let mut curr = as_relocatable(vm, start)?;
            let end = as_relocatable(vm, end)?;
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
            .ok_or_else(|| HintError::NoKeyInAccessIndices(Box::new(key)))?;

        if n != Felt252::from(access_indices_at_key.len()) {
            return Err(HintError::NumUsedAccessesAssertFail(Box::new((
                n,
                access_indices_at_key.len(),
                key,
            ))));
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
            if dict_squash_exec_scope
                .current_access_indices()
                .ok_or_else(|| {
                    HintError::CustomHint("no indices accessed".to_string().into_boxed_str())
                })?
                .len()
                > 1
            {
                0
            } else {
                1
            },
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
        let value: MaybeRelocatable = if let ResOperand::Deref(cell) = value {
            get_mayberelocatable(vm, cell)?
        } else {
            MaybeRelocatable::Int(res_operand_get_val(vm, value)?)
        };
        let dict_manager_exec_scope = exec_scopes
            .get_mut_ref::<DictManagerExecScope>("dict_manager_exec_scope")
            .map_err(|_| {
                HintError::CustomHint(
                    "Trying to write to a dict while dict manager was not initialized."
                        .to_string()
                        .into_boxed_str(),
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
            .ok_or_else(|| {
                HintError::CustomHint("no accessed index".to_string().into_boxed_str())
            })?;
        let index_delta_minus_1_val =
            *dict_squash_exec_scope
                .current_access_index()
                .ok_or_else(|| {
                    HintError::CustomHint("no index accessed".to_string().into_boxed_str())
                })?
                - prev_access_index
                - 1_u64;

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
        first_key: &CellRef,
    ) -> Result<(), HintError> {
        let dict_access_size = 3;
        let rangecheck_bound = Felt252::from(u128::MAX) + 1u64;

        exec_scopes.assign_or_update_variable(
            "dict_squash_exec_scope",
            Box::<DictSquashExecScope>::default(),
        );
        let dict_squash_exec_scope =
            exec_scopes.get_mut_ref::<DictSquashExecScope>("dict_squash_exec_scope")?;
        let (dict_accesses_base, dict_accesses_offset) = extract_buffer(dict_accesses)?;
        let dict_accesses_address = get_ptr(vm, dict_accesses_base, &dict_accesses_offset)?;
        let n_accesses = res_operand_get_val(vm, n_accesses)?
            .to_usize()
            .ok_or_else(|| {
                HintError::CustomHint(
                    "Number of accesses is too large or negative."
                        .to_string()
                        .into_boxed_str(),
                )
            })?;

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

        let val = if dict_squash_exec_scope.keys[0] < rangecheck_bound {
            Felt252::from(0)
        } else {
            Felt252::from(1)
        };

        vm.insert_value(cell_ref_to_relocatable(big_keys, vm)?, val)?;

        vm.insert_value(
            cell_ref_to_relocatable(first_key, vm)?,
            dict_squash_exec_scope.current_key().ok_or_else(|| {
                HintError::CustomHint("No current key".to_string().into_boxed_str())
            })?,
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
                .ok_or_else(|| {
                    HintError::CustomHint("No current accessed index".to_string().into_boxed_str())
                })?;
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

        vm.insert_value(cell_ref_to_relocatable(dst, vm)?, should_continue)
            .map_err(HintError::from)
    }

    fn field_sqrt(
        &self,
        vm: &mut VirtualMachine,
        val: &ResOperand,
        sqrt: &CellRef,
    ) -> Result<(), HintError> {
        let value = Fq::from(res_operand_get_val(vm, val)?.to_biguint());

        let three_fq = Fq::from(BigUint::from(3_u32));
        let res = if value.legendre().is_qr() {
            value
        } else {
            value * three_fq
        };

        if let Some(root) = res.sqrt() {
            let root0: BigUint = root.into_bigint().into();
            let root1: BigUint = (-root).into_bigint().into();
            let root = Felt252::from(&core::cmp::min(root0, root1));
            vm.insert_value(cell_ref_to_relocatable(sqrt, vm)?, root)
                .map_err(HintError::from)
        } else {
            Err(HintError::CustomHint(
                "Field element is not a square".to_string().into_boxed_str(),
            ))
        }
    }

    fn wide_mul_128(
        &self,
        vm: &mut VirtualMachine,
        lhs: &ResOperand,
        rhs: &ResOperand,
        high: &CellRef,
        low: &CellRef,
    ) -> Result<(), HintError> {
        let mask128 = BigUint::from(u128::MAX);
        let lhs_val = res_operand_get_val(vm, lhs)?.to_biguint();
        let rhs_val = res_operand_get_val(vm, rhs)?.to_biguint();
        let prod = lhs_val * rhs_val;
        vm.insert_value(
            cell_ref_to_relocatable(high, vm)?,
            Felt252::from(&(prod.clone() >> 128)),
        )?;
        vm.insert_value(
            cell_ref_to_relocatable(low, vm)?,
            Felt252::from(&(prod & mask128)),
        )
        .map_err(HintError::from)
    }

    #[allow(clippy::too_many_arguments)]
    fn u256_inv_mod_n(
        &self,
        vm: &mut VirtualMachine,
        b0: &ResOperand,
        b1: &ResOperand,
        n0: &ResOperand,
        n1: &ResOperand,
        g0_or_no_inv: &CellRef,
        g1_option: &CellRef,
        s_or_r0: &CellRef,
        s_or_r1: &CellRef,
        t_or_k0: &CellRef,
        t_or_k1: &CellRef,
    ) -> Result<(), HintError> {
        let pow_2_128 = BigInt::from(u128::MAX) + 1u32;
        let b0 = get_val(vm, b0)?.to_bigint();
        let b1 = get_val(vm, b1)?.to_bigint();
        let n0 = get_val(vm, n0)?.to_bigint();
        let n1 = get_val(vm, n1)?.to_bigint();
        let b: BigInt = b0.clone() + b1.clone().shl(128);
        let n: BigInt = n0 + n1.shl(128);
        let ExtendedGcd {
            gcd: mut g,
            x: _,
            y: mut r,
        } = n.extended_gcd(&b);
        if n == 1.into() {
            vm.insert_value(cell_ref_to_relocatable(s_or_r0, vm)?, Felt252::from(b0))?;
            vm.insert_value(cell_ref_to_relocatable(s_or_r1, vm)?, Felt252::from(b1))?;
            vm.insert_value(cell_ref_to_relocatable(t_or_k0, vm)?, Felt252::from(1))?;
            vm.insert_value(cell_ref_to_relocatable(t_or_k1, vm)?, Felt252::from(0))?;
            vm.insert_value(cell_ref_to_relocatable(g0_or_no_inv, vm)?, Felt252::from(1))?;
            vm.insert_value(cell_ref_to_relocatable(g1_option, vm)?, Felt252::from(0))?;
        } else if g != 1.into() {
            // This makes sure `g0_or_no_inv` is always non-zero in the no inverse case.
            if g.is_even() {
                g = 2u32.into();
            }
            let (limb1, limb0) = (&b / &g).div_rem(&pow_2_128);
            vm.insert_value(cell_ref_to_relocatable(s_or_r0, vm)?, Felt252::from(limb0))?;
            vm.insert_value(cell_ref_to_relocatable(s_or_r1, vm)?, Felt252::from(limb1))?;
            let (limb1, limb0) = (&n / &g).div_rem(&pow_2_128);
            vm.insert_value(cell_ref_to_relocatable(t_or_k0, vm)?, Felt252::from(limb0))?;
            vm.insert_value(cell_ref_to_relocatable(t_or_k1, vm)?, Felt252::from(limb1))?;
            let (limb1, limb0) = g.div_rem(&pow_2_128);
            vm.insert_value(
                cell_ref_to_relocatable(g0_or_no_inv, vm)?,
                Felt252::from(limb0),
            )?;
            vm.insert_value(
                cell_ref_to_relocatable(g1_option, vm)?,
                Felt252::from(limb1),
            )?;
        } else {
            r %= &n;
            if r.is_negative() {
                r += &n;
            }
            let k: BigInt = (&r * b - 1) / n;
            let (limb1, limb0) = r.div_rem(&pow_2_128);
            vm.insert_value(cell_ref_to_relocatable(s_or_r0, vm)?, Felt252::from(limb0))?;
            vm.insert_value(cell_ref_to_relocatable(s_or_r1, vm)?, Felt252::from(limb1))?;
            let (limb1, limb0) = k.div_rem(&pow_2_128);
            vm.insert_value(cell_ref_to_relocatable(t_or_k0, vm)?, Felt252::from(limb0))?;
            vm.insert_value(cell_ref_to_relocatable(t_or_k1, vm)?, Felt252::from(limb1))?;
            vm.insert_value(cell_ref_to_relocatable(g0_or_no_inv, vm)?, Felt252::from(0))?;
        }
        Ok(())
    }
    fn eval_circuit(
        &self,
        vm: &mut VirtualMachine,
        n_add_mods: &ResOperand,
        add_mod_builtin_ptr: &ResOperand,
        n_mul_mods: &ResOperand,
        mul_mod_builtin_ptr: &ResOperand,
    ) -> Result<(), HintError> {
        let n_add_mods = get_val(vm, n_add_mods)?;
        let n_add_mods =
            n_add_mods
                .to_usize()
                .ok_or(HintError::Math(MathError::Felt252ToUsizeConversion(
                    Box::from(n_add_mods),
                )))?;
        let n_mul_mods = get_val(vm, n_mul_mods)?;
        let n_mul_mods =
            n_mul_mods
                .to_usize()
                .ok_or(HintError::Math(MathError::Felt252ToUsizeConversion(
                    Box::from(n_mul_mods),
                )))?;

        let (add_mod_builtin_base, add_mod_builtin_offset) = extract_buffer(add_mod_builtin_ptr)?;
        let (mul_mod_builtin_base, mul_mod_builtin_offset) = extract_buffer(mul_mod_builtin_ptr)?;

        let add_mod_builtin_address = get_ptr(vm, add_mod_builtin_base, &add_mod_builtin_offset)?;
        let mul_mod_builtin_address = get_ptr(vm, mul_mod_builtin_base, &mul_mod_builtin_offset)?;

        circuit::eval_circuit(
            vm,
            n_add_mods,
            add_mod_builtin_address,
            n_mul_mods,
            mul_mod_builtin_address,
        )?;

        Ok(())
    }
}

impl HintProcessorLogic for Cairo1HintProcessor {
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
        _references: &[HintReference],
    ) -> Result<Box<dyn Any>, VirtualMachineError> {
        let data = hint_code.parse().ok().and_then(|x: usize| self.hints.get(&x).cloned())
        .ok_or_else(|| VirtualMachineError::CompileHintFail(
            format!("No hint found for pc {hint_code}. Cairo1HintProccesor can only be used when running CasmContractClass").into_boxed_str()
    ))?;
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

impl ResourceTracker for Cairo1HintProcessor {
    fn consumed(&self) -> bool {
        self.run_resources.consumed()
    }

    fn consume_step(&mut self) {
        self.run_resources.consume_step()
    }

    fn get_n_steps(&self) -> Option<usize> {
        self.run_resources.get_n_steps()
    }

    fn run_resources(&self) -> &RunResources {
        &self.run_resources
    }
}
