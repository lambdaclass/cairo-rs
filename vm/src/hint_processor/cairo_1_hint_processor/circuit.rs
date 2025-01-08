// Most of the `EvalCircuit` implementation is derived from the `cairo-lang-runner` crate.
// https://github.com/starkware-libs/cairo/blob/main/crates/cairo-lang-runner/src/casm_run/circuit.rs

use core::ops::Deref;

use ark_ff::{One, Zero};
use num_bigint::{BigInt, BigUint, ToBigInt};
use num_integer::Integer;
use num_traits::Signed;
use starknet_types_core::felt::Felt;

use crate::{
    stdlib::boxed::Box,
    types::relocatable::{MaybeRelocatable, Relocatable},
    vm::{
        errors::{hint_errors::HintError, memory_errors::MemoryError},
        vm_core::VirtualMachine,
    },
};

// A gate is defined by 3 offsets, the first two are the inputs and the third is the output.
const OFFSETS_PER_GATE: usize = 3;
// Represents the number of limbs use to represent a single value in a circuit
const LIMBS_COUNT: usize = 4;
// Representes the size of a MulMod and AddMod instance
const MOD_BUILTIN_INSTACE_SIZE: usize = 7;

struct Circuit<'a> {
    vm: &'a mut VirtualMachine,
    values_ptr: Relocatable,
    add_mod_offsets: Relocatable,
    mul_mod_offsets: Relocatable,
    modulus: BigUint,
}

impl Circuit<'_> {
    fn read_add_mod_value(&mut self, offset: usize) -> Result<Option<BigUint>, MemoryError> {
        self.read_circuit_value((self.add_mod_offsets + offset)?)
    }

    fn read_mul_mod_value(&mut self, offset: usize) -> Result<Option<BigUint>, MemoryError> {
        self.read_circuit_value((self.mul_mod_offsets + offset)?)
    }

    fn read_circuit_value(&mut self, offset: Relocatable) -> Result<Option<BigUint>, MemoryError> {
        let value_ptr = self.get_value_ptr(offset)?;
        read_circuit_value(self.vm, value_ptr)
    }

    fn write_add_mod_value(&mut self, offset: usize, value: BigUint) -> Result<(), MemoryError> {
        self.write_circuit_value((self.add_mod_offsets + offset)?, value)?;

        Ok(())
    }

    fn write_mul_mod_value(&mut self, offset: usize, value: BigUint) -> Result<(), MemoryError> {
        self.write_circuit_value((self.mul_mod_offsets + offset)?, value)?;

        Ok(())
    }

    fn write_circuit_value(
        &mut self,
        offset: Relocatable,
        value: BigUint,
    ) -> Result<(), MemoryError> {
        let value_ptr = self.get_value_ptr(offset)?;
        write_circuit_value(self.vm, value_ptr, value)?;

        Ok(())
    }

    fn get_value_ptr(&self, address: Relocatable) -> Result<Relocatable, MemoryError> {
        (self.values_ptr + self.vm.get_integer(address)?.as_ref()).map_err(MemoryError::Math)
    }
}

fn read_circuit_value(
    vm: &mut VirtualMachine,
    add: Relocatable,
) -> Result<Option<BigUint>, MemoryError> {
    let mut res = BigUint::zero();

    for l in (0..LIMBS_COUNT).rev() {
        let add_l = (add + l)?;
        match vm.get_maybe(&add_l) {
            Some(MaybeRelocatable::Int(limb)) => res = (res << 96) + limb.to_biguint(),
            _ => return Ok(None),
        }
    }

    Ok(Some(res))
}

fn write_circuit_value(
    vm: &mut VirtualMachine,
    add: Relocatable,
    mut value: BigUint,
) -> Result<(), MemoryError> {
    for l in 0..LIMBS_COUNT {
        // get the nth limb from a circuit value
        let (new_value, rem) = value.div_rem(&(BigUint::one() << 96u8));
        vm.insert_value((add + l)?, Felt::from(rem))?;
        value = new_value;
    }

    Ok(())
}

// Finds the inverse of a value.
//
// If the value has no inverse, find a nullifier so that:
// value * nullifier = 0 (mod modulus)
fn find_inverse(value: BigUint, modulus: &BigUint) -> Result<(bool, BigUint), HintError> {
    let ex_gcd = value
        .to_bigint()
        .ok_or(HintError::BigUintToBigIntFail)?
        .extended_gcd(&modulus.to_bigint().ok_or(HintError::BigUintToBigIntFail)?);

    let gcd = ex_gcd
        .gcd
        .to_biguint()
        .ok_or(HintError::BigIntToBigUintFail)?;
    if gcd.is_one() {
        return Ok((true, get_modulus(&ex_gcd.x, modulus)));
    }

    let nullifier = modulus / gcd;

    Ok((false, nullifier))
}

fn get_modulus(value: &BigInt, modulus: &BigUint) -> BigUint {
    let value_magnitud = value.magnitude().mod_floor(modulus);
    if value.is_negative() {
        modulus - value_magnitud
    } else {
        value_magnitud
    }
}

fn compute_gates(
    vm: &mut VirtualMachine,
    values_ptr: Relocatable,
    add_mod_offsets: Relocatable,
    n_add_mods: usize,
    mul_mod_offsets: Relocatable,
    n_mul_mods: usize,
    modulus_ptr: Relocatable,
) -> Result<usize, HintError> {
    let modulus = read_circuit_value(vm, modulus_ptr)?.ok_or(HintError::Memory(
        MemoryError::ExpectedInteger(Box::from(modulus_ptr)),
    ))?;

    let mut circuit = Circuit {
        vm,
        values_ptr,
        add_mod_offsets,
        mul_mod_offsets,
        modulus,
    };

    let mut addmod_idx = 0;
    let mut mulmod_idx = 0;

    // Only mul gates can make the evaluation fail
    let mut first_failure_idx = n_mul_mods;

    loop {
        while addmod_idx < n_add_mods {
            let lhs = circuit.read_add_mod_value(3 * addmod_idx)?;
            let rhs = circuit.read_add_mod_value(3 * addmod_idx + 1)?;

            match (lhs, rhs) {
                (Some(l), Some(r)) => {
                    let res = (l + r) % &circuit.modulus;
                    circuit.write_add_mod_value(3 * addmod_idx + 2, res)?;
                }
                // sub gate: lhs = res - rhs
                (None, Some(r)) => {
                    let Some(res) = circuit.read_add_mod_value(3 * addmod_idx + 2)? else {
                        break;
                    };
                    let value = (res + &circuit.modulus - r) % &circuit.modulus;
                    circuit.write_add_mod_value(3 * addmod_idx, value)?;
                }
                _ => break,
            }

            addmod_idx += 1;
        }

        if mulmod_idx == n_mul_mods {
            break;
        }

        let lhs = circuit.read_mul_mod_value(3 * mulmod_idx)?;
        let rhs = circuit.read_mul_mod_value(3 * mulmod_idx + 1)?;

        match (lhs, rhs) {
            (Some(l), Some(r)) => {
                let res = (l * r) % &circuit.modulus;
                circuit.write_mul_mod_value(3 * mulmod_idx + 2, res)?;
            }
            // inverse gate: lhs = 1 / rhs
            (None, Some(r)) => {
                let (success, res) = find_inverse(r, &circuit.modulus)?;
                circuit.write_mul_mod_value(3 * mulmod_idx, res)?;

                if !success {
                    first_failure_idx = mulmod_idx;
                    break;
                }
            }
            _ => {
                // this should be unreachable as it would mean that the
                //circuit being evaluated is not complete and therefore invalid
                return Err(HintError::CircuitEvaluationFailed(Box::from(
                    "Unexpected None value while filling mul_mod gate",
                )));
            }
        }

        mulmod_idx += 1;
    }

    Ok(first_failure_idx)
}

fn fill_instances(
    vm: &mut VirtualMachine,
    built_ptr: Relocatable,
    n_instances: usize,
    modulus: [Felt; LIMBS_COUNT],
    values_ptr: Relocatable,
    mut offsets_ptr: Relocatable,
) -> Result<(), HintError> {
    for i in 0..n_instances {
        let instance_ptr = (built_ptr + i * MOD_BUILTIN_INSTACE_SIZE)?;

        for (idx, value) in modulus.iter().enumerate() {
            vm.insert_value((instance_ptr + idx)?, *value)?;
        }

        vm.insert_value((instance_ptr + 4)?, values_ptr)?;
        vm.insert_value((instance_ptr + 5)?, offsets_ptr)?;
        offsets_ptr += OFFSETS_PER_GATE;
        vm.insert_value((instance_ptr + 6)?, n_instances - i)?;
    }
    Ok(())
}

/// Computes the circuit.
///
/// If theres a failure, it returs the index of the gate in which the failure occurred, else
/// returns the total amount of mul gates.
pub fn eval_circuit(
    vm: &mut VirtualMachine,
    n_add_mods: usize,
    add_mod_builtin_address: Relocatable,
    n_mul_mods: usize,
    mul_mod_builtin_address: Relocatable,
) -> Result<(), HintError> {
    let modulus_ptr = mul_mod_builtin_address;
    let mul_mod_values_offset = 4;
    let mul_mod_offset = 5;

    let values_ptr = vm.get_relocatable((mul_mod_builtin_address + mul_mod_values_offset)?)?;
    let mul_mod_offsets = vm.get_relocatable((mul_mod_builtin_address + mul_mod_offset)?)?;
    let add_mod_offsets = if n_add_mods == 0 {
        mul_mod_offsets
    } else {
        vm.get_relocatable((add_mod_builtin_address + mul_mod_offset)?)?
    };

    let n_computed_gates = compute_gates(
        vm,
        values_ptr,
        add_mod_offsets,
        n_add_mods,
        mul_mod_offsets,
        n_mul_mods,
        modulus_ptr,
    )?;

    let modulus: [Felt; 4] = [
        *vm.get_integer(modulus_ptr)?.deref(),
        *vm.get_integer((modulus_ptr + 1)?)?.deref(),
        *vm.get_integer((modulus_ptr + 2)?)?.deref(),
        *vm.get_integer((modulus_ptr + 3)?)?.deref(),
    ];

    fill_instances(
        vm,
        add_mod_builtin_address,
        n_add_mods,
        modulus,
        values_ptr,
        add_mod_offsets,
    )?;
    fill_instances(
        vm,
        mul_mod_builtin_address,
        n_computed_gates,
        modulus,
        values_ptr,
        mul_mod_offsets,
    )?;

    Ok(())
}
