# Cairo1-run

A cairo-vm crate to run Cairo 1 Programs

Once you are inside the `./cairo1-run` folder, use the CLI with the following commands

To install the required dependencies(cairo corelib) run

```bash
make deps
```

Now that you have the dependencies necessary to run the tests, you can run:

```bash
make test
```

To execute a Cairo 1 program (either as Cairo 1 source file or Sierra)

```bash
cargo run ../cairo_programs/cairo-1-programs/fibonacci.cairo
```

Arguments to generate the trace and memory files

```bash
cargo run ../cairo_programs/cairo-1-programs/fibonacci.cairo --trace_file ../cairo_programs/cairo-1-programs/fibonacci.trace --memory_file ../cairo_programs/cairo-1-programs/fibonacci.memory
```

To pass arguments to `main`

* Separate arguments with a whitespace inbetween
* In order to pass arrays, wrap array values between brackets

Example:

```bash

cargo run ../cairo_programs/cairo-1-programs/with_input/array_input_sum.cairo --layout all_cairo --args '2 [1 2 3 4] 0 [9 8]'

```

To execute all the cairo 1 programs inside `../cairo_programs/cairo-1-programs/` and generate the corresponding trace and the memory files

```bash
make run
```

## CLI argument list

The cairo1-run cli supports the following optional arguments:

* `--layout <LAYOUT>`: Sets the layout for the cairo_run. This will limit the available builtins. The deafult layout is `plain`, which has no builtins. For general purpose, the `all_cairo` layout contains all currently available builtins. More info about layouts [here](https://docs.cairo-lang.org/how_cairo_works/builtins.html#layouts).

* `--args <ARGUMENTS>`: Receives the arguments to be passed to the program's main function. Receives whitespace-separated values which can be numbers or arrays, with arrays consisting of whitespace-separated numbers wrapped between brackets

* `--args_file <FILENAME>`: Receives the name of the file from where arguments should be read. Expects the same argument format of the `--args` flag. Should be used if the list of arguments exceeds the shell's capacity.

* `--trace_file <TRACE_FILE>`: Receives the name of a file and outputs the relocated trace into it

* `--memory_file <MEMORY_FILE>`: Receives the name of a file and outputs the relocated memory into it

* `--proof_mode`: Runs the program in proof_mode. Only allows `Array<felt252>` as return and input value.

* `--air_public_input <AIR_PUBLIC_INPUT>`: Receives the name of a file and outputs the AIR public inputs into it. Can only be used if proof_mode is also enabled.

* `--air_private_input <AIR_PRIVATE_INPUT>`: Receives the name of a file and outputs the AIR private inputs into it. Can only be used if proof_mode, trace_file & memory_file are also enabled.

* `--cairo_pie_output <CAIRO_PIE_OUTPUT>`: Receives the name of a file and outputs the Cairo PIE into it. Can only be used if proof_mode, is not enabled.

* `--append_return_values`: Adds extra instructions to the program in order to append the return and input values to the output builtin's segment. This is the default behaviour for proof_mode. Only allows `Array<felt252>` as return and input value.

## Running circuits

Circuits in cairo 1 require to enable the `mod_builtin` feature in order for the `AddMod`, `MulMod` and `RangeCheck96` builtins to be taken into account.

# Running scarb projects

As cairo1-run skips gas checks when running, you will need to add the following to your Scarb.toml to ensure that compilation is done without adding gas checks:

```toml
[cairo]
enable-gas = false
```

First compile your project running `scarb build`

Then run the compiled project's sierra file located at `project_name/target/project_name.sierra.json`

Example:
```bash
  cargo run path-to-project/target/project_name.sierra.json
```

# Known bugs & issues

## Libfunc `get_builtin_costs` &  function `poseidon_hash_many`
Compiling without gas checks removes libfuncs associated with gas checks that are generated during compilation but it cannot remove those in the cairo code itself. Therefore code using the external functions on the `gas` corelib moudle (`withdraw_gas`, `withdraw_gas_all` & `get_builtin_costs`) will fail to compile.
One notable case of this issue is the `poseidon_hash_span` function, which uses `get_builtin_costs` in its implementation. We advise using the `HashStateTrait` impl instead. The `poseidon_hash_span` function can also be modified so that it no longer relies on gas, an example of this can be found on the test file `poseidon.cairo` under the `cairo_porgrams/cairo-1-programs` folder.

## Nullable<Box<T>>
There is currently a bug in cairo 2.6.3 affecting `Nullable<Box<T>>` types.
Tracking issue: https://github.com/starkware-libs/cairo/issues/5411

Proposed solution:

Add the helper function:
```
#[inline(never)]
fn identity<T>(t: T) -> T { t }
```

And use it when creating the `Nullable<Box<T>>`:
```
NullableTrait::<Box<T>::new(BoxTrait::new(identity(value)))
```
