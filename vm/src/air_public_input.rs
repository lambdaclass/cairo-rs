use std::collections::HashMap;

use felt::Felt252;
use serde::Serialize;
use thiserror_no_std::Error;

use crate::{
    types::layout::CairoLayout,
    vm::{
        errors::{trace_errors::TraceError, vm_errors::VirtualMachineError},
        trace::trace_entry::TraceEntry,
    },
};

#[derive(Serialize, Debug)]
pub struct PublicMemoryEntry {
    address: usize,
    page: usize,
    #[serde(serialize_with = "mem_value_serde::serialize")]
    value: Option<Felt252>,
}

mod mem_value_serde {
    use super::*;
    use serde::Serializer;

    pub(crate) fn serialize<S: Serializer>(
        value: &Option<Felt252>,
        serializer: S,
    ) -> Result<S::Ok, S::Error> {
        if let Some(value) = value {
            serializer.serialize_str(&format!("0x{}", value.to_str_radix(16)))
        } else {
            serializer.serialize_none()
        }
    }
}

#[derive(Serialize, Debug)]
pub struct PublicInput<'a> {
    layout: &'a str,
    layout_params: Option<&'a CairoLayout>,
    rc_min: isize,
    rc_max: isize,
    n_steps: usize,
    memory_segments: HashMap<&'a str, (usize, usize)>,
    public_memory: Vec<PublicMemoryEntry>,
}

impl<'a> PublicInput<'a> {
    pub fn new(
        memory: &[Option<Felt252>],
        layout: &'a str,
        dyn_layout_params: Option<&'a CairoLayout>,
        public_memory_addresses: &[(usize, usize)],
        memory_segment_addresses: HashMap<&'static str, (usize, usize)>,
        trace: &[TraceEntry],
        rc_limits: (isize, isize),
    ) -> Result<Self, PublicInputError> {
        let memory_entry =
            |addresses: &(usize, usize)| -> Result<PublicMemoryEntry, PublicInputError> {
                let (address, page) = addresses;
                Ok(PublicMemoryEntry {
                    address: *address,
                    page: *page,
                    value: memory
                        .get(*address)
                        .ok_or(PublicInputError::MemoryNotFound(*address))?
                        .clone(),
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
            layout_params: dyn_layout_params,
            rc_min,
            rc_max,
            n_steps: trace.len(),
            memory_segments: {
                let mut memory_segment_addresses = memory_segment_addresses.clone();
                memory_segment_addresses.insert("program", (trace_first.pc, trace_last.pc));
                memory_segment_addresses.insert("execution", (trace_first.ap, trace_last.ap));
                memory_segment_addresses
            },
            public_memory,
        })
    }

    pub fn write(&self, file_path: &str) -> Result<(), PublicInputError> {
        std::fs::write(file_path, serde_json::to_string_pretty(&self)?)?;
        Ok(())
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
    #[error("Failed to interact with the file system")]
    IO(#[from] std::io::Error),
    #[error("Failed to (de)serialize data")]
    Serde(#[from] serde_json::Error),
    #[error(transparent)]
    VirtualMachine(#[from] VirtualMachineError),
    #[error(transparent)]
    Trace(#[from] TraceError),
}
