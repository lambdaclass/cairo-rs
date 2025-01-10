use crate::stdlib::{any::Any, boxed::Box, collections::HashMap, prelude::*};

use crate::Felt252;
use crate::{
    hint_processor::{
        builtin_hint_processor::hint_utils::{
            get_integer_from_var_name, get_ptr_from_var_name, insert_value_from_var_name,
        },
        hint_processor_definition::HintReference,
    },
    serde::deserialize_program::ApTracking,
    types::exec_scope::ExecutionScopes,
    vm::{errors::hint_errors::HintError, vm_core::VirtualMachine},
};

use num_traits::ToPrimitive;

pub fn usort_enter_scope(exec_scopes: &mut ExecutionScopes) -> Result<(), HintError> {
    if let Ok(usort_max_size) = exec_scopes.get::<Felt252>("usort_max_size") {
        let boxed_max_size: Box<dyn Any> = Box::new(usort_max_size);
        exec_scopes.enter_scope(HashMap::from([(
            "usort_max_size".to_string(),
            boxed_max_size,
        )]));
    } else {
        exec_scopes.enter_scope(HashMap::new());
    }
    Ok(())
}

pub fn usort_body(
    vm: &mut VirtualMachine,
    exec_scopes: &mut ExecutionScopes,
    ids_data: &HashMap<String, HintReference>,
    ap_tracking: &ApTracking,
) -> Result<(), HintError> {
    let input_ptr = get_ptr_from_var_name("input", vm, ids_data, ap_tracking)?;
    let usort_max_size = exec_scopes.get::<u64>("usort_max_size");
    let input_len = get_integer_from_var_name("input_len", vm, ids_data, ap_tracking)?;
    let input_len_u64 = input_len.to_u64().ok_or(HintError::BigintToUsizeFail)?;

    if let Ok(usort_max_size) = usort_max_size {
        if input_len_u64 > usort_max_size {
            return Err(HintError::UsortOutOfRange(Box::new((
                usort_max_size,
                input_len,
            ))));
        }
    }
    let mut positions_dict: HashMap<Felt252, Vec<u64>> = HashMap::new();
    let mut output: Vec<Felt252> = Vec::new();
    for i in 0..input_len_u64 {
        let val = vm.get_integer((input_ptr + i as usize)?)?.into_owned();
        if let Err(output_index) = output.binary_search(&val) {
            output.insert(output_index, val);
        }
        positions_dict.entry(val).or_default().push(i);
    }

    let mut multiplicities: Vec<usize> = Vec::with_capacity(output.len());
    for k in output.iter() {
        multiplicities.push(positions_dict[k].len());
    }
    exec_scopes.insert_value("positions_dict", positions_dict);
    let output_base = vm.add_memory_segment();
    let multiplicities_base = vm.add_memory_segment();
    let output_len = output.len();

    for (i, sorted_element) in output.into_iter().enumerate() {
        vm.insert_value((output_base + i)?, sorted_element)?;
    }

    for (i, repetition_amount) in multiplicities.into_iter().enumerate() {
        vm.insert_value((multiplicities_base + i)?, Felt252::from(repetition_amount))?;
    }

    insert_value_from_var_name(
        "output_len",
        Felt252::from(output_len),
        vm,
        ids_data,
        ap_tracking,
    )?;
    insert_value_from_var_name("output", output_base, vm, ids_data, ap_tracking)?;
    insert_value_from_var_name(
        "multiplicities",
        multiplicities_base,
        vm,
        ids_data,
        ap_tracking,
    )?;
    Ok(())
}

pub fn verify_usort(
    vm: &mut VirtualMachine,
    exec_scopes: &mut ExecutionScopes,
    ids_data: &HashMap<String, HintReference>,
    ap_tracking: &ApTracking,
) -> Result<(), HintError> {
    let value = get_integer_from_var_name("value", vm, ids_data, ap_tracking)?;
    let mut positions = exec_scopes
        .get_mut_dict_ref::<Felt252, Vec<u64>>("positions_dict")?
        .remove(value.as_ref())
        .ok_or(HintError::UnexpectedPositionsDictFail)?;
    positions.reverse();
    exec_scopes.insert_value("positions", positions);
    exec_scopes.insert_value("last_pos", Felt252::ZERO);
    Ok(())
}

pub fn verify_multiplicity_assert(exec_scopes: &mut ExecutionScopes) -> Result<(), HintError> {
    let positions_len = exec_scopes.get_list_ref::<u64>("positions")?.len();
    if positions_len == 0 {
        Ok(())
    } else {
        Err(HintError::PositionsLengthNotZero)
    }
}

pub fn verify_multiplicity_body(
    vm: &mut VirtualMachine,
    exec_scopes: &mut ExecutionScopes,
    ids_data: &HashMap<String, HintReference>,
    ap_tracking: &ApTracking,
) -> Result<(), HintError> {
    let current_pos = exec_scopes
        .get_mut_list_ref::<u64>("positions")?
        .pop()
        .ok_or(HintError::CouldntPopPositions)?;
    let pos_diff = Felt252::from(current_pos) - exec_scopes.get::<Felt252>("last_pos")?;
    insert_value_from_var_name("next_item_index", pos_diff, vm, ids_data, ap_tracking)?;
    exec_scopes.insert_value("last_pos", Felt252::from(current_pos + 1));
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        any_box,
        hint_processor::{
            builtin_hint_processor::{
                builtin_hint_processor_definition::{BuiltinHintProcessor, HintProcessorData},
                hint_code::USORT_BODY,
            },
            hint_processor_definition::HintProcessorLogic,
        },
        types::exec_scope::ExecutionScopes,
        utils::test_utils::*,
        vm::vm_core::VirtualMachine,
    };
    use assert_matches::assert_matches;

    #[cfg(target_arch = "wasm32")]
    use wasm_bindgen_test::*;

    #[test]
    #[cfg_attr(target_arch = "wasm32", wasm_bindgen_test)]
    fn usort_with_max_size() {
        let mut exec_scopes = scope![("usort_max_size", 1_u64)];
        assert_matches!(usort_enter_scope(&mut exec_scopes), Ok(()));
    }

    #[test]
    #[cfg_attr(target_arch = "wasm32", wasm_bindgen_test)]
    fn usort_out_of_range() {
        let mut vm = vm_with_range_check!();
        vm.run_context.fp = 2;
        add_segments!(vm, 1);
        vm.segments = segments![((1, 0), (2, 1)), ((1, 1), 5)];
        //Create hint_data
        let ids_data = ids_data!["input", "input_len"];
        let mut exec_scopes = scope![("usort_max_size", 1_u64)];
        assert_matches!(
            run_hint!(vm, ids_data, USORT_BODY, &mut exec_scopes),
            Err(HintError::UsortOutOfRange(bx)) if *bx == (1, Felt252::from(5_i32))
        );
    }
}
