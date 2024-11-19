use crate::Felt252;
use serde::{Deserialize, Serialize};
use thiserror_no_std::Error;

use crate::{
    stdlib::{
        collections::HashMap,
        prelude::{String, Vec},
    },
    vm::{
        errors::{trace_errors::TraceError, vm_errors::VirtualMachineError},
        trace::trace_entry::RelocatedTraceEntry,
    },
};

#[derive(Serialize, Deserialize, Debug, PartialEq)]
pub struct PublicMemoryEntry {
    pub address: usize,
    #[serde(serialize_with = "mem_value_serde::serialize")]
    #[serde(deserialize_with = "mem_value_serde::deserialize")]
    pub value: Option<Felt252>,
    pub page: usize,
}

mod mem_value_serde {
    use core::fmt;

    use super::*;

    use serde::{de, Deserializer, Serializer};

    pub(crate) fn serialize<S: Serializer>(
        value: &Option<Felt252>,
        serializer: S,
    ) -> Result<S::Ok, S::Error> {
        if let Some(value) = value {
            serializer.serialize_str(&format!("0x{:x}", value))
        } else {
            serializer.serialize_none()
        }
    }

    pub(crate) fn deserialize<'de, D: Deserializer<'de>>(
        d: D,
    ) -> Result<Option<Felt252>, D::Error> {
        d.deserialize_str(Felt252OptionVisitor)
    }

    struct Felt252OptionVisitor;

    impl<'de> de::Visitor<'de> for Felt252OptionVisitor {
        type Value = Option<Felt252>;

        fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
            formatter.write_str("Could not deserialize hexadecimal string")
        }

        fn visit_none<E>(self) -> Result<Self::Value, E>
        where
            E: de::Error,
        {
            Ok(None)
        }

        fn visit_str<E>(self, value: &str) -> Result<Self::Value, E>
        where
            E: de::Error,
        {
            let value = value.strip_prefix("0x").unwrap_or(value);
            Felt252::from_hex(value)
                .map_err(de::Error::custom)
                .map(Some)
        }
    }
}

#[derive(Serialize, Deserialize, Debug, PartialEq)]
pub struct MemorySegmentAddresses {
    pub begin_addr: usize,
    pub stop_ptr: usize,
}

impl From<(usize, usize)> for MemorySegmentAddresses {
    fn from(addresses: (usize, usize)) -> Self {
        let (begin_addr, stop_ptr) = addresses;
        MemorySegmentAddresses {
            begin_addr,
            stop_ptr,
        }
    }
}

#[allow(clippy::manual_non_exhaustive)]
#[derive(Serialize, Deserialize, Debug)]
pub struct PublicInput<'a> {
    pub layout: &'a str,
    pub rc_min: isize,
    pub rc_max: isize,
    pub n_steps: usize,
    pub memory_segments: HashMap<&'a str, MemorySegmentAddresses>,
    pub public_memory: Vec<PublicMemoryEntry>,
    #[serde(skip_deserializing)] // This is set to None by default so we can skip it
    dynamic_params: (),
}

impl<'a> PublicInput<'a> {
    pub fn new(
        memory: &[Option<Felt252>],
        layout: &'a str,
        public_memory_addresses: &[(usize, usize)],
        memory_segment_addresses: HashMap<&'static str, (usize, usize)>,
        trace: &[RelocatedTraceEntry],
        rc_limits: (isize, isize),
    ) -> Result<Self, PublicInputError> {
        let memory_entry =
            |addresses: &(usize, usize)| -> Result<PublicMemoryEntry, PublicInputError> {
                let (address, page) = addresses;
                Ok(PublicMemoryEntry {
                    address: *address,
                    page: *page,
                    value: *memory
                        .get(*address)
                        .ok_or(PublicInputError::MemoryNotFound(*address))?,
                })
            };
        let public_memory = public_memory_addresses
            .iter()
            .map(memory_entry)
            .collect::<Result<Vec<_>, _>>()?;

        let (rc_min, rc_max) = rc_limits;

        let trace_first = trace.first().ok_or(PublicInputError::EmptyTrace)?;
        let trace_last = trace.last().ok_or(PublicInputError::EmptyTrace)?;

        Ok(PublicInput {
            layout,
            dynamic_params: (),
            rc_min,
            rc_max,
            n_steps: trace.len(),
            memory_segments: {
                let mut memory_segment_addresses = memory_segment_addresses
                    .into_iter()
                    .map(|(n, s)| (n, s.into()))
                    .collect::<HashMap<_, MemorySegmentAddresses>>();

                memory_segment_addresses.insert("program", (trace_first.pc, trace_last.pc).into());
                memory_segment_addresses
                    .insert("execution", (trace_first.ap, trace_last.ap).into());
                memory_segment_addresses
            },
            public_memory,
        })
    }

    pub fn serialize_json(&self) -> Result<String, PublicInputError> {
        serde_json::to_string_pretty(&self).map_err(PublicInputError::from)
    }
}

#[derive(Debug, Error)]
pub enum PublicInputError {
    #[error("The trace slice provided is empty")]
    EmptyTrace,
    #[error("The provided memory doesn't contain public address {0}")]
    MemoryNotFound(usize),
    #[error("Range check values are missing")]
    NoRangeCheckLimits,
    #[error("Failed to (de)serialize data")]
    Serde(#[from] serde_json::Error),
    #[error(transparent)]
    VirtualMachine(#[from] VirtualMachineError),
    #[error(transparent)]
    Trace(#[from] TraceError),
}
#[cfg(test)]
mod tests {
    #[cfg(feature = "std")]
    use super::*;
    #[cfg(feature = "std")]
    use rstest::rstest;

    #[cfg(feature = "std")]
    #[rstest]
    #[case(include_bytes!("../../cairo_programs/proof_programs/fibonacci.json"))]
    #[case(include_bytes!("../../cairo_programs/proof_programs/bitwise_output.json"))]
    #[case(include_bytes!("../../cairo_programs/proof_programs/keccak_builtin.json"))]
    #[case(include_bytes!("../../cairo_programs/proof_programs/poseidon_builtin.json"))]
    #[case(include_bytes!("../../cairo_programs/proof_programs/relocate_temporary_segment_append.json"))]
    #[case(include_bytes!("../../cairo_programs/proof_programs/pedersen_test.json"))]
    #[case(include_bytes!("../../cairo_programs/proof_programs/ec_op.json"))]
    fn serialize_and_deserialize_air_public_input(#[case] program_content: &[u8]) {
        use crate::types::layout_name::LayoutName;

        let config = crate::cairo_run::CairoRunConfig {
            proof_mode: true,
            relocate_mem: true,
            trace_enabled: true,
            layout: LayoutName::all_cairo,
            ..Default::default()
        };
        let runner = crate::cairo_run::cairo_run(program_content, &config, &mut crate::hint_processor::builtin_hint_processor::builtin_hint_processor_definition::BuiltinHintProcessor::new_empty()).unwrap();
        let public_input = runner.get_air_public_input().unwrap();
        // We already know serialization works as expected due to the comparison against python VM
        let serialized_public_input = public_input.serialize_json().unwrap();
        let deserialized_public_input: PublicInput =
            serde_json::from_str(&serialized_public_input).unwrap();
        // Check that the deserialized public input is equal to the one we obtained from the vm first
        assert_eq!(public_input.layout, deserialized_public_input.layout);
        assert_eq!(public_input.rc_max, deserialized_public_input.rc_max);
        assert_eq!(public_input.rc_min, deserialized_public_input.rc_min);
        assert_eq!(public_input.n_steps, deserialized_public_input.n_steps);
        assert_eq!(
            public_input.memory_segments,
            deserialized_public_input.memory_segments
        );
        assert_eq!(
            public_input.public_memory,
            deserialized_public_input.public_memory
        );
    }
}
