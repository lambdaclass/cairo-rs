## Cairo-VM Changelog

#### Upcoming Changes

* fix: Fix compilation of cairo-1-run and cairo-vm on wasm32 [#1792](https://github.com/lambdaclass/cairo-vm/pull/1792)

* fix: Remove validation of CairoPie memory values [#1783](https://github.com/lambdaclass/cairo-vm/pull/1783)

* fix: Handle `GasBuiltin` in cairo1-run crate [#1789](https://github.com/lambdaclass/cairo-vm/pull/1789)
  * Load `initial_gas` into vm instead of creating it via instructions.
  * Fix bug affecting programs with input arguments and gas builtin.

* fix: Change (de)serialization of CairoPie's `OutputBuiltinAdditionalData`'s `PublicMemoryPage` to vectors of length 2. [#1781](https://github.com/lambdaclass/cairo-vm/pull/1781)

* fix: Fixed deserialization issue when signature additional data is empty, and the name of the builtin range_check96 [#1785](https://github.com/lambdaclass/cairo-vm/pull/1785)

* refactor + bugfix: Improve arg handling for cairo1-run [#1782](https://github.com/lambdaclass/cairo-vm/pull/1782)
  * Now uses ascii whitespace as separator, preventing errors when using newlines in args file
  * No longer gets stuck on improperly-formatted arrays
  * Returns an informative clap error upon invalid felt strings instead of unwrapping

* fix: Ignore memory order when comparing instances of `CairoPieMemory` [#1780](https://github.com/lambdaclass/cairo-vm/pull/1780)

* feat: Add `EXCESS_BALANCE` hint [#1777](https://github.com/lambdaclass/cairo-vm/pull/1777)

* feat(BREAKING): Use a cheatcode to relocate all dicts + Make temporary segment usage configurable [#1776](https://github.com/lambdaclass/cairo-vm/pull/1776)
  * Add the flags `segment_arena_validation` & `use_temporary_segments` to the `Cairo1HintProcessor` & `DictManagerExecScope` respectively. These flags will determine if real segments or temporary segments will be used when creating dictionaries.
  * `DictManagerExecScope::finalize_segment` no longer performs relocation and is ignored if `use_temporary_segments` is set to false.
  * Add method `DictManagerExecScope::relocate_all_dictionaries` that adds relocation rules for all tracked dictionaries, relocating them one next to the other in a new segment.
  * Add cheatcode `RelocateAllDictionaries` to the `Cairo1HintProcessor`, which calls the aforementioned method.
  * Add casm instruction to call the aforementioned cheatcode in `create_entry_code` if either `proof_mode` or `append_return_values` are set to true, and segment arena is present.

* Bump `starknet-types-core` version + Use the lib's pedersen hash [#1734](https://github.com/lambdaclass/cairo-vm/pull/1734)

* refactor: Add boolean method Cairo1RunConfig::copy_to_output + Update Doc [#1778](https://github.com/lambdaclass/cairo-vm/pull/1778)

* feat: Filter implicit arguments from return value in cairo1-run crate [#1775](https://github.com/lambdaclass/cairo-vm/pull/1775)

* feat(BREAKING): Serialize inputs into output segment in cairo1-run crate:
  * Checks that only `Array<Felt252>` can be received by the program main function when running with with either `--proof_mode` or `--append_return_values`.
  * Copies the input value to the output segment right after the output in the format `[array_len, arr[0], arr[1],.., arr[n]]`.

                  * feat: specify initial value for `exec_scopes` in `cairo_run_program` [1772](https://github.com/lambdaclass/cairo-vm/pull/1772)

* fix: make MemorySegmentManager.finalize() public [#1771](https://github.com/lambdaclass/cairo-vm/pull/1771)

* feat: load Cairo PIE from bytes [#1773](https://github.com/lambdaclass/cairo-vm/pull/1773)

* feat(BREAKING): Serialize `Array<Felt252>` return value into output segment in cairo1-run crate:
  * Checks that only `PanicResult<Array<Felt252>>` or `Array<Felt252>` can be returned by the program when running with either `--proof_mode` or `--append_return_values`.
  * Serializes return values into the output segment under the previous conditions following the format:
    * `PanicResult<Array<Felt252>>` -> `[panic_flag, array_len, arr[0], arr[1],.., arr[n]]`
    * `<Array<Felt252>` -> `[array_len, arr[0], arr[1],.., arr[n]]`

* feat: Handle `BoundedInt` variant in `serialize_output`, `cairo1-run` crate  [#1768](https://github.com/lambdaclass/cairo-vm/pull/1768)

* fix: make `OutputBuiltinState` public [#1769](https://github.com/lambdaclass/cairo-vm/pull/1769)

* feat: Load arguments into VM instead of creating them via instructions in cairo1-run [#1759](https://github.com/lambdaclass/cairo-vm/pull/1759)

#### [1.0.0-rc3] - 2024-05-14

* bugfix: Fix handling of return values wrapped in `PanicResult` in cairo1-run crate [#1763](https://github.com/lambdaclass/cairo-vm/pull/1763)

* refactor(BREAKING): Move the VM back to the CairoRunner [#1743](https://github.com/lambdaclass/cairo-vm/pull/1743)
  * `CairoRunner` has a new public field `vm: VirtualMachine`
  * `CairoRunner` no longer derives `Debug`
  * `CairoRunner` methods `new_v2` & `new` take an extra boolean argument `trace_enabled`.
  * Functions `cairo_run` , `cairo_run_program` & `cairo_run_fuzzed_program` from `vm` crate and `cairo_run_program` from `cairo1-run` crate now retun only `CairoRunner` instead of `(CairoRunner, VirtualMachine)`
  * `CairoRunner` methods no longer take a reference to `VirtualMachine`. Methods that took an immutable reference to self and a mutable reference to the VM now take a mutable reference to self. Affected methods:
    * `initialize`
    * `initialize_builtins`
    * `initialize_all_builtins`
    * `initialize_segments`
    * `initialize_state`
    * `initialize_function_entrypoint`
    * `initialize_state`
    * `initialize_main_entrypoint`
    * `initialize_vm`
    * `run_until_pc`
    * `run_for_steps`
    * `run_until_steps`
    * `run_until_power_of_2`
    * `get_perm_range_check_limits`
    * `check_range_check_usage`
    * `get_memory_holes`
    * `check_diluted_check_usage`
    * `end_run`
    * `relocate_trace`
    * `relocate_memory`
    * `relocate`
    * `get_builtin_segments_info`
    * `get_builtin_segments_info_for_pie`
    * `get_execution_resources`
    * `finalize_segments`
    * `run_from_entrypoint`
    * `check_used_cells`
    * `check_memory_usage`
    * `initialize_function_runner_cairo_1`
    * `initialize_function_runner`
    * `read_return_values`
    * `get_builtins_final_stack`
    * `get_cairo_pie`
    * `get_air_public_input`
    * `get_air_private_input`
    * `get_memory_segment_addresses`
  * Functions & methods taking a reference to `CairoRunner` & `VirtualMachine` now only take a reference to `CairoRunner`:
    * `start_tracer`
    * `VmException::from_vm_error`
    * `get_error_attr_value`
    * `get_traceback`
    * `verify_secure_runner`
  * [hooks feature] `BeforeFirstStepHookFunc` dyn Fn no longer takes a mutable reference to `CairoRunner`, along with `VirtualMachine::execute_before_first_step`.

* fix: add support for arrays shorter than 2 as arguments for cairo1-run [#1737](https://github.com/lambdaclass/cairo-vm/pull/1737)

* bugfix: Fix BuiltinRunner::final_stack for SegmentArena[#1747](https://github.com/lambdaclass/cairo-vm/pull/1747)

* feat: unify `arbitrary`, `hooks`, `print` and `skip_next_instruction_hint` features as a single `test_utils` feature [#1755](https://github.com/lambdaclass/cairo-vm/pull/1755)
  * BREAKING: removed the above features

* bugfix: cairo1-run CLI: Set finalize_builtins to true when using --air_public_input flag [#1744](https://github.com/lambdaclass/cairo-vm/pull/1752)

* feat: Add hint `U256InvModN` to `Cairo1HintProcessor` [#1744](https://github.com/lambdaclass/cairo-vm/pull/1744)

* perf: use a more compact representation for `MemoryCell` [#1672](https://github.com/lambdaclass/cairo-vm/pull/1672)
  * BREAKING: `Memory::get_value` will now always return `Cow::Owned` variants, code that relied on `Cow::Borrowed` may break

#### [1.0.0-rc2] - 2024-05-02

* `cairo1-run` CLI: Allow loading arguments from file[#1739](https://github.com/lambdaclass/cairo-vm/pull/1739)

* BREAKING: Remove unused `CairoRunner` field `original_steps`[#1742](https://github.com/lambdaclass/cairo-vm/pull/1742)

* feat: Add `--run_from_cairo_pie` to `cairo-vm-cli` + workflow [#1730](https://github.com/lambdaclass/cairo-vm/pull/1730)

* Serialize directly into writer in `CairoPie::write_zip_file`[#1736](https://github.com/lambdaclass/cairo-vm/pull/1736)

* feat: Add support for cairo1 run with segements arena validation.
  * Refactored the runner CASM code generation to user a more high level builder.
  * Added segment merging of the dictionary segments.
  * Added validation of the generated segment arena in cairo1 run.

* refactor: Add `lib.rs` to cairo1-run[#1714](https://github.com/lambdaclass/cairo-vm/pull/1714)

* feat: Implement `CairoPie::read_zip_file`[#1729](https://github.com/lambdaclass/cairo-vm/pull/1729)

* feat: Bump to 2.6.3 + Remove gas checks[#1709](https://github.com/lambdaclass/cairo-vm/pull/1709)
  * Bump cairo_lang crates & corelib to v2.6.3
  * Disable gas checks when compiling to sierra & casm
  * Add `Known bugs & issues` segment to README, poining out issues derived from the removal of gas checks and cairo v2.6.3

* feat: Implement running from `CairoPie`[#1720](https://github.com/lambdaclass/cairo-vm/pull/1720)
  * Add function `cairo_run_pie`
  * Add `CairoPie` methods `run_validity_checks` & `check_pie_compatibility`
  * Add `Program` method `from_stripped_program`

* bugfix: Don't assume outer deref when fetching integer values from references[#1732](https://github.com/lambdaclass/cairo-vm/pull/1732)

* feat: Implement `extend_additional_data` for `BuiltinRunner`[#1726](https://github.com/lambdaclass/cairo-vm/pull/1726)

* BREAKING: Set dynamic params as null by default on air public input [#1716](https://github.com/lambdaclass/cairo-vm/pull/1716)
  * `PublicInput` field `layout_params` renamed to `dynamic_params` & type changed from`&'a CairoLayout` to `()`.

* feat: `cairo1-run` accepts Sierra programs [#1719](https://github.com/lambdaclass/cairo-vm/pull/1719)

* refactor(BREAKING): Use `BuiltinName` enum instead of string representation [#1722](https://github.com/lambdaclass/cairo-vm/pull/1722)
  * `BuiltinName` moved from `crate::serde::deserialize_program` module to `crate::types::builtin_name`.
    * Implement `BuiltinName` methods `to_str`, `to_str_with_suffix`, `from_str` & `from_str_with_suffix`.
  * Remove `BuiltinName` method `name`.
  * All builtin-related error variants now store `BuiltinName` instead of `&'static str` or `String`.
  * Remove constants: `OUTPUT_BUILTIN_NAME`, `HASH_BUILTIN_NAME`, `RANGE_CHECK_BUILTIN_NAME`,`RANGE_CHECK_96_BUILTIN_NAME`, `SIGNATURE_BUILTIN_NAME`, `BITWISE_BUILTIN_NAME`, `EC_OP_BUILTIN_NAME`, `KECCAK_BUILTIN_NAME`, `POSEIDON_BUILTIN_NAME`, `SEGMENT_ARENA_BUILTIN_NAME`, `ADD_MOD_BUILTIN_NAME` &
`MUL_MOD_BUILTIN_NAME`.
  * Remove `BuiltinRunner` & `ModBuiltinRunner` method `identifier`
  * Structs containing string representation of builtin names now use `BuiltinName` instead:
    * `AirPrivateInput(pub HashMap<&'static str, Vec<PrivateInput>>)` ->  `AirPrivateInput(pub HashMap<BuiltinName, Vec<PrivateInput>>)`.
    * `CairoPieMetadata` field `additional_data`: `HashMap<String, BuiltinAdditionalData>,` -> `CairoPieAdditionalData` with `CairoPieAdditionalData(pub HashMap<BuiltinName, BuiltinAdditionalData>)`
    * `CairoPieMetadata` field `builtin_segments`: `HashMap<String, SegmentInfo>` -> `HashMap<BuiltinName, SegmentInfo>`.
    * `ExecutiobResources` field `builtin_instance_counter`: `HashMap<String, usize>` -> `HashMap<BuiltinName, usize>`
  * Methods returning string representation of builtin names now use `BuiltinName` instead:
    * `BuiltinRunner`, `ModBuiltinRunner` & `RangeCheckBuiltinRunner` method `name`: `&'static str` -> `BuiltinName`.
    * `CairoRunner` method `get_builtin_segment_info_for_pie`: `Result<HashMap<String, cairo_pie::SegmentInfo>, RunnerError>` -> `Result<HashMap<BuiltinName, cairo_pie::SegmentInfo>, RunnerError>`

  Notes: Serialization of vm outputs that now contain `BuiltinName` & `Display` implementation of `BuiltinName` have not been affected by this PR

* feat: Add `recursive_with_poseidon` layout[#1724](https://github.com/lambdaclass/cairo-vm/pull/1724)

* refactor(BREAKING): Use an enum to represent layout name[#1715](https://github.com/lambdaclass/cairo-vm/pull/1715)
  * Add enum `LayoutName` to represent cairo layout names.
  * `CairoRunConfig`, `Cairo1RunConfig` & `CairoRunner` field `layout` type changed from `String` to `LayoutName`.
  * `CairoLayout` field `name` type changed from `String` to `LayoutName`.

* fix(BREAKING): Remove unsafe impl of `Add<usize> for &'a Relocatable`[#1718](https://github.com/lambdaclass/cairo-vm/pull/1718)

* fix(BREAKING): Handle triple dereference references[#1708](https://github.com/lambdaclass/cairo-vm/pull/1708)
  * Replace `ValueAddress` boolean field `dereference` with boolean fields `outer_dereference` & `inner_dereference`
  * Replace `HintReference` boolean field `dereference` with boolean fields `outer_dereference` & `inner_dereference`
  * Reference parsing now handles the case of dereferences inside the cast. Aka references of type `cast([A + B], type)` such as `cast([[fp + 2] + 2], felt)`.

* Bump `starknet-types-core` version + Use the lib's pedersen hash [#1692](https://github.com/lambdaclass/cairo-vm/pull/1692)

* refactor: Remove unused code & use constants whenever possible for builtin instance definitions[#1707](https://github.com/lambdaclass/cairo-vm/pull/1707)

* feat: missing EC hints for Starknet OS 0.13.1 [#1706](https://github.com/lambdaclass/cairo-vm/pull/1706)

* fix(BREAKING): Use program builtins in `initialize_main_entrypoint` & `read_return_values`[#1703](https://github.com/lambdaclass/cairo-vm/pull/1703)
  * `initialize_main_entrypoint` now iterates over the program builtins when building the stack & inserts 0 for any missing builtin
  * `read_return_values` now only computes the final stack of the builtins in the program
  * BREAKING: `read_return_values` now takes a boolean argument `allow_missing_builtins`
  * Added method `BuiltinRunner::identifier` to get the `BuiltinName` of each builtin
  * BREAKING: `OutputBuiltinRunner::get_public_memory` now takes a reference to `MemorySegmentManager`
  * BREAKING: method `VirtualMachine::get_memory_segment_addresses` moved to `CairoRunner::get_memory_segment_addresses`

* feat(BREAKING): Add range_check96 builtin[#1698](https://github.com/lambdaclass/cairo-vm/pull/1698)
  * Add the new `range_check96` builtin to the `all_cairo` layout.
  * `RangeCheckBuiltinRunner` changes:
    * Remove field `n_parts`, replacing it with const generic `N_PARTS`.
    * Remome `n_parts` argument form method `new`.
    * Remove field `_bound`, replacing it with public method `bound`.
    * Add public methods `name` & `n_parts`.

* feat(BREAKING): Add mod builtin [#1673](https://github.com/lambdaclass/cairo-vm/pull/1673)

  Main Changes:
  * Add the new `ModBuiltinRunner`, implementing the builtins `add_mod` & `mul_mod`
  * Adds `add_mod` & `mul_mod` to the `all_cairo` & `dynamic` layouts under the `mod_builtin` feature flag. This will be added to the main code in a future update.
  * Add method `VirtualMachine::fill_memory` in order to perform the new builtin's main logic from within hints
  * Add hints to run arithmetic circuits using `add_mod` and/or `mul_mod` builtins

  Other Changes:
  * BREAKING: BuiltinRunner method signature change from
  `air_private_input(&self, memory: &Memory) -> Vec<PrivateInput>` to `pub fn air_private_input(&self, segments: &MemorySegmentManager) -> Vec<PrivateInput>`
  * Add `MayleRelocatable::sub_usize`
  * Implement `Add<u32> for Relocatable`
  * Add `Memory::get_usize`
  * BREAKING: Clean up unused/duplicated code from builtins module:
    * Remove unused method `get_memory_segment_addresses` from all builtin runners & the enum
    * Remove empty implementations of `deduce_memory_cell` & `add_validation_rules` from all builtin runners
    * Remove duplicated implementation of `final_stack` from all builtin runners except output and move it to the enum implementation

* bugfix(BREAKING): Handle off2 immediate case in `get_integer_from_reference`[#1701](https://github.com/lambdaclass/cairo-vm/pull/1701)
  * `get_integer_from_reference` & `get_integer_from_var_name` output changed from `Result<Cow<'a, Felt252>, HintError>` to `Result<Felt252, HintError>`

* feat: Reorganized builtins to be in the top of stack at the end of a run (Cairo1).

* BREAKING: Remove `CairoRunner::add_additional_hash_builtin` & `VirtualMachine::disable_trace`[#1658](https://github.com/lambdaclass/cairo-vm/pull/1658)

* feat: output builtin add_attribute method [#1691](https://github.com/lambdaclass/cairo-vm/pull/1691)
 
* feat: add a method to retrieve the output builtin from the VM [#1690](https://github.com/lambdaclass/cairo-vm/pull/1690)

* feat: Add zero segment [#1668](https://github.com/lambdaclass/cairo-vm/pull/1668)

* feat: Bump cairo_lang to 0.13.1 in testing env [#1687](https://github.com/lambdaclass/cairo-vm/pull/1687)

* feat(BREAKING): Use return type info from sierra when serializing return values in cairo1-run crate [#1665](https://github.com/lambdaclass/cairo-vm/pull/1665)
  * Removed public function `serialize_output`.
  * Add field `serialize_output` to `Cairo1RunConfig`.
  * Function `cairo_run_program` now returns an extra `Option<String>` value with the serialized output if `serialize_output` is enabled in the config.
  * Output serialization improved as it now uses the sierra program data to identify return value's types.

* feat: Create hyper_threading crate to benchmark the `cairo-vm` in a hyper-threaded environment [#1679](https://github.com/lambdaclass/cairo-vm/pull/1679)

* feat: add a `--tracer` option which hosts a web server that shows the line by line execution of cairo code along with memory registers [#1265](https://github.com/lambdaclass/cairo-vm/pull/1265)

* feat: Fix error handling in `initialize_state`[#1657](https://github.com/lambdaclass/cairo-vm/pull/1657)

* feat: Make air public inputs deserializable [#1657](https://github.com/lambdaclass/cairo-vm/pull/1648)

* feat: Show only layout builtins in air private input [#1651](https://github.com/lambdaclass/cairo-vm/pull/1651)

* feat: Sort builtin segment info upon serialization for Cairo PIE [#1654](https://github.com/lambdaclass/cairo-vm/pull/1654)

* feat: Fix output serialization for cairo 1 [#1645](https://github.com/lambdaclass/cairo-vm/pull/1645)
  * Reverts changes added by #1630
  * Extends the serialization of Arrays added by the `print_output` flag to Spans and Dictionaries
  * Now dereferences references upon serialization

* feat: Add flag to append return values to output segment when not running in proof_mode [#1646](https://github.com/lambdaclass/cairo-vm/pull/1646)
  * Adds the flag `append_return_values` to both the CLI and `Cairo1RunConfig` struct.
  * Enabling flag will add the output builtin and the necessary instructions to append the return values to the output builtin's memory segment.

* feat: Compute program hash chain [#1647](https://github.com/lambdaclass/cairo-vm/pull/1647)

* feat: Add cairo1-run output pretty-printing for felts, arrays/spans and dicts [#1630](https://github.com/lambdaclass/cairo-vm/pull/1630)

* feat: output builtin features for bootloader support [#1580](https://github.com/lambdaclass/cairo-vm/pull/1580)

#### [1.0.0-rc1] - 2024-02-23

* Bump `starknet-types-core` dependency version to 0.0.9 [#1628](https://github.com/lambdaclass/cairo-vm/pull/1628)

* feat: Implement `Display` for `MemorySegmentManager`[#1606](https://github.com/lambdaclass/cairo-vm/pull/1606)

* fix: make Felt252DictEntryUpdate work with MaybeRelocatable instead of only Felt [#1624](https://github.com/lambdaclass/cairo-vm/pull/1624).

* chore: bump `cairo-lang-` dependencies to 2.5.4 [#1629](https://github.com/lambdaclass/cairo-vm/pull/1629)

* chore: bump `cairo-lang-` dependencies to 2.5.3 [#1596](https://github.com/lambdaclass/cairo-vm/pull/1596)

* refactor: Refactor `cairo1-run` crate [#1601](https://github.com/lambdaclass/cairo-vm/pull/1601)
  * Add function `cairo_run_program` & struct `Cairo1RunConfig` in `cairo1-run::cairo_run` module.
  * Function `serialize_output` & structs `FuncArg` and `Error` in crate `cairo1-run` are now public.

* feat(BREAKING): Add `allow_missing_builtins` flag [#1600](https://github.com/lambdaclass/cairo-vm/pull/1600)

    This new flag will skip the check that all builtins used by the program need to be present in the selected layout if enabled. It will also be enabled by default when running in proof_mode.

  * Add `allow_missing_builtins` flag to `cairo-vm-cli` crate
  * Add `allow_missing_builtins` field to `CairoRunConfig` struct
  * Add `allow_missing_builtins` boolean argument to `CairoRunner` methods `initialize` & `initialize_builtins`

* feat: Append return values to the output segment when running cairo1-run in proof_mode [#1597](https://github.com/lambdaclass/cairo-vm/pull/1597)
  * Add instructions to the proof_mode header to copy return values to the output segment before initiating the infinite loop
  * Output builtin is now always included when running cairo 1 programs in proof_mode

* feat: deserialize AIR private input [#1589](https://github.com/lambdaclass/cairo-vm/pull/1589)

* feat(BREAKING): Remove unecessary conversion functions between `Felt` & `BigUint`/`BigInt` [#1562](https://github.com/lambdaclass/cairo-vm/pull/1562)
  * Remove the following functions:
    * felt_from_biguint
    * felt_from_bigint
    * felt_to_biguint
    * felt_to_bigint

* perf: optimize instruction cache allocations by using `VirtualMachine::load_data` [#1441](https://github.com/lambdaclass/cairo-vm/pull/1441)

* feat: Add `print_output` flag to `cairo-1` crate [#1575] (https://github.com/lambdaclass/cairo-vm/pull/1575)

* bugfixes(BREAKING): Fix memory hole count inconsistencies #[1585] (https://github.com/lambdaclass/cairo-vm/pull/1585)
  * Output builtin memory segment is no longer skipped when counting memory holes
  * Temporary memory cells now keep their accessed status when relocated
  * BREAKING: Signature change: `get_memory_holes(&self, builtin_count: usize) -> Result<usize, MemoryError>` ->  `get_memory_holes(&self, builtin_count: usize,  has_output_builtin: bool) -> Result<usize, MemoryError>`

* feat: Add `cairo_pie_output` flag to `cairo1-run` [#1581] (https://github.com/lambdaclass/cairo-vm/pull/1581)

* feat: Add `cairo_pie_output` flag to `cairo_vm_cli` [#1578] (https://github.com/lambdaclass/cairo-vm/pull/1578)
  * Fix serialization of CairoPie to be fully compatible with the python version
  * Add `CairoPie::write_zip_file`
  * Move handling of required and exclusive arguments in `cairo-vm-cli` to struct definition using clap derives

* feat: Add doc + default impl for ResourceTracker trait [#1576] (https://github.com/lambdaclass/cairo-vm/pull/1576)

* feat: Add `air_private_input` flag to `cairo1-run` [#1559] (https://github.com/lambdaclass/cairo-vm/pull/1559)

* feat: Add `args` flag to `cairo1-run` [#1551] (https://github.com/lambdaclass/cairo-vm/pull/1551)

* feat: Add `air_public_input` flag to `cairo1-run` [#1539] (https://github.com/lambdaclass/cairo-vm/pull/1539)

* feat: Implement air_private_input [#1552](https://github.com/lambdaclass/cairo-vm/pull/1552)

* feat: Add `proof_mode` flag to `cairo1-run` [#1537] (https://github.com/lambdaclass/cairo-vm/pull/1537)
  * The cairo1-run crate no longer compiles and executes in proof_mode by default
  * Add flag `proof_mode` to cairo1-run crate. Activating this flag will enable proof_mode compilation and execution

* dev: bump cairo 1 compiler dep to 2.4 [#1530](https://github.com/lambdaclass/cairo-vm/pull/1530)

#### [1.0.0-rc0] - 2024-1-5

* feat: Use `ProjectivePoint` from types-rs in ec_op builtin impl [#1532](https://github.com/lambdaclass/cairo-vm/pull/1532)

* feat(BREAKING): Replace `cairo-felt` crate with `starknet-types-core` (0.0.5) [#1408](https://github.com/lambdaclass/cairo-vm/pull/1408)

* feat(BREAKING): Add Cairo 1 proof mode compilation and execution [#1517] (https://github.com/lambdaclass/cairo-vm/pull/1517)
    * In the cairo1-run crate, now the Cairo 1 Programs are compiled and executed in proof-mode
    * BREAKING: Remove `CairoRunner.proof_mode: bool` field and replace it with `CairoRunner.runner_mode: RunnerMode`

* perf: Add `extensive_hints` feature to prevent performance regression for the common use-case [#1503] (https://github.com/lambdaclass/cairo-vm/pull/1503)

  * Gates changes added by #1491 under the feature flag `extensive_hints`

* chore: remove cancel-duplicates workflow [#1497](https://github.com/lambdaclass/cairo-vm/pull/1497)

* feat: Handle `pc`s outside of program segment in `VmException` [#1501] (https://github.com/lambdaclass/cairo-vm/pull/1501)

  * `VmException` now shows the full pc value instead of just the offset (`VmException.pc` field type changed to `Relocatable`)
  * `VmException.traceback` now shows the full pc value for each entry instead of hardcoding its index to 0.
  * Disable debug information for errors produced when `pc` is outside of the program segment (segment_index != 0). `VmException` fields `inst_location` & `error_attr_value` will be `None` in such case.

* feat: Allow running instructions from pcs outside the program segement [#1493](https://github.com/lambdaclass/cairo-vm/pull/1493)

* BREAKING: Partially Revert `Optimize trace relocation #906` [#1492](https://github.com/lambdaclass/cairo-vm/pull/1492)

  * Remove methods `VirtualMachine::get_relocated_trace`& `VirtualMachine::relocate_trace`.
  * Add `relocated_trace` field  & `relocate_trace` method to `CairoRunner`.
  * Swap `TraceEntry` for `RelocatedTraceEntry` type in `write_encoded_trace` & `PublicInput::new` signatures.
  * Now takes into account the program counter's segment index when building the execution trace instead of assuming it to be 0.

* feat: Add HintProcessor::execute_hint_extensive + refactor hint_ranges [#1491](https://github.com/lambdaclass/cairo-vm/pull/1491)

  * Add trait method `HintProcessorLogic::execute_hint_extensive`:
    * This method has a similar behaviour to `HintProcessorLogic::execute_hint` but it also returns a `HintExtension` (type alias for `HashMap<Relocatable, Vec<Box<dyn Any>>>`) that can be used to extend the current map of hints used by the VM. This behaviour achieves what the `vm_load_data` primitive does for cairo-lang, and is needed to implement os hints.
    * This method is now used by the VM to execute hints instead of `execute_hint`, but it's default implementation calls `execute_hint`, so current implementors of the `HintProcessor` trait won't notice any change.

  * Signature changes:
    * `pub fn step_hint(&mut self, hint_executor: &mut dyn HintProcessor, exec_scopes: &mut ExecutionScopes, hint_datas: &mut Vec<Box<dyn Any>>, constants: &HashMap<String, Felt252>) -> Result<(), VirtualMachineError>` -> `pub fn step_hint(&mut self, hint_processor: &mut dyn HintProcessor, exec_scopes: &mut ExecutionScopes, hint_datas: &mut Vec<Box<dyn Any>>, hint_ranges: &mut HashMap<Relocatable, HintRange>, constants: &HashMap<String, Felt252>) -> Result<(), VirtualMachineError>`
    * `pub fn step(&mut self, hint_executor: &mut dyn HintProcessor, exec_scopes: &mut ExecutionScopes, hint_data: &[Box<dyn Any>], constants: &HashMap<String, Felt252>) -> Result<(), VirtualMachineError>` -> `pub fn step(&mut self, hint_processor: &mut dyn HintProcessor, exec_scopes: &mut ExecutionScopes, hint_datas: &mut Vec<Box<dyn Any>>, hint_ranges: &mut HashMap<Relocatable, HintRange>, constants: &HashMap<String, Felt252>) -> Result<(), VirtualMachineError>`

* feat: add debugging capabilities behind `print` feature flag. [#1476](https://github.com/lambdaclass/cairo-vm/pull/1476)

* feat: add `cairo_run_program` function that takes a `Program` as an arg. [#1496](https://github.com/lambdaclass/cairo-vm/pull/1496)

#### [0.9.1] - 2023-11-16

* chore: bump `cairo-lang-` dependencies to 2.3.1 [#1482](https://github.com/lambdaclass/cairo-vm/pull/1482), [#1483](https://github.com/lambdaclass/cairo-vm/pull/1483)

* feat: Make PublicInput fields public [#1474](https://github.com/lambdaclass/cairo-vm/pull/1474)

* chore: bump starknet-crypto to v0.6.1 [#1469](https://github.com/lambdaclass/cairo-vm/pull/1469)

* feat: Implement the Serialize and Deserialize methods for the Program struct [#1458](https://github.com/lambdaclass/cairo-vm/pull/1458)

* feat: Use only program builtins when running cairo 1 programs [#1457](https://github.com/lambdaclass/cairo-vm/pull/1457)

* feat: Use latest cairo-vm version in cairo1-run crate [#1455](https://github.com/lambdaclass/cairo-vm/pull/1455)

* feat: Implement a CLI to run cairo 1 programs [#1370](https://github.com/lambdaclass/cairo-vm/pull/1370)

* fix: Fix string code of `BLAKE2S_ADD_UINT256` hint [#1454](https://github.com/lambdaclass/cairo-vm/pull/1454)

#### [0.9.0] - 2023-10-03

* fix: Default to empty attributes vector when the field is missing from the program JSON [#1450](https://github.com/lambdaclass/cairo-vm/pull/1450)

* fix: Change serialization of CairoPieMemory to match Python's binary format [#1447](https://github.com/lambdaclass/cairo-vm/pull/1447)

* fix: Remove Deserialize derive from CairoPie and fix Serialize implementation to match Python's [#1444](https://github.com/lambdaclass/cairo-vm/pull/1444)

* fix: ec_recover hints no longer panic when divisor is 0 [#1433](https://github.com/lambdaclass/cairo-vm/pull/1433)

* feat: Implement the Serialize and Deserialize traits for the CairoPie struct [#1438](https://github.com/lambdaclass/cairo-vm/pull/1438)

* fix: Using UINT256_HINT no longer panics when b is greater than 2^256 [#1430](https://github.com/lambdaclass/cairo-vm/pull/1430)

* feat: Added a differential fuzzer for programs with whitelisted hints [#1358](https://github.com/lambdaclass/cairo-vm/pull/1358)

* fix(breaking): Change return type of `get_execution_resources` to `RunnerError` [#1398](https://github.com/lambdaclass/cairo-vm/pull/1398)

* Don't build wasm-demo in `build` target + add ci job to run the wasm demo [#1393](https://github.com/lambdaclass/cairo-vm/pull/1393)

    * Adds default-members to workspace
    * Crate `examples/wasm-demo` is no longer built during `make build`
    * `make check` no longer compiles the cairo file used in the wasm-demo
    * Removes Makefile targets `examples/wasm-demo/src/array_sum.json` & `example_program`
    * `wasm-demo` now uses the compiled cairo file in `cairo_programs` directory instead of its own copy

* feat: Add `Program::new_for_proof` [#1396](https://github.com/lambdaclass/cairo-vm/pull/1396)

#### [0.8.7] - 2023-8-28

* Add REDUCE_V2 hint [#1420](https://github.com/lambdaclass/cairo-vm/pull/1420):
    * Implement REDUCE_V2 hint
    * Rename hint REDUCE -> REDUCE_V1

* BREAKING: Add `disable_trace_padding` to `CairoRunConfig`[#1233](https://github.com/lambdaclass/cairo-rs/pull/1233)

* feat: Implement `CairoRunner.get_cairo_pie`[#1375](https://github.com/lambdaclass/cairo-vm/pull/1375)

* fix: Compare air_public_inputs against python vm + Fix how public memory is built [#391](https://github.com/lambdaclass/cairo-vm/pull/1391)

    BugFixes:

    *  `CairoRunner.finalize_segments` now builds the output builtin's public memory (if applicable).
    * `MemorySegmentManager.get_public_memory_addresses` logic fixed.
    * `MemorySegmentManager.finalize` no longer skips segments when their public memory is None

    Minor changes:

    * `VirtualMachine.get_public_memory_addresses` now strips the "_builtin" suffix from builtin names
    * `MemorySegmentAddresses.stop_address` renamed to `stop_ptr`

    Overall these changes make the the air public input file (obtained through the --air_public_input flag) equivalent to the ones outputted by the cairo-lang version

* fix: Fix `SPLIT_FELT` hint [#1387](https://github.com/lambdaclass/cairo-vm/pull/1387)

* refactor: combine `Program.hints` and `Program.hints_ranges` into custom collection [#1366](https://github.com/lambdaclass/cairo-vm/pull/1366)

* fix: Fix div_mod [#1383](https://github.com/lambdaclass/cairo-vm/pull/1383)

  * Fixes `div_mod` function so that it behaves like the cairo-lang version
  * Various functions in the `math_utils` crate can now return a `MathError` : `div_mod`, `ec_add`, `line_slope`, `ec_double`, `ec_double_slope`.
  * Fixes `UINT256_MUL_INV_MOD_P` hint so that it behaves like the python code.

#### [0.8.6] - 2023-8-11

* fix: Handle error in hint `UINT256_MUL_DIV_MOD` when divides by zero [#1367](https://github.com/lambdaclass/cairo-vm/pull/1367)

* Add HintError::SyscallError and VmErrors::HINT_ERROR_STR constant [#1357](https://github.com/lambdaclass/cairo-rs/pull/1357)

* feat: make *arbitrary* feature also enable a `proptest::arbitrary::Arbitrary` implementation for `Felt252` [#1355](https://github.com/lambdaclass/cairo-vm/pull/1355)

* fix: correctly display invalid signature error message [#1361](https://github.com/lambdaclass/cairo-vm/pull/1361)

#### [0.8.5] - 2023-7-31

* fix: `Program` comparison depending on `hints_ranges` ordering [#1351](https://github.com/lambdaclass/cairo-rs/pull/1351)

* feat: implement the `--air_public_input` flag to the runner for outputting public inputs into a file [#1268](https://github.com/lambdaclass/cairo-rs/pull/1268)

* fix: CLI errors bad formatting and handling

* perf: replace insertion with bit-setting in validated addresses [#1208](https://github.com/lambdaclass/cairo-vm/pull/1208)

* fix: return error when a parsed hint's PC is invalid [#1340](https://github.com/lambdaclass/cairo-vm/pull/1340)

* chore(deps): bump _cairo-lang_ dependencies to v2.1.0-rc2 [#1345](https://github.com/lambdaclass/cairo-vm/pull/1345)

* chore(examples): remove _wee_alloc_ dependency from _wasm-demo_ example and _ensure-no_std_ dummy crate [#1337](https://github.com/lambdaclass/cairo-vm/pull/1337)

* docs: improved crate documentation [#1334](https://github.com/lambdaclass/cairo-vm/pull/1334)

* chore!: made `deserialize_utils` module private [#1334](https://github.com/lambdaclass/cairo-vm/pull/1334)
  BREAKING:
  * `deserialize_utils` is no longer exported
  * functions `maybe_add_padding`, `parse_value`, and `take_until_unbalanced` are no longer exported
  * `ReferenceParseError` is no more

* perf: changed `ok_or` usage for `ok_or_else` in expensive cases [#1332](https://github.com/lambdaclass/cairo-vm/pull/1332)

* feat: updated the old WASM example and moved it to [`examples/wasm-demo`](examples/wasm-demo/) [#1315](https://github.com/lambdaclass/cairo-vm/pull/1315)

* feat(fuzzing): add `arbitrary` feature to enable arbitrary derive in `Program` and `CairoRunConfig` [#1306](https://github.com/lambdaclass/cairo-vm/pull/1306) [#1330](https://github.com/lambdaclass/cairo-vm/pull/1330)

* perf: remove pointless iterator from rc limits tracking [#1316](https://github.com/lambdaclass/cairo-vm/pull/1316)

* feat(felt): add `from_bytes_le` and `from_bytes_ne` methods to `Felt252` [#1326](https://github.com/lambdaclass/cairo-vm/pull/1326)

* perf: change `Program::shared_program_data::hints` from `HashMap<usize, Vec<Box<dyn Any>>>` to `Vec<Box<dyn Any>>` and refer to them as ranges stored in a `Vec<_>` indexed by PC with run time reductions of up to 12% [#931](https://github.com/lambdaclass/cairo-vm/pull/931)
  BREAKING:
  * `get_hint_dictionary(&self, &[HintReference], &mut dyn HintProcessor) -> Result<HashMap<usize, Vec<Box<dyn Any>>, VirtualMachineError>` ->
    `get_hint_data(self, &[HintReference], &mut dyn HintProcessor) -> Result<Vec<Box<dyn Any>, VirtualMachineError>`
  * Hook methods receive `&[Box<dyn Any>]` rather than `&HashMap<usize, Vec<Box<dyn Any>>>`

#### [0.8.4]
**YANKED**

#### [0.8.3]
**YANKED**

#### [0.8.2] - 2023-7-10

* chore: update dependencies, particularly lamdaworks 0.1.2 -> 0.1.3 [#1323](https://github.com/lambdaclass/cairo-vm/pull/1323)

* fix: fix `UINT256_MUL_DIV_MOD` hint [#1320](https://github.com/lambdaclass/cairo-vm/pull/1320)

* feat: add dependency installation script `install.sh` [#1298](https://github.com/lambdaclass/cairo-vm/pull/1298)

* fix: specify resolver version 2 in the virtual workspace's manifest [#1311](https://github.com/lambdaclass/cairo-vm/pull/1311)

* feat: add `lambdaworks-felt` feature to `cairo-vm-cli` [#1308](https://github.com/lambdaclass/cairo-vm/pull/1308)

* chore: update dependencies, particularly clap 3.2 -> 4.3 [#1309](https://github.com/lambdaclass/cairo-vm/pull/1309)
  * this removes dependency on _atty_, that's no longer mantained

* chore: remove unused dependencies [#1307](https://github.com/lambdaclass/cairo-vm/pull/1307)
  * rand_core
  * serde_bytes
  * rusty-hook (_dev-dependency_)

* chore: bump `cairo-lang-starknet` and `cairo-lang-casm` dependencies to 2.0.0 [#1313](https://github.com/lambdaclass/cairo-vm/pull/1313)

#### [0.8.1] - 2023-6-29

* chore: change mentions of *cairo-rs-py* to *cairo-vm-py* [#1296](https://github.com/lambdaclass/cairo-vm/pull/1296)

* rename github repo from https://github.com/lambdaclass/cairo-rs to https://github.com/lambdaclass/cairo-vm [#1289](https://github.com/lambdaclass/cairo-vm/pull/1289)

* fix(security): avoid OOM crashes when programs jump to very high invalid addresses [#1285](https://github.com/lambdaclass/cairo-vm/pull/1285)

* fix: add `to_bytes_be` to the felt when `lambdaworks-felt` feature is active [#1290](https://github.com/lambdaclass/cairo-vm/pull/1290)

* chore: mark `modpow` and `to_signed_bytes_le` as *deprecated* [#1290](https://github.com/lambdaclass/cairo-vm/pull/1290)

* fix: bump *lambdaworks-math* to latest version, that fixes no-std support [#1293](https://github.com/lambdaclass/cairo-vm/pull/1293)

* build: remove dependency to `thiserror` (use `thiserror-no-std/std` instead)

* chore: use LambdaWorks' implementation of bit operations for `Felt252` [#1291](https://github.com/lambdaclass/cairo-vm/pull/1291)

* update `cairo-lang-starknet` and `cairo-lang-casm` dependencies to v2.0.0-rc6 [#1299](https://github.com/lambdaclass/cairo-vm/pull/1299)

#### [0.8.0] - 2023-6-26

* feat: Add feature `lambdaworks-felt` to `felt` & `cairo-vm` crates [#1281](https://github.com/lambdaclass/cairo-vm/pull/1281)

    Changes under this feature:
  * `Felt252` now uses *LambdaWorks*' `FieldElement` internally
  * BREAKING: some methods of `Felt252` were removed, namely: `modpow` and `to_signed_bytes_le`

#### [0.7.0] - 2023-6-26

* BREAKING: Integrate `RunResources` logic into `HintProcessor` trait [#1274](https://github.com/lambdaclass/cairo-vm/pull/1274)
  * Rename trait `HintProcessor` to `HintProcessorLogic`
  * Add trait `ResourceTracker`
  * Trait `HintProcessor` is now `HintProcessor: HintProcessorLogic + ResourceTracker`
  * `BuiltinHintProcessor::new` & `Cairo1HintProcessor::new` now receive the argumet `run_resources: RunResources`
  * `HintProcessorLogic::execute_hint` no longer receives `run_resources: &mut RunResources`
  * Remove argument `run_resources: &mut RunResources` from `CairoRunner::run_until_pc` & `CairoRunner::run_from_entrypoint`

* build: remove unused implicit features from cairo-vm [#1266](https://github.com/lambdaclass/cairo-vm/pull/1266)


#### [0.6.1] - 2023-6-23

* fix: updated the `custom_hint_example` and added it to the workspace [#1258](https://github.com/lambdaclass/cairo-vm/pull/1258)

* Add path to cairo-vm README.md [#1276](https://github.com/lambdaclass/cairo-vm/pull/1276)

* fix: change error returned when subtracting two `MaybeRelocatable`s to better reflect the cause [#1271](https://github.com/lambdaclass/cairo-vm/pull/1271)

* fix: CLI error message when using --help [#1270](https://github.com/lambdaclass/cairo-vm/pull/1270)

#### [0.6.0] - 2023-6-18

* fix: `dibit` hint no longer fails when called with an `m` of zero [#1247](https://github.com/lambdaclass/cairo-vm/pull/1247)

* fix(security): avoid denial of service on malicious input exploiting the scientific notation parser [#1239](https://github.com/lambdaclass/cairo-vm/pull/1239)

* BREAKING: Change `RunResources` usage:
    * Modify field type `RunResources.n_steps: Option<usize>,`

    * Public Api Changes:
        *  CairoRunner::run_until_pc: Now receive a `&mut RunResources` instead of an `&mut Option<RunResources>`
        *  CairoRunner::run_from_entrypoint: Now receive a `&mut RunResources` instead of an `&mut Option<RunResources>`
        * VirtualMachine::Step: Add `&mut RunResources` as input
        * Trait HintProcessor::execute_hint: Add  `&mut RunResources` as an input

* perf: accumulate `min` and `max` instruction offsets during run to speed up range check [#1080](https://github.com/lambdaclass/cairo-vm/pull/)
  BREAKING: `Cairo_runner::get_perm_range_check_limits` no longer returns an error when called without trace enabled, as it no longer depends on it

* perf: process reference list on `Program` creation only [#1214](https://github.com/lambdaclass/cairo-vm/pull/1214)
  Also keep them in a `Vec<_>` instead of a `HashMap<_, _>` since it will be continuous anyway.
  BREAKING:
  * `HintProcessor::compile_hint` now receies a `&[HintReference]` rather than `&HashMap<usize, HintReference>`
  * Public `CairoRunner::get_reference_list` has been removed

* BREAKING: Add no_std compatibility to cairo-vm (cairo-1-hints feature still not supported)
    * Move the vm to its own directory and crate, different from the workspace [#1215](https://github.com/lambdaclass/cairo-vm/pull/1215)

    * Add an `ensure_no_std` crate that the CI will use to check that new changes don't revert `no_std` support [#1215](https://github.com/lambdaclass/cairo-vm/pull/1215) [#1232](https://github.com/lambdaclass/cairo-vm/pull/1232)

    * replace the use of `num-prime::is_prime` by a custom implementation, therefore restoring `no_std` compatibility [#1238](https://github.com/lambdaclass/cairo-vm/pull/1238)

#### [0.5.2] - 2023-6-12

* BREAKING: Compute `ExecutionResources.n_steps` without requiring trace [#1222](https://github.com/lambdaclass/cairo-vm/pull/1222)

  * `CairoRunner::get_execution_resources` return's `n_steps` field value is now set to `vm.current_step` instead of `0` if both `original_steps` and `trace` are set to `None`

* Add `RunResources::get_n_steps` method [#1225](https://github.com/lambdaclass/cairo-vm/pull/1225)

* refactor: simplify `mem_eq`

* fix: pin Cairo compiler version [#1220](https://github.com/lambdaclass/cairo-vm/pull/1220)

* perf: make `inner_rc_bound` a constant, improving performance of the range-check builtin

* fix: substraction of `MaybeRelocatable` always behaves as signed [#1218](https://github.com/lambdaclass/cairo-vm/pull/1218)

#### [0.5.1] - 2023-6-7

* fix: fix overflow for `QUAD_BIT` and `DI_BIT` hints [#1209](https://github.com/lambdaclass/cairo-vm/pull/1209)
  Fixes [#1205](https://github.com/lambdaclass/cairo-vm/issue/1205)

* fix: fix hints `UINT256_UNSIGNED_DIV_REM` && `UINT256_EXPANDED_UNSIGNED_DIV_REM` [#1203](https://github.com/lambdaclass/cairo-vm/pull/1203)

* bugfix: fix deserialization of scientific notation with fractional values [#1202](https://github.com/lambdaclass/cairo-vm/pull/1202)

* feat: implement `mem_eq` function to test for equality of two ranges in memory [#1198](https://github.com/lambdaclass/cairo-vm/pull/1198)

* perf: use `mem_eq` in `set_add` [#1198](https://github.com/lambdaclass/cairo-vm/pull/1198)

* feat: wrap big variants of `HintError`, `VirtualMachineError`, `RunnerError`, `MemoryError`, `MathError`, `InsufficientAllocatedCellsError` in `Box` [#1193](https://github.com/lambdaclass/cairo-vm/pull/1193)
  * BREAKING: all tuple variants of `HintError` with a single `Felt252` or multiple elements now receive a single `Box`

* Add `Program::builtins_len method` [#1194](https://github.com/lambdaclass/cairo-vm/pull/1194)

* fix: Handle the deserialization of serde_json::Number with scientific notation (e.g.: Number(1e27)) in felt_from_number function [#1188](https://github.com/lambdaclass/cairo-vm/pull/1188)

* feat: Add RunResources Struct [#1175](https://github.com/lambdaclass/cairo-vm/pull/1175)
  * BREAKING: Modify `CairoRunner::run_until_pc` arity. Add `run_resources: &mut Option<RunResources>` input
  * BREAKING: Modify `CairoRunner::run_from_entrypoint` arity. Add `run_resources: &mut Option<RunResources>` input

* fix: Fix 'as_int' conversion usage in hints `ASSERT_250_BIT` &  `SIGNED_DIV_REM` [#1191](https://github.com/lambdaclass/cairo-vm/pull/1191)


* bugfix: Use cairo constants in `ASSERT_250_BIT` hint [#1187](https://github.com/lambdaclass/cairo-vm/pull/1187)

* bugfix: Fix `EC_DOUBLE_ASSIGN_NEW_X_V2` hint not taking `SECP_P` value from the current execution scope [#1186](https://github.com/lambdaclass/cairo-vm/pull/1186)

* fix: Fix hint `BIGINT_PACK_DIV_MOD` [#1189](https://github.com/lambdaclass/cairo-vm/pull/1189)

* fix: Fix possible subtraction overflow in `QUAD_BIT` & `DI_BIT` hints [#1185](https://github.com/lambdaclass/cairo-vm/pull/1185)

  * These hints now return an error when ids.m equals zero

* fix: felt_from_number not properly returning parse errors [#1012](https://github.com/lambdaclass/cairo-vm/pull/1012)

* fix: Fix felt sqrt and Signed impl [#1150](https://github.com/lambdaclass/cairo-vm/pull/1150)

  * BREAKING: Fix `Felt252` methods `abs`, `signum`, `is_positive`, `is_negative` and `sqrt`
  * BREAKING: Remove function `math_utils::sqrt`(Now moved to `Felt252::sqrt`)

* feat: Add method `CairoRunner::initialize_function_runner_cairo_1` [#1151](https://github.com/lambdaclass/cairo-vm/pull/1151)

  * Add method `pub fn initialize_function_runner_cairo_1(
        &mut self,
        vm: &mut VirtualMachine,
        program_builtins: &[BuiltinName],
    ) -> Result<(), RunnerError>` to `CairoRunner`

  * BREAKING: Move field `builtins` from `SharedProgramData` to `Program`
  * BREAKING: Remove argument `add_segment_arena_builtin` from `CairoRunner::initialize_function_runner`, it is now always false
  * BREAKING: Add `segment_arena` enum variant to `BuiltinName`

* Fix implementation of `InitSquashData` and `ShouldSkipSquashLoop`

* Add more hints to `Cairo1HintProcessor` [#1171](https://github.com/lambdaclass/cairo-vm/pull/1171)
                                          [#1143](https://github.com/lambdaclass/cairo-vm/pull/1143)

    * `Cairo1HintProcessor` can now run the following hints:
        * Felt252DictEntryInit
        * Felt252DictEntryUpdate
        * GetCurrentAccessDelta
        * InitSquashData
        * AllocConstantSize
        * GetCurrentAccessIndex
        * ShouldContinueSquashLoop
        * FieldSqrt
        * Uint512DivMod

* Add some small considerations regarding Cairo 1 programs [#1144](https://github.com/lambdaclass/cairo-vm/pull/1144):

  * Ignore Casm and Sierra files
  * Add special flag to compile Cairo 1 programs

* Make the VM able to run `CasmContractClass` files under `cairo-1-hints` feature [#1098](https://github.com/lambdaclass/cairo-vm/pull/1098)

  * Implement `TryFrom<CasmContractClass> for Program`
  * Add `Cairo1HintProcessor`

#### 0.5.0
**YANKED**

#### [0.4.0] - 2023-05-12

* perf: insert elements from the tail in `load_data` so reallocation happens only once [#1117](https://github.com/lambdaclass/cairo-vm/pull/1117)

* Add `CairoRunner::get_program method` [#1123](https://github.com/lambdaclass/cairo-vm/pull/1123)

* Use to_signed_felt as function for felt252 as BigInt within [-P/2, P/2] range and use to_bigint as function for representation as BigInt. [#1100](https://github.com/lambdaclass/cairo-vm/pull/1100)

* Implement hint on field_arithmetic lib [#1090](https://github.com/lambdaclass/cairo-vm/pull/1090)

    `BuiltinHintProcessor` now supports the following hints:

    ```python
        %{
            def split(num: int, num_bits_shift: int, length: int):
                a = []
                for _ in range(length):
                    a.append( num & ((1 << num_bits_shift) - 1) )
                    num = num >> num_bits_shift
                return tuple(a)

            def pack(z, num_bits_shift: int) -> int:
                limbs = (z.d0, z.d1, z.d2)
                return sum(limb << (num_bits_shift * i) for i, limb in enumerate(limbs))

            a = pack(ids.a, num_bits_shift = 128)
            b = pack(ids.b, num_bits_shift = 128)
            p = pack(ids.p, num_bits_shift = 128)

            res = (a - b) % p


            res_split = split(res, num_bits_shift=128, length=3)

            ids.res.d0 = res_split[0]
            ids.res.d1 = res_split[1]
            ids.res.d2 = res_split[2]
        %}
    ```

* Add missing hint on cairo_secp lib [#1089](https://github.com/lambdaclass/cairo-vm/pull/1089):
    `BuiltinHintProcessor` now supports the following hint:

    ```python

    from starkware.cairo.common.cairo_secp.secp_utils import pack

    slope = pack(ids.slope, PRIME)
    x0 = pack(ids.point0.x, PRIME)
    x1 = pack(ids.point1.x, PRIME)
    y0 = pack(ids.point0.y, PRIME)

    value = new_x = (pow(slope, 2, SECP_P) - x0 - x1) % SECP_P
    ```

* Add missing hint on vrf.json whitelist [#1055](https://github.com/lambdaclass/cairo-vm/pull/1055):

     `BuiltinHintProcessor` now supports the following hint:

     ```python
    %{
        PRIME = 2**255 - 19
        II = pow(2, (PRIME - 1) // 4, PRIME)

        xx = ids.xx.low + (ids.xx.high<<128)
        x = pow(xx, (PRIME + 3) // 8, PRIME)
        if (x * x - xx) % PRIME != 0:
            x = (x * II) % PRIME
        if x % 2 != 0:
            x = PRIME - x
        ids.x.low = x & ((1<<128)-1)
        ids.x.high = x >> 128
    %}
    ```

* Implement hint variant for finalize_blake2s[#1072](https://github.com/lambdaclass/cairo-vm/pull/1072)

    `BuiltinHintProcessor` now supports the following hint:

     ```python
    %{
        # Add dummy pairs of input and output.
        from starkware.cairo.common.cairo_blake2s.blake2s_utils import IV, blake2s_compress

        _n_packed_instances = int(ids.N_PACKED_INSTANCES)
        assert 0 <= _n_packed_instances < 20
        _blake2s_input_chunk_size_felts = int(ids.BLAKE2S_INPUT_CHUNK_SIZE_FELTS)
        assert 0 <= _blake2s_input_chunk_size_felts < 100

        message = [0] * _blake2s_input_chunk_size_felts
        modified_iv = [IV[0] ^ 0x01010020] + IV[1:]
        output = blake2s_compress(
            message=message,
            h=modified_iv,
            t0=0,
            t1=0,
            f0=0xffffffff,
            f1=0,
        )
        padding = (message + modified_iv + [0, 0xffffffff] + output) * (_n_packed_instances - 1)
        segments.write_arg(ids.blake2s_ptr_end, padding)
        %}
        ```

* Implement fast_ec_add hint variant [#1087](https://github.com/lambdaclass/cairo-vm/pull/1087)

`BuiltinHintProcessor` now supports the following hint:

    ```python
    %{
        from starkware.cairo.common.cairo_secp.secp_utils import SECP_P, pack

        slope = pack(ids.slope, PRIME)
        x0 = pack(ids.pt0.x, PRIME)
        x1 = pack(ids.pt1.x, PRIME)
        y0 = pack(ids.pt0.y, PRIME)

        value = new_x = (pow(slope, 2, SECP_P) - x0 - x1) % SECP_P
    %}
    ```

* feat(hints): Add alternative string for hint IS_ZERO_PACK_EXTERNAL_SECP [#1082](https://github.com/lambdaclass/cairo-vm/pull/1082)

    `BuiltinHintProcessor` now supports the following hint:

    ```python
    %{
        from starkware.cairo.common.cairo_secp.secp_utils import pack
        x = pack(ids.x, PRIME) % SECP_P
    %}
    ```

* Add alternative hint code for ec_double hint [#1083](https://github.com/lambdaclass/cairo-vm/pull/1083)

    `BuiltinHintProcessor` now supports the following hint:

    ```python
    %{
        from starkware.cairo.common.cairo_secp.secp_utils import SECP_P, pack

        slope = pack(ids.slope, PRIME)
        x = pack(ids.pt.x, PRIME)
        y = pack(ids.pt.y, PRIME)

        value = new_x = (pow(slope, 2, SECP_P) - 2 * x) % SECP_P
    %}
    ```

* fix(security)!: avoid DoS on malicious insertion to memory [#1099](https://github.com/lambdaclass/cairo-vm/pull/1099)
    * A program could crash the library by attempting to insert a value at an address with a big offset; fixed by trying to reserve to check for allocation failure
    * A program could crash the program by exploiting an integer overflow when attempting to insert a value at an address with offset `usize::MAX`

    BREAKING: added a new error variant `MemoryError::VecCapacityExceeded`

* perf: specialize addition for `u64` and `Felt252` [#932](https://github.com/lambdaclass/cairo-vm/pull/932)
    * Avoids the creation of a new `Felt252` instance for additions with a very restricted valid range
    * This impacts specially the addition of `Relocatable` with `Felt252` values in `update_pc`, which take a significant amount of time in some benchmarks

* fix(starknet-crypto): bump version to `0.5.0` [#1088](https://github.com/lambdaclass/cairo-vm/pull/1088)
    * This includes the fix for a `panic!` in `ecdsa::verify`.
      See: [#365](https://github.com/xJonathanLEI/starknet-rs/issues/365) and [#366](https://github.com/xJonathanLEI/starknet-rs/pulls/366)

* feat(hints): Add alternative string for hint IS_ZERO_PACK [#1081](https://github.com/lambdaclass/cairo-vm/pull/1081)

    `BuiltinHintProcessor` now supports the following hint:

    ```python
    %{
        from starkware.cairo.common.cairo_secp.secp_utils import SECP_P, pack
        x = pack(ids.x, PRIME) % SECP_P
    %}

* Add missing hints `NewHint#55`, `NewHint#56`, and `NewHint#57` [#1077](https://github.com/lambdaclass/cairo-vm/issues/1077)

    `BuiltinHintProcessor` now supports the following hints:

    ```python
    from starkware.cairo.common.cairo_secp.secp_utils import pack
    SECP_P=2**255-19

    x = pack(ids.x, PRIME) % SECP_P
    ```

    ```python
    from starkware.cairo.common.cairo_secp.secp_utils import pack
    SECP_P=2**255-19

    value = pack(ids.x, PRIME) % SECP_P
    ```

    ```python
    SECP_P=2**255-19
    from starkware.python.math_utils import div_mod

    value = x_inv = div_mod(1, x, SECP_P)
    ```

* Implement hint for `starkware.cairo.common.cairo_keccak.keccak._copy_inputs` as described by whitelist `starknet/security/whitelists/cairo_keccak.json` [#1058](https://github.com/lambdaclass/cairo-vm/pull/1058)

    `BuiltinHintProcessor` now supports the following hint:

    ```python
    %{ ids.full_word = int(ids.n_bytes >= 8) %}
    ```

* perf: cache decoded instructions [#944](https://github.com/lambdaclass/cairo-vm/pull/944)
    * Creates a new cache field in `VirtualMachine` that stores the `Instruction` instances as they get decoded from memory, significantly reducing decoding overhead, with gains up to 9% in runtime according to benchmarks in the performance server

* Add alternative hint code for nondet_bigint3 hint [#1071](https://github.com/lambdaclass/cairo-vm/pull/1071)

    `BuiltinHintProcessor` now supports the following hint:

    ```python
    %{
        from starkware.cairo.common.cairo_secp.secp_utils import split
        segments.write_arg(ids.res.address_, split(value))
    %}
    ```

* Add missing hint on vrf.json lib [#1052](https://github.com/lambdaclass/cairo-vm/pull/1052):

    `BuiltinHintProcessor` now supports the following hint:

    ```python
    %{
        from starkware.cairo.common.cairo_secp.secp_utils import pack
        SECP_P = 2**255-19

        slope = pack(ids.slope, PRIME)
        x0 = pack(ids.point0.x, PRIME)
        x1 = pack(ids.point1.x, PRIME)
        y0 = pack(ids.point0.y, PRIME)

        value = new_x = (pow(slope, 2, SECP_P) - x0 - x1) % SECP_P
    %}
    ```

* Implement hint for cairo_sha256_arbitrary_input_length whitelist [#1091](https://github.com/lambdaclass/cairo-vm/pull/1091)

    `BuiltinHintProcessor` now supports the following hint:

    ```python
    %{
        from starkware.cairo.common.cairo_sha256.sha256_utils import (
            compute_message_schedule, sha2_compress_function)

        _sha256_input_chunk_size_felts = int(ids.SHA256_INPUT_CHUNK_SIZE_FELTS)
        assert 0 <= _sha256_input_chunk_size_felts < 100
        _sha256_state_size_felts = int(ids.SHA256_STATE_SIZE_FELTS)
        assert 0 <= _sha256_state_size_felts < 100
        w = compute_message_schedule(memory.get_range(
            ids.sha256_start, _sha256_input_chunk_size_felts))
        new_state = sha2_compress_function(memory.get_range(ids.state, _sha256_state_size_felts), w)
        segments.write_arg(ids.output, new_state)
    %}
    ```

* Add missing hint on vrf.json lib [#1053](https://github.com/lambdaclass/cairo-vm/pull/1053):

     `BuiltinHintProcessor` now supports the following hint:

     ```python
    %{
        from starkware.cairo.common.cairo_secp.secp_utils import SECP_P, pack
        SECP_P = 2**255-19

        slope = pack(ids.slope, PRIME)
        x = pack(ids.point.x, PRIME)
        y = pack(ids.point.y, PRIME)

        value = new_x = (pow(slope, 2, SECP_P) - 2 * x) % SECP_P
    %}
    ```

* Implement hint on 0.6.0.json whitelist [#1044](https://github.com/lambdaclass/cairo-vm/pull/1044):

     `BuiltinHintProcessor` now supports the following hints:

    ```python
    %{
       ids.a_lsb = ids.a & 1
       ids.b_lsb = ids.b & 1
    %}
    ```

* Implement hint for `starkware.cairo.common.cairo_keccak.keccak._block_permutation` as described by whitelist `starknet/security/whitelists/cairo_keccak.json` [#1046](https://github.com/lambdaclass/cairo-vm/pull/1046)

    `BuiltinHintProcessor` now supports the following hint:

    ```python
    %{
        from starkware.cairo.common.cairo_keccak.keccak_utils import keccak_func
        _keccak_state_size_felts = int(ids.KECCAK_STATE_SIZE_FELTS)
        assert 0 <= _keccak_state_size_felts < 100
        output_values = keccak_func(memory.get_range(
            ids.keccak_ptr_start, _keccak_state_size_felts))
        segments.write_arg(ids.output, output_values)
    %}
    ```

* Implement hint on cairo_blake2s whitelist [#1040](https://github.com/lambdaclass/cairo-vm/pull/1040)

    `BuiltinHintProcessor` now supports the following hint:

    ```python
    %{
        from starkware.cairo.common.cairo_blake2s.blake2s_utils import IV, blake2s_compress

        _blake2s_input_chunk_size_felts = int(ids.BLAKE2S_INPUT_CHUNK_SIZE_FELTS)
        assert 0 <= _blake2s_input_chunk_size_felts < 100

        new_state = blake2s_compress(
            message=memory.get_range(ids.blake2s_start, _blake2s_input_chunk_size_felts),
            h=[IV[0] ^ 0x01010020] + IV[1:],
            t0=ids.n_bytes,
            t1=0,
            f0=0xffffffff,
            f1=0,
        )

        segments.write_arg(ids.output, new_state)
    %}
    ```

* Implement hint on cairo_blake2s whitelist [#1039](https://github.com/lambdaclass/cairo-vm/pull/1039)

    `BuiltinHintProcessor` now supports the following hint:

    ```python

    %{
        # Add dummy pairs of input and output.
        from starkware.cairo.common.cairo_blake2s.blake2s_utils import IV, blake2s_compress

        _n_packed_instances = int(ids.N_PACKED_INSTANCES)
        assert 0 <= _n_packed_instances < 20
        _blake2s_input_chunk_size_felts = int(ids.BLAKE2S_INPUT_CHUNK_SIZE_FELTS)
        assert 0 <= _blake2s_input_chunk_size_felts < 100

        message = [0] * _blake2s_input_chunk_size_felts
        modified_iv = [IV[0] ^ 0x01010020] + IV[1:]
        output = blake2s_compress(
            message=message,
            h=modified_iv,
            t0=0,
            t1=0,
            f0=0xffffffff,
            f1=0,
        )
        padding = (modified_iv + message + [0, 0xffffffff] + output) * (_n_packed_instances - 1)
        segments.write_arg(ids.blake2s_ptr_end, padding)
    %}

* Add `Program::iter_identifiers(&self) -> Iterator<Item = (&str, &Identifier)>` to get an iterator over the program's identifiers [#1079](https://github.com/lambdaclass/cairo-vm/pull/1079)

* Implement hint on `assert_le_felt` for versions 0.6.0 and 0.8.2 [#1047](https://github.com/lambdaclass/cairo-vm/pull/1047):

     `BuiltinHintProcessor` now supports the following hints:

     ```python

     %{
        from starkware.cairo.common.math_utils import assert_integer
        assert_integer(ids.a)
        assert_integer(ids.b)
        assert (ids.a % PRIME) <= (ids.b % PRIME), \
            f'a = {ids.a % PRIME} is not less than or equal to b = {ids.b % PRIME}.'
    %}

     ```

     ```python

    %{
        from starkware.cairo.common.math_utils import assert_integer
        assert_integer(ids.a)
        assert_integer(ids.b)
        a = ids.a % PRIME
        b = ids.b % PRIME
        assert a <= b, f'a = {a} is not less than or equal to b = {b}.'

        ids.small_inputs = int(
            a < range_check_builtin.bound and (b - a) < range_check_builtin.bound)
    %}

     ```

* Add missing hints on whitelist [#1073](https://github.com/lambdaclass/cairo-vm/pull/1073):

    `BuiltinHintProcessor` now supports the following hints:

    ```python
        ids.is_250 = 1 if ids.addr < 2**250 else 0
    ```

    ```python
        # Verify the assumptions on the relationship between 2**250, ADDR_BOUND and PRIME.
        ADDR_BOUND = ids.ADDR_BOUND % PRIME
        assert (2**250 < ADDR_BOUND <= 2**251) and (2 * 2**250 < PRIME) and (
                ADDR_BOUND * 2 > PRIME), \
            'normalize_address() cannot be used with the current constants.'
        ids.is_small = 1 if ids.addr < ADDR_BOUND else 0
    ```

* Implement hint on ec_recover.json whitelist [#1038](https://github.com/lambdaclass/cairo-vm/pull/1038):

    `BuiltinHintProcessor` now supports the following hint:

    ```python
    %{
         value = k = product // m
    %}
    ```

* Implement hint on ec_recover.json whitelist [#1037](https://github.com/lambdaclass/cairo-vm/pull/1037):

    `BuiltinHintProcessor` now supports the following hint:

    ```python
    %{
        from starkware.cairo.common.cairo_secp.secp_utils import pack
        from starkware.python.math_utils import div_mod, safe_div

        a = pack(ids.a, PRIME)
        b = pack(ids.b, PRIME)
        product = a * b
        m = pack(ids.m, PRIME)

        value = res = product % m

    %}
    ```

* Implement hint for `starkware.cairo.common.cairo_keccak.keccak.finalize_keccak` as described by whitelist `starknet/security/whitelists/cairo_keccak.json` [#1041](https://github.com/lambdaclass/cairo-vm/pull/1041)

    `BuiltinHintProcessor` now supports the following hint:

    ```python
    %{
        # Add dummy pairs of input and output.
        _keccak_state_size_felts = int(ids.KECCAK_STATE_SIZE_FELTS)
        _block_size = int(ids.BLOCK_SIZE)
        assert 0 <= _keccak_state_size_felts < 100
        assert 0 <= _block_size < 1000
        inp = [0] * _keccak_state_size_felts
        padding = (inp + keccak_func(inp)) * _block_size
        segments.write_arg(ids.keccak_ptr_end, padding)
    %}
    ```

* Implement hint on ec_recover.json whitelist [#1036](https://github.com/lambdaclass/cairo-vm/pull/1036):

    `BuiltinHintProcessor` now supports the following hint:

    ```python

    %{
        from starkware.cairo.common.cairo_secp.secp_utils import pack
        from starkware.python.math_utils import div_mod, safe_div

        a = pack(ids.a, PRIME)
        b = pack(ids.b, PRIME)

        value = res = a - b
    %}

    ```

* Add missing hint on vrf.json lib [#1054](https://github.com/lambdaclass/cairo-vm/pull/1054):

    `BuiltinHintProcessor` now supports the following hint:

    ```python
        from starkware.cairo.common.cairo_secp.secp_utils import pack
        SECP_P = 2**255-19

        y = pack(ids.point.y, PRIME) % SECP_P
        # The modulo operation in python always returns a nonnegative number.
        value = (-y) % SECP_P
    ```

* Implement hint on ec_recover.json whitelist [#1032](https://github.com/lambdaclass/cairo-vm/pull/1032):

    `BuiltinHintProcessor` now supports the following hint:

    ```python
    %{
        from starkware.cairo.common.cairo_secp.secp_utils import pack
        from starkware.python.math_utils import div_mod, safe_div

        N = pack(ids.n, PRIME)
        x = pack(ids.x, PRIME) % N
        s = pack(ids.s, PRIME) % N,
        value = res = div_mod(x, s, N)
    %}
    ```

* Implement hints on field_arithmetic lib (Part 2) [#1004](https://github.com/lambdaclass/cairo-vm/pull/1004)

    `BuiltinHintProcessor` now supports the following hint:

    ```python
    %{
        from starkware.python.math_utils import div_mod

        def split(num: int, num_bits_shift: int, length: int):
            a = []
            for _ in range(length):
                a.append( num & ((1 << num_bits_shift) - 1) )
                num = num >> num_bits_shift
            return tuple(a)

        def pack(z, num_bits_shift: int) -> int:
            limbs = (z.d0, z.d1, z.d2)
            return sum(limb << (num_bits_shift * i) for i, limb in enumerate(limbs))

        a = pack(ids.a, num_bits_shift = 128)
        b = pack(ids.b, num_bits_shift = 128)
        p = pack(ids.p, num_bits_shift = 128)
        # For python3.8 and above the modular inverse can be computed as follows:
        # b_inverse_mod_p = pow(b, -1, p)
        # Instead we use the python3.7-friendly function div_mod from starkware.python.math_utils
        b_inverse_mod_p = div_mod(1, b, p)


        b_inverse_mod_p_split = split(b_inverse_mod_p, num_bits_shift=128, length=3)

        ids.b_inverse_mod_p.d0 = b_inverse_mod_p_split[0]
        ids.b_inverse_mod_p.d1 = b_inverse_mod_p_split[1]
        ids.b_inverse_mod_p.d2 = b_inverse_mod_p_split[2]
    %}
    ```

* Optimizations for hash builtin [#1029](https://github.com/lambdaclass/cairo-vm/pull/1029):
  * Track the verified addresses by offset in a `Vec<bool>` rather than storing the address in a `Vec<Relocatable>`

* Add missing hint on vrf.json whitelist [#1056](https://github.com/lambdaclass/cairo-vm/pull/1056):

    `BuiltinHintProcessor` now supports the following hint:

    ```python
    %{
        from starkware.python.math_utils import ec_double_slope
        from starkware.cairo.common.cairo_secp.secp_utils import pack
        SECP_P = 2**255-19

        # Compute the slope.
        x = pack(ids.point.x, PRIME)
        y = pack(ids.point.y, PRIME)
        value = slope = ec_double_slope(point=(x, y), alpha=42204101795669822316448953119945047945709099015225996174933988943478124189485, p=SECP_P)
    %}
    ```

* Add missing hint on vrf.json whitelist [#1035](https://github.com/lambdaclass/cairo-vm/pull/1035):

    `BuiltinHintProcessor` now supports the following hint:

    ```python
    %{
        from starkware.python.math_utils import line_slope
        from starkware.cairo.common.cairo_secp.secp_utils import pack
        SECP_P = 2**255-19
        # Compute the slope.
        x0 = pack(ids.point0.x, PRIME)
        y0 = pack(ids.point0.y, PRIME)
        x1 = pack(ids.point1.x, PRIME)
        y1 = pack(ids.point1.y, PRIME)
        value = slope = line_slope(point1=(x0, y0), point2=(x1, y1), p=SECP_P)
    %}
    ```

* Add missing hint on vrf.json whitelist [#1035](https://github.com/lambdaclass/cairo-vm/pull/1035):

    `BuiltinHintProcessor` now supports the following hint:

    ```python
    %{
        from starkware.cairo.common.cairo_secp.secp_utils import pack
        SECP_P = 2**255-19
        to_assert = pack(ids.val, PRIME)
        q, r = divmod(pack(ids.val, PRIME), SECP_P)
        assert r == 0, f"verify_zero: Invalid input {ids.val.d0, ids.val.d1, ids.val.d2}."
        ids.q = q % PRIME
    %}
    ```

* Add missing hint on vrf.json whitelist [#1000](https://github.com/lambdaclass/cairo-vm/pull/1000):

    `BuiltinHintProcessor` now supports the following hint:

    ```python
        def pack_512(u, num_bits_shift: int) -> int:
            limbs = (u.d0, u.d1, u.d2, u.d3)
            return sum(limb << (num_bits_shift * i) for i, limb in enumerate(limbs))

        x = pack_512(ids.x, num_bits_shift = 128)
        p = ids.p.low + (ids.p.high << 128)
        x_inverse_mod_p = pow(x,-1, p)

        x_inverse_mod_p_split = (x_inverse_mod_p & ((1 << 128) - 1), x_inverse_mod_p >> 128)

        ids.x_inverse_mod_p.low = x_inverse_mod_p_split[0]
        ids.x_inverse_mod_p.high = x_inverse_mod_p_split[1]
    ```

* BREAKING CHANGE: Fix `CairoRunner::get_memory_holes` [#1027](https://github.com/lambdaclass/cairo-vm/pull/1027):

  * Skip builtin segements when counting memory holes
  * Check amount of memory holes for all tests in cairo_run_test
  * Remove duplicated tests in cairo_run_test
  * BREAKING CHANGE: `MemorySegmentManager.get_memory_holes` now also receives the amount of builtins in the vm. Signature is now `pub fn get_memory_holes(&self, builtin_count: usize) -> Result<usize, MemoryError>`

* Add missing hints on cairo_secp lib [#1026](https://github.com/lambdaclass/cairo-vm/pull/1026):

    `BuiltinHintProcessor` now supports the following hints:

    ```python
    from starkware.cairo.common.cairo_secp.secp256r1_utils import SECP256R1_ALPHA as ALPHA
    ```
    and:

    ```python
    from starkware.cairo.common.cairo_secp.secp256r1_utils import SECP256R1_N as N
    ```

* Add missing hint on vrf.json lib [#1043](https://github.com/lambdaclass/cairo-vm/pull/1043):

    `BuiltinHintProcessor` now supports the following hint:

    ```python
        from starkware.python.math_utils import div_mod

        def split(a: int):
            return (a & ((1 << 128) - 1), a >> 128)

        def pack(z, num_bits_shift: int) -> int:
            limbs = (z.low, z.high)
            return sum(limb << (num_bits_shift * i) for i, limb in enumerate(limbs))

        a = pack(ids.a, 128)
        b = pack(ids.b, 128)
        p = pack(ids.p, 128)
        # For python3.8 and above the modular inverse can be computed as follows:
        # b_inverse_mod_p = pow(b, -1, p)
        # Instead we use the python3.7-friendly function div_mod from starkware.python.math_utils
        b_inverse_mod_p = div_mod(1, b, p)

        b_inverse_mod_p_split = split(b_inverse_mod_p)

        ids.b_inverse_mod_p.low = b_inverse_mod_p_split[0]
        ids.b_inverse_mod_p.high = b_inverse_mod_p_split[1]
    ```

* Add missing hints `NewHint#35` and `NewHint#36` [#975](https://github.com/lambdaclass/cairo-vm/issues/975)

    `BuiltinHintProcessor` now supports the following hint:

    ```python
    from starkware.cairo.common.cairo_secp.secp_utils import pack
    from starkware.cairo.common.math_utils import as_int
    from starkware.python.math_utils import div_mod, safe_div

    p = pack(ids.P, PRIME)
    x = pack(ids.x, PRIME) + as_int(ids.x.d3, PRIME) * ids.BASE ** 3 + as_int(ids.x.d4, PRIME) * ids.BASE ** 4
    y = pack(ids.y, PRIME)

    value = res = div_mod(x, y, p)
    ```

    ```python
    k = safe_div(res * y - x, p)
    value = k if k > 0 else 0 - k
    ids.flag = 1 if k > 0 else 0
    ```

* Add missing hint on cairo_secp lib [#1057](https://github.com/lambdaclass/cairo-vm/pull/1057):

    `BuiltinHintProcessor` now supports the following hint:

    ```python
        from starkware.cairo.common.cairo_secp.secp_utils import pack
        from starkware.python.math_utils import ec_double_slope

        # Compute the slope.
        x = pack(ids.point.x, PRIME)
        y = pack(ids.point.y, PRIME)
        value = slope = ec_double_slope(point=(x, y), alpha=ALPHA, p=SECP_P)
    ```

* Add missing hint on uint256_improvements lib [#1025](https://github.com/lambdaclass/cairo-vm/pull/1025):

    `BuiltinHintProcessor` now supports the following hint:

    ```python
        from starkware.python.math_utils import isqrt
        n = (ids.n.high << 128) + ids.n.low
        root = isqrt(n)
        assert 0 <= root < 2 ** 128
        ids.root = root
    ```

* Add missing hint on vrf.json lib [#1045](https://github.com/lambdaclass/cairo-vm/pull/1045):

    `BuiltinHintProcessor` now supports the following hint:

    ```python
        from starkware.python.math_utils import is_quad_residue, sqrt

        def split(a: int):
            return (a & ((1 << 128) - 1), a >> 128)

        def pack(z) -> int:
            return z.low + (z.high << 128)

        generator = pack(ids.generator)
        x = pack(ids.x)
        p = pack(ids.p)

        success_x = is_quad_residue(x, p)
        root_x = sqrt(x, p) if success_x else None
        success_gx = is_quad_residue(generator*x, p)
        root_gx = sqrt(generator*x, p) if success_gx else None

        # Check that one is 0 and the other is 1
        if x != 0:
            assert success_x + success_gx == 1

        # `None` means that no root was found, but we need to transform these into a felt no matter what
        if root_x == None:
            root_x = 0
        if root_gx == None:
            root_gx = 0
        ids.success_x = int(success_x)
        ids.success_gx = int(success_gx)
        split_root_x = split(root_x)
        # print('split root x', split_root_x)
        split_root_gx = split(root_gx)
        ids.sqrt_x.low = split_root_x[0]
        ids.sqrt_x.high = split_root_x[1]
        ids.sqrt_gx.low = split_root_gx[0]
        ids.sqrt_gx.high = split_root_gx[1]
    ```

* Add missing hint on uint256_improvements lib [#1024](https://github.com/lambdaclass/cairo-vm/pull/1024):

    `BuiltinHintProcessor` now supports the following hint:

    ```python
        res = ids.a + ids.b
        ids.carry = 1 if res >= ids.SHIFT else 0
    ```

* BREAKING CHANGE: move `Program::identifiers` to `SharedProgramData::identifiers` [#1023](https://github.com/lambdaclass/cairo-vm/pull/1023)
    * Optimizes `CairoRunner::new`, needed for sequencers and other workflows reusing the same `Program` instance across `CairoRunner`s
    * Breaking change: make all fields in `Program` and `SharedProgramData` `pub(crate)`, since we break by moving the field let's make it the last break for this struct
    * Add `Program::get_identifier(&self, id: &str) -> &Identifier` to get a single identifier by name

* Implement hints on field_arithmetic lib[#985](https://github.com/lambdaclass/cairo-vm/pull/983)

    `BuiltinHintProcessor` now supports the following hint:

    ```python
        %{
            from starkware.python.math_utils import is_quad_residue, sqrt

            def split(num: int, num_bits_shift: int = 128, length: int = 3):
                a = []
                for _ in range(length):
                    a.append( num & ((1 << num_bits_shift) - 1) )
                    num = num >> num_bits_shift
                return tuple(a)

            def pack(z, num_bits_shift: int = 128) -> int:
                limbs = (z.d0, z.d1, z.d2)
                return sum(limb << (num_bits_shift * i) for i, limb in enumerate(limbs))


            generator = pack(ids.generator)
            x = pack(ids.x)
            p = pack(ids.p)

            success_x = is_quad_residue(x, p)
            root_x = sqrt(x, p) if success_x else None

            success_gx = is_quad_residue(generator*x, p)
            root_gx = sqrt(generator*x, p) if success_gx else None

            # Check that one is 0 and the other is 1
            if x != 0:
                assert success_x + success_gx ==1

            # `None` means that no root was found, but we need to transform these into a felt no matter what
            if root_x == None:
                root_x = 0
            if root_gx == None:
                root_gx = 0
            ids.success_x = int(success_x)
            ids.success_gx = int(success_gx)
            split_root_x = split(root_x)
            split_root_gx = split(root_gx)
            ids.sqrt_x.d0 = split_root_x[0]
            ids.sqrt_x.d1 = split_root_x[1]
            ids.sqrt_x.d2 = split_root_x[2]
            ids.sqrt_gx.d0 = split_root_gx[0]
            ids.sqrt_gx.d1 = split_root_gx[1]
            ids.sqrt_gx.d2 = split_root_gx[2]
        %}
    ```

* Add missing hint on vrf.json lib [#1050](https://github.com/lambdaclass/cairo-vm/pull/1050):

    `BuiltinHintProcessor` now supports the following hint:

    ```python
        sum_low = ids.a.low + ids.b.low
        ids.carry_low = 1 if sum_low >= ids.SHIFT else 0
    ```

* Add missing hint on uint256_improvements lib [#1016](https://github.com/lambdaclass/cairo-vm/pull/1016):

    `BuiltinHintProcessor` now supports the following hint:

    ```python
        def split(num: int, num_bits_shift: int = 128, length: int = 2):
            a = []
            for _ in range(length):
                a.append( num & ((1 << num_bits_shift) - 1) )
                num = num >> num_bits_shift
            return tuple(a)

        def pack(z, num_bits_shift: int = 128) -> int:
            limbs = (z.low, z.high)
            return sum(limb << (num_bits_shift * i) for i, limb in enumerate(limbs))

        a = pack(ids.a)
        b = pack(ids.b)
        res = (a - b)%2**256
        res_split = split(res)
        ids.res.low = res_split[0]
        ids.res.high = res_split[1]
    ```

* Implement hint on vrf.json lib [#1049](https://github.com/lambdaclass/cairo-vm/pull/1049)

    `BuiltinHintProcessor` now supports the following hint:

    ```python
        def split(num: int, num_bits_shift: int, length: int):
            a = []
            for _ in range(length):
                a.append( num & ((1 << num_bits_shift) - 1) )
                num = num >> num_bits_shift
            return tuple(a)

        def pack(z, num_bits_shift: int) -> int:
            limbs = (z.d0, z.d1, z.d2)
            return sum(limb << (num_bits_shift * i) for i, limb in enumerate(limbs))

        def pack_extended(z, num_bits_shift: int) -> int:
            limbs = (z.d0, z.d1, z.d2, z.d3, z.d4, z.d5)
            return sum(limb << (num_bits_shift * i) for i, limb in enumerate(limbs))

        a = pack_extended(ids.a, num_bits_shift = 128)
        div = pack(ids.div, num_bits_shift = 128)

        quotient, remainder = divmod(a, div)

        quotient_split = split(quotient, num_bits_shift=128, length=6)

        ids.quotient.d0 = quotient_split[0]
        ids.quotient.d1 = quotient_split[1]
        ids.quotient.d2 = quotient_split[2]
        ids.quotient.d3 = quotient_split[3]
        ids.quotient.d4 = quotient_split[4]
        ids.quotient.d5 = quotient_split[5]

        remainder_split = split(remainder, num_bits_shift=128, length=3)
        ids.remainder.d0 = remainder_split[0]
        ids.remainder.d1 = remainder_split[1]
        ids.remainder.d2 = remainder_split[2]
    ```

    _Note: this hint is similar to the one in #983, but with some trailing whitespace removed_

* Add missing hint on vrf.json whitelist [#1030](https://github.com/lambdaclass/cairo-vm/pull/1030):

    `BuiltinHintProcessor` now supports the following hint:

    ```python
        def split(num: int, num_bits_shift: int, length: int):
            a = []
            for _ in range(length):
                a.append( num & ((1 << num_bits_shift) - 1) )
                num = num >> num_bits_shift
            return tuple(a)

        def pack(z, num_bits_shift: int) -> int:
            limbs = (z.low, z.high)
            return sum(limb << (num_bits_shift * i) for i, limb in enumerate(limbs))

        def pack_extended(z, num_bits_shift: int) -> int:
            limbs = (z.d0, z.d1, z.d2, z.d3)
            return sum(limb << (num_bits_shift * i) for i, limb in enumerate(limbs))

        x = pack_extended(ids.x, num_bits_shift = 128)
        div = pack(ids.div, num_bits_shift = 128)

        quotient, remainder = divmod(x, div)

        quotient_split = split(quotient, num_bits_shift=128, length=4)

        ids.quotient.d0 = quotient_split[0]
        ids.quotient.d1 = quotient_split[1]
        ids.quotient.d2 = quotient_split[2]
        ids.quotient.d3 = quotient_split[3]

        remainder_split = split(remainder, num_bits_shift=128, length=2)
        ids.remainder.low = remainder_split[0]
        ids.remainder.high = remainder_split[1]
    ```

* Add method `Program::data_len(&self) -> usize` to get the number of data cells in a given program [#1022](https://github.com/lambdaclass/cairo-vm/pull/1022)

* Add missing hint on uint256_improvements lib [#1013](https://github.com/lambdaclass/cairo-vm/pull/1013):

    `BuiltinHintProcessor` now supports the following hint:

    ```python
        a = (ids.a.high << 128) + ids.a.low
        div = (ids.div.b23 << 128) + ids.div.b01
        quotient, remainder = divmod(a, div)

        ids.quotient.low = quotient & ((1 << 128) - 1)
        ids.quotient.high = quotient >> 128
        ids.remainder.low = remainder & ((1 << 128) - 1)
        ids.remainder.high = remainder >> 128
    ```

* Add missing hint on cairo_secp lib [#1010](https://github.com/lambdaclass/cairo-vm/pull/1010):

    `BuiltinHintProcessor` now supports the following hint:

    ```python
        memory[ap] = int(x == 0)
    ```

* Implement hint on `get_felt_bitlength` [#993](https://github.com/lambdaclass/cairo-vm/pull/993)

  `BuiltinHintProcessor` now supports the following hint:
  ```python
  x = ids.x
  ids.bit_length = x.bit_length()
  ```
  Used by the [`Garaga` library function `get_felt_bitlength`](https://github.com/keep-starknet-strange/garaga/blob/249f8a372126b3a839f9c1e1080ea8c6f9374c0c/src/utils.cairo#L54)

* Add missing hint on cairo_secp lib [#1009](https://github.com/lambdaclass/cairo-vm/pull/1009):

    `BuiltinHintProcessor` now supports the following hint:

    ```python
        ids.dibit = ((ids.scalar_u >> ids.m) & 1) + 2 * ((ids.scalar_v >> ids.m) & 1)
    ```

* Add getters to read properties of a `Program` [#1017](https://github.com/lambdaclass/cairo-vm/pull/1017):
  * `prime(&self) -> &str`: get the prime associated to data in hex representation
  * `iter_data(&self) -> Iterator<Item = &MaybeRelocatable>`: get an iterator over all elements in the program data
  * `iter_builtins(&self) -> Iterator<Item = &BuiltinName>`: get an iterator over the names of required builtins

* Add missing hint on cairo_secp lib [#1008](https://github.com/lambdaclass/cairo-vm/pull/1008):

    `BuiltinHintProcessor` now supports the following hint:

    ```python
        ids.len_hi = max(ids.scalar_u.d2.bit_length(), ids.scalar_v.d2.bit_length())-1
    ```

* Update `starknet-crypto` to version `0.4.3` [#1011](https://github.com/lambdaclass/cairo-vm/pull/1011)
  * The new version carries an 85% reduction in execution time for ECDSA signature verification

* BREAKING CHANGE: refactor `Program` to optimize `Program::clone` [#999](https://github.com/lambdaclass/cairo-vm/pull/999)

    * Breaking change: many fields that were (unnecessarily) public become hidden by the refactor.

* BREAKING CHANGE: Add _builtin suffix to builtin names e.g.: output -> output_builtin [#1005](https://github.com/lambdaclass/cairo-vm/pull/1005)

* Implement hint on uint384_extension lib [#983](https://github.com/lambdaclass/cairo-vm/pull/983)

    `BuiltinHintProcessor` now supports the following hint:

    ```python
        def split(num: int, num_bits_shift: int, length: int):
            a = []
            for _ in range(length):
                a.append( num & ((1 << num_bits_shift) - 1) )
                num = num >> num_bits_shift
            return tuple(a)

        def pack(z, num_bits_shift: int) -> int:
            limbs = (z.d0, z.d1, z.d2)
            return sum(limb << (num_bits_shift * i) for i, limb in enumerate(limbs))

        def pack_extended(z, num_bits_shift: int) -> int:
            limbs = (z.d0, z.d1, z.d2, z.d3, z.d4, z.d5)
            return sum(limb << (num_bits_shift * i) for i, limb in enumerate(limbs))

        a = pack_extended(ids.a, num_bits_shift = 128)
        div = pack(ids.div, num_bits_shift = 128)

        quotient, remainder = divmod(a, div)

        quotient_split = split(quotient, num_bits_shift=128, length=6)

        ids.quotient.d0 = quotient_split[0]
        ids.quotient.d1 = quotient_split[1]
        ids.quotient.d2 = quotient_split[2]
        ids.quotient.d3 = quotient_split[3]
        ids.quotient.d4 = quotient_split[4]
        ids.quotient.d5 = quotient_split[5]

        remainder_split = split(remainder, num_bits_shift=128, length=3)
        ids.remainder.d0 = remainder_split[0]
        ids.remainder.d1 = remainder_split[1]
        ids.remainder.d2 = remainder_split[2]
    ```

* BREAKING CHANGE: optimization for instruction decoding [#942](https://github.com/lambdaclass/cairo-vm/pull/942):
    * Avoids copying immediate arguments to the `Instruction` structure, as they get inferred from the offset anyway
    * Breaking: removal of the field `Instruction::imm`

* Add missing `\n` character in traceback string [#997](https://github.com/lambdaclass/cairo-vm/pull/997)
    * BugFix: Add missing `\n` character after traceback lines when the filename is missing ("Unknown Location")

* 0.11 Support
    * Add missing hints [#1014](https://github.com/lambdaclass/cairo-vm/pull/1014):
        `BuiltinHintProcessor` now supports the following hints:
        ```python
            from starkware.cairo.common.cairo_secp.secp256r1_utils import SECP256R1_P as SECP_P
        ```
        and:
        ```python
            from starkware.cairo.common.cairo_secp.secp_utils import pack
            from starkware.python.math_utils import line_slope

            # Compute the slope.
            x0 = pack(ids.point0.x, PRIME)
            y0 = pack(ids.point0.y, PRIME)
            x1 = pack(ids.point1.x, PRIME)
            y1 = pack(ids.point1.y, PRIME)
            value = slope = line_slope(point1=(x0, y0), point2=(x1, y1), p=SECP_P)
        ```
    * Add missing hints on cairo_secp lib [#991](https://github.com/lambdaclass/cairo-vm/pull/991):
        `BuiltinHintProcessor` now supports the following hints:
        ```python
        from starkware.cairo.common.cairo_secp.secp_utils import pack
        from starkware.python.math_utils import div_mod, safe_div

        N = 0xfffffffffffffffffffffffffffffffebaaedce6af48a03bbfd25e8cd0364141
        x = pack(ids.x, PRIME) % N
        s = pack(ids.s, PRIME) % N
        value = res = div_mod(x, s, N)
        ```
        and:
        ```python
        value = k = safe_div(res * s - x, N)
        ```
    * Layouts update [#874](https://github.com/lambdaclass/cairo-vm/pull/874)
    * Keccak builtin updated [#873](https://github.com/lambdaclass/cairo-vm/pull/873), [#883](https://github.com/lambdaclass/cairo-vm/pull/883)
    * Changes to `ec_op` [#876](https://github.com/lambdaclass/cairo-vm/pull/876)
    * Poseidon builtin [#875](https://github.com/lambdaclass/cairo-vm/pull/875)
    * Renamed Felt to Felt252 [#899](https://github.com/lambdaclass/cairo-vm/pull/899)
    * Added SegmentArenaBuiltinRunner [#913](https://github.com/lambdaclass/cairo-vm/pull/913)
    * Added `program_segment_size` argument to `verify_secure_runner` & `run_from_entrypoint` [#928](https://github.com/lambdaclass/cairo-vm/pull/928)
    * Added dynamic layout [#879](https://github.com/lambdaclass/cairo-vm/pull/879)
    * `get_segment_size` was exposed [#934](https://github.com/lambdaclass/cairo-vm/pull/934)

* Add missing hint on cairo_secp lib [#1006](https://github.com/lambdaclass/cairo-vm/pull/1006):

    `BuiltinHintProcessor` now supports the following hint:

    ```python
        ids.quad_bit = (
            8 * ((ids.scalar_v >> ids.m) & 1)
            + 4 * ((ids.scalar_u >> ids.m) & 1)
            + 2 * ((ids.scalar_v >> (ids.m - 1)) & 1)
            + ((ids.scalar_u >> (ids.m - 1)) & 1)
        )
    ```

* Add missing hint on cairo_secp lib [#1003](https://github.com/lambdaclass/cairo-vm/pull/1003):

    `BuiltinHintProcessor` now supports the following hint:

    ```python
        from starkware.cairo.common.cairo_secp.secp_utils import pack

        x = pack(ids.x, PRIME) % SECP_P
    ```

* Add missing hint on cairo_secp lib [#996](https://github.com/lambdaclass/cairo-vm/pull/996):

    `BuiltinHintProcessor` now supports the following hint:

    ```python
        from starkware.python.math_utils import div_mod
        value = x_inv = div_mod(1, x, SECP_P)
    ```

* Add missing hints on cairo_secp lib [#994](https://github.com/lambdaclass/cairo-vm/pull/994):

    `BuiltinHintProcessor` now supports the following hints:

    ```python
        from starkware.cairo.common.cairo_secp.secp_utils import pack
        from starkware.python.math_utils import div_mod, safe_div

        a = pack(ids.a, PRIME)
        b = pack(ids.b, PRIME)
        value = res = div_mod(a, b, N)
    ```

    ```python
        value = k_plus_one = safe_div(res * b - a, N) + 1
    ```

* Add missing hint on cairo_secp lib [#992](https://github.com/lambdaclass/cairo-vm/pull/992):

    `BuiltinHintProcessor` now supports the following hint:

    ```python
        from starkware.cairo.common.cairo_secp.secp_utils import pack

        q, r = divmod(pack(ids.val, PRIME), SECP_P)
        assert r == 0, f"verify_zero: Invalid input {ids.val.d0, ids.val.d1, ids.val.d2}."
        ids.q = q % PRIME
    ```

* Add missing hint on cairo_secp lib [#990](https://github.com/lambdaclass/cairo-vm/pull/990):

    `BuiltinHintProcessor` now supports the following hint:

    ```python
        from starkware.cairo.common.cairo_secp.secp_utils import pack

        slope = pack(ids.slope, PRIME)
        x = pack(ids.point.x, PRIME)
        y = pack(ids.point.y, PRIME)

        value = new_x = (pow(slope, 2, SECP_P) - 2 * x) % SECP_P
    ```

* Add missing hint on cairo_secp lib [#989](https://github.com/lambdaclass/cairo-vm/pull/989):

    `BuiltinHintProcessor` now supports the following hint:

    ```python
        from starkware.cairo.common.cairo_secp.secp_utils import SECP_P
        q, r = divmod(pack(ids.val, PRIME), SECP_P)
        assert r == 0, f"verify_zero: Invalid input {ids.val.d0, ids.val.d1, ids.val.d2}."
        ids.q = q % PRIME
    ```

* Add missing hint on cairo_secp lib [#986](https://github.com/lambdaclass/cairo-vm/pull/986):

    `BuiltinHintProcessor` now supports the following hint:

    ```python
        from starkware.cairo.common.cairo_secp.secp_utils import SECP_P, pack
        from starkware.python.math_utils import div_mod

        # Compute the slope.
        x = pack(ids.pt.x, PRIME)
        y = pack(ids.pt.y, PRIME)
        value = slope = div_mod(3 * x ** 2, 2 * y, SECP_P)
    ```

* Add missing hint on cairo_secp lib [#984](https://github.com/lambdaclass/cairo-vm/pull/984):

    `BuiltinHintProcessor` now supports the following hint:

    ```python
        from starkware.cairo.common.cairo_secp.secp_utils import SECP_P, pack
        from starkware.python.math_utils import div_mod

        # Compute the slope.
        x0 = pack(ids.pt0.x, PRIME)
        y0 = pack(ids.pt0.y, PRIME)
        x1 = pack(ids.pt1.x, PRIME)
        y1 = pack(ids.pt1.y, PRIME)
        value = slope = div_mod(y0 - y1, x0 - x1, SECP_P)
    ```

* Implement hints on uint384 lib (Part 2) [#971](https://github.com/lambdaclass/cairo-vm/pull/971)

    `BuiltinHintProcessor` now supports the following hint:

    ```python
        memory[ap] = 1 if 0 <= (ids.a.d2 % PRIME) < 2 ** 127 else 0
    ```

 * Add alternative hint code for hint on _block_permutation used by 0.10.3 whitelist [#958](https://github.com/lambdaclass/cairo-vm/pull/958)

     `BuiltinHintProcessor` now supports the following hint:

    ```python
        from starkware.cairo.common.keccak_utils.keccak_utils import keccak_func
        _keccak_state_size_felts = int(ids.KECCAK_STATE_SIZE_FELTS)
        assert 0 <= _keccak_state_size_felts < 100

        output_values = keccak_func(memory.get_range(
            ids.keccak_ptr - _keccak_state_size_felts, _keccak_state_size_felts))
        segments.write_arg(ids.keccak_ptr, output_values)
    ```

* Make  hints code `src/hint_processor/builtin_hint_processor/hint_code.rs` public [#988](https://github.com/lambdaclass/cairo-vm/pull/988)

* Implement hints on uint384 lib (Part 1) [#960](https://github.com/lambdaclass/cairo-vm/pull/960)

    `BuiltinHintProcessor` now supports the following hints:

    ```python
        def split(num: int, num_bits_shift: int, length: int):
        a = []
        for _ in range(length):
            a.append( num & ((1 << num_bits_shift) - 1) )
            num = num >> num_bits_shift
        return tuple(a)

        def pack(z, num_bits_shift: int) -> int:
            limbs = (z.d0, z.d1, z.d2)
            return sum(limb << (num_bits_shift * i) for i, limb in enumerate(limbs))

        a = pack(ids.a, num_bits_shift = 128)
        div = pack(ids.div, num_bits_shift = 128)
        quotient, remainder = divmod(a, div)

        quotient_split = split(quotient, num_bits_shift=128, length=3)
        assert len(quotient_split) == 3

        ids.quotient.d0 = quotient_split[0]
        ids.quotient.d1 = quotient_split[1]
        ids.quotient.d2 = quotient_split[2]

        remainder_split = split(remainder, num_bits_shift=128, length=3)
        ids.remainder.d0 = remainder_split[0]
        ids.remainder.d1 = remainder_split[1]
        ids.remainder.d2 = remainder_split[2]
    ```

    ```python
        ids.low = ids.a & ((1<<128) - 1)
        ids.high = ids.a >> 128
    ```

    ```python
            sum_d0 = ids.a.d0 + ids.b.d0
        ids.carry_d0 = 1 if sum_d0 >= ids.SHIFT else 0
        sum_d1 = ids.a.d1 + ids.b.d1 + ids.carry_d0
        ids.carry_d1 = 1 if sum_d1 >= ids.SHIFT else 0
        sum_d2 = ids.a.d2 + ids.b.d2 + ids.carry_d1
        ids.carry_d2 = 1 if sum_d2 >= ids.SHIFT else 0
    ```

    ```python
        from starkware.python.math_utils import isqrt

        def split(num: int, num_bits_shift: int, length: int):
            a = []
            for _ in range(length):
                a.append( num & ((1 << num_bits_shift) - 1) )
                num = num >> num_bits_shift
            return tuple(a)

        def pack(z, num_bits_shift: int) -> int:
            limbs = (z.d0, z.d1, z.d2)
            return sum(limb << (num_bits_shift * i) for i, limb in enumerate(limbs))

        a = pack(ids.a, num_bits_shift=128)
        root = isqrt(a)
        assert 0 <= root < 2 ** 192
        root_split = split(root, num_bits_shift=128, length=3)
        ids.root.d0 = root_split[0]
        ids.root.d1 = root_split[1]
        ids.root.d2 = root_split[2]
    ```

* Re-export the `cairo-felt` crate as `cairo_vm::felt` [#981](https://github.com/lambdaclass/cairo-vm/pull/981)
  * Removes the need of explicitly importing `cairo-felt` in downstream projects
  and helps ensure there is no version mismatch caused by that

* Implement hint on `uint256_mul_div_mod`[#957](https://github.com/lambdaclass/cairo-vm/pull/957)

    `BuiltinHintProcessor` now supports the following hint:

    ```python
    a = (ids.a.high << 128) + ids.a.low
    b = (ids.b.high << 128) + ids.b.low
    div = (ids.div.high << 128) + ids.div.low
    quotient, remainder = divmod(a * b, div)

    ids.quotient_low.low = quotient & ((1 << 128) - 1)
    ids.quotient_low.high = (quotient >> 128) & ((1 << 128) - 1)
    ids.quotient_high.low = (quotient >> 256) & ((1 << 128) - 1)
    ids.quotient_high.high = quotient >> 384
    ids.remainder.low = remainder & ((1 << 128) - 1)
    ids.remainder.high = remainder >> 128"
    ```

    Used by the common library function `uint256_mul_div_mod`

#### [0.3.0-rc1] - 2023-04-13
* Derive Deserialize for ExecutionResources [#922](https://github.com/lambdaclass/cairo-vm/pull/922)
* Remove builtin names from VirtualMachine.builtin_runners [#921](https://github.com/lambdaclass/cairo-vm/pull/921)
* Implemented hints on common/ec.cairo [#888](https://github.com/lambdaclass/cairo-vm/pull/888)
* Changed `Memory.insert` argument types [#902](https://github.com/lambdaclass/cairo-vm/pull/902)
* feat: implemented `Deserialize` on Program by changing builtins field type to enum [#896](https://github.com/lambdaclass/cairo-vm/pull/896)
* Effective size computation from the VM exposed [#887](https://github.com/lambdaclass/cairo-vm/pull/887)
* Wasm32 Support! [#828](https://github.com/lambdaclass/cairo-vm/pull/828), [#893](https://github.com/lambdaclass/cairo-vm/pull/893)
* `MathError` added for math operation [#855](https://github.com/lambdaclass/cairo-vm/pull/855)
* Check for overflows in relocatable operations [#859](https://github.com/lambdaclass/cairo-vm/pull/859)
* Use `Relocatable` instead of `&MaybeRelocatable` in `load_data` and `get_range`[#860](https://github.com/lambdaclass/cairo-vm/pull/860) [#867](https://github.com/lambdaclass/cairo-vm/pull/867)
* Memory-related errors moved to `MemoryError` [#854](https://github.com/lambdaclass/cairo-vm/pull/854)
    * Removed unused error variants
    * Moved memory-related error variants to `MemoryError`
    * Changed memory getters to return `MemoryError` instead of `VirtualMachineError`
    * Changed all memory-related errors in hint from `HintError::Internal(VmError::...` to `HintError::Memory(MemoryError::...`
* feat: Builder pattern for `VirtualMachine` [#820](https://github.com/lambdaclass/cairo-vm/pull/820)
* Simplified `Memory::get` return type to `Option` [#852](https://github.com/lambdaclass/cairo-vm/pull/852)
* Improved idenitifier variable error handling [#851](https://github.com/lambdaclass/cairo-vm/pull/851)
* `CairoRunner::write_output` now prints missing and relocatable values [#853](https://github.com/lambdaclass/cairo-vm/pull/853)
* `VirtualMachineError::FailedToComputeOperands` error message expanded [#848](https://github.com/lambdaclass/cairo-vm/pull/848)
* Builtin names made public [#849](https://github.com/lambdaclass/cairo-vm/pull/849)
* `secure_run` flag moved to `CairoRunConfig` struct [#832](https://github.com/lambdaclass/cairo-vm/pull/832)
* `vm_core` error types revised and iimplemented `AddAssign` for `Relocatable` [#837](https://github.com/lambdaclass/cairo-vm/pull/837)
* `to_bigint` and `to_biguint` deprecated [#757](https://github.com/lambdaclass/cairo-vm/pull/757)
* `Memory` moved into `MemorySegmentManager` [#830](https://github.com/lambdaclass/cairo-vm/pull/830)
    * To reduce the complexity of the VM's memory and enforce proper usage (as the memory and its segment manager are now a "unified" entity)
    * Removed `memory` field from `VirtualMachine`
    * Added `memory` field to `MemorySegmentManager`
    * Removed `Memory` argument from methods where `MemorySegmentManager` is also an argument
    * Added test macro `segments` (an extension of the `memory` macro)
* `Display` trait added to Memory struct [#812](https://github.com/lambdaclass/cairo-vm/pull/812)
* feat: Extensible VirtualMachineError and removed PartialEq trait [#783](https://github.com/lambdaclass/cairo-vm/pull/783)
    * `VirtualMachineError::Other(anyhow::Error)` was added to allow to returning custom errors when using `cairo-vm`
    * The `PartialEq` trait was removed from the `VirtualMachineError` enum
* VM hooks added as a conditional feature [#761](https://github.com/lambdaclass/cairo-vm/pull/761)
    * Cairo-vm based testing tools such as cairo-foundry or those built by FuzzingLabs need access to the state of the VM at specific points during the execution.
    * This PR adds the possibility for users of the cairo-vm lib to execute their custom additional code during the program execution.
    * The Rust "feature" mechanism was used in order to guarantee that this ability is only available when the lib user needs it, and is not compiled when it's not required.
    * Three hooks were created:
        * before the first step
        * before each step
        * after each step
* ExecutionResource operations: add and substract [#774](https://github.com/lambdaclass/cairo-vm/pull/774), multiplication [#908](https://github.com/lambdaclass/cairo-vm/pull/908) , and `AddAssign` [#914](https://github.com/lambdaclass/cairo-vm/pull/914)

* Move `Memory` into `MemorySegmentManager` [#830](https://github.com/lambdaclass/cairo-vm/pull/830)
    * Structural changes:
        * Remove `memory: Memory` field from `VirtualMachine`
        * Add `memory: Memory` field to `MemorySegmentManager`
    * As a result of this, multiple public methods' signatures changed:
        * `BuiltinRunner` (and its inner enum types):
            * `initialize_segments(&mut self, segments: &mut MemorySegmentManager, memory: &mut Memory)` -> `initialize_segments(&mut self, segments: &mut MemorySegmentManager)`
            * `final_stack(&mut self, segments: &MemorySegmentManager, memory: &Memory, stack_pointer: Relocatable) -> Result<Relocatable, RunnerError>` -> `final_stack(&mut self, segments: &MemorySegmentManager, stack_pointer: Relocatable) -> Result<Relocatable, RunnerError>`
        * `MemorySegmentManager`
            * `add(&mut self, memory: &mut Memory) -> Relocatable` -> `add(&mut self) -> Relocatable`
            * `add_temporary_segment(&mut self, memory: &mut Memory) -> Relocatable` -> `add_temporary_segment(&mut self) -> Relocatable`
            * `load_data(&mut self, memory: &mut Memory, ptr: &MaybeRelocatable, data: &Vec<MaybeRelocatable>) -> Result<MaybeRelocatable, MemoryError>` -> `load_data(&mut self, ptr: &MaybeRelocatable, data: &Vec<MaybeRelocatable>) -> Result<MaybeRelocatable, MemoryError>`
            * `compute_effective_sizes(&mut self, memory: &Memory) -> &Vec<usize>` -> `compute_effective_sizes(&mut self) -> &Vec<usize>`
            * `gen_arg(&mut self, arg: &dyn Any, memory: &mut Memory) -> Result<MaybeRelocatable, VirtualMachineError>` -> `gen_arg(&mut self, arg: &dyn Any) -> Result<MaybeRelocatable, VirtualMachineError>`
            * `gen_cairo_arg(&mut self, arg: &CairoArg, memory: &mut Memory) -> Result<MaybeRelocatable, VirtualMachineError>` -> `gen_cairo_arg(&mut self, arg: &CairoArg) -> Result<MaybeRelocatable, VirtualMachineError>`
            * `write_arg(&mut self, memory: &mut Memory, ptr: &Relocatable, arg: &dyn Any) -> Result<MaybeRelocatable, MemoryError>` -> `write_arg(&mut self, ptr: &Relocatable, arg: &dyn Any) -> Result<MaybeRelocatable, MemoryError>`

* Refactor `Memory::relocate memory` [#784](https://github.com/lambdaclass/cairo-vm/pull/784)
    * Bugfixes:
        * `Memory::relocate_memory` now moves data in the temporary memory relocated by a relocation rule to the real memory
    * Aditional Notes:
        * When relocating temporary memory produces clashes with pre-existing values in the real memory, an InconsistentMemory error is returned instead of keeping the last inserted value. This differs from the original implementation.

* Restrict addresses to Relocatable + fix some error variants used in signature.rs [#792](https://github.com/lambdaclass/cairo-vm/pull/792)
    * Public Api Changes:
        * Change `ValidationRule` inner type to `Box<dyn Fn(&Memory, &Relocatable) -> Result<Vec<Relocatable>, MemoryError>>`.
        * Change `validated_addresses` field of `Memory` to `HashSet<Relocatable>`.
        * Change `validate_memory_cell(&mut self, address: &MaybeRelocatable) -> Result<(), MemoryError>` to `validate_memory_cell(&mut self, addr: &Relocatable) -> Result<(), MemoryError>`.

* Add `VmException` to `CairoRunner::run_from_entrypoint`[#775](https://github.com/lambdaclass/cairo-vm/pull/775)
    * Public Api Changes:
        * Change error return type of `CairoRunner::run_from_entrypoint` to `CairoRunError`.
        * Convert `VirtualMachineError`s outputed during the vm run to `VmException` in `CairoRunner::run_from_entrypoint`.
        * Make `VmException` fields public

* Fix `BuiltinRunner::final_stack` and remove quick fix [#778](https://github.com/lambdaclass/cairo-vm/pull/778)
    * Public Api changes:
        * Various changes to public `BuiltinRunner` method's signatures:
            * `final_stack(&self, vm: &VirtualMachine, pointer: Relocatable) -> Result<(Relocatable, usize), RunnerError>` to `final_stack(&mut self, segments: &MemorySegmentManager, memory: &Memory, pointer: Relocatable) -> Result<Relocatable,RunnerError>`.
            * `get_used_cells(&self, vm: &VirtualMachine) -> Result<usize, MemoryError>` to  `get_used_cells(&self, segments: &MemorySegmentManager) -> Result<usize, MemoryError>`.
            * `get_used_instances(&self, vm: &VirtualMachine) -> Result<usize, MemoryError>` to `get_used_instances(&self, segments: &MemorySegmentManager) -> Result<usize, MemoryError>`.
    * Bugfixes:
        * `BuiltinRunner::final_stack` now updates the builtin's stop_ptr instead of returning it. This replaces the bugfix on PR #768.

#### [0.1.3] - 2023-01-26
* Add secure_run flag + integrate verify_secure_runner into cairo-run [#771](https://github.com/lambdaclass/cairo-vm/pull/777)
    * Public Api changes:
        * Add command_line argument `secure_run`
        * Add argument `secure_run: Option<bool>` to `cairo_run`
        * `verify_secure_runner` is now called inside `cairo-run` when `secure_run` is set to true or when it not set and the run is not on `proof_mode`
    * Bugfixes:
        * `EcOpBuiltinRunner::deduce_memory_cell` now checks that both points are on the curve instead of only the first one
        * `EcOpBuiltinRunner::deduce_memory_cell` now returns the values of the point coordinates instead of the indices when a `PointNotOnCurve` error is returned

* Refactor `Refactor verify_secure_runner` [#768](https://github.com/lambdaclass/cairo-vm/pull/768)
    * Public Api changes:
        * Remove builtin name from the return value of `BuiltinRunner::get_memory_segment_addresses`
        * Simplify the return value of `CairoRunner::get_builtin_segments_info` to `Vec<(usize, usize)>`
        * CairoRunner::read_return_values now receives a mutable reference to VirtualMachine
    * Bugfixes:
        * CairoRunner::read_return_values now updates the `stop_ptr` of each builtin after calling `BuiltinRunner::final_stack`

* Use CairoArg enum instead of Any in CairoRunner::run_from_entrypoint [#686](https://github.com/lambdaclass/cairo-vm/pull/686)
    * Public Api changes:
        * Remove `Result` from `MaybeRelocatable::mod_floor`, it now returns a `MaybeRelocatable`
        * Add struct `CairoArg`
        * Change `arg` argument of `CairoRunner::run_from_entrypoint` from `Vec<&dyn Any>` to `&[&CairoArg]`
        * Remove argument `typed_args` from `CairoRunner::run_from_entrypoint`
        * Remove no longer used method `gen_typed_arg` from `VirtualMachine` & `MemorySegmentManager`
        * Add methods `MemorySegmentManager::gen_cairo_arg` & `MemorySegmentManager::write_simple_args` as typed counterparts to `MemorySegmentManager::gen_arg` & `MemorySegmentManager::write_arg`

#### [0.1.1] - 2023-01-11

* Add input file contents to traceback [#666](https://github.com/lambdaclass/cairo-vm/pull/666/files)
    * Public Api changes:
        * `VirtualMachineError` enum variants containing `MaybeRelocatable` and/or `Relocatable` values now use the `Display` format instead of `Debug` in their `Display` implementation
        * `get_traceback` now adds the source code line to each traceback entry
* Use hint location instead of instruction location when building VmExceptions from hint failure [#673](https://github.com/lambdaclass/cairo-vm/pull/673/files)
    * Public Api changes:
        * `hints` field added to `InstructionLocation`
        * `Program.instruction_locations` type changed from `Option<HashMap<usize, Location>>` to `Option<HashMap<usize, InstructionLocation>>`
        * `VirtualMachineError`s produced by `HintProcessor::execute_hint()` will be wrapped in a `VirtualMachineError::Hint` error containing their hint_index
        * `get_location()` now receives an an optional usize value `hint_index`, used to obtain hint locations
* Default implementation of compile_hint [#680](https://github.com/lambdaclass/cairo-vm/pull/680)
    * Internal changes:
        * Make the `compile_hint` implementation which was in the `BuiltinHintProcessor` the default implementation in the trait.
* Add new error type `HintError` [#676](https://github.com/lambdaclass/cairo-vm/pull/676)
    * Public Api changes:
        * `HintProcessor::execute_hint()` now returns a `HintError` instead of a `VirtualMachineError`
        * Helper functions on `hint_processor_utils.rs` now return a `HintError`
* Change the Dictionary used in dict hints to store MaybeRelocatable instead of BigInt [#687](https://github.com/lambdaclass/cairo-vm/pull/687)
    * Public Api changes:
        * `DictManager`, its dictionaries, and all dict module hints implemented in rust now use `MaybeRelocatable` for keys and values instead of `BigInt`
        * Add helper functions that allow extracting ids variables as `MaybeRelocatable`: `get_maybe_relocatable_from_var_name` & `get_maybe_relocatable_from_reference`
        * Change inner value type of dict-related `HintError` variants to `MaybeRelocatable`

* Implement `substitute_error_message_attribute_references` [#689] (https://github.com/lambdaclass/cairo-vm/pull/689)
    * Public Api changes:
        * Remove `error_message_attributes` field from `VirtualMachine`, and `VirtualMachine::new`
        * Add `flow_tracking_data` field to `Attribute`
        * `get_error_attr_value` now replaces the references in the error message with the corresponding cairo values.
        * Remove duplicated handling of error attribute messages leading to duplicated into in the final error display.
* Fix multiplicative inverse bug [#697](https://github.com/lambdaclass/cairo-vm/pull/697) [#698](https://github.com/lambdaclass/cairo-vm/pull/698). The VM was using integer division rather than prime field inverse when deducing `op0` or `op1` for the multiplication opcode

#### [0.1.0] - 2022-12-30
* Add traceback to VmException [#657](https://github.com/lambdaclass/cairo-vm/pull/657)
    * Public API changes:
        * `traceback` field added to `VmException` struct
        * `pub fn from_vm_error(runner: &CairoRunner, error: VirtualMachineError, pc: usize) -> Self` is now `pub fn from_vm_error(runner: &CairoRunner, vm: &VirtualMachine, error: VirtualMachineError) -> Self`
        * `pub fn get_location(pc: &usize, runner: &CairoRunner) -> Option<Location>` is now `pub fn get_location(pc: usize, runner: &CairoRunner) -> Option<Location>`
        * `pub fn decode_instruction(encoded_instr: i64, mut imm: Option<BigInt>) -> Result<instruction::Instruction, VirtualMachineError>` is now `pub fn decode_instruction(encoded_instr: i64, mut imm: Option<&BigInt>) -> Result<instruction::Instruction, VirtualMachineError>`
        * `VmException` fields' string format now mirrors their cairo-lang counterparts.
