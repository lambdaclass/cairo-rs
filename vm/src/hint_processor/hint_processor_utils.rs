use crate::stdlib::boxed::Box;

use crate::{
    serde::deserialize_program::{ApTracking, OffsetValue},
    types::{
        errors::math_errors::MathError,
        instruction::Register,
        relocatable::{MaybeRelocatable, Relocatable},
    },
    vm::{errors::hint_errors::HintError, vm_core::VirtualMachine},
};

use super::hint_processor_definition::HintReference;
use crate::Felt252;

use num_traits::ToPrimitive;

/// Inserts value into the address of the given ids variable
pub fn insert_value_from_reference(
    value: impl Into<MaybeRelocatable>,
    vm: &mut VirtualMachine,
    hint_reference: &HintReference,
    ap_tracking: &ApTracking,
) -> Result<(), HintError> {
    let addr = compute_addr_from_reference(hint_reference, vm, ap_tracking)
        .ok_or(HintError::UnknownIdentifierInternal)?;
    vm.insert_value(addr, value).map_err(HintError::Memory)
}

///Returns the Integer value stored in the given ids variable
/// Returns an internal error, users should map it into a more informative type
pub fn get_integer_from_reference(
    vm: &VirtualMachine,
    hint_reference: &HintReference,
    ap_tracking: &ApTracking,
) -> Result<Felt252, HintError> {
    get_maybe_relocatable_from_reference(vm, hint_reference, ap_tracking)
        .ok_or(HintError::UnknownIdentifierInternal)?
        .get_int()
        .ok_or(HintError::WrongIdentifierTypeInternal)
}

///Returns the Relocatable value stored in the given ids variable
pub fn get_ptr_from_reference(
    vm: &VirtualMachine,
    hint_reference: &HintReference,
    ap_tracking: &ApTracking,
) -> Result<Relocatable, HintError> {
    get_maybe_relocatable_from_reference(vm, hint_reference, ap_tracking)
        .ok_or(HintError::UnknownIdentifierInternal)?
        .get_relocatable()
        .ok_or(HintError::WrongIdentifierTypeInternal)
}

///Returns the value given by a reference as [MaybeRelocatable]
pub fn get_maybe_relocatable_from_reference(
    vm: &VirtualMachine,
    hint_reference: &HintReference,
    ap_tracking: &ApTracking,
) -> Option<MaybeRelocatable> {
    let offset1 = get_offset_value(
        vm,
        &hint_reference.offset1,
        &hint_reference.ap_tracking_data,
        ap_tracking,
    )?;
    let offset2 = get_offset_value(
        vm,
        &hint_reference.offset2,
        &hint_reference.ap_tracking_data,
        ap_tracking,
    )?;
    let mut val = match hint_reference.offset2 {
        OffsetValue::Reference(_, _, _, true)
        | OffsetValue::Immediate(_)
        | OffsetValue::Value(_) => offset1.add(&offset2).ok()?,
        OffsetValue::Reference(_, _, _, false) => offset1.sub(&offset2).ok()?,
    };
    if hint_reference.inner_dereference && hint_reference.outer_dereference {
        val = vm.get_maybe(&val)?;
    }
    if hint_reference.inner_dereference || hint_reference.outer_dereference {
        val = vm.get_maybe(&val)?;
    }
    Some(val)
}

/// Computes the memory address of the ids variable indicated by the HintReference as a [Relocatable]
pub fn compute_addr_from_reference(
    hint_reference: &HintReference,
    vm: &VirtualMachine,
    ap_tracking: &ApTracking,
) -> Option<Relocatable> {
    let offset1 = get_offset_value(
        vm,
        &hint_reference.offset1,
        &hint_reference.ap_tracking_data,
        ap_tracking,
    )?;
    let offset2 = get_offset_value(
        vm,
        &hint_reference.offset2,
        &hint_reference.ap_tracking_data,
        ap_tracking,
    )?;
    let mut val = offset1.add(&offset2).ok()?;
    if hint_reference.inner_dereference {
        val = vm.get_maybe(&val)?;
    };
    val.get_relocatable()
}

fn apply_ap_tracking_correction(
    ap: Relocatable,
    ref_ap_tracking: &ApTracking,
    hint_ap_tracking: &ApTracking,
) -> Option<Relocatable> {
    // check that both groups are the same
    if ref_ap_tracking.group != hint_ap_tracking.group {
        return None;
    }
    let ap_diff = hint_ap_tracking.offset - ref_ap_tracking.offset;
    (ap - ap_diff).ok()
}

//Tries to convert a Felt252 value to usize
pub fn felt_to_usize(felt: &Felt252) -> Result<usize, MathError> {
    felt.to_usize()
        .ok_or_else(|| MathError::Felt252ToUsizeConversion(Box::new(*felt)))
}

///Tries to convert a Felt252 value to u32
pub fn felt_to_u32(felt: &Felt252) -> Result<u32, MathError> {
    felt.to_u32()
        .ok_or_else(|| MathError::Felt252ToU32Conversion(Box::new(*felt)))
}

fn get_offset_value(
    vm: &VirtualMachine,
    offset_value: &OffsetValue,
    reference_ap_tracking: &Option<ApTracking>,
    hint_ap_tracking: &ApTracking,
) -> Option<MaybeRelocatable> {
    match offset_value {
        OffsetValue::Immediate(f) => Some(f.into()),
        OffsetValue::Value(v) => Some(Felt252::from(*v).into()),
        OffsetValue::Reference(register, offset, deref, _) => {
            let addr = (if matches!(register, Register::FP) {
                vm.get_fp()
            } else {
                apply_ap_tracking_correction(
                    vm.get_ap(),
                    reference_ap_tracking.as_ref()?,
                    hint_ap_tracking,
                )?
            } + *offset)
                .ok()?;

            if *deref {
                vm.get_maybe(&addr)
            } else {
                Some(addr.into())
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use crate::{relocatable, utils::test_utils::*, vm::vm_memory::memory::Memory};
    use assert_matches::assert_matches;

    #[cfg(target_arch = "wasm32")]
    use wasm_bindgen_test::*;

    #[test]
    #[cfg_attr(target_arch = "wasm32", wasm_bindgen_test)]
    fn get_integer_from_reference_with_immediate_value() {
        // Reference: cast(2, felt)
        let mut vm = vm!();
        vm.segments = segments![((1, 0), 0)];
        let mut hint_ref = HintReference::new(0, 0, false, false, true);
        hint_ref.offset1 = OffsetValue::Immediate(Felt252::from(2));

        assert_eq!(
            get_integer_from_reference(&vm, &hint_ref, &ApTracking::new())
                .expect("Unexpected get integer fail"),
            Felt252::from(2)
        );
    }

    #[test]
    #[cfg_attr(target_arch = "wasm32", wasm_bindgen_test)]
    fn get_offset_value_reference_valid() {
        let mut vm = vm!();
        vm.segments = segments![((1, 0), 0)];
        let mut hint_ref = HintReference::new(0, 0, false, true, true);
        hint_ref.offset1 = OffsetValue::Reference(Register::FP, 2_i32, false, true);

        assert_matches!(
            get_offset_value(&vm, &hint_ref.offset1, &hint_ref.ap_tracking_data, &ApTracking::new()),
            Some(x) if x == mayberelocatable!(1, 2)
        );
    }

    #[test]
    #[cfg_attr(target_arch = "wasm32", wasm_bindgen_test)]
    fn get_offset_value_invalid() {
        let mut vm = vm!();
        vm.segments = segments![((1, 0), 0)];
        let mut hint_ref = HintReference::new(0, 0, false, true, true);
        hint_ref.offset1 = OffsetValue::Reference(Register::FP, -2_i32, false, true);

        assert_matches!(
            get_offset_value(
                &vm,
                &hint_ref.offset1,
                &hint_ref.ap_tracking_data,
                &ApTracking::new()
            ),
            None
        );
    }

    #[test]
    #[cfg_attr(target_arch = "wasm32", wasm_bindgen_test)]
    fn get_ptr_from_reference_short_path() {
        let mut vm = vm!();
        vm.segments = segments![((1, 0), (2, 0))];

        assert_matches!(
            get_ptr_from_reference(
                &vm,
                &HintReference::new(0, 0, false, false, true),
                &ApTracking::new()
            ),
            Ok(x) if x == relocatable!(1, 0)
        );
    }

    #[test]
    #[cfg_attr(target_arch = "wasm32", wasm_bindgen_test)]
    fn get_ptr_from_reference_with_dereference() {
        let mut vm = vm!();
        vm.segments = segments![((1, 0), (3, 0))];

        assert_matches!(
            get_ptr_from_reference(
                &vm,
                &HintReference::new(0, 0, false, true, true),
                &ApTracking::new()
            ),
            Ok(x) if x == relocatable!(3, 0)
        );
    }

    #[test]
    #[cfg_attr(target_arch = "wasm32", wasm_bindgen_test)]
    fn get_ptr_from_reference_with_dereference_and_imm() {
        let mut vm = vm!();
        vm.segments = segments![((1, 0), (4, 0))];
        let mut hint_ref = HintReference::new(0, 0, true, false, true);
        hint_ref.offset2 = OffsetValue::Value(2);

        assert_matches!(
            get_ptr_from_reference(&vm, &hint_ref, &ApTracking::new()),
            Ok(x) if x == relocatable!(4, 2)
        );
    }

    #[test]
    #[cfg_attr(target_arch = "wasm32", wasm_bindgen_test)]
    fn compute_addr_from_reference_no_regiter_in_reference() {
        let mut vm = vm!();
        vm.segments = segments![((1, 0), (4, 0))];
        let mut hint_reference = HintReference::new(0, 0, false, false, true);
        hint_reference.offset1 = OffsetValue::Immediate(Felt252::from(2_i32));

        assert!(compute_addr_from_reference(&hint_reference, &vm, &ApTracking::new()).is_none());
    }

    #[test]
    #[cfg_attr(target_arch = "wasm32", wasm_bindgen_test)]
    fn compute_addr_from_reference_failed_to_get_ids() {
        let mut vm = vm!();
        vm.segments = segments![((1, 0), 4)];
        // vm.run_context.fp = -1;
        let mut hint_reference = HintReference::new(0, 0, false, false, true);
        hint_reference.offset1 = OffsetValue::Reference(Register::FP, -1, true, true);

        assert_matches!(
            compute_addr_from_reference(&hint_reference, &vm, &ApTracking::new()),
            None
        );
    }

    #[test]
    #[cfg_attr(target_arch = "wasm32", wasm_bindgen_test)]
    fn tracking_correction_valid() {
        let mut ref_ap_tracking = ApTracking::new();
        ref_ap_tracking.group = 1;
        let mut hint_ap_tracking = ApTracking::new();
        hint_ap_tracking.group = 1;

        assert_matches!(
            apply_ap_tracking_correction(relocatable!(1, 0), &ref_ap_tracking, &hint_ap_tracking),
            Some(relocatable!(1, 0))
        );
    }

    #[test]
    #[cfg_attr(target_arch = "wasm32", wasm_bindgen_test)]
    fn tracking_correction_invalid_group() {
        let mut ref_ap_tracking = ApTracking::new();
        ref_ap_tracking.group = 1;
        let mut hint_ap_tracking = ApTracking::new();
        hint_ap_tracking.group = 2;

        assert!(apply_ap_tracking_correction(
            relocatable!(1, 0),
            &ref_ap_tracking,
            &hint_ap_tracking
        )
        .is_none());
    }

    #[test]
    #[cfg_attr(target_arch = "wasm32", wasm_bindgen_test)]
    fn get_maybe_relocatable_from_reference_valid() {
        let mut vm = vm!();
        vm.segments = segments![((1, 0), (0, 0))];
        let hint_ref = HintReference::new_simple(0);
        assert_matches!(
            get_maybe_relocatable_from_reference(&vm, &hint_ref, &ApTracking::new()),
            Some(x) if x == mayberelocatable!(0, 0)
        );
    }

    #[test]
    #[cfg_attr(target_arch = "wasm32", wasm_bindgen_test)]
    fn get_maybe_relocatable_from_reference_invalid() {
        let mut vm = vm!();
        vm.segments.memory = Memory::new();
        let hint_ref = HintReference::new_simple(0);
        assert_matches!(
            get_maybe_relocatable_from_reference(&vm, &hint_ref, &ApTracking::new()),
            None
        );
    }

    #[test]
    #[cfg_attr(target_arch = "wasm32", wasm_bindgen_test)]
    fn get_integer_from_reference_with_triple_deref() {
        // Reference: [cast([[fp + 2)] + 2], felt*)]
        let mut vm = vm!();
        vm.segments = segments![
            ((1, 2), (0, 0)), // [fp + 2] -> [(1, 0) + 2] -> [(1, 2)] -> (0, 0)
            ((0, 2), (0, 5)), // [[fp + 2] + 2] -> [(0, 0) + 2] -> [(0, 2)] -> (0, 5)
            ((0, 5), 3)       // [[[fp + 2] + 2]] -> [(0, 5)] -> 3
        ];
        let hint_ref = HintReference {
            offset1: OffsetValue::Reference(Register::FP, 2, true, true),
            offset2: OffsetValue::Value(2),
            outer_dereference: true,
            inner_dereference: true,
            ap_tracking_data: Default::default(),
            cairo_type: None,
        };

        assert_eq!(
            get_integer_from_reference(&vm, &hint_ref, &ApTracking::new())
                .expect("Unexpected get integer fail"),
            Felt252::THREE
        );
    }

    #[test]
    #[cfg_attr(target_arch = "wasm32", wasm_bindgen_test)]
    fn get_integer_from_reference_without_outer_defer() {
        // Reference: cast([fp + 4] + (-5), felt)
        let mut vm = vm!();
        vm.segments = segments![
            ((1, 4), 8), // [fp + 4]
        ];
        // [fp + 4] + (-5) = 8 - 5 = 3
        let hint_ref = HintReference {
            offset1: OffsetValue::Reference(Register::FP, 4, true, true),
            offset2: OffsetValue::Immediate(Felt252::from(-5)),
            outer_dereference: false,
            inner_dereference: false,
            ap_tracking_data: Default::default(),
            cairo_type: None,
        };

        assert_eq!(
            get_integer_from_reference(&vm, &hint_ref, &ApTracking::new())
                .expect("Unexpected get integer fail"),
            Felt252::THREE
        );
    }
}
