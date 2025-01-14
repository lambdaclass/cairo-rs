use cairo_vm::{
    types::{layout_name::LayoutName, program::Program},
    vm::runners::cairo_runner::CairoRunner,
};
use criterion::{black_box, criterion_group, criterion_main, BatchSize, Criterion};

use mimalloc::MiMalloc;

#[global_allocator]
static ALLOC: MiMalloc = MiMalloc;

fn parse_program(c: &mut Criterion) {
    //Picked the biggest one at the time of writing
    let program = include_bytes!("../cairo_programs/benchmarks/keccak_integration_benchmark.json");
    c.bench_function("parse program", |b| {
        b.iter_with_large_drop(|| {
            _ = Program::from_bytes(black_box(program.as_slice()), black_box(Some("main")))
                .unwrap();
        })
    });
}

fn build_many_runners(c: &mut Criterion) {
    //Picked the biggest one at the time of writing
    let program = include_bytes!("../cairo_programs/benchmarks/keccak_integration_benchmark.json");
    let program = Program::from_bytes(program.as_slice(), Some("main")).unwrap();
    c.bench_function("build runner", |b| {
        b.iter_with_large_drop(|| {
            _ = black_box(
                CairoRunner::new(
                    black_box(&program),
                    black_box(LayoutName::starknet_with_keccak),
                    black_box(None),
                    black_box(false),
                    black_box(false),
                    black_box(false),
                )
                .unwrap(),
            );
        })
    });
}

fn load_program_data(c: &mut Criterion) {
    //Picked the biggest one at the time of writing
    let program = include_bytes!("../cairo_programs/benchmarks/keccak_integration_benchmark.json");
    let program = Program::from_bytes(program.as_slice(), Some("main")).unwrap();
    c.bench_function("initialize", |b| {
        b.iter_batched(
            || {
                CairoRunner::new(
                    &program,
                    LayoutName::starknet_with_keccak,
                    None,
                    false,
                    false,
                    false,
                )
                .unwrap()
            },
            |mut runner| _ = black_box(runner.initialize(false).unwrap()),
            BatchSize::SmallInput,
        )
    });
}

criterion_group!(runner, build_many_runners, load_program_data, parse_program);
criterion_main!(runner);
