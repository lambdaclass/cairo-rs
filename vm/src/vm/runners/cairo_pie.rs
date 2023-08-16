use crate::felt::Felt252;
use crate::types::program::StrippedProgram;
use crate::types::relocatable::{MaybeRelocatable, Relocatable};
use std::collections::HashMap;

use super::cairo_runner::ExecutionResources;

#[derive(PartialEq, Clone, Debug)]
pub struct SegmentInfo {
    pub index: isize,
    pub size: usize,
}

impl From<(isize, usize)> for SegmentInfo {
    fn from(value: (isize, usize)) -> Self {
        SegmentInfo {
            index: value.0,
            size: value.1,
        }
    }
}

// A simplified version of Memory, without any additional data besides its elements
// Contains all addr-value pairs, ordered by index and offset
// Allows practical serialization + conversion between CairoPieMemory & Memory
pub type CairoPieMemory = Vec<((usize, usize), MaybeRelocatable)>;

#[derive(PartialEq, Clone, Debug)]
pub struct PublicMemoryPage {
    pub start: usize,
    pub size: usize,
}

// Hashmap value based on starknet/core/os/output.cairo usage
pub type Attributes = HashMap<String, Vec<usize>>;
pub type Pages = HashMap<usize, PublicMemoryPage>;

#[derive(PartialEq, Clone, Debug)]
pub struct OutputBuiltinAdditionalData {
    pub pages: Pages,
    pub attributes: Attributes,
}

#[derive(PartialEq, Clone, Debug)]
pub enum BuiltinAdditionalData {
    // Contains verified addresses as contiguous index, value pairs
    Hash(Vec<usize>),
    Output(OutputBuiltinAdditionalData),
    // Signatures are composed of (r, s) tuples
    Signature(HashMap<Relocatable, (Felt252, Felt252)>),
    None,
}

pub struct CairoPie {
    pub metadata: CairoPieMetadata,
    pub memory: CairoPieMemory,
    pub execution_resources: ExecutionResources,
    pub additional_data: HashMap<String, BuiltinAdditionalData>,
}

pub struct CairoPieMetadata {
    pub program: StrippedProgram,
    pub program_segment: SegmentInfo,
    pub execution_segment: SegmentInfo,
    pub ret_fp_segment: SegmentInfo,
    pub ret_pc_segment: SegmentInfo,
    pub builtin_segments: HashMap<String, SegmentInfo>,
    pub extra_segments: Vec<SegmentInfo>,
}