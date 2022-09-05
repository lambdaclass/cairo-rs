use std::collections::HashMap;

use super::{
    hint_processor_definition::HintReference,
    hint_processor_utils::{apply_ap_tracking_correction, bigint_to_usize},
};
use crate::{
    serde::deserialize_program::ApTracking,
    types::{
        instruction::Register,
        relocatable::{MaybeRelocatable, Relocatable},
    },
    vm::{
        context::run_context::RunContext, errors::vm_errors::VirtualMachineError,
        vm_core::VirtualMachine, vm_memory::memory::Memory,
    },
};

//Returns the VM's Memory, excluding the program segment in the format HashMap<Relocatable, MaybeRelocatable>
pub fn get_python_compatible_memory(memory: &Memory) -> HashMap<(usize, usize), MaybeRelocatable> {
    let mut py_mem: HashMap<(usize, usize), MaybeRelocatable> = HashMap::new();
    for segment_index in 1..memory.data.len() {
        for offset in 0..memory.data[segment_index].len() {
            if let Some(elem) = &memory.data[segment_index][offset] {
                py_mem.insert((segment_index, offset), elem.clone());
            }
        }
    }
    py_mem
}

//Returns a HashMap of ids values, ready to be sent to a python process
pub fn get_python_compatible_ids(
    vm: &VirtualMachine,
    ids_data: &HashMap<String, HintReference>,
    ap_tracking: &ApTracking,
) -> Result<HashMap<String, Option<MaybeRelocatable>>, VirtualMachineError> {
    let mut ids = HashMap::new();
    for (name, reference) in ids_data.iter() {
        ids.insert(
            name.clone(),
            get_value_from_reference(vm, reference, ap_tracking)?,
        );
    }
    Ok(ids)
}

///Returns the Value given by a reference as an Option<MaybeRelocatable>
pub fn get_value_from_reference(
    vm: &VirtualMachine,
    hint_reference: &HintReference,
    ap_tracking: &ApTracking,
) -> Result<Option<MaybeRelocatable>, VirtualMachineError> {
    let var_addr =
        compute_addr_from_reference(hint_reference, &vm.run_context, &vm.memory, ap_tracking)?;
    let value = if hint_reference.dereference {
        vm.memory.get(&var_addr)?
    } else {
        if let (None, Some(num)) = (&hint_reference.register, &hint_reference.immediate) {
            return Ok(Some(MaybeRelocatable::from(num)));
        } else {
            return Ok(Some(MaybeRelocatable::from(var_addr)));
        }
    };
    Ok(match &value {
        Some(&MaybeRelocatable::RelocatableValue(ref rel)) => {
            if let Some(immediate) = &hint_reference.immediate {
                let modified_value = rel + bigint_to_usize(immediate)?;
                Some(MaybeRelocatable::from(modified_value))
            } else {
                value.cloned()
            }
        }
        None | Some(&MaybeRelocatable::Int(_)) => value.cloned(),
    })
}

///Computes the memory address of the ids variable indicated by the HintReference as a Relocatable
pub fn compute_addr_from_reference(
    //Reference data of the ids variable
    hint_reference: &HintReference,
    run_context: &RunContext,
    memory: &Memory,
    //ApTracking of the Hint itself
    hint_ap_tracking: &ApTracking,
) -> Result<Relocatable, VirtualMachineError> {
    let base_addr = match hint_reference.register {
        //This should never fail
        Some(Register::FP) => run_context.get_fp().get_relocatable()?.clone(),
        Some(Register::AP) => {
            let var_ap_trackig = hint_reference
                .ap_tracking_data
                .as_ref()
                .ok_or(VirtualMachineError::NoneApTrackingData)?;

            let ap = run_context.get_ap().get_relocatable()?.clone();

            apply_ap_tracking_correction(&ap, var_ap_trackig, hint_ap_tracking)?
        }
        None => return Err(VirtualMachineError::NoRegisterInReference),
    };
    if hint_reference.offset1.is_negative()
        && base_addr.offset < hint_reference.offset1.abs() as usize
    {
        return Err(VirtualMachineError::FailedToGetIds);
    }
    if !hint_reference.inner_dereference {
        Ok(base_addr + hint_reference.offset1 + hint_reference.offset2)
    } else {
        let addr = base_addr + hint_reference.offset1;
        let dereferenced_addr = memory
            .get_relocatable(&addr)
            .map_err(|_| VirtualMachineError::FailedToGetIds)?;
        if let Some(imm) = &hint_reference.immediate {
            Ok(dereferenced_addr + bigint_to_usize(imm)?)
        } else {
            Ok(dereferenced_addr + hint_reference.offset2)
        }
    }
}
