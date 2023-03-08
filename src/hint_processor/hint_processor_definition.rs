use crate::stdlib::{any::Any, boxed::Box, collections::HashMap, prelude::*};

use crate::any_box;
use crate::serde::deserialize_program::ApTracking;
use crate::serde::deserialize_program::OffsetValue;
use crate::serde::deserialize_program::Reference;
use crate::types::exec_scope::ExecutionScopes;
use crate::types::instruction::Register;
use crate::vm::errors::hint_errors::HintError;
use crate::vm::errors::vm_errors::VirtualMachineError;
use crate::vm::vm_core::VirtualMachine;

use super::builtin_hint_processor::builtin_hint_processor_definition::HintProcessorData;
use felt::Felt;

pub trait HintProcessor {
    //Executes the hint which's data is provided by a dynamic structure previously created by compile_hint
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
        constants: &HashMap<String, Felt>,
    ) -> Result<(), HintError>;

    //Transforms hint data outputed by the VM into whichever format will be later used by execute_hint
    fn compile_hint(
        &self,
        //Block of hint code as String
        hint_code: &str,
        //Ap Tracking Data corresponding to the Hint
        ap_tracking_data: &ApTracking,
        //Map from variable name to reference id number
        //(may contain other variables aside from those used by the hint)
        reference_ids: &HashMap<String, usize>,
        //List of all references (key corresponds to element of the previous dictionary)
        references: &HashMap<usize, HintReference>,
    ) -> Result<Box<dyn Any>, VirtualMachineError> {
        Ok(any_box!(HintProcessorData {
            code: hint_code.to_string(),
            ap_tracking: ap_tracking_data.clone(),
            ids_data: get_ids_data(reference_ids, references)?,
        }))
    }
}

fn get_ids_data(
    reference_ids: &HashMap<String, usize>,
    references: &HashMap<usize, HintReference>,
) -> Result<HashMap<String, HintReference>, VirtualMachineError> {
    let mut ids_data = HashMap::<String, HintReference>::new();
    for (path, ref_id) in reference_ids {
        let name = path
            .rsplit('.')
            .next()
            .ok_or(VirtualMachineError::Unexpected)?;
        ids_data.insert(
            name.to_string(),
            references
                .get(ref_id)
                .ok_or(VirtualMachineError::Unexpected)?
                .clone(),
        );
    }
    Ok(ids_data)
}

#[derive(Debug, PartialEq, Eq, Clone)]
pub struct HintReference {
    pub offset1: OffsetValue,
    pub offset2: OffsetValue,
    pub dereference: bool,
    pub ap_tracking_data: Option<ApTracking>,
    pub cairo_type: Option<String>,
}

impl HintReference {
    pub fn new_simple(offset1: i32) -> Self {
        HintReference {
            offset1: OffsetValue::Reference(Register::FP, offset1, false),
            offset2: OffsetValue::Value(0),
            ap_tracking_data: None,
            dereference: true,
            cairo_type: None,
        }
    }

    pub fn new(offset1: i32, offset2: i32, inner_dereference: bool, dereference: bool) -> Self {
        HintReference {
            offset1: OffsetValue::Reference(Register::FP, offset1, inner_dereference),
            offset2: OffsetValue::Value(offset2),
            ap_tracking_data: None,
            dereference,
            cairo_type: None,
        }
    }
}

impl From<Reference> for HintReference {
    fn from(reference: Reference) -> Self {
        HintReference {
            offset1: reference.value_address.offset1.clone(),
            offset2: reference.value_address.offset2.clone(),
            dereference: reference.value_address.dereference,
            // only store `ap` tracking data if the reference is referred to it
            ap_tracking_data: match (
                &reference.value_address.offset1,
                &reference.value_address.offset2,
            ) {
                (OffsetValue::Reference(Register::AP, _, _), _)
                | (_, OffsetValue::Reference(Register::AP, _, _)) => {
                    Some(reference.ap_tracking_data.clone())
                }
                _ => None,
            },
            cairo_type: Some(reference.value_address.value_type.clone()),
        }
    }
}
