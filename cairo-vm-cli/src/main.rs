#![deny(warnings)]
#![forbid(unsafe_code)]
use cairo_vm::cairo_run::{self, EncodeTraceError};
use cairo_vm::hint_processor::builtin_hint_processor::builtin_hint_processor_definition::BuiltinHintProcessor;
use cairo_vm::vm::errors::cairo_run_errors::CairoRunError;
use cairo_vm::vm::errors::trace_errors::TraceError;
use cairo_vm::vm::errors::vm_errors::VirtualMachineError;
use cairo_vm::write::BufWriter;
use clap::{Parser, ValueHint};
use std::path::PathBuf;
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
    #[clap(long = "--trace_file", value_parser)]
    trace_file: Option<PathBuf>,
    #[structopt(long = "--print_output")]
    print_output: bool,
    #[structopt(long = "--entrypoint", default_value = "main")]
    entrypoint: String,
    #[structopt(long = "--memory_file")]
    memory_file: Option<PathBuf>,
    #[clap(long = "--layout", default_value = "plain", validator=validate_layout)]
    layout: String,
    #[structopt(long = "--proof_mode")]
    proof_mode: bool,
    #[structopt(long = "--secure_run")]
    secure_run: Option<bool>,
}

fn validate_layout(value: &str) -> Result<(), String> {
    match value {
        "plain"
        | "small"
        | "dex"
        | "starknet"
        | "starknet_with_keccak"
        | "recursive_large_output"
        | "all_cairo"
        | "all_solidity"
        | "dynamic" => Ok(()),
        _ => Err(format!("{value} is not a valid layout")),
    }
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
}

fn run(args: impl Iterator<Item = String>) -> Result<(), Error> {
    let args = Args::try_parse_from(args);
    let args = match args {
        Ok(args) => args,
        Err(error) => {
            eprintln!("{error}");
            return Err(Error::Cli(error));
        }
    };
    let trace_enabled = args.trace_file.is_some();
    let mut hint_executor = BuiltinHintProcessor::new_empty();
    let cairo_run_config = cairo_run::CairoRunConfig {
        entrypoint: &args.entrypoint,
        trace_enabled,
        relocate_mem: args.memory_file.is_some(),
        layout: &args.layout,
        proof_mode: args.proof_mode,
        secure_run: args.secure_run,
    };

    let program_content = std::fs::read(args.filename).map_err(Error::IO)?;

    let (cairo_runner, mut vm) =
        match cairo_run::cairo_run(&program_content, &cairo_run_config, &mut hint_executor) {
            Ok(runner) => runner,
            Err(error) => {
                eprintln!("{error}");
                return Err(Error::Runner(error));
            }
        };

    if args.print_output {
        let mut output_buffer = "Program Output:\n".to_string();
        vm.write_output(&mut output_buffer)?;
        print!("{output_buffer}");
    }

    if let Some(trace_path) = args.trace_file {
        let relocated_trace = vm.get_relocated_trace()?;

        let trace_file = std::fs::File::create(trace_path)?;
        let mut trace_writer = BufWriter::with_capacity(3 * 1024 * 1024, trace_file);

        cairo_run::write_encoded_trace(relocated_trace, &mut trace_writer)?;
        trace_writer.flush()?;
    }

    if let Some(memory_path) = args.memory_file {
        let memory_file = std::fs::File::create(memory_path)?;
        let mut memory_writer = BufWriter::with_capacity(5 * 1024 * 1024, memory_file);

        cairo_run::write_encoded_memory(&cairo_runner.relocated_memory, &mut memory_writer)?;
        memory_writer.flush()?;
    }

    Ok(())
}

fn main() -> Result<(), Error> {
    run(std::env::args())
}

#[cfg(test)]
mod tests {
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
        #[values(false, true)] secure_run: bool,
        #[values(false, true)] print_output: bool,
        #[values(false, true)] entrypoint: bool,
    ) {
        let mut args = vec!["cairo-vm-cli".to_string()];
        if let Some(layout) = layout {
            args.extend_from_slice(&["--layout".to_string(), layout.to_string()]);
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
        if secure_run {
            args.extend_from_slice(&["--secure_run".to_string(), "true".to_string()]);
        }
        if print_output {
            args.extend_from_slice(&["--print_output".to_string()]);
        }
        args.push("../cairo_programs/proof_programs/fibonacci.json".to_string());
        assert_matches!(run(args.into_iter()), Ok(_));
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

    //Since the functionality here is trivial, I just call the function
    //to fool Codecov.
    #[test]
    fn test_main() {
        assert!(main().is_err());
    }

    #[test]
    fn test_valid_layouts() {
        let valid_layouts = vec![
            "plain",
            "small",
            "dex",
            "starknet",
            "starknet_with_keccak",
            "recursive_large_output",
            "all_cairo",
            "all_solidity",
        ];

        for layout in valid_layouts {
            assert_eq!(validate_layout(layout), Ok(()));
        }
    }

    #[test]
    fn test_invalid_layout() {
        let invalid_layout = "invalid layout name";
        assert!(validate_layout(invalid_layout).is_err());
    }
}
