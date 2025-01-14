#![deny(warnings)]
#![forbid(unsafe_code)]
use std::fs::{self, File};
use std::io::BufReader;

use cairo_vm::stdlib::collections::{HashMap, HashSet};
use cairo_vm::{
    hint_processor::{
        builtin_hint_processor::builtin_hint_processor_definition::BuiltinHintProcessor,
        hint_processor_definition::HintProcessorLogic,
    },
    serde::deserialize_program::ApTracking,
    types::exec_scope::ExecutionScopes,
    vm::{errors::hint_errors::HintError, vm_core::VirtualMachine},
};

use serde::Deserialize;
use serde_json::Value;

#[derive(Deserialize)]
struct AllowedHintExpression {
    #[serde(rename(deserialize = "allowed_expressions"))]
    _allowed_expressions: Option<Value>,
    hint_lines: Vec<Box<str>>,
}

#[derive(Deserialize)]
struct Whitelist {
    #[serde(rename(deserialize = "allowed_reference_expressions_for_hint"))]
    allowed_hint_expressions: Vec<AllowedHintExpression>,
}

const CAIRO_LANG_PATH: &str = "cairo-lang/src/starkware/starknet/security/whitelists";

fn run() {
    // We use the files in the cairo-lang repo, cloned from the latest version
    let whitelist_paths = fs::read_dir(CAIRO_LANG_PATH).expect(
        "Failed to read whitelist directory from cairo-lang, did you forget to clone it?\n",
    );
    let mut whitelists = Vec::new();
    for path in whitelist_paths {
        let path = path.expect("Failed to get path").path();
        if path.to_str().unwrap_or_default().ends_with(".json") {
            let file = File::open(path).expect("Failed to open whitelist file");
            let mut reader = BufReader::new(file);

            let whitelist_file: Whitelist =
                serde_json::from_reader(&mut reader).expect("Failed to parse whitelist");
            whitelists.push(whitelist_file.allowed_hint_expressions);
        }
    }
    let mut vm = VirtualMachine::new(false, false);
    let mut hint_executor = BuiltinHintProcessor::new_empty();
    let (ap_tracking_data, reference_ids, references, mut exec_scopes, constants) = (
        ApTracking::default(),
        HashMap::new(),
        Vec::new(),
        ExecutionScopes::new(),
        HashMap::new(),
    );
    let missing_hints: HashSet<_> = whitelists
        .into_iter()
        .flatten()
        .map(|ahe| ahe.hint_lines.join("\n"))
        .filter(|h| {
            let hint_data = hint_executor
                .compile_hint(h, &ap_tracking_data, &reference_ids, &references)
                .expect("this implementation is infallible");
            matches!(
                hint_executor.execute_hint(&mut vm, &mut exec_scopes, &hint_data, &constants,),
                Err(HintError::UnknownHint(_)),
            )
        })
        .collect();

    println!("{} missing hints:", missing_hints.len());
    for hint in missing_hints.iter() {
        println!();
        println!("```");
        println!("%{{");
        println!("{hint}");
        println!("%}}");
        println!("```");
    }
}

fn main() {
    run()
}
