use crate::air_private_input::{PrivateInput, PrivateInputSignature, SignatureInput};
use crate::math_utils::div_mod;
use crate::stdlib::{cell::RefCell, collections::HashMap, prelude::*, rc::Rc};

use crate::types::builtin_name::BuiltinName;
use crate::types::instance_definitions::ecdsa_instance_def::CELLS_PER_SIGNATURE;
use crate::vm::errors::runner_errors::RunnerError;
use crate::vm::runners::cairo_pie::BuiltinAdditionalData;
use crate::Felt252;
use crate::{
    types::relocatable::{MaybeRelocatable, Relocatable},
    vm::{
        errors::memory_errors::MemoryError,
        vm_memory::{
            memory::{Memory, ValidationRule},
            memory_segments::MemorySegmentManager,
        },
    },
};
use lazy_static::lazy_static;
use num_bigint::{BigInt, Sign};
use num_integer::div_ceil;
use num_traits::{Num, One};
use starknet_crypto::{verify, Signature};

lazy_static! {
    static ref EC_ORDER: BigInt = BigInt::from_str_radix(
        "3618502788666131213697322783095070105526743751716087489154079457884512865583",
        10
    )
    .unwrap();
}

#[derive(Debug, Clone)]
pub struct SignatureBuiltinRunner {
    pub(crate) included: bool,
    ratio: Option<u32>,
    base: usize,
    pub(crate) stop_ptr: Option<usize>,
    signatures: Rc<RefCell<HashMap<Relocatable, Signature>>>,
}

impl SignatureBuiltinRunner {
    pub(crate) fn new(ratio: Option<u32>, included: bool) -> Self {
        SignatureBuiltinRunner {
            base: 0,
            included,
            ratio,
            stop_ptr: None,
            signatures: Rc::new(RefCell::new(HashMap::new())),
        }
    }

    pub fn add_signature(
        &mut self,
        relocatable: Relocatable,
        (r, s): &(Felt252, Felt252),
    ) -> Result<(), MemoryError> {
        let r_be_bytes = r.to_bytes_be();
        let s_be_bytes = s.to_bytes_be();
        let (r_felt, s_felt) = (
            Felt252::from_bytes_be(&r_be_bytes),
            Felt252::from_bytes_be(&s_be_bytes),
        );

        let signature = Signature {
            r: r_felt,
            s: s_felt,
        };

        self.signatures
            .borrow_mut()
            .entry(relocatable)
            .or_insert(signature);

        Ok(())
    }
}

impl SignatureBuiltinRunner {
    pub fn initialize_segments(&mut self, segments: &mut MemorySegmentManager) {
        self.base = segments.add().segment_index as usize // segments.add() always returns a positive index
    }

    pub fn initial_stack(&self) -> Vec<MaybeRelocatable> {
        if self.included {
            vec![MaybeRelocatable::from((self.base as isize, 0))]
        } else {
            vec![]
        }
    }

    pub fn base(&self) -> usize {
        self.base
    }
    pub fn add_validation_rule(&self, memory: &mut Memory) {
        let cells_per_instance = CELLS_PER_SIGNATURE;
        let signatures = Rc::clone(&self.signatures);
        let rule: ValidationRule = ValidationRule(Box::new(
            move |memory: &Memory, addr: Relocatable| -> Result<Vec<Relocatable>, MemoryError> {
                let cell_index = addr.offset % cells_per_instance as usize;

                let (pubkey_addr, message_addr) = match cell_index {
                    0 => (addr, (addr + 1)?),
                    1 => match addr - 1 {
                        Ok(prev_addr) => (prev_addr, addr),
                        Err(_) => return Ok(vec![]),
                    },
                    _ => return Ok(vec![]),
                };

                let pubkey = match memory.get_integer(pubkey_addr) {
                    Ok(num) => num,
                    Err(_) if cell_index == 1 => return Ok(vec![]),
                    _ => return Err(MemoryError::PubKeyNonInt(Box::new(pubkey_addr))),
                };

                let msg = match memory.get_integer(message_addr) {
                    Ok(num) => num,
                    Err(_) if cell_index == 0 => return Ok(vec![]),
                    _ => return Err(MemoryError::MsgNonInt(Box::new(message_addr))),
                };

                let signatures_map = signatures.borrow();
                let signature = signatures_map
                    .get(&pubkey_addr)
                    .ok_or_else(|| MemoryError::SignatureNotFound(Box::new(pubkey_addr)))?;

                let public_key = Felt252::from_bytes_be(&pubkey.to_bytes_be());
                let (r, s) = (signature.r, signature.s);
                let message = Felt252::from_bytes_be(&msg.to_bytes_be());
                match verify(&public_key, &message, &r, &s) {
                    Ok(true) => Ok(vec![]),
                    _ => Err(MemoryError::InvalidSignature(Box::new((
                        format!("({}, {})", signature.r, signature.s),
                        pubkey.into_owned(),
                        msg.into_owned(),
                    )))),
                }
            },
        ));
        memory.add_validation_rule(self.base, rule);
    }

    pub fn ratio(&self) -> Option<u32> {
        self.ratio
    }

    pub fn get_used_cells(&self, segments: &MemorySegmentManager) -> Result<usize, MemoryError> {
        segments
            .get_segment_used_size(self.base)
            .ok_or(MemoryError::MissingSegmentUsedSizes)
    }

    pub fn get_used_instances(
        &self,
        segments: &MemorySegmentManager,
    ) -> Result<usize, MemoryError> {
        let used_cells = self.get_used_cells(segments)?;
        Ok(div_ceil(used_cells, CELLS_PER_SIGNATURE as usize))
    }

    pub fn get_additional_data(&self) -> BuiltinAdditionalData {
        // Convert signatures to Felt tuple
        let signatures: HashMap<Relocatable, (Felt252, Felt252)> = self
            .signatures
            .borrow()
            .iter()
            .map(|(k, v)| {
                (
                    *k,
                    (
                        Felt252::from_bytes_be(&v.r.to_bytes_be()),
                        Felt252::from_bytes_be(&v.s.to_bytes_be()),
                    ),
                )
            })
            .collect();
        BuiltinAdditionalData::Signature(signatures)
    }

    pub fn extend_additional_data(
        &mut self,
        additional_data: &BuiltinAdditionalData,
    ) -> Result<(), RunnerError> {
        let additional_data = match additional_data {
            BuiltinAdditionalData::Signature(d) => d,
            BuiltinAdditionalData::Empty(_) => return Ok(()),
            _ => return Err(RunnerError::InvalidAdditionalData(BuiltinName::ecdsa)),
        };
        for (addr, (r, s)) in additional_data {
            if addr.segment_index != self.base as isize {
                return Err(RunnerError::InvalidAdditionalData(BuiltinName::ecdsa));
            }
            self.signatures.borrow_mut().insert(
                *addr,
                Signature {
                    r: Felt252::from_bytes_be(&r.to_bytes_be()),
                    s: Felt252::from_bytes_be(&s.to_bytes_be()),
                },
            );
        }
        Ok(())
    }

    pub fn air_private_input(&self, memory: &Memory) -> Vec<PrivateInput> {
        let mut private_inputs = vec![];

        // Collect and sort the signatures by their index before the loop
        let binding = self.signatures.borrow();
        let mut sorted_signatures: Vec<_> = binding.iter().collect();
        sorted_signatures.sort_by_key(|(addr, _)| {
            addr.offset
                .checked_div(CELLS_PER_SIGNATURE as usize)
                .unwrap_or_default()
        });

        for (addr, signature) in sorted_signatures {
            if let (Ok(pubkey), Some(msg)) = (
                memory.get_integer(*addr),
                (*addr + 1_usize)
                    .ok()
                    .and_then(|addr| memory.get_integer(addr).ok()),
            ) {
                private_inputs.push(PrivateInput::Signature(PrivateInputSignature {
                    index: addr
                        .offset
                        .checked_div(CELLS_PER_SIGNATURE as usize)
                        .unwrap_or_default(),
                    pubkey: *pubkey,
                    msg: *msg,
                    signature_input: SignatureInput {
                        r: Felt252::from_bytes_be(&signature.r.to_bytes_be()),
                        w: Felt252::from(
                            &div_mod(
                                &BigInt::one(),
                                &BigInt::from_bytes_be(Sign::Plus, &signature.s.to_bytes_be()),
                                &EC_ORDER,
                            )
                            .unwrap_or_default(),
                        ),
                    },
                }))
            }
        }
        private_inputs
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        relocatable,
        types::builtin_name::BuiltinName,
        utils::test_utils::*,
        vm::{
            errors::{
                memory_errors::{InsufficientAllocatedCellsError, MemoryError},
                runner_errors::RunnerError,
            },
            runners::builtin_runner::BuiltinRunner,
            vm_memory::{memory::Memory, memory_segments::MemorySegmentManager},
        },
    };

    use crate::felt_str;
    #[cfg(target_arch = "wasm32")]
    use wasm_bindgen_test::*;

    #[test]
    fn get_used_cells_and_allocated_size_valid() {
        let builtin: BuiltinRunner = SignatureBuiltinRunner::new(Some(10), true).into();
        let mut vm = vm!();
        vm.current_step = 110;
        vm.segments.segment_used_sizes = Some(vec![1]);
        assert_eq!(builtin.get_used_cells_and_allocated_size(&vm), Ok((1, 22)));
    }

    #[test]
    #[cfg_attr(target_arch = "wasm32", wasm_bindgen_test)]
    fn initialize_segments_for_ecdsa() {
        let mut builtin = SignatureBuiltinRunner::new(Some(512), true);
        let mut segments = MemorySegmentManager::new();
        builtin.initialize_segments(&mut segments);
        assert_eq!(builtin.base, 0);
    }

    #[test]
    #[cfg_attr(target_arch = "wasm32", wasm_bindgen_test)]
    fn get_used_instances() {
        let builtin: BuiltinRunner = SignatureBuiltinRunner::new(Some(512), true).into();

        let mut vm = vm!();
        vm.segments.segment_used_sizes = Some(vec![1]);

        assert_eq!(builtin.get_used_instances(&vm.segments), Ok(1));
    }

    #[test]
    #[cfg_attr(target_arch = "wasm32", wasm_bindgen_test)]
    fn final_stack() {
        let mut builtin: BuiltinRunner = SignatureBuiltinRunner::new(Some(512), true).into();

        let mut vm = vm!();

        vm.segments = segments![
            ((0, 0), (0, 0)),
            ((0, 1), (0, 1)),
            ((2, 0), (0, 0)),
            ((2, 1), (0, 0))
        ];

        vm.segments.segment_used_sizes = Some(vec![0]);

        let pointer = Relocatable::from((2, 2));

        assert_eq!(
            builtin.final_stack(&vm.segments, pointer).unwrap(),
            Relocatable::from((2, 1))
        );
    }

    #[test]
    #[cfg_attr(target_arch = "wasm32", wasm_bindgen_test)]
    fn final_stack_error_stop_pointer() {
        let mut builtin: BuiltinRunner = SignatureBuiltinRunner::new(Some(512), true).into();

        let mut vm = vm!();

        vm.segments = segments![
            ((0, 0), (0, 0)),
            ((0, 1), (0, 1)),
            ((2, 0), (0, 0)),
            ((2, 1), (0, 0))
        ];

        vm.segments.segment_used_sizes = Some(vec![998]);

        let pointer = Relocatable::from((2, 2));

        assert_eq!(
            builtin.final_stack(&vm.segments, pointer),
            Err(RunnerError::InvalidStopPointer(Box::new((
                BuiltinName::ecdsa,
                relocatable!(0, 998),
                relocatable!(0, 0)
            ))))
        );
    }

    #[test]
    #[cfg_attr(target_arch = "wasm32", wasm_bindgen_test)]
    fn final_stack_error_non_relocatable() {
        let mut builtin: BuiltinRunner = SignatureBuiltinRunner::new(Some(512), true).into();

        let mut vm = vm!();

        vm.segments = segments![
            ((0, 0), (0, 0)),
            ((0, 1), (0, 1)),
            ((2, 0), (0, 0)),
            ((2, 1), 2)
        ];

        vm.segments.segment_used_sizes = Some(vec![0]);

        let pointer = Relocatable::from((2, 2));

        assert_eq!(
            builtin.final_stack(&vm.segments, pointer),
            Err(RunnerError::NoStopPointer(Box::new(BuiltinName::ecdsa)))
        );
    }

    #[test]
    #[cfg_attr(target_arch = "wasm32", wasm_bindgen_test)]
    fn get_used_cells_missing_segment_used_sizes() {
        let builtin = BuiltinRunner::Signature(SignatureBuiltinRunner::new(Some(512), true));
        let vm = vm!();

        assert_eq!(
            builtin.get_used_cells(&vm.segments),
            Err(MemoryError::MissingSegmentUsedSizes)
        );
    }

    #[test]
    #[cfg_attr(target_arch = "wasm32", wasm_bindgen_test)]
    fn get_used_cells_empty() {
        let builtin = BuiltinRunner::Signature(SignatureBuiltinRunner::new(Some(512), true));
        let mut vm = vm!();

        vm.segments.segment_used_sizes = Some(vec![0]);
        assert_eq!(builtin.get_used_cells(&vm.segments), Ok(0));
    }

    #[test]
    #[cfg_attr(target_arch = "wasm32", wasm_bindgen_test)]
    fn get_used_cells() {
        let builtin = BuiltinRunner::Signature(SignatureBuiltinRunner::new(Some(512), true));
        let mut vm = vm!();

        vm.segments.segment_used_sizes = Some(vec![4]);
        assert_eq!(builtin.get_used_cells(&vm.segments), Ok(4));
    }

    #[test]
    #[cfg_attr(target_arch = "wasm32", wasm_bindgen_test)]
    fn get_initial_stack_for_range_check_with_base() {
        let mut builtin = SignatureBuiltinRunner::new(Some(512), true);
        builtin.base = 1;
        let initial_stack = builtin.initial_stack();
        assert_eq!(
            initial_stack[0].clone(),
            MaybeRelocatable::RelocatableValue((builtin.base() as isize, 0).into())
        );
        assert_eq!(initial_stack.len(), 1);
    }

    #[test]
    #[cfg_attr(target_arch = "wasm32", wasm_bindgen_test)]
    fn initial_stack_not_included_test() {
        let ecdsa_builtin = SignatureBuiltinRunner::new(Some(512), false);
        assert_eq!(ecdsa_builtin.initial_stack(), Vec::new())
    }

    #[test]
    #[cfg_attr(target_arch = "wasm32", wasm_bindgen_test)]
    fn deduce_memory_cell_test() {
        let memory = Memory::new();
        let builtin: BuiltinRunner = SignatureBuiltinRunner::new(Some(512), true).into();
        let result = builtin.deduce_memory_cell(Relocatable::from((0, 5)), &memory);
        assert_eq!(result, Ok(None));
    }

    #[test]
    #[cfg_attr(target_arch = "wasm32", wasm_bindgen_test)]
    fn test_ratio() {
        let builtin = SignatureBuiltinRunner::new(Some(512), true);
        assert_eq!(builtin.ratio(), Some(512));
    }

    #[test]
    #[cfg_attr(target_arch = "wasm32", wasm_bindgen_test)]
    fn test_base() {
        let builtin = SignatureBuiltinRunner::new(Some(512), true);
        assert_eq!(builtin.base(), 0);
    }

    #[test]
    #[cfg_attr(target_arch = "wasm32", wasm_bindgen_test)]
    fn get_allocated_memory_min_step_not_reached() {
        let builtin: BuiltinRunner = SignatureBuiltinRunner::new(Some(512), true).into();
        let mut vm = vm!();
        vm.current_step = 500;
        assert_eq!(
            builtin.get_allocated_memory_units(&vm),
            Err(MemoryError::InsufficientAllocatedCells(
                InsufficientAllocatedCellsError::MinStepNotReached(Box::new((
                    512,
                    BuiltinName::ecdsa
                )))
            ))
        )
    }

    #[test]
    #[cfg_attr(target_arch = "wasm32", wasm_bindgen_test)]
    fn get_used_cells_and_allocated_size_insufficient_allocated() {
        let builtin: BuiltinRunner = SignatureBuiltinRunner::new(Some(512), true).into();
        let mut vm = vm!();
        vm.segments.segment_used_sizes = Some(vec![50]);
        vm.current_step = 512;
        assert_eq!(
            builtin.get_used_cells_and_allocated_size(&vm),
            Err(MemoryError::InsufficientAllocatedCells(
                InsufficientAllocatedCellsError::BuiltinCells(Box::new((
                    BuiltinName::ecdsa,
                    50,
                    2
                )))
            ))
        )
    }

    #[test]
    #[cfg_attr(target_arch = "wasm32", wasm_bindgen_test)]
    fn final_stack_invalid_stop_pointer() {
        let mut builtin: BuiltinRunner = SignatureBuiltinRunner::new(Some(512), true).into();
        let mut vm = vm!();
        vm.segments = segments![((0, 0), (1, 0))];
        assert_eq!(
            builtin.final_stack(&vm.segments, (0, 1).into()),
            Err(RunnerError::InvalidStopPointerIndex(Box::new((
                BuiltinName::ecdsa,
                relocatable!(1, 0),
                0
            ))))
        )
    }

    #[test]
    #[cfg_attr(target_arch = "wasm32", wasm_bindgen_test)]
    fn final_stack_no_used_instances() {
        let mut builtin: BuiltinRunner = SignatureBuiltinRunner::new(Some(512), true).into();
        let mut vm = vm!();
        vm.segments = segments![((0, 0), (0, 0))];
        assert_eq!(
            builtin.final_stack(&vm.segments, (0, 1).into()),
            Err(RunnerError::Memory(MemoryError::MissingSegmentUsedSizes))
        )
    }

    #[test]
    fn get_additional_data() {
        let mut builtin = SignatureBuiltinRunner::new(Some(512), true);
        let signatures = HashMap::from([(
            Relocatable::from((4, 0)),
            Signature {
                r: Felt252::from_dec_str("45678").unwrap(),
                s: Felt252::from_dec_str("1239").unwrap(),
            },
        )]);
        builtin.signatures = Rc::new(RefCell::new(signatures));
        let signatures = HashMap::from([(
            Relocatable::from((4, 0)),
            (felt_str!("45678"), felt_str!("1239")),
        )]);
        assert_eq!(
            builtin.get_additional_data(),
            BuiltinAdditionalData::Signature(signatures)
        )
    }

    #[test]
    fn get_and_extend_additional_data() {
        let mut builtin_a = SignatureBuiltinRunner::new(Some(512), true);
        let signatures = HashMap::from([(
            Relocatable::from((0, 0)),
            Signature {
                r: Felt252::from_dec_str("45678").unwrap(),
                s: Felt252::from_dec_str("1239").unwrap(),
            },
        )]);
        builtin_a.signatures = Rc::new(RefCell::new(signatures));
        let additional_data = builtin_a.get_additional_data();
        let mut builtin_b = SignatureBuiltinRunner::new(Some(512), true);
        builtin_b.extend_additional_data(&additional_data).unwrap();
        // Signature doesn't implement PartialEq so we can't comapre the list of signatures directly
        let signatures_a = builtin_a.signatures.borrow();
        let signatures_b = builtin_b.signatures.borrow();
        assert_eq!(signatures_a.len(), signatures_b.len());
        for ((addr_a, signature_a), (addr_b, signature_b)) in
            signatures_a.iter().zip(signatures_b.iter())
        {
            assert_eq!(addr_a, addr_b);
            assert_eq!(signature_a.r, signature_b.r);
            assert_eq!(signature_a.s, signature_b.s);
        }
    }
    #[test]
    fn get_air_private_input() {
        let mut builtin = SignatureBuiltinRunner::new(Some(512), true);

        builtin.base = 0;

        let signature1_r = Felt252::from(1234);
        let signature1_s = Felt252::from(5678);
        let signature2_r = Felt252::from(8765);
        let signature2_s = Felt252::from(4321);

        let sig1_addr = Relocatable::from((builtin.base as isize, 0));
        let sig2_addr = Relocatable::from((builtin.base as isize, CELLS_PER_SIGNATURE as usize));

        builtin
            .add_signature(sig1_addr, &(signature1_r, signature1_s))
            .unwrap();
        builtin
            .add_signature(sig2_addr, &(signature2_r, signature2_s))
            .unwrap();

        let pubkey1 = Felt252::from(1111);
        let msg1 = Felt252::from(2222);
        let pubkey2 = Felt252::from(3333);
        let msg2 = Felt252::from(4444);

        let segments = segments![
            ((0, 0), 1111),
            ((0, 1), 2222),
            ((0, 2), 3333),
            ((0, 3), 4444)
        ];
        let w1 =
            Felt252::from(&div_mod(&BigInt::one(), &signature1_s.to_bigint(), &EC_ORDER).unwrap());

        let w2 =
            Felt252::from(&div_mod(&BigInt::one(), &signature2_s.to_bigint(), &EC_ORDER).unwrap());

        let expected_private_inputs = vec![
            PrivateInput::Signature(PrivateInputSignature {
                index: 0,
                pubkey: pubkey1,
                msg: msg1,
                signature_input: SignatureInput {
                    r: signature1_r,
                    w: w1,
                },
            }),
            PrivateInput::Signature(PrivateInputSignature {
                index: 1,
                pubkey: pubkey2,
                msg: msg2,
                signature_input: SignatureInput {
                    r: signature2_r,
                    w: w2,
                },
            }),
        ];

        let private_inputs = builtin.air_private_input(&segments.memory);

        assert_eq!(private_inputs, expected_private_inputs);
    }
}
