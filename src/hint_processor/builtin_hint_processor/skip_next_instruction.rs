use crate::vm::{errors::vm_errors::VirtualMachineError, vm_core::VirtualMachine};

/*
This hint doesn't belong to the Cairo common library
It's only added for testing proposes

Implements hint:
%{ skip_next_instruction() %}
*/
pub fn skip_next_instruction(vm: &mut VirtualMachine) -> Result<(), VirtualMachineError> {
    vm.skip_next_instruction_execution();
    Ok(())
}
