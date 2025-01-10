use std::collections::{BTreeMap, HashMap};

use cairo_vm::vm::trace::trace_entry::RelocatedTraceEntry;
use cairo_vm::{
    serde::deserialize_program::{DebugInfo, InstructionLocation},
    types::{
        instruction::Op1Addr,
        program::Program,
        relocatable::{MaybeRelocatable, Relocatable},
    },
    vm::{context::run_context::RunContext, decoding::decoder::decode_instruction},
    Felt252,
};
use num_bigint::BigUint;
use num_traits::ToPrimitive;

use crate::{error::trace_data_errors::TraceDataError, types::memory_access::MemoryAccess};

#[derive(Clone)]
pub struct InputCodeFile {
    content: String,
    lines: Vec<String>,
    tags: Vec<(usize, isize, String)>,
}

impl InputCodeFile {
    fn new(content: &str) -> Self {
        let lines: Vec<String> = content.lines().map(|line| line.to_string()).collect();
        InputCodeFile {
            content: content.to_string(),
            lines,
            tags: Vec::new(),
        }
    }

    fn mark_text(
        &mut self,
        line_start: usize,
        col_start: usize,
        line_end: usize,
        col_end: usize,
        classes: &[&str],
    ) {
        let offset_start = self
            .lines
            .iter()
            .take(line_start - 1)
            .map(|line| line.len())
            .sum::<usize>()
            + line_start
            + col_start
            - 2;

        let offset_end = self
            .lines
            .iter()
            .take(line_end - 1)
            .map(|line| line.len())
            .sum::<usize>()
            + line_end
            + col_end
            - 2;

        self.tags.push((
            offset_start,
            -(offset_end as isize),
            format!("<span class=\"{}\">", classes.join(" ")),
        ));
        self.tags
            .push((offset_end, -isize::MAX, "</span>".to_string()));
    }

    pub fn to_html(&self) -> String {
        let mut res = self.content.replace(' ', "\0");
        let mut sorted_tags = self.tags.clone();
        sorted_tags.sort_by_key(|&(key, _, _)| key);
        for &(pos, _, ref tag_content) in sorted_tags.iter().rev() {
            res.insert_str(pos, tag_content);
        }
        res.replace('\0', "&nbsp;").replace('\n', "<br/>\n")
    }
}

// TODO: add support for taking air_public_input as an argument
#[derive(Clone)]
pub struct TracerData {
    pub(crate) _program: Program,
    pub(crate) memory: Vec<Option<Felt252>>,
    pub(crate) trace: Vec<RelocatedTraceEntry>,
    pub(crate) _program_base: u64, // TODO: adjust size based on maximum instructions possible
    pub(crate) _debug_info: Option<DebugInfo>,
    pub(crate) memory_accesses: Vec<MemoryAccess>,
    pub(crate) input_files: HashMap<String, InputCodeFile>,
}

impl TracerData {
    pub fn new(
        program: Program,
        memory: Vec<Option<Felt252>>,
        trace: Vec<RelocatedTraceEntry>,
        program_base: u64,
        debug_info: Option<DebugInfo>,
    ) -> Result<TracerData, TraceDataError> {
        let mut input_files = HashMap::<String, InputCodeFile>::new();

        if let Some(debug_info) = debug_info.clone() {
            // loop over debug_info

            //sort hashmap by key
            let instruction_locations: BTreeMap<usize, InstructionLocation> =
                debug_info.get_instruction_locations().into_iter().collect();
            for (pc_offset, instruction_location) in instruction_locations.iter() {
                let loc = &instruction_location.inst;
                let filename = &loc.input_file.filename;
                let content = loc
                    .input_file
                    .get_content()
                    .map_err(|_| TraceDataError::FailedToReadFile(filename.clone()))?;
                if !input_files.contains_key(filename) {
                    input_files.insert(filename.clone(), InputCodeFile::new(content.as_str()));
                }
                let input_file = input_files.get_mut(filename);
                if input_file.is_none() {
                    return Err(TraceDataError::InputFileIsNone(filename.clone()));
                }
                let input_file = input_file.unwrap();

                input_file.mark_text(
                    loc.start_line as usize,
                    loc.start_col as usize,
                    loc.end_line as usize,
                    loc.end_col as usize,
                    &[format!("inst{}", pc_offset).as_str(), "instruction"],
                );
            }
        }

        let mut memory_accesses: Vec<MemoryAccess> = Vec::with_capacity(trace.len());
        //loop of trace
        for entry in trace.iter() {
            let run_context = RunContext::new(Relocatable::from((0, entry.pc)), entry.ap, entry.fp);

            let (instruction_encoding, _) =
                get_instruction_encoding(entry.pc, &memory, program.prime())?;

            let instruction_encoding = instruction_encoding.to_u64();
            if instruction_encoding.is_none() {
                return Err(TraceDataError::FailedToConvertInstructionEncoding);
            }
            let instruction_encoding = instruction_encoding.unwrap();
            let instruction = decode_instruction(instruction_encoding)?;

            // get dst_addr
            let dst_addr = run_context.compute_dst_addr(&instruction)?.offset;

            // get op0_addr
            let op0_addr = run_context.compute_op0_addr(&instruction)?.offset;

            // get op1_addr
            let mut op0: Result<Option<MaybeRelocatable>, TraceDataError> = Ok(None);
            if instruction.op1_addr == Op1Addr::Op0 {
                let op0_memory = &memory[op0_addr];
                op0 = match op0_memory {
                    None => Ok(None),
                    Some(felt) => {
                        let offset = felt.clone().to_usize();
                        if offset.is_none() {
                            return Err(TraceDataError::FailedToConvertOffset);
                        }
                        let offset = offset.unwrap();
                        Ok(Some(MaybeRelocatable::RelocatableValue(Relocatable {
                            segment_index: 1_isize,
                            offset,
                        })))
                    }
                };
            }
            let op0 = op0?;
            let op1_addr = run_context
                .compute_op1_addr(&instruction, op0.as_ref())?
                .offset;

            // add to memory access
            memory_accesses.push(MemoryAccess {
                dst: dst_addr,
                op0: op0_addr,
                op1: op1_addr,
            });
        }

        Ok(TracerData {
            _program: program,
            memory,
            trace,
            _program_base: program_base,
            _debug_info: debug_info,
            memory_accesses,
            input_files,
        })
    }
}

// Returns the encoded instruction (the value at pc) and the immediate value (the value at
// pc + 1, if it exists in the memory).
pub fn get_instruction_encoding(
    pc: usize,
    memory: &[Option<Felt252>],
    prime: &str,
) -> Result<(Felt252, Option<Felt252>), TraceDataError> {
    if memory[pc].is_none() {
        return Err(TraceDataError::InstructionIsNone(pc.to_string()));
    }
    let instruction_encoding = memory[pc].unwrap();
    let prime = BigUint::parse_bytes(prime[2..].as_bytes(), 16).unwrap();

    let imm_addr = BigUint::from(pc + 1) % prime;
    let imm_addr = usize::try_from(imm_addr.clone())
        .map_err(|_| TraceDataError::FailedToImmAddress(imm_addr.to_string()))?;
    let optional_imm = memory[imm_addr];
    Ok((instruction_encoding, optional_imm))
}
