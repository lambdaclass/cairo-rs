use core::hint::black_box;
use iai_callgrind::main;

use cairo_vm::{
    types::{layout_name::LayoutName, program::Program},
    vm::runners::cairo_runner::CairoRunner,
};

use mimalloc::MiMalloc;

#[global_allocator]
static ALLOC: MiMalloc = MiMalloc;

#[inline(never)]
fn parse_program() {
    //Picked the biggest one at the time of writing
    let program = include_bytes!("../cairo_programs/benchmarks/keccak_integration_benchmark.json");
    let program =
        Program::from_bytes(black_box(program.as_slice()), black_box(Some("main"))).unwrap();
    core::mem::drop(black_box(program));
}

#[export_name = "helper::parse_program"]
#[inline(never)]
fn parse_program_helper() -> Program {
    //Picked the biggest one at the time of writing
    let program = include_bytes!("../cairo_programs/benchmarks/keccak_integration_benchmark.json");
    Program::from_bytes(program.as_slice(), Some("main")).unwrap()
}

#[inline(never)]
fn build_runner() {
    let program = parse_program_helper();
    let runner = CairoRunner::new(
        black_box(&program),
        LayoutName::starknet_with_keccak,
        None,
        false,
        false,
        false,
    )
    .unwrap();
    core::mem::drop(black_box(runner));
}

#[export_name = "helper::build_runner"]
#[inline(never)]
fn build_runner_helper() -> CairoRunner {
    //Picked the biggest one at the time of writing
    let program = include_bytes!("../cairo_programs/benchmarks/keccak_integration_benchmark.json");
    let program = Program::from_bytes(program.as_slice(), Some("main")).unwrap();
    CairoRunner::new(
        &program,
        LayoutName::starknet_with_keccak,
        None,
        false,
        false,
        false,
    )
    .unwrap()
}

#[inline(never)]
fn load_program_data() {
    let mut runner = build_runner_helper();
    _ = black_box(runner.initialize(false).unwrap());
}

main!(
    callgrind_args = "toggle-collect=helper::*,core::mem::drop";
    functions = parse_program, build_runner, load_program_data
);
