use bincode::enc::write::Writer;
use cairo1_run::error::Error;
use cairo1_run::{cairo_run_program, Cairo1RunConfig, FuncArg};
use cairo_lang_compiler::{
    compile_prepared_db, db::RootDatabase, project::setup_project, CompilerConfig,
};
use cairo_vm::types::layout::CairoLayoutParams;
use cairo_vm::{
    air_public_input::PublicInputError, types::layout_name::LayoutName,
    vm::errors::trace_errors::TraceError, Felt252,
};
use clap::{Parser, ValueHint};
use itertools::Itertools;
use std::{
    io::{self, Write},
    path::PathBuf,
};

#[derive(Parser, Debug)]
#[clap(author, version, about, long_about = None)]
struct Args {
    #[clap(value_parser, value_hint=ValueHint::FilePath)]
    filename: PathBuf,
    #[clap(long = "trace_file", value_parser)]
    trace_file: Option<PathBuf>,
    #[structopt(long = "memory_file")]
    memory_file: Option<PathBuf>,
    /// When using dynamic layout, it's parameters must be specified through a layout params file.
    #[clap(long = "layout", default_value = "plain", value_enum)]
    layout: LayoutName,
    /// Required when using with dynamic layout.
    /// Ignored otherwise.
    #[clap(long = "cairo_layout_params_file", required_if_eq("layout", "dynamic"))]
    cairo_layout_params_file: Option<PathBuf>,
    #[clap(long = "proof_mode", value_parser)]
    proof_mode: bool,
    #[clap(long = "air_public_input", requires = "proof_mode")]
    air_public_input: Option<PathBuf>,
    #[clap(
        long = "air_private_input",
        requires_all = ["proof_mode", "trace_file", "memory_file"]
    )]
    air_private_input: Option<PathBuf>,
    #[clap(
        long = "cairo_pie_output",
        // We need to add these air_private_input & air_public_input or else
        // passing cairo_pie_output + either of these without proof_mode will not fail
        conflicts_with_all = ["proof_mode", "air_private_input", "air_public_input"]
    )]
    cairo_pie_output: Option<PathBuf>,
    // Arguments should be spaced, with array elements placed between brackets
    // For example " --args '1 2 [1 2 3]'" will yield 3 arguments, with the last one being an array of 3 elements
    #[clap(long = "args", default_value = "", value_parser=process_args, conflicts_with = "args_file")]
    args: FuncArgs,
    // Same rules from `args` apply here
    #[clap(long = "args_file", value_parser, value_hint=ValueHint::FilePath, conflicts_with = "args")]
    args_file: Option<PathBuf>,
    #[clap(long = "print_output", value_parser)]
    print_output: bool,
    #[clap(
        long = "append_return_values",
        // We need to add these air_private_input & air_public_input or else
        // passing cairo_pie_output + either of these without proof_mode will not fail
        conflicts_with_all = ["proof_mode", "air_private_input", "air_public_input"]
    )]
    append_return_values: bool,
}

#[derive(Debug, Clone, Default)]
struct FuncArgs(Vec<FuncArg>);

/// Processes an iterator of format [s1, s2,.., sn, "]", ...], stopping at the first "]" string
/// and returning the array [f1, f2,.., fn] where fi = Felt::from_dec_str(si)
fn process_array<'a>(iter: &mut impl Iterator<Item = &'a str>) -> Result<FuncArg, String> {
    let mut array = vec![];
    for value in iter {
        match value {
            "]" => break,
            _ => array.push(
                Felt252::from_dec_str(value)
                    .map_err(|_| format!("\"{}\" is not a valid felt", value))?,
            ),
        }
    }
    Ok(FuncArg::Array(array))
}

/// Parses a string of ascii whitespace separated values, containing either numbers or series of numbers wrapped in brackets
/// Returns an array of felts and felt arrays
fn process_args(value: &str) -> Result<FuncArgs, String> {
    let mut args = Vec::new();
    // Split input string into numbers and array delimiters
    let mut input = value.split_ascii_whitespace().flat_map(|mut x| {
        // We don't have a way to split and keep the separate delimiters so we do it manually
        let mut res = vec![];
        if let Some(val) = x.strip_prefix('[') {
            res.push("[");
            x = val;
        }
        if let Some(val) = x.strip_suffix(']') {
            if !val.is_empty() {
                res.push(val)
            }
            res.push("]")
        } else if !x.is_empty() {
            res.push(x)
        }
        res
    });
    // Process iterator of numbers & array delimiters
    while let Some(value) = input.next() {
        match value {
            "[" => args.push(process_array(&mut input)?),
            _ => args.push(FuncArg::Single(
                Felt252::from_dec_str(value)
                    .map_err(|_| format!("\"{}\" is not a valid felt", value))?,
            )),
        }
    }
    Ok(FuncArgs(args))
}

pub struct FileWriter {
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

fn run(args: impl Iterator<Item = String>) -> Result<Option<String>, Error> {
    let mut args = Args::try_parse_from(args)?;
    if let Some(filename) = args.args_file {
        args.args = process_args(&std::fs::read_to_string(filename)?).unwrap();
    }

    let cairo_layout_params = match args.cairo_layout_params_file {
        Some(file) => Some(CairoLayoutParams::from_file(&file)?),
        None => None,
    };

    let cairo_run_config = Cairo1RunConfig {
        proof_mode: args.proof_mode,
        serialize_output: args.print_output,
        relocate_mem: args.memory_file.is_some() || args.air_public_input.is_some(),
        layout: args.layout,
        trace_enabled: args.trace_file.is_some() || args.air_public_input.is_some(),
        args: &args.args.0,
        finalize_builtins: args.air_public_input.is_some() || args.cairo_pie_output.is_some(),
        append_return_values: args.append_return_values,
        dynamic_layout_params: cairo_layout_params,
    };

    // Try to parse the file as a sierra program
    let file = std::fs::read(&args.filename)?;
    let sierra_program = match serde_json::from_slice(&file) {
        Ok(program) => program,
        Err(_) => {
            // If it fails, try to compile it as a cairo program
            let compiler_config = CompilerConfig {
                replace_ids: true,
                ..CompilerConfig::default()
            };
            let mut db = RootDatabase::builder()
                .detect_corelib()
                .skip_auto_withdraw_gas()
                .build()
                .unwrap();
            let main_crate_ids = setup_project(&mut db, &args.filename).unwrap();
            let sierra_program_with_dbg =
                compile_prepared_db(&db, main_crate_ids, compiler_config).unwrap();

            sierra_program_with_dbg.program
        }
    };

    let (runner, _, serialized_output) = cairo_run_program(&sierra_program, cairo_run_config)?;

    if let Some(file_path) = args.air_public_input {
        let json = runner.get_air_public_input()?.serialize_json()?;
        std::fs::write(file_path, json)?;
    }

    if let (Some(file_path), Some(trace_file), Some(memory_file)) = (
        args.air_private_input,
        args.trace_file.clone(),
        args.memory_file.clone(),
    ) {
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

        let json = runner
            .get_air_private_input()
            .to_serializable(trace_path, memory_path)
            .serialize_json()
            .map_err(PublicInputError::Serde)?;
        std::fs::write(file_path, json)?;
    }

    if let Some(ref file_path) = args.cairo_pie_output {
        runner.get_cairo_pie()?.write_zip_file(file_path)?
    }

    if let Some(trace_path) = args.trace_file {
        let relocated_trace = runner
            .relocated_trace
            .ok_or(Error::Trace(TraceError::TraceNotRelocated))?;
        let trace_file = std::fs::File::create(trace_path)?;
        let mut trace_writer =
            FileWriter::new(io::BufWriter::with_capacity(3 * 1024 * 1024, trace_file));

        cairo_vm::cairo_run::write_encoded_trace(&relocated_trace, &mut trace_writer)?;
        trace_writer.flush()?;
    }
    if let Some(memory_path) = args.memory_file {
        let memory_file = std::fs::File::create(memory_path)?;
        let mut memory_writer =
            FileWriter::new(io::BufWriter::with_capacity(5 * 1024 * 1024, memory_file));

        cairo_vm::cairo_run::write_encoded_memory(&runner.relocated_memory, &mut memory_writer)?;
        memory_writer.flush()?;
    }

    Ok(serialized_output)
}

fn main() -> Result<(), Error> {
    match run(std::env::args()) {
        Err(Error::Cli(err)) => err.exit(),
        Ok(output) => {
            if let Some(output_string) = output {
                println!("Program Output : {}", output_string);
            }
            Ok(())
        }
        Err(Error::RunPanic(panic_data)) => {
            if !panic_data.is_empty() {
                let panic_data_string_list = panic_data
                    .iter()
                    .map(|m| {
                        // Try to parse to utf8 string
                        let msg = String::from_utf8(m.to_bytes_be().to_vec());
                        if let Ok(msg) = msg {
                            format!("{} ('{}')", m, msg)
                        } else {
                            m.to_string()
                        }
                    })
                    .join(", ");
                println!("Run panicked with: [{}]", panic_data_string_list);
            }
            Ok(())
        }
        Err(err) => Err(err),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use assert_matches::assert_matches;
    use rstest::rstest;

    #[rstest]
    #[case(
        "ecdsa_recover.cairo",
        "3490001189944926769628658346285649224182856084131963744896357527096042836716",
        "[3490001189944926769628658346285649224182856084131963744896357527096042836716]",
        None,
        None
    )]
    #[case(
        "tensor_new.cairo",
        "[1 2] [1 false 1 true]", // Struct { span [1 2] span [struct {1 false} struct {1 true}]}
        "[2 1 2 2 1 0 1 1]", // len: 2 [1 2] len 2: [{1 0} {1 0}]
        None, None
    )]
    #[case("bytes31_ret.cairo", "123", "[123]", None, None)]
    #[case("null_ret.cairo", "null", "[]", None, None)]
    #[case(
        "felt_dict_squash.cairo",
        "{66675: [4 5 6] 66676: [1 2 3]}",
        "[66675 3 4 5 6 66676 3 1 2 3]",
        None,
        None
    )]
    #[case(
        "dict_with_struct.cairo",
        "{0: 1 true 1: 1 false 2: 1 true}",
        "[0 1 1 1 1 0 2 1 1]",
        None,
        None
    )]
    #[case(
        "nullable_box_vec.cairo",
        "{0: 10 1: 20 2: 30} 3",
        "[0 10 1 20 2 30 3]",
        None,
        None
    )]
    #[case("array_integer_tuple.cairo", "[1] 1", "[1 1 1]", None, None)]
    #[case(
        "felt_dict.cairo",
        "{66675: [8 9 10 11] 66676: [1 2 3]}",
        "[66675 4 8 9 10 11 66676 3 1 2 3]",
        None,
        None
    )]
    #[case("felt_span.cairo", "[8 9 10 11]", "[4 8 9 10 11]", None, None)]
    #[case("nullable_dict.cairo", "", "[]", None, None)]
    #[case(
        "struct_span_return.cairo",
        "[[4 3] [2 1]]",
        "[2 2 4 3 2 2 1]",
        None,
        None
    )]
    #[case("null_ret.cairo", "null", "[]", None, None)]
    #[case(
        "with_input/tensor.cairo",
        "1",
        "[1]",
        Some("[2 2] [1 2 3 4]"),
        Some("[2 2 2 4 1 2 3 4]")
    )]
    #[case(
        "with_input/array_input_sum.cairo",
        "12",
        "[12]",
        Some("2 [1 2 3 4] 0 [9 8]"),
        Some("[2 4 1 2 3 4 0 2 9 8]")
    )]
    #[case(
        "with_input/array_length.cairo",
        "5",
        "[5]",
        Some("[1 2 3 4] [1]"),
        Some("[4 1 2 3 4 1 1]")
    )]
    #[case(
        "with_input/array_length.cairo",
        "4",
        "[4]",
        Some("[1 2 3 4] []"),
        Some("[4 1 2 3 4 0]")
    )]
    #[case("with_input/branching.cairo", "0", "[0]", Some("17"), Some("[17]"))]
    #[case("with_input/branching.cairo", "1", "[1]", Some("0"), Some("[0]"))]
    #[case("dictionaries.cairo", "1024", "[1024]", None, None)]
    #[case("simple_struct.cairo", "100", "[100]", None, None)]
    #[case("simple.cairo", "true", "[1]", None, None)]
    #[case(
        "pedersen_example.cairo",
        "1089549915800264549621536909767699778745926517555586332772759280702396009108",
        "[1089549915800264549621536909767699778745926517555586332772759280702396009108]",
        None,
        None
    )]
    #[case(
        "poseidon_pedersen.cairo",
        "1036257840396636296853154602823055519264738423488122322497453114874087006398",
        "[1036257840396636296853154602823055519264738423488122322497453114874087006398]",
        None,
        None
    )]
    #[case(
        "poseidon.cairo",
        "1099385018355113290651252669115094675591288647745213771718157553170111442461",
        "[1099385018355113290651252669115094675591288647745213771718157553170111442461]",
        None,
        None
    )]
    #[case("sample.cairo", "5050", "[5050]", None, None)]
    #[case(
        "recursion.cairo",
        "1154076154663935037074198317650845438095734251249125412074882362667803016453",
        "[1154076154663935037074198317650845438095734251249125412074882362667803016453]",
        None,
        None
    )]
    #[case("print.cairo", "", "[]", None, None)]
    #[case("ops.cairo", "6", "[6]", None, None)]
    #[case("hello.cairo", "1234", "[1 1234]", None, None)]
    #[case(
        "enum_match.cairo",
        "10 3618502788666131213697322783095070105623107215331596699973092056135872020471",
        "[10 3618502788666131213697322783095070105623107215331596699973092056135872020471]",
        None,
        None
    )]
    #[case("enum_flow.cairo", "300", "[300]", None, None)]
    #[case("array_get.cairo", "3", "[3]", None, None)]
    #[case("bitwise.cairo", "11772", "[11772]", None, None)]
    #[case("factorial.cairo", "3628800", "[3628800]", None, None)]
    #[case("fibonacci.cairo", "89", "[89]", None, None)]
    #[case(
        "with_input/dict_with_input.cairo",
        "[17 18]",
        "[17 18]",
        Some("[17 18]"),
        Some("[17 18]")
    )]
    #[cfg_attr(feature = "mod_builtin", case(
        "circuit.cairo",
        "36699840570117848377038274035 72042528776886984408017100026 54251667697617050795983757117 7",
        "[36699840570117848377038274035 72042528776886984408017100026 54251667697617050795983757117 7]",
        None,
        None
    ))]
    fn test_run_program(
        #[case] program: &str,
        #[case] expected_output: &str,
        #[case] expected_serialized_output: &str,
        #[case] inputs: Option<&str>,
        #[case] serialized_inputs: Option<&str>,
        #[values(
        &["--cairo_pie_output", "/dev/null"], // Non proof-mode
        &["--cairo_pie_output", "/dev/null", "--append_return_values"], // Non proof-mode & appending return values to ouput
        &["--proof_mode", "--air_public_input", "/dev/null", "--air_private_input", "/dev/null"], // Proof mode
    )]
        extra_flags: &[&str],
    ) {
        let common_flags = &[
            "--print_output",
            "--trace_file",
            "/dev/null",
            "--memory_file",
            "/dev/null",
            "--layout",
            "all_cairo",
        ];
        let mut args = vec!["cairo1-run"];
        let has_serialized_output = extra_flags
            .iter()
            .any(|flag| flag == &"--append_return_values" || flag == &"--proof_mode");
        let filename = if has_serialized_output {
            format!(
                "../cairo_programs/cairo-1-programs/serialized_output/{}",
                program
            )
        } else {
            format!("../cairo_programs/cairo-1-programs/{}", program)
        };

        args.push(&filename);
        args.extend_from_slice(common_flags);
        args.extend_from_slice(extra_flags);
        if let (Some(inputs), false) = (inputs, has_serialized_output) {
            args.extend_from_slice(&["--args", inputs])
        }
        if let (Some(inputs), true) = (serialized_inputs, has_serialized_output) {
            args.extend_from_slice(&["--args", inputs])
        }
        let args = args.iter().cloned().map(String::from);
        let expected_output = if has_serialized_output {
            expected_serialized_output
        } else {
            expected_output
        };
        assert_matches!(run(args), Ok(Some(res)) if res == expected_output, "Program {} failed with flags {}", program, extra_flags.concat());
    }

    #[test]
    fn test_run_dynamic_params() {
        let mut args = vec!["cairo1-run".to_string()];
        args.extend_from_slice(&["--layout".to_string(), "dynamic".to_string()]);
        args.extend_from_slice(&[
            "--cairo_layout_params_file".to_string(),
            "../vm/src/tests/cairo_layout_params_file.json".to_string(),
        ]);
        args.push("../cairo_programs/cairo-1-programs/fibonacci.cairo".to_string());

        assert_matches!(run(args.into_iter()), Ok(_));
    }

    // these tests are separated so as to run them without --append_return_values and --proof_mode options
    // since they require to use the squashed version of felt252
    #[rstest]
    #[case(
        "dict_non_squashed/dict_with_struct_non_squash.cairo",
        "{0: 1 true 1: 1 false 2: 1 true}",
        None
    )]
    #[case(
        "dict_non_squashed/nullable_box_vec_non_squash.cairo",
        "{0: 10 1: 20 2: 30} 3",
        None
    )]
    #[case(
        "dict_non_squashed/felt_dict_non_squash.cairo",
        "{66675: [8 9 10 11] 66676: [1 2 3]}",
        None
    )]
    fn test_run_progarm_non_proof(
        #[case] program: &str,
        #[case] expected_output: &str,
        #[case] inputs: Option<&str>,
        #[values(
        &["--cairo_pie_output", "/dev/null"], // Non proof-mode
    )]
        extra_flags: &[&str],
    ) {
        let common_flags = &[
            "--print_output",
            "--trace_file",
            "/dev/null",
            "--memory_file",
            "/dev/null",
            "--layout",
            "all_cairo",
        ];
        let mut args = vec!["cairo1-run"];
        let filename = format!("../cairo_programs/cairo-1-programs/{}", program);

        args.push(&filename);
        args.extend_from_slice(common_flags);
        args.extend_from_slice(extra_flags);
        if let Some(inputs) = inputs {
            args.extend_from_slice(&["--args", inputs])
        }
        let args = args.iter().cloned().map(String::from);
        assert_matches!(run(args), Ok(Some(res)) if res == expected_output, "Program {} failed with flags {}", program, extra_flags.concat());
    }

    #[rstest]
    #[case(["cairo1-run", "../cairo_programs/cairo-1-programs/serialized_output/with_input/branching.cairo", "--layout", "all_cairo", "--cairo_pie_output", "/dev/null"].as_slice())]
    #[case(["cairo1-run", "../cairo_programs/cairo-1-programs/serialized_output/with_input/branching.cairo", "--layout", "all_cairo", "--proof_mode"].as_slice())]
    fn test_run_branching_no_args(#[case] args: &[&str]) {
        let args = args.iter().cloned().map(String::from);
        assert_matches!(run(args), Err(Error::ArgumentsSizeMismatch { expected, actual }) if expected == 2 && actual == 0);
    }

    #[rstest]
    #[case(["cairo1-run", "../cairo_programs/cairo-1-programs/serialized_output/with_input/branching.cairo", "--layout", "all_cairo","--args", "1 2 3"].as_slice())]
    #[case(["cairo1-run", "../cairo_programs/cairo-1-programs/serialized_output/with_input/branching.cairo", "--layout", "all_cairo", "--proof_mode", "--args", "1 2 3"].as_slice())]
    fn test_run_branching_too_many_args(#[case] args: &[&str]) {
        let args = args.iter().cloned().map(String::from);
        assert_matches!(run(args), Err(Error::ArgumentsSizeMismatch { expected, actual }) if expected == 2 && actual == 3);
    }
}
