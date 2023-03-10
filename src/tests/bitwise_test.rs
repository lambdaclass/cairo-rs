use crate::stdlib::prelude::*;

use crate::{
    hint_processor::builtin_hint_processor::builtin_hint_processor_definition::BuiltinHintProcessor,
    types::program::Program,
    vm::{
        runners::cairo_runner::CairoRunner, trace::trace_entry::RelocatedTraceEntry,
        vm_core::VirtualMachine,
    },
};

use assert_matches::assert_matches;

#[cfg(target_arch = "wasm32")]
use wasm_bindgen_test::*;

#[test]
#[cfg_attr(target_arch = "wasm32", wasm_bindgen_test)]
fn bitwise_integration_test() {
    let program = Program::from_bytes(
        include_bytes!("../../cairo_programs/bitwise_builtin_test.json"),
        Some("main"),
    )
    .unwrap();
    let mut hint_processor = BuiltinHintProcessor::new_empty();
    let mut cairo_runner = CairoRunner::new(&program, "all_cairo", false).unwrap();
    let mut vm = VirtualMachine::new(true);
    let end = cairo_runner.initialize(&mut vm).unwrap();
    assert_matches!(
        cairo_runner.run_until_pc(end, &mut vm, &mut hint_processor),
        Ok(()),
        "Execution failed"
    );
    assert_matches!(cairo_runner.relocate(&mut vm,), Ok(()), "Execution failed");

    let python_vm_relocated_trace: Vec<RelocatedTraceEntry> = vec![
        RelocatedTraceEntry {
            pc: 27,
            ap: 72,
            fp: 72,
        },
        RelocatedTraceEntry {
            pc: 28,
            ap: 73,
            fp: 72,
        },
        RelocatedTraceEntry {
            pc: 30,
            ap: 74,
            fp: 72,
        },
        RelocatedTraceEntry {
            pc: 32,
            ap: 75,
            fp: 72,
        },
        RelocatedTraceEntry {
            pc: 9,
            ap: 77,
            fp: 77,
        },
        RelocatedTraceEntry {
            pc: 10,
            ap: 77,
            fp: 77,
        },
        RelocatedTraceEntry {
            pc: 11,
            ap: 77,
            fp: 77,
        },
        RelocatedTraceEntry {
            pc: 13,
            ap: 78,
            fp: 77,
        },
        RelocatedTraceEntry {
            pc: 14,
            ap: 79,
            fp: 77,
        },
        RelocatedTraceEntry {
            pc: 34,
            ap: 79,
            fp: 72,
        },
        RelocatedTraceEntry {
            pc: 36,
            ap: 79,
            fp: 72,
        },
        RelocatedTraceEntry {
            pc: 37,
            ap: 80,
            fp: 72,
        },
        RelocatedTraceEntry {
            pc: 39,
            ap: 81,
            fp: 72,
        },
        RelocatedTraceEntry {
            pc: 41,
            ap: 82,
            fp: 72,
        },
        RelocatedTraceEntry {
            pc: 15,
            ap: 84,
            fp: 84,
        },
        RelocatedTraceEntry {
            pc: 16,
            ap: 84,
            fp: 84,
        },
        RelocatedTraceEntry {
            pc: 17,
            ap: 84,
            fp: 84,
        },
        RelocatedTraceEntry {
            pc: 19,
            ap: 85,
            fp: 84,
        },
        RelocatedTraceEntry {
            pc: 20,
            ap: 86,
            fp: 84,
        },
        RelocatedTraceEntry {
            pc: 43,
            ap: 86,
            fp: 72,
        },
        RelocatedTraceEntry {
            pc: 45,
            ap: 86,
            fp: 72,
        },
        RelocatedTraceEntry {
            pc: 46,
            ap: 87,
            fp: 72,
        },
        RelocatedTraceEntry {
            pc: 48,
            ap: 88,
            fp: 72,
        },
        RelocatedTraceEntry {
            pc: 50,
            ap: 89,
            fp: 72,
        },
        RelocatedTraceEntry {
            pc: 21,
            ap: 91,
            fp: 91,
        },
        RelocatedTraceEntry {
            pc: 22,
            ap: 91,
            fp: 91,
        },
        RelocatedTraceEntry {
            pc: 23,
            ap: 91,
            fp: 91,
        },
        RelocatedTraceEntry {
            pc: 25,
            ap: 92,
            fp: 91,
        },
        RelocatedTraceEntry {
            pc: 26,
            ap: 93,
            fp: 91,
        },
        RelocatedTraceEntry {
            pc: 52,
            ap: 93,
            fp: 72,
        },
        RelocatedTraceEntry {
            pc: 54,
            ap: 93,
            fp: 72,
        },
        RelocatedTraceEntry {
            pc: 55,
            ap: 94,
            fp: 72,
        },
        RelocatedTraceEntry {
            pc: 57,
            ap: 95,
            fp: 72,
        },
        RelocatedTraceEntry {
            pc: 59,
            ap: 96,
            fp: 72,
        },
        RelocatedTraceEntry {
            pc: 1,
            ap: 98,
            fp: 98,
        },
        RelocatedTraceEntry {
            pc: 2,
            ap: 98,
            fp: 98,
        },
        RelocatedTraceEntry {
            pc: 3,
            ap: 98,
            fp: 98,
        },
        RelocatedTraceEntry {
            pc: 5,
            ap: 99,
            fp: 98,
        },
        RelocatedTraceEntry {
            pc: 6,
            ap: 100,
            fp: 98,
        },
        RelocatedTraceEntry {
            pc: 7,
            ap: 101,
            fp: 98,
        },
        RelocatedTraceEntry {
            pc: 8,
            ap: 102,
            fp: 98,
        },
        RelocatedTraceEntry {
            pc: 61,
            ap: 102,
            fp: 72,
        },
        RelocatedTraceEntry {
            pc: 63,
            ap: 102,
            fp: 72,
        },
        RelocatedTraceEntry {
            pc: 65,
            ap: 102,
            fp: 72,
        },
        RelocatedTraceEntry {
            pc: 67,
            ap: 102,
            fp: 72,
        },
        RelocatedTraceEntry {
            pc: 68,
            ap: 103,
            fp: 72,
        },
    ];
    for (i, entry) in python_vm_relocated_trace.iter().enumerate() {
        assert_eq!(&cairo_runner.relocated_trace.as_ref().unwrap()[i], entry);
    }
}
