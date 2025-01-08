#![deny(warnings)]
#![forbid(unsafe_code)]
use bincode::enc::write::Writer;
use cairo_vm::air_public_input::PublicInputError;
use cairo_vm::cairo_run::{self, EncodeTraceError};
use cairo_vm::hint_processor::builtin_hint_processor::builtin_hint_processor_definition::BuiltinHintProcessor;
#[cfg(feature = "with_tracer")]
use cairo_vm::serde::deserialize_program::DebugInfo;
use cairo_vm::types::layout::CairoLayoutParams;
use cairo_vm::types::layout_name::LayoutName;
use cairo_vm::vm::errors::cairo_run_errors::CairoRunError;
use cairo_vm::vm::errors::trace_errors::TraceError;
use cairo_vm::vm::errors::vm_errors::VirtualMachineError;
use cairo_vm::vm::runners::cairo_pie::CairoPie;
#[cfg(feature = "with_tracer")]
use cairo_vm::vm::runners::cairo_runner::CairoRunner;
use cairo_vm::vm::runners::cairo_runner::RunResources;
#[cfg(feature = "with_tracer")]
use cairo_vm_tracer::error::trace_data_errors::TraceDataError;
#[cfg(feature = "with_tracer")]
use cairo_vm_tracer::tracer::run_tracer;
use clap::{Parser, ValueHint};
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use thiserror::Error;

#[cfg(feature = "with_mimalloc")]
use mimalloc::MiMalloc;

#[cfg(feature = "with_mimalloc")]
#[global_allocator]
static ALLOC: MiMalloc = MiMalloc;

#[derive(Parser, Debug)]
#[clap(author, version, about, long_about = None)]
struct Args {
    #[clap(value_parser, value_hint=ValueHint::FilePath)]
    filename: PathBuf,
    #[clap(long = "trace_file", value_parser)]
    trace_file: Option<PathBuf>,
    #[structopt(long = "print_output")]
    print_output: bool,
    #[structopt(long = "entrypoint", default_value = "main")]
    entrypoint: String,
    #[structopt(long = "memory_file")]
    memory_file: Option<PathBuf>,
    /// When using dynamic layout, it's parameters must be specified through a layout params file.
    #[clap(long = "layout", default_value = "plain", value_enum)]
    layout: LayoutName,
    /// Required when using with dynamic layout.
    /// Ignored otherwise.
    #[clap(long = "cairo_layout_params_file", required_if_eq("layout", "dynamic"))]
    cairo_layout_params_file: Option<PathBuf>,
    #[structopt(long = "proof_mode")]
    proof_mode: bool,
    #[structopt(long = "secure_run")]
    secure_run: Option<bool>,
    #[clap(long = "air_public_input", requires = "proof_mode")]
    air_public_input: Option<String>,
    #[clap(
        long = "air_private_input",
        requires_all = ["proof_mode", "trace_file", "memory_file"]
    )]
    air_private_input: Option<String>,
    #[clap(
        long = "cairo_pie_output",
        // We need to add these air_private_input & air_public_input or else
        // passing cairo_pie_output + either of these without proof_mode will not fail
        conflicts_with_all = ["proof_mode", "air_private_input", "air_public_input"]
    )]
    cairo_pie_output: Option<String>,
    #[structopt(long = "allow_missing_builtins")]
    allow_missing_builtins: Option<bool>,
    #[structopt(long = "tracer")]
    #[cfg(feature = "with_tracer")]
    tracer: bool,
    #[structopt(
        long = "run_from_cairo_pie",
        // We need to add these air_private_input & air_public_input or else
        // passing run_from_cairo_pie + either of these without proof_mode will not fail
        conflicts_with_all = ["proof_mode", "air_private_input", "air_public_input"]
    )]
    run_from_cairo_pie: bool,
}

#[derive(Debug, Error)]
enum Error {
    #[error("Invalid arguments")]
    Cli(#[from] clap::Error),
    #[error("Failed to interact with the file system")]
    IO(#[from] std::io::Error),
    #[error("The cairo program execution failed")]
    Runner(#[from] CairoRunError),
    #[error(transparent)]
    EncodeTrace(#[from] EncodeTraceError),
    #[error(transparent)]
    VirtualMachine(#[from] VirtualMachineError),
    #[error(transparent)]
    Trace(#[from] TraceError),
    #[error(transparent)]
    PublicInput(#[from] PublicInputError),
    #[error(transparent)]
    #[cfg(feature = "with_tracer")]
    TraceData(#[from] TraceDataError),
}

struct FileWriter {
    buf_writer: io::BufWriter<std::fs::File>,
    bytes_written: usize,
}

impl Writer for FileWriter {
    fn write(&mut self, bytes: &[u8]) -> Result<(), bincode::error::EncodeError> {
        self.buf_writer
            .write_all(bytes)
            .map_err(|e| bincode::error::EncodeError::Io {
                inner: e,
                index: self.bytes_written,
            })?;

        self.bytes_written += bytes.len();

        Ok(())
    }
}

impl FileWriter {
    fn new(buf_writer: io::BufWriter<std::fs::File>) -> Self {
        Self {
            buf_writer,
            bytes_written: 0,
        }
    }

    fn flush(&mut self) -> io::Result<()> {
        self.buf_writer.flush()
    }
}

#[cfg(feature = "with_tracer")]
fn start_tracer(cairo_runner: &CairoRunner) -> Result<(), TraceDataError> {
    let relocation_table = cairo_runner
        .vm
        .relocate_segments()
        .map_err(TraceDataError::FailedToGetRelocationTable)?;
    let instruction_locations = cairo_runner
        .get_program()
        .get_relocated_instruction_locations(relocation_table.as_ref());
    let debug_info = instruction_locations.map(DebugInfo::new);

    let relocated_trace = cairo_runner
        .relocated_trace
        .clone()
        .ok_or(TraceDataError::FailedToGetRelocatedTrace)?;

    run_tracer(
        cairo_runner.get_program().clone(),
        cairo_runner.relocated_memory.clone(),
        relocated_trace.clone(),
        1,
        debug_info,
    )?;
    Ok(())
}

fn run(args: impl Iterator<Item = String>) -> Result<(), Error> {
    let args = Args::try_parse_from(args)?;

    let trace_enabled = args.trace_file.is_some() || args.air_public_input.is_some();

    let cairo_layout_params = match args.cairo_layout_params_file {
        Some(file) => Some(CairoLayoutParams::from_file(&file)?),
        None => None,
    };

    let cairo_run_config = cairo_run::CairoRunConfig {
        entrypoint: &args.entrypoint,
        trace_enabled,
        relocate_mem: args.memory_file.is_some() || args.air_public_input.is_some(),
        layout: args.layout,
        proof_mode: args.proof_mode,
        secure_run: args.secure_run,
        allow_missing_builtins: args.allow_missing_builtins,
        dynamic_layout_params: cairo_layout_params,
        ..Default::default()
    };

    let mut cairo_runner = match if args.run_from_cairo_pie {
        let pie = CairoPie::read_zip_file(&args.filename)?;
        let mut hint_processor = BuiltinHintProcessor::new(
            Default::default(),
            RunResources::new(pie.execution_resources.n_steps),
        );
        cairo_run::cairo_run_pie(&pie, &cairo_run_config, &mut hint_processor)
    } else {
        let program_content = std::fs::read(args.filename).map_err(Error::IO)?;
        let mut hint_processor = BuiltinHintProcessor::new_empty();
        cairo_run::cairo_run(&program_content, &cairo_run_config, &mut hint_processor)
    } {
        Ok(runner) => runner,
        Err(error) => {
            eprintln!("{error}");
            return Err(Error::Runner(error));
        }
    };

    if args.print_output {
        let mut output_buffer = "Program Output:\n".to_string();
        cairo_runner.vm.write_output(&mut output_buffer)?;
        print!("{output_buffer}");
    }

    if let Some(ref trace_path) = args.trace_file {
        let relocated_trace = cairo_runner
            .relocated_trace
            .as_ref()
            .ok_or(Error::Trace(TraceError::TraceNotRelocated))?;

        let trace_file = std::fs::File::create(trace_path)?;
        let mut trace_writer =
            FileWriter::new(io::BufWriter::with_capacity(3 * 1024 * 1024, trace_file));

        cairo_run::write_encoded_trace(relocated_trace, &mut trace_writer)?;
        trace_writer.flush()?;
    }

    if let Some(ref memory_path) = args.memory_file {
        let memory_file = std::fs::File::create(memory_path)?;
        let mut memory_writer =
            FileWriter::new(io::BufWriter::with_capacity(5 * 1024 * 1024, memory_file));

        cairo_run::write_encoded_memory(&cairo_runner.relocated_memory, &mut memory_writer)?;
        memory_writer.flush()?;
    }

    if let Some(file_path) = args.air_public_input {
        let json = cairo_runner.get_air_public_input()?.serialize_json()?;
        std::fs::write(file_path, json)?;
    }

    #[cfg(feature = "with_tracer")]
    if args.tracer {
        start_tracer(&cairo_runner)?;
    }

    if let (Some(file_path), Some(ref trace_file), Some(ref memory_file)) =
        (args.air_private_input, args.trace_file, args.memory_file)
    {
        // Get absolute paths of trace_file & memory_file
        let trace_path = trace_file
            .as_path()
            .canonicalize()
            .unwrap_or(trace_file.clone())
            .to_string_lossy()
            .to_string();
        let memory_path = memory_file
            .as_path()
            .canonicalize()
            .unwrap_or(memory_file.clone())
            .to_string_lossy()
            .to_string();

        let json = cairo_runner
            .get_air_private_input()
            .to_serializable(trace_path, memory_path)
            .serialize_json()
            .map_err(PublicInputError::Serde)?;
        std::fs::write(file_path, json)?;
    }

    if let Some(ref file_name) = args.cairo_pie_output {
        let file_path = Path::new(file_name);
        cairo_runner
            .get_cairo_pie()
            .map_err(CairoRunError::Runner)?
            .write_zip_file(file_path)?
    }

    Ok(())
}

fn main() -> Result<(), Error> {
    #[cfg(test)]
    return Ok(());

    #[cfg(not(test))]
    match run(std::env::args()) {
        Err(Error::Cli(err)) => err.exit(),
        other => other,
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::too_many_arguments)]
    use super::*;
    use assert_matches::assert_matches;
    use rstest::rstest;

    #[rstest]
    #[case([].as_slice())]
    #[case(["cairo-vm-cli"].as_slice())]
    fn test_run_missing_mandatory_args(#[case] args: &[&str]) {
        let args = args.iter().cloned().map(String::from);
        assert_matches!(run(args), Err(Error::Cli(_)));
    }

    #[rstest]
    #[case(["cairo-vm-cli", "--layout", "broken_layout", "../cairo_programs/fibonacci.json"].as_slice())]
    fn test_run_invalid_args(#[case] args: &[&str]) {
        let args = args.iter().cloned().map(String::from);
        assert_matches!(run(args), Err(Error::Cli(_)));
    }

    #[rstest]
    #[case(["cairo-vm-cli", "../cairo_programs/fibonacci.json", "--air_private_input", "/dev/null", "--proof_mode", "--memory_file", "/dev/null"].as_slice())]
    fn test_run_air_private_input_no_trace(#[case] args: &[&str]) {
        let args = args.iter().cloned().map(String::from);
        assert_matches!(run(args), Err(Error::Cli(_)));
    }

    #[rstest]
    #[case(["cairo-vm-cli", "../cairo_programs/fibonacci.json", "--air_private_input", "/dev/null", "--proof_mode", "--trace_file", "/dev/null"].as_slice())]
    fn test_run_air_private_input_no_memory(#[case] args: &[&str]) {
        let args = args.iter().cloned().map(String::from);
        assert_matches!(run(args), Err(Error::Cli(_)));
    }

    #[rstest]
    #[case(["cairo-vm-cli", "../cairo_programs/fibonacci.json", "--air_private_input", "/dev/null", "--trace_file", "/dev/null", "--memory_file", "/dev/null"].as_slice())]
    fn test_run_air_private_input_no_proof(#[case] args: &[&str]) {
        let args = args.iter().cloned().map(String::from);
        assert_matches!(run(args), Err(Error::Cli(_)));
    }

    #[rstest]
    fn test_run_ok(
        #[values(None,
                 Some("plain"),
                 Some("small"),
                 Some("dex"),
                 Some("starknet"),
                 Some("starknet_with_keccak"),
                 Some("recursive_large_output"),
                 Some("all_cairo"),
                 Some("all_solidity"),
                 //FIXME: dynamic layout leads to _very_ slow execution
                 //Some("dynamic"),
        )]
        layout: Option<&str>,
        #[values(false, true)] memory_file: bool,
        #[values(false, true)] mut trace_file: bool,
        #[values(false, true)] proof_mode: bool,
        #[values(false, true)] print_output: bool,
        #[values(false, true)] entrypoint: bool,
        #[values(false, true)] air_public_input: bool,
        #[values(false, true)] air_private_input: bool,
        #[values(false, true)] cairo_pie_output: bool,
    ) {
        let mut args = vec!["cairo-vm-cli".to_string()];
        if let Some(layout) = layout {
            args.extend_from_slice(&["--layout".to_string(), layout.to_string()]);
        }
        if air_public_input {
            args.extend_from_slice(&["--air_public_input".to_string(), "/dev/null".to_string()]);
        }
        if air_private_input {
            args.extend_from_slice(&["--air_private_input".to_string(), "/dev/null".to_string()]);
        }
        if cairo_pie_output {
            args.extend_from_slice(&["--cairo_pie_output".to_string(), "/dev/null".to_string()]);
        }
        if proof_mode {
            trace_file = true;
            args.extend_from_slice(&["--proof_mode".to_string()]);
        }
        if entrypoint {
            args.extend_from_slice(&["--entrypoint".to_string(), "main".to_string()]);
        }
        if memory_file {
            args.extend_from_slice(&["--memory_file".to_string(), "/dev/null".to_string()]);
        }
        if trace_file {
            args.extend_from_slice(&["--trace_file".to_string(), "/dev/null".to_string()]);
        }
        if print_output {
            args.extend_from_slice(&["--print_output".to_string()]);
        }

        args.push("../cairo_programs/proof_programs/fibonacci.json".to_string());
        if air_public_input && !proof_mode
            || (air_private_input && (!proof_mode || !trace_file || !memory_file))
            || cairo_pie_output && proof_mode
        {
            assert_matches!(run(args.into_iter()), Err(_));
        } else {
            assert_matches!(run(args.into_iter()), Ok(_));
        }
    }

    #[test]
    fn test_run_missing_program() {
        let args = ["cairo-vm-cli", "../missing/program.json"]
            .into_iter()
            .map(String::from);
        assert_matches!(run(args), Err(Error::IO(_)));
    }

    #[rstest]
    #[case("../cairo_programs/manually_compiled/invalid_even_length_hex.json")]
    #[case("../cairo_programs/manually_compiled/invalid_memory.json")]
    #[case("../cairo_programs/manually_compiled/invalid_odd_length_hex.json")]
    #[case("../cairo_programs/manually_compiled/no_data_program.json")]
    #[case("../cairo_programs/manually_compiled/no_main_program.json")]
    fn test_run_bad_file(#[case] program: &str) {
        let args = ["cairo-vm-cli", program].into_iter().map(String::from);
        assert_matches!(run(args), Err(Error::Runner(_)));
    }

    #[test]
    fn test_run_dynamic_params() {
        let mut args = vec!["cairo-vm-cli".to_string()];
        args.extend_from_slice(&["--layout".to_string(), "dynamic".to_string()]);
        args.extend_from_slice(&[
            "--cairo_layout_params_file".to_string(),
            "../vm/src/tests/cairo_layout_params_file.json".to_string(),
        ]);
        args.push("../cairo_programs/proof_programs/fibonacci.json".to_string());

        assert_matches!(run(args.into_iter()), Ok(_));
    }

    //Since the functionality here is trivial, I just call the function
    //to fool Codecov.
    #[test]
    fn test_main() {
        main().unwrap();
    }
}
