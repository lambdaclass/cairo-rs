use core::fmt::{Debug, Formatter};

use crate::Felt252;
use num_traits::ToPrimitive;

use crate::hint_processor::builtin_hint_processor::dict_manager::Dictionary;
use crate::hint_processor::builtin_hint_processor::hint_utils::{
    get_integer_from_var_name, get_ptr_from_var_name,
};
use crate::serde::deserialize_program::ApTracking;
use crate::stdlib::collections::HashMap;

use crate::types::exec_scope::ExecutionScopes;
use crate::types::relocatable::MaybeRelocatable;
use crate::vm::errors::hint_errors::HintError;
use crate::{
    hint_processor::hint_processor_definition::HintReference, vm::vm_core::VirtualMachine,
};

pub fn print_felt(
    vm: &VirtualMachine,
    ids_data: &HashMap<String, HintReference>,
    ap_tracking: &ApTracking,
) -> Result<(), HintError> {
    let val = get_integer_from_var_name("x", vm, ids_data, ap_tracking)?;
    println!("{val}");
    Ok(())
}

fn print_name(
    vm: &VirtualMachine,
    ids_data: &HashMap<String, HintReference>,
    ap_tracking: &ApTracking,
) -> Result<(), HintError> {
    let name = get_integer_from_var_name("name", vm, ids_data, ap_tracking)?;
    let name = String::from_utf8(name.as_ref().to_bigint().to_signed_bytes_be())
        .map_err(|err| HintError::CustomHint(err.to_string().into_boxed_str()))?;
    println!("{name}");
    Ok(())
}

pub fn print_array(
    vm: &VirtualMachine,
    ids_data: &HashMap<String, HintReference>,
    ap_tracking: &ApTracking,
) -> Result<(), HintError> {
    print_name(vm, ids_data, ap_tracking)?;

    let arr = get_ptr_from_var_name("arr", vm, ids_data, ap_tracking)?;
    let arr_len = get_integer_from_var_name("arr_len", vm, ids_data, ap_tracking)?;
    let arr_len = arr_len.to_usize().ok_or_else(|| {
        HintError::CustomHint(String::from("arr_len must be a positive integer").into_boxed_str())
    })?;
    let mut acc = Vec::with_capacity(arr_len);
    for i in 0..arr_len {
        let val = vm.get_integer((arr + i)?)?;
        acc.push(val);
    }
    println!("{:?}", acc);
    Ok(())
}

enum DictValue {
    Int(Felt252),
    Relocatable(Vec<Felt252>),
}

impl Debug for DictValue {
    fn fmt(&self, f: &mut Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::Int(int) => write!(f, "{:?}", int),
            Self::Relocatable(relocatable) => write!(f, "{:?}", relocatable),
        }
    }
}

pub fn print_dict(
    vm: &VirtualMachine,
    exec_scopes: &ExecutionScopes,
    ids_data: &HashMap<String, HintReference>,
    ap_tracking: &ApTracking,
) -> Result<(), HintError> {
    print_name(vm, ids_data, ap_tracking)?;

    let dict_ptr = get_ptr_from_var_name("dict_ptr", vm, ids_data, ap_tracking)?;
    let pointer_size = get_integer_from_var_name("pointer_size", vm, ids_data, ap_tracking)?;
    let pointer_size = pointer_size.to_usize().ok_or_else(|| {
        HintError::CustomHint(
            String::from("pointer_size must be a positive integer").into_boxed_str(),
        )
    })?;

    let dict_manager = exec_scopes.get_dict_manager()?;
    let dict_manager = dict_manager.borrow();
    let tracker = dict_manager.get_tracker(dict_ptr)?;

    let map = match &tracker.data {
        Dictionary::SimpleDictionary(dict) => dict,
        Dictionary::DefaultDictionary { dict, .. } => dict,
    };

    let mut acc = HashMap::new();
    for (k, v) in map.iter() {
        let key = k.get_int_ref().ok_or_else(|| {
            HintError::CustomHint(String::from("Expected felt key for dict").into_boxed_str())
        })?;
        match v {
            MaybeRelocatable::Int(value) => {
                acc.insert(key, DictValue::Int(*value));
            }
            MaybeRelocatable::RelocatableValue(val) => {
                let mut structure = Vec::with_capacity(pointer_size);
                for i in 0..pointer_size {
                    let val = *vm.get_integer((*val + i)?)?.as_ref();
                    structure.push(val);
                }
                acc.insert(key, DictValue::Relocatable(structure));
            }
        }
    }

    println!("{:?}", acc);
    Ok(())
}
