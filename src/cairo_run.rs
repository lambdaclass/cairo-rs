use crate::{
    hint_processor::hint_processor_definition::HintProcessor,
    types::program::Program,
    vm::{
        errors::{cairo_run_errors::CairoRunError, vm_exception::VmException},
        runners::cairo_runner::CairoRunner,
        security::verify_secure_runner,
        vm_core::VirtualMachine,
    },
    write::Writer,
};

use felt::Felt252;

#[cfg(feature = "std")]
use thiserror::Error;
#[cfg(not(feature = "std"))]
use thiserror_no_std::Error;

pub struct CairoRunConfig<'a> {
    pub entrypoint: &'a str,
    pub trace_enabled: bool,
    pub relocate_mem: bool,
    pub layout: &'a str,
    pub proof_mode: bool,
    pub secure_run: Option<bool>,
}

impl<'a> Default for CairoRunConfig<'a> {
    fn default() -> Self {
        CairoRunConfig {
            entrypoint: "main",
            trace_enabled: false,
            relocate_mem: false,
            layout: "plain",
            proof_mode: false,
            secure_run: None,
        }
    }
}

pub fn cairo_run(
    program_content: &[u8],
    cairo_run_config: &CairoRunConfig,
    hint_executor: &mut dyn HintProcessor,
) -> Result<(CairoRunner, VirtualMachine), CairoRunError> {
    let program = Program::from_bytes(program_content, Some(cairo_run_config.entrypoint))?;

    let secure_run = cairo_run_config
        .secure_run
        .unwrap_or(!cairo_run_config.proof_mode);

    let mut cairo_runner = CairoRunner::new(
        &program,
        cairo_run_config.layout,
        cairo_run_config.proof_mode,
    )?;

    let mut vm = VirtualMachine::new(cairo_run_config.trace_enabled);
    let end = cairo_runner.initialize(&mut vm)?;
    // check step calculation
    let mut run_resources = None;

    cairo_runner
        .run_until_pc(end, &mut run_resources, &mut vm, hint_executor)
        .map_err(|err| VmException::from_vm_error(&cairo_runner, &vm, err))?;
    cairo_runner.end_run(false, false, &mut vm, hint_executor)?;

    vm.verify_auto_deductions()?;
    cairo_runner.read_return_values(&mut vm)?;
    if cairo_run_config.proof_mode {
        cairo_runner.finalize_segments(&mut vm)?;
    }
    if secure_run {
        verify_secure_runner(&cairo_runner, true, None, &mut vm)?;
    }
    cairo_runner.relocate(&mut vm, cairo_run_config.relocate_mem)?;

    Ok((cairo_runner, vm))
}

#[derive(Debug, Error)]
#[error("Failed to encode trace at position {0}, serialize error: {1}")]
pub struct EncodeTraceError(usize, bincode::error::EncodeError);

/// Writes the trace binary representation.
///
/// Bincode encodes to little endian by default and each trace entry is composed of
/// 3 usize values that are padded to always reach 64 bit size.
pub fn write_encoded_trace(
    relocated_trace: &[crate::vm::trace::trace_entry::TraceEntry],
    dest: &mut impl Writer,
) -> Result<(), EncodeTraceError> {
    for (i, entry) in relocated_trace.iter().enumerate() {
        dest.write(&((entry.ap as u64).to_le_bytes()))
            .map_err(|e| EncodeTraceError(i, e))?;
        dest.write(&((entry.fp as u64).to_le_bytes()))
            .map_err(|e| EncodeTraceError(i, e))?;
        dest.write(&((entry.pc as u64).to_le_bytes()))
            .map_err(|e| EncodeTraceError(i, e))?;
    }

    Ok(())
}

/// Writes a binary representation of the relocated memory.
///
/// The memory pairs (address, value) are encoded and concatenated:
/// * address -> 8-byte encoded
/// * value -> 32-byte encoded
pub fn write_encoded_memory(
    relocated_memory: &[Option<Felt252>],
    dest: &mut impl Writer,
) -> Result<(), EncodeTraceError> {
    for (i, memory_cell) in relocated_memory.iter().enumerate() {
        match memory_cell {
            None => continue,
            Some(unwrapped_memory_cell) => {
                dest.write(&(i as u64).to_le_bytes())
                    .map_err(|e| EncodeTraceError(i, e))?;
                dest.write(&unwrapped_memory_cell.to_le_bytes())
                    .map_err(|e| EncodeTraceError(i, e))?;
            }
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::stdlib::prelude::*;
    use crate::{
        hint_processor::{
            builtin_hint_processor::builtin_hint_processor_definition::BuiltinHintProcessor,
            hint_processor_definition::HintProcessor,
        },
        utils::test_utils::*,
    };
    use bincode::enc::write::SliceWriter;
    use felt::Felt252;

    #[cfg(target_arch = "wasm32")]
    use wasm_bindgen_test::*;

    fn run_test_program(
        program_content: &[u8],
        hint_processor: &mut dyn HintProcessor,
    ) -> Result<(CairoRunner, VirtualMachine), CairoRunError> {
        let program = Program::from_bytes(program_content, Some("main")).unwrap();
        let mut cairo_runner = cairo_runner!(program);
        let mut vm = vm!(true);
        let end = cairo_runner
            .initialize(&mut vm)
            .map_err(CairoRunError::Runner)?;

        assert!(cairo_runner
            .run_until_pc(end, &mut None, &mut vm, hint_processor)
            .is_ok());

        Ok((cairo_runner, vm))
    }

    #[test]
    #[cfg_attr(target_arch = "wasm32", wasm_bindgen_test)]
    fn cairo_run_custom_entry_point() {
        let program = Program::from_bytes(
            include_bytes!("../cairo_programs/not_main.json"),
            Some("not_main"),
        )
        .unwrap();
        let mut vm = vm!();
        let mut hint_processor = BuiltinHintProcessor::new_empty();
        let mut cairo_runner = cairo_runner!(program);

        let end = cairo_runner.initialize(&mut vm).unwrap();
        assert!(cairo_runner
            .run_until_pc(end, &mut None, &mut vm, &mut hint_processor)
            .is_ok());
        assert!(cairo_runner.relocate(&mut vm, true).is_ok());
        // `main` returns without doing nothing, but `not_main` sets `[ap]` to `1`
        // Memory location was found empirically and simply hardcoded
        assert_eq!(cairo_runner.relocated_memory[2], Some(Felt252::new(123)));
    }

    #[test]
    #[cfg_attr(target_arch = "wasm32", wasm_bindgen_test)]
    fn cairo_run_with_no_data_program() {
        // a compiled program with no `data` key.
        // it should fail when the program is loaded.
        let mut hint_processor = BuiltinHintProcessor::new_empty();
        let no_data_program_path =
            include_bytes!("../cairo_programs/manually_compiled/no_data_program.json");
        let cairo_run_config = CairoRunConfig::default();
        assert!(cairo_run(no_data_program_path, &cairo_run_config, &mut hint_processor,).is_err());
    }

    #[test]
    #[cfg_attr(target_arch = "wasm32", wasm_bindgen_test)]
    fn cairo_run_with_no_main_program() {
        // a compiled program with no main scope
        // it should fail when trying to run initialize_main_entrypoint.
        let mut hint_processor = BuiltinHintProcessor::new_empty();
        let no_main_program =
            include_bytes!("../cairo_programs/manually_compiled/no_main_program.json");
        let cairo_run_config = CairoRunConfig::default();
        assert!(cairo_run(no_main_program, &cairo_run_config, &mut hint_processor,).is_err());
    }

    #[test]
    #[cfg_attr(target_arch = "wasm32", wasm_bindgen_test)]
    fn cairo_run_with_invalid_memory() {
        // the program invalid_memory.json has an invalid memory cell and errors when trying to
        // decode the instruction.
        let mut hint_processor = BuiltinHintProcessor::new_empty();
        let invalid_memory =
            include_bytes!("../cairo_programs/manually_compiled/invalid_memory.json");
        let cairo_run_config = CairoRunConfig::default();
        assert!(cairo_run(invalid_memory, &cairo_run_config, &mut hint_processor,).is_err());
    }

    #[test]
    #[cfg_attr(target_arch = "wasm32", wasm_bindgen_test)]
    fn write_output_program() {
        let program_content = include_bytes!("../cairo_programs/bitwise_output.json");
        let mut hint_processor = BuiltinHintProcessor::new_empty();
        let (_, mut vm) = run_test_program(program_content, &mut hint_processor)
            .expect("Couldn't initialize cairo runner");

        let mut output_buffer = String::new();
        vm.write_output(&mut output_buffer).unwrap();
        assert_eq!(&output_buffer, "0\n");
    }

    #[test]
    #[cfg_attr(target_arch = "wasm32", wasm_bindgen_test)]
    fn write_binary_trace_file() {
        let program_content = include_bytes!("../cairo_programs/struct.json");
        let expected_encoded_trace =
            include_bytes!("../cairo_programs/trace_memory/cairo_trace_struct");

        // run test program until the end
        let mut hint_processor = BuiltinHintProcessor::new_empty();
        let cairo_runner_result = run_test_program(program_content, &mut hint_processor);
        let (mut cairo_runner, mut vm) = cairo_runner_result.unwrap();

        // relocate memory so we can dump it to file
        assert!(cairo_runner.relocate(&mut vm, false).is_ok());

        let trace_entries = vm.get_relocated_trace().unwrap();
        let mut buffer = [0; 24];
        let mut buff_writer = SliceWriter::new(&mut buffer);
        // write cairo_rs vm trace file
        write_encoded_trace(trace_entries, &mut buff_writer).unwrap();

        // compare that the original cairo vm trace file and cairo_rs vm trace files are equal
        assert_eq!(buffer, *expected_encoded_trace);
    }

    #[test]
    #[cfg_attr(target_arch = "wasm32", wasm_bindgen_test)]
    fn write_binary_memory_file() {
        let program_content = include_bytes!("../cairo_programs/struct.json");
        let expected_encoded_memory =
            include_bytes!("../cairo_programs/trace_memory/cairo_memory_struct");

        // run test program until the end
        let mut hint_processor = BuiltinHintProcessor::new_empty();
        let cairo_runner_result = run_test_program(program_content, &mut hint_processor);
        let (mut cairo_runner, mut vm) = cairo_runner_result.unwrap();

        // relocate memory so we can dump it to file
        assert!(cairo_runner.relocate(&mut vm, true).is_ok());

        let mut buffer = [0; 120];
        let mut buff_writer = SliceWriter::new(&mut buffer);
        // write cairo_rs vm memory file
        write_encoded_memory(&cairo_runner.relocated_memory, &mut buff_writer).unwrap();

        // compare that the original cairo vm memory file and cairo_rs vm memory files are equal
        assert_eq!(*expected_encoded_memory, buffer);
    }

    #[test]
    #[cfg_attr(target_arch = "wasm32", wasm_bindgen_test)]
    fn run_with_no_trace() {
        let program = Program::from_bytes(
            include_bytes!("../cairo_programs/struct.json"),
            Some("main"),
        )
        .unwrap();

        let mut hint_processor = BuiltinHintProcessor::new_empty();
        let mut cairo_runner = cairo_runner!(program);
        let mut vm = vm!();
        let end = cairo_runner.initialize(&mut vm).unwrap();
        assert!(cairo_runner
            .run_until_pc(end, &mut None, &mut vm, &mut hint_processor)
            .is_ok());
        assert!(cairo_runner.relocate(&mut vm, false).is_ok());
        assert!(vm.get_relocated_trace().is_err());
    }
}
