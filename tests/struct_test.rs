use cairo_vm::{
    hint_processor::builtin_hint_processor::builtin_hint_processor_definition::BuiltinHintProcessor,
    vm::vm_core::VirtualMachine,
};
use std::path::Path;

use cairo_vm::{
    types::program::Program,
    vm::{runners::cairo_runner::CairoRunner, trace::trace_entry::RelocatedTraceEntry},
};

#[test]
fn struct_integration_test() {
    let program = Program::from_file(Path::new("cairo_programs/struct.json"), Some("main"))
        .expect("Failed to deserialize program");
    let mut hint_processor = BuiltinHintProcessor::new_empty();
    let mut cairo_runner = CairoRunner::new(&program, "all", false).unwrap();
    let mut vm = VirtualMachine::new(true);
    let end = cairo_runner.initialize(&mut vm).unwrap();

    assert!(
        cairo_runner.run_until_pc(end, &mut vm, &mut hint_processor) == Ok(()),
        "Execution failed"
    );
    assert!(cairo_runner.relocate(&mut vm) == Ok(()), "Execution failed");
    let relocated_entry = RelocatedTraceEntry {
        pc: 1,
        ap: 4,
        fp: 4,
    };

    assert_eq!(cairo_runner.relocated_trace, Some(vec![relocated_entry]));
}
