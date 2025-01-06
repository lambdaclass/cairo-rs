use crate::stdlib::prelude::*;
use crate::types::relocatable::MaybeRelocatable;
use crate::types::{errors::math_errors::MathError, relocatable::Relocatable};
use crate::vm::errors::{hint_errors::HintError, vm_errors::VirtualMachineError};
use crate::vm::vm_core::VirtualMachine;
use crate::Felt252;
use cairo_lang_casm::operand::{CellRef, DerefOrImmediate, Operation, Register, ResOperand};

/// Extracts a parameter assumed to be a buffer.
pub(crate) fn extract_buffer(buffer: &ResOperand) -> Result<(&CellRef, Felt252), HintError> {
    let (cell, base_offset) = match buffer {
        ResOperand::Deref(cell) => (cell, 0.into()),
        ResOperand::BinOp(bin_op) => {
            if let DerefOrImmediate::Immediate(val) = &bin_op.b {
                (&bin_op.a, Felt252::from(&val.value))
            } else {
                return Err(HintError::CustomHint(
                    "Failed to extract buffer, expected ResOperand of BinOp type to have Inmediate b value".to_owned().into_boxed_str()
                ));
            }
        }
        _ => {
            return Err(HintError::CustomHint(
                "Illegal argument for a buffer."
                    .to_string()
                    .into_boxed_str(),
            ))
        }
    };
    Ok((cell, base_offset))
}

pub(crate) fn get_mayberelocatable(
    vm: &VirtualMachine,
    cell: &CellRef,
) -> Result<MaybeRelocatable, VirtualMachineError> {
    let relocatable = cell_ref_to_relocatable(cell, vm)?;
    vm.get_maybe(&relocatable).ok_or_else(|| {
        VirtualMachineError::InvalidMemoryValueTemporaryAddress(Box::new(relocatable))
    })
}

pub(crate) fn cell_ref_to_relocatable(
    cell_ref: &CellRef,
    vm: &VirtualMachine,
) -> Result<Relocatable, MathError> {
    let base = match cell_ref.register {
        Register::AP => vm.get_ap(),
        Register::FP => vm.get_fp(),
    };
    base + (cell_ref.offset as i32)
}

pub(crate) fn get_cell_val(
    vm: &VirtualMachine,
    cell: &CellRef,
) -> Result<Felt252, VirtualMachineError> {
    Ok(*vm.get_integer(cell_ref_to_relocatable(cell, vm)?)?.as_ref())
}

pub(crate) fn get_ptr(
    vm: &VirtualMachine,
    cell: &CellRef,
    offset: &Felt252,
) -> Result<Relocatable, VirtualMachineError> {
    Ok((vm.get_relocatable(cell_ref_to_relocatable(cell, vm)?)? + offset)?)
}

#[cfg(feature = "std")]
pub(crate) fn as_relocatable(
    vm: &mut VirtualMachine,
    value: &ResOperand,
) -> Result<Relocatable, HintError> {
    let (base, offset) = extract_buffer(value)?;
    get_ptr(vm, base, &offset).map_err(HintError::from)
}

pub(crate) fn get_double_deref_val(
    vm: &VirtualMachine,
    cell: &CellRef,
    offset: &Felt252,
) -> Result<Felt252, VirtualMachineError> {
    Ok(*vm.get_integer(get_ptr(vm, cell, offset)?)?.as_ref())
}

/// Fetches the value of `res_operand` from the vm.
pub(crate) fn res_operand_get_val(
    vm: &VirtualMachine,
    res_operand: &ResOperand,
) -> Result<Felt252, VirtualMachineError> {
    match res_operand {
        ResOperand::Deref(cell) => get_cell_val(vm, cell),
        ResOperand::DoubleDeref(cell, offset) => {
            get_double_deref_val(vm, cell, &Felt252::from(*offset as i32))
        }
        ResOperand::Immediate(x) => Ok(Felt252::from(&x.value)),
        ResOperand::BinOp(op) => {
            let a = get_cell_val(vm, &op.a)?;
            let b = match &op.b {
                DerefOrImmediate::Deref(cell) => get_cell_val(vm, cell)?,
                DerefOrImmediate::Immediate(x) => Felt252::from(&x.value),
            };
            match op.op {
                Operation::Add => Ok(a + b),
                Operation::Mul => Ok(a * b),
            }
        }
    }
}

#[cfg(feature = "std")]
pub(crate) fn as_cairo_short_string(value: &Felt252) -> Option<String> {
    let mut as_string = String::default();
    let mut is_end = false;
    for byte in value
        .to_bytes_be()
        .into_iter()
        .skip_while(num_traits::Zero::is_zero)
    {
        if byte == 0 {
            is_end = true;
        } else if is_end || !byte.is_ascii() {
            return None;
        } else {
            as_string.push(byte as char);
        }
    }
    Some(as_string)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn simple_as_cairo_short_string() {
        // Values extracted from cairo book example
        let s = "Hello, Scarb!";
        let x = Felt252::from(5735816763073854913753904210465_u128);
        assert!(s.is_ascii());
        let cairo_string = as_cairo_short_string(&x).expect("call to as_cairo_short_string failed");
        assert_eq!(cairo_string, s);
    }
}
