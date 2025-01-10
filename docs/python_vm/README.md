# How does the original Cairo VM work?

## Flow Diagram

We've created a diagram that illustrates function calls and where each of them are defined for a simple cairo program execution.

<p float="left">
  <img src="./diagram/cairo_vm_color_key.png" width="200" />
</p>

![diagram](./diagram/cairo_vm_flow_diagram.jpg)

This diagram was produced using this [mermaid code](./diagram/cairo_vm_flow_diagram.md).

## How does the vm manage memory?

Cairo's memory is read-only (can only be written once), and requires that memory addresses accessed by the program must be contiguous. If gaps are present they will be filled with arbitrary values.

The memory itself is a list of continuous segments, the size of each segment may vary and can only be known once the program terminates. Absolute addresses of every memory cell within a segment can only be determined at the end of a vm run (Relocatable values are used to represent them, indicating the segment number and the offset).

The different segments:
* Program Segment: Contains cairo bytecode. pc starts at the beginning of this segment
* Execution Segment: Where data is generated during the run of a Cairo program. Length is variable(depends on program input). Allocation Pointer (ap) and Frame Pointer (fp) start here.
* Builtin Segment: Each builtin has its own continuous area in memory. Length is variable

## Registers

**Allocation Pointer (ap)** Points to an unused memory cell.
**Frame Pointer (fp)** Points to the frame of the current function. The addresses of all the function's arguments and local variables are relative to the value of this register. It's equal to ap when the function starts, and remains constant throughout the scope of it. When a function2 is called inside a function1, the value of fp changes once function2 starts, but it is restored back to function1's value once function2 ends (this is needed to keep track of function1's value, as ap will have changed).
**Program Counter (pc)** Points to the current instruction. Each instruction takes 1 or 2 felts(2 when an imm is used), and pc advances by 1 or 2 after each instruction, other ways of changing this register is by jumps.

Imm (Immediate) is: 
The second value of an operand (such as 5 in [ap -1] = [fp + 10] + 5)
The standalone value for assignments (such as 5 in [fp + 2] = 5)

Jumps:
* Absolute: changes pc to given value (pc = n)
* Relative: advances pc by a given value (pc + n)
* Jump to label: relative jump, value is computed by the compiler

Other info:

let creates a reference
tempvar is based on ap
local is based on fp
alloc_locals = ap += SIZE_OF_LOCALS


# Cairo VM Code Analysis

Let's look at how the VM is composed:
The [VirtualMachine](https://github.com/starkware-libs/cairo-lang/blob/b614d1867c64f3fb2cf4a4879348cfcf87c3a5a7/src/starkware/cairo/lang/vm/vm_core.py#L96) structure:
* [`run_context`](#context)
* `program` : [ProgramBase](#progbase)
* `program_base` : Optional(MaybeRelocatable)(if none, it is set to run_context.pc)
* `builtin_runners` : (Optional Dict or set to {})
* `hint_locals` : (Dict)
* `static_locals` : (Optional Dict)
VirtualMachineBase's init is used to set these values (plus other ones), the next ones are exclusive to the VirtualMachine:
* `accessed_addresses` : (Set that keeps track of memory addresses accessed by cairo instructions)
* `trace` : (List of `TraceEntry`s, each containing the run_context's pc, ap, and fp at that moment. A `TraceEntry` is added after every instruction (Before update_registers is called))
* `current_step` : (initialized with 0)
* `skip_instruction_execution` : (= False), used by hints to skip execution of current step

Functions:
* `[update_registers(instruction, operands)]`(https://github.com/starkware-libs/cairo-lang/blob/b614d1867c64f3fb2cf4a4879348cfcf87c3a5a7/src/starkware/cairo/lang/vm/vm_core.py#L143): Updates fp, ap, and pc, based on the instruction's [FpUpdate, ApUpdate and PcUpdate](#updatereg)
* [`deduce_op0(instruction, dst, op1) -> Tuple(Op(MaybeRelocatable), Op(MaybeRelocatable) `](https://github.com/starkware-libs/cairo-lang/blob/b614d1867c64f3fb2cf4a4879348cfcf87c3a5a7/src/starkware/cairo/lang/vm/vm_core.py#L189)deduces op0 from op1 and dst, also returns dst as deduced_res in case of an ASSERT_EQ opcode.
* [`deduce_op1(instruction, dst, op0) -> Tuple(Op(Mr), Op(Mr))`](https://github.com/starkware-libs/cairo-lang/blob/b614d1867c64f3fb2cf4a4879348cfcf87c3a5a7/src/starkware/cairo/lang/vm/vm_core.py#L214) deduces op1 from op0 and dst, also returns dst as deduced_res in case of an ASSERT_EQ opcode.
* [`compute_res(instruction, op0, op1)`](https://github.com/starkware-libs/cairo-lang/blob/b614d1867c64f3fb2cf4a4879348cfcf87c3a5a7/src/starkware/cairo/lang/vm/vm_core.py#L241) -> Optional(MaybeRelocatable): Returns computed res based on instruction.res: returns op1 if OP1, op1+op2 mod prime if ADD, op0 * op1 mod prime if MUL (and both operands are not relocatable), none if UNCONSTRAINED (handled elsewhere???, should be inverse of dst), or fails otherwise
* [`compute_operands(instruction) -> Tuple(Operands, List(int))`](#computeop) -> Returns the Operands(name is plural, its one) based on the Instruction, computes dst, op0 and op1 addresses, deduces op0 and op1, validates and updates dst, op0, op1.
* Replaceable functions: `is_zero`, `isinstance`, `is_integer_value`
* `decode_instruction(encoded_inst: int, imm : Optional(int))` -> Instruction : calls decode_instruction on compiler/encode.py
* [`decode_current_instruction -> Instruction`](#decodei) : gets instruction at pc
* [`opcode_assertion(instruction, Operands)`](https://github.com/starkware-libs/cairo-lang/blob/b614d1867c64f3fb2cf4a4879348cfcf87c3a5a7/src/starkware/cairo/lang/vm/vm_core.py#L383): Checks the opcodes, and makes sure everything makes sense. Possible opcodes:
    * ASSERT_EQ
    * CALL
    * RET
    * NOP
Doesn't check anything for the last two.
* [`run_instruction(instruction)`](https://github.com/starkware-libs/cairo-lang/blob/b614d1867c64f3fb2cf4a4879348cfcf87c3a5a7/src/starkware/cairo/lang/vm/vm_core.py#L410): compute_operands, opcode_assertion, writes pc, ap and fp to trace, updates accessed_adresses, update_registers, and increases the current_step by one
* [`step()`](https://github.com/starkware-libs/cairo-lang/blob/b614d1867c64f3fb2cf4a4879348cfcf87c3a5a7/src/starkware/cairo/lang/vm/vm_core.py#L443) disables skip_next_instruction, executes hints, then clears ids and memory(used by hints), skips if skip_next_instruction is enabled, decode_current_instruction, runs the decoded instruction with run_instruction.

This VirtualMachine is built from a VirtualMachineBase

# What is the VirtualMachineBase? (Incomplete Section)
The VirtualMachineBase structure has:
* `run_context` : RunContextBase
* `program` : [ProgramBase](#progbase)
* `hint_locals`: Dict
* `static_locals`: Optional Dict -> if None, it is updated with the values PRIME : self.prime, fadd (a+b mod p), fsub (a - b mod p), fmul (a * b mod p), fdiv (divmod(a,b,p)), fpow (pow(a,b,p)), fis_quad_residue (is_quadratic_residue(a)), fsqrt (sqrt(a,p)), safe_div(safe_div), and to_felt_or_relocatable_value
* `builtin_runners` : Dict
* `program_base`: MaybeRelocatable
* `prime` = program.prime
* `exec_scopes`: List[dict] = [] 
* `hints`:  Dict[MaybeRelocatable, List[CompiledHint]] = {} 
* `hint_pc_and_index` : Dict[int, Tuple[MaybeRelocatable, int]] = {} 
* `instruction_debug_info`: Dict[MaybeRelocatable, InstructionLocation] = {} 
* `debug_file_contents`: Dict[str, str] = {} 
* `error_message_attributes`: List[VmAttributeScope] = [] 
* `validated_memory` = ValidatedMemoryDict(memory=run_context.memory)
* `auto_deduction`: Dict[int, List[Tuple[Rule, tuple]]] = {}

Functions (The ones used by VirtualMachine):
* `exec_hint`(code, globals, hint_index) Executes a hint
* `as_vm_exception` Returns a VmException with additional info
* `deduce_memory_cell(addr)` Tries to deduce value at addr by calling the rules in self.auto_deduction at the addr's segment index with addr

## What is this auto_deduction?

It contains a dictionary that maps a memory index segment to a list of rules (A tuple with the rule and the args), that will allow the deduction of the value of a memory cell within the segment. Rule depends on Protocol, and can be called as a function

<a id="valmem">
    
## What is this validated_memory?
</a>

ValidatedMemoryDict structure:
* `__memory` : MemoryDict
* `__validation_rules` : Dict[int, List[Tuple[ValidationRule, tuple]]] = {} 
* `__validated_adresses` : Set[RelocatableValue]

validated_memory[addr] = `__memory`[addr]

### What are these validation_rules?

It is a dictionary that maps a segment index to a list of validation rules (a tuple with the rule and the args). A ValidationRule is callable. It behaves the same way a Rule would (despite different definition), it can be used to validate a memory cell by calling `__validate_memory_cell` with the address and the value (from the MemoryDict), or by using `validate_existing_memory` to validate every address in memory.

<a id="progbase">
    
## What is the ProgramBase? 
</a>

This is found at compiler/program.py
The structure contains:
* `prime` : int
* `data` : List[int]
* `builtins`: List[str]
* `main` : Optional(int)

It is the base for `StrippedProgram` and `Program`
The `StrippedProgram` contains minimal information, it doesn't have hints, identifiers, etc. It can be used for verifying reasons. If a program is a `StrippedProgram`, there is no need to use `load_program` when initializing the vm.

### The Program structure: 
* `prime`: an int that can be serialized as a hex string
* `data` : a list of ints which can be serialized as hex strings
* `hints`: a dictionary that maps ints to a list of `CairoHint`
* `builtins` : a list of strings
* `main_scope` : a `ScopedName` which can be serialized as a string
* `identifiers` : an I`dentifierManager` (serializable)
* `reference_manager` : a `ReferenceManager`
* `attributes` : a list of AttributeScope (serializable)
* `debug_info` : an optional DebugInfo

This class is a marshmellowdataclass, this allows many of its attributes to be serialized
[Marshmellow Documentation](https://marshmallow.readthedocs.io/en/stable/)
    
A `CairoHint` contains a `code` (a string) and accessible_scopes (a list of ScopedName, that can be serialized as strings), and `flow_tracking_data`, a FlowTrackingDataActual.
Where `ScopedName` is a frozen dataclass that contains a path as a string tuple and a SEPARATOR ("."), this should be the path for imports (shuch as starkware.common.serialize). 

A `FlowTrackingDataActual` contains an `ap_tracking` (as a RegTrackingData), a `reference_ids`, as a dictionary that maps reference names (ScopedName) to reference instances (int). A RegTrackingData is used to track the progress of a register during a run. It contains a group (int) which starts at zero and increases by one each time an unknown change happens, and an offset (int) which begins at zero and increases the same way the register does.

An `IdentifierManager` has a `root` (an `IdentifierScope`) and a `dict` ( a MutableMapping between a ScopedName and an IdentifierDefinition).

An `IdentifierScope` contains a `manager` (an IdentifierManager), a `fullname` (a ScopedName), `subscopes`(a dictionary that maps a string to an IdentifierScope), and identifiers (a MutableMapping between a string and an IdentifierDefinition) an IdentifierDefinition can be many kinds of definitions, such as future, alias, const, member, structs, type, label, function, namespace, reference, etc. they all have a TYPE that will contain the name of their type of definition as a ClassVar[str]

A `ReferenceManager` contains a list of `Reference`s, and methods to add a reference while returning its position (`alloc_id`), and to get a reference by its position (`get_ref`)
A `Reference` is a reference to a memory address for a specific location in the program (This is the reference that is created when you use `let`). It contains `pc` (int), `value` (Expression), `ap_tracking_data` (`RegTrackingData`), `locations` (A list of `Location`, contains a list of definition sites from the reference, it will contain multiple locations when the reference is defined from the convergence of multiple reference definitions), `definition_code_element` (an optional `CodeElement`, the code element that created this reference).

A `Location` consists of a start_line, start_col, end_line, end_col (ints), and an input_file (file), it indicates the precise location of a reference definition in the source code, and it also contains an optional `parent_location`, when the location points to a reference definition due to a reference expansion. A ParentLocation is a tuple of (location, string).

A `CodeElement` can be any kind of element found in the code (such as hints, function calls, tempvars, locals, references, etc), its structure varies by type, but all of them implement `format` and `get_children`

# Execution flow:


```
step -> decode_instruction
        run instruction   -> compute_operands
                             opcode_assertion
                             adds TraceEntry
                             updates accessed_adresses
                             update_registers
                             increase current step                   
```
<a id="computeop">
    
## How does [compute_operands](https://github.com/starkware-libs/cairo-lang/blob/b614d1867c64f3fb2cf4a4879348cfcf87c3a5a7/src/starkware/cairo/lang/vm/vm_core.py#L265) work?
</a>

First it will try to obtain dst, op0 and op1 by computing their addresses (`compute_dst_addr`, `compute_op0_addr`, `compute_op1_addr`), and looking them up in the [validated_memory](#valmem). If op0 and/or op1 can't be obtained this way, they will be deduced, first by calling `deduce_memory_cell` on their previously obtained addresses (At this moment, it is determined whether dst, op0 and op1 will need to be updated on the validated_memory (if they haven't been obtained yet, they will need to be)). If this also fails for one of them, op0 and op1 will be deduced from each other and the dst using `deduce_op0(instruction, dst, op1)` and `deduce_op1(instruction, dst, op0)`. After this, if op1 and/or is yet unobtained, they will be force-pulled from validated_memory to obtain an error message. After op0 and op1 have been handled, the res will be computed (via `compute_res`), if it hasn't been obtained earlier from `deduce_op0` and `deduce_op1`. Then, if dst wasn't obtained at the beginning, it will be assigned based on the instructions Opcode (dst = res if ASSERT_EQ, or dst = run_context.fp if CALL), otherwise it will be force-pulled from validated_memory as with op0 and op1. Afterwards, if dst, op0 and op1 need to be updated, they will be added to the validated_memory, and an Operands is returned with dst, op0, op1, res, + a list with the addresses for dst, op0 and op1.

<a id="decodei">

## How does [decode_instruction](https://github.com/starkware-libs/cairo-lang/blob/b614d1867c64f3fb2cf4a4879348cfcf87c3a5a7/src/starkware/cairo/lang/vm/vm_core.py#L371) work?
</a>

This function calls decode_instruction at comiler/encode.py, where `flags`, and the encoded offsets are obtained from calling decode_instruction_values (this function is defined under the Instruction class) on the encoded instruction (an int). This encoded offsets will then become the instruction's offsets after subtracting a constant value, and all other values of the Instruction will be determined by "reading" `flags`, by checking specific bits (ie: `flags >> OPCODE_CALL_BIT) & 1` will determine if opcode will be set Instruction.Opcode.CALL).
Some register updates will also be assigned based on the determined opcode: ap_update will be set to ADD2 if the opcode is CALL (ADD2 won't be assigned by reading `flag`, instead, this will be REGULAR), and the fp_update will be determined solely based on the opcode (without using `flag`). `imm` will be set to None unless op1_addr is IMM.

# What is an Operands?

[Operands](https://github.com/starkware-libs/cairo-lang/blob/b614d1867c64f3fb2cf4a4879348cfcf87c3a5a7/src/starkware/cairo/lang/vm/vm_core.py#L19) structure:
* `dst`  (MaybeRelocatable)
* `res`  (Op(MaybeRelocatable))
* `op0`  (MaybeRelocatable)
* `op1`  (MaybeRelocatable)

Where MaybeRelocatable is a Union of int and RelocatableValue (can be one or the other, this has to do with how memory is variable till the vm run finishes). And RelocatableValue contains a segment_index and an offset (both ints). 

Operands are computed at the start of each instruction

<a id="context">
    
# What is the run_context?
 </a>
 
[RunContext](https://github.com/starkware-libs/cairo-lang/blob/b614d1867c64f3fb2cf4a4879348cfcf87c3a5a7/src/starkware/cairo/lang/vm/vm_core.py#L31) contains:
* `memory`: A MemoryDict -> contains `data`(Dict)   `frozen`(bool) and relocation_rules` (Dict[int, RelocatableValue])
* `pc` : MaybeRelocatable
* `ap` : MaybeRelocatable
* `fp` : MaybeRelocatable
* `prime` : int
Functions:
* [`get_instruction_encoding() -> (encoded_instruction, imm)]`(https://github.com/starkware-libs/cairo-lang/blob/b614d1867c64f3fb2cf4a4879348cfcf87c3a5a7/src/starkware/cairo/lang/vm/vm_core.py#L42): returns instruction at pc, and the value at pc + 1 if it exists
* [`compute_dst_addr(instruction)`](https://github.com/starkware-libs/cairo-lang/blob/b614d1867c64f3fb2cf4a4879348cfcf87c3a5a7/src/starkware/cairo/lang/vm/vm_core.py#L59): returns either ap or fp depending on the instruction's dst_register + off0 mod prime
* [`compute_op0_addr(instruction)`](https://github.com/starkware-libs/cairo-lang/blob/b614d1867c64f3fb2cf4a4879348cfcf87c3a5a7/src/starkware/cairo/lang/vm/vm_core.py#L69): returns either ap or fp depending on the instruction's op0_register + off1 mod prime
* [`compute_op1_addr(instruction)`](https://github.com/starkware-libs/cairo-lang/blob/b614d1867c64f3fb2cf4a4879348cfcf87c3a5a7/src/starkware/cairo/lang/vm/vm_core.py#L79): returns either fp, ap, or pc depending on the instruction's op1_addr + off2 mod prime
* 
RunContext is built from a RunContextBase, this class has the same structure as RunContext with the function `get_traceback_entries`

# What is a Instruction?
Instruction Structure:
* `off0` : int
* `off1` : int  These offsets are within the range [ -2^15 ][ 2^15 ]
* `off2` : int
* `imm` : Optional(int) (Immediate)
* `dst_register`: Register (Where Register contains AP and FP)
* `op0_register`: Register (Where Register contains AP and FP)

* `op1_addr` : Op1Addr(Enum) These are used by `compute_op1_addr`
    * IMM -> means op1_addr should be run_context.pc
    * AP -> means op1_addr should be run_context.ap
    * FP -> means op1_addr should be run_context.fp
    * OP0 -> means op1_addr should be op0 (Optionally received)
    
* `res`: Res(Enum) These are used by `compute_res`
    * OP1 -> means res should be op1
    * ADD -> means res should be op0 + op1 mod prime
    * MUL -> means res should be op0 + op1 mod prime
    * UNCONSTRAINED -> res = None (Handled elsewhere), Res is UNCONSTRAINED when a Jump to label occurs (Res.JNZ), This is checked when the instruction is decoded
<a id="updatereg"> 
   
* `pc_update`: PcUpdate(Enum) These are used by `update_register`
    * REGULAR -> means run_context.pc should be += instruction.size
    * JUMP -> means run_context.pc should be = operands.res
    * JUMP_REL -> means run_context.pc should be += operands.res
    * JNZ -> means run_context.pc should be += instruction_size if operands.dst == 0, or += operands.op1 (This would be a Jump to label)
   
* `ap_update`: ApUpdate(Enum) These are used by `update_register`
    * REGULAR -> no update
    * ADD -> means run_context.ap should be += operand.res
    * ADD1 -> means run_context.ap should be +=1
    * ADD2 -> means run_context.ap should be +=2
   
* `fp_update`: FpUpdate(Enum) These are used by `update_register`
    * REGULAR -> no update
    * AP_PLUS2 -> means run_context.fp should be .ap + 2
    * DST -> means run_context.fp should be = operands.dst
   
</a>

* `opcode`: Opcode(Enum)
    * NOP
    * ASSERT_EQ
    * CALL
    * RET

Functions:
* `size()` 2 if it has immediate, 1 if not
* [`decode_instruction_values(encoded_instruction)`](https://github.com/starkware-libs/cairo-lang/blob/b614d1867c64f3fb2cf4a4879348cfcf87c3a5a7/src/starkware/cairo/lang/compiler/instruction.py#L117) returns tuple with flags, off0, off1, off2

These values will then be used by decode_instruction (at compiler/encode).`flag` will be used to determine the enums of the Instruction (Some register updates will be then changed based on the Opcode). off0, off1 and off2 will become the instruction's off0, off1 and off2 after subtracting a constant offset value.

# What happens before and after step()?
*Left side of flow diagram analysis*

## Clarifications on memory:
CairoRunner's memory is the same as vm_memory and segments.memory (where segments is a MemorySegmentManager that contains a reference to memory), this memory will then become the VirtualMachine's run_context's memory, but changes to one wont affect the other. CairoRunner's Memory doesnt change after each step, unless a hint is executed. When hints interact with memory (ie when alloc() creates a new memory segment) the run_context's and the CairoRunner's memory will both be affected equally.

## [`cairo_run`](https://github.com/starkware-libs/cairo-lang/blob/b614d1867c64f3fb2cf4a4879348cfcf87c3a5a7/src/starkware/cairo/lang/vm/cairo_run.py#L219)

*Overall, this function loads the program, creates a CairoRunner, and calls the necessary functions in order to carry out a vm run.*

For a simple cairo program execution, without any special flags besides print_output:
First loads the program with `load_program` and sets the initial memory as an empty MemoryDict. 
Then creates a CairoRunner with the program and initial memory, which it will then use to call `initialize_segments`, `initialize_main_entrypoint`, `initialize_vm`, `run_until_pc`, `end_run`, `relocate`, `print_output`.
This function will also create the necessary files, and customize the run according to specific flags.

## [`load_program`](https://github.com/starkware-libs/cairo-lang/blob/b614d1867c64f3fb2cf4a4879348cfcf87c3a5a7/src/starkware/cairo/lang/vm/cairo_run.py#L207)

Creates a ProgramBase with the data from the json file (the one declared with --program).

## [`initialize_segments`](https://github.com/starkware-libs/cairo-lang/blob/b614d1867c64f3fb2cf4a4879348cfcf87c3a5a7/src/starkware/cairo/lang/vm/cairo_runner.py#L175)

*Overall we could say initialize_segments creates the empty memory segments needed for the program itself, its execution, and each builtin.*

This function sets the `program_base` as the received program base, or creates a new segment for it, and asigns its first address (offset 0).
Creates a new segment for the execution_base and asigns its first address (offset 0).
Iterates over the builtin runners and calls their `initialize_segments`, this method differs between each subclass of BuiltinRunner and adds memory segments for the builtin. SimpleBuiltinRunner just creates a memory segment for itself.

The function `add` is the one responsible for creating an empty memory segment. The function `finalize` is only called when the function receives a non-zero size, which is not the case in this initialization.

## [`initialize_main_entrypoint`](https://github.com/starkware-libs/cairo-lang/blob/b614d1867c64f3fb2cf4a4879348cfcf87c3a5a7/src/starkware/cairo/lang/vm/cairo_runner.py#L186) 

Initializes state for running a program from the main() entrypoint. (Starts from label if proof_mode, we wont analyse this case).
Creates a stack.
Iterates over the builtins and appends each builtin's `initial_stack()` (initial stack elements enforced by this builtin) to the stack. For a simple builtin, this stack can be composed of an empty list, or a list with its base (A relocatable value). 
Creates a new segment for return_fp.
Checks that the main exists.
Calls `initialize_function_entrypoint` with the program's main, the stack and the return_fp.

#### [`initialize_function_entrypoint`](https://github.com/starkware-libs/cairo-lang/blob/b614d1867c64f3fb2cf4a4879348cfcf87c3a5a7/src/starkware/cairo/lang/vm/cairo_runner.py#L225)

*Overall what this function does is write the program's data and the builtin's stack into memory, set the initial fp, ap, and final pc, and return this final pc.*

Creates a stack with the previous stack, and appends the return_fp and a new memory segment (called end).
Calls `initialize_state` with this stack and main (now called entrypoint).

**[`initialize_state`](https://github.com/starkware-libs/cairo-lang/blob/b614d1867c64f3fb2cf4a4879348cfcf87c3a5a7/src/starkware/cairo/lang/vm/cairo_runner.py#L238)** 

Loads the program and the stack, by calling `load_data` with the program_base and the program's data, and then with the execution_base and the stack

**[`load_data`](https://github.com/starkware-libs/cairo-lang/blob/b614d1867c64f3fb2cf4a4879348cfcf87c3a5a7/src/starkware/cairo/lang/vm/cairo_runner.py#L564)** 

Writes data into the memory at address ptr and returns the first address after the data.
Directly inserts the data into the memory.

After thse function calls, `initial_fp` and `initial_ap` are set to `execution_base + 2`, and `final_pc` is set to end (the empty memory segment we created before) and returns it. This is then returned by initialize_main_entrypoint.

## [`initialize_vm`](https://github.com/starkware-libs/cairo-lang/blob/b614d1867c64f3fb2cf4a4879348cfcf87c3a5a7/src/starkware/cairo/lang/vm/cairo_runner.py#L245)

*Overall, this function creates and initializes the VirtualMachine structure, and validates the current memory*

Creates a RunContext with the initial ap, pc and fp, memory and prime from the CairoRunner.
Creates a VirtualMachine, with the hint_locals it receives (these are the program input), the previously created hints, an empty dictionary for static_locals (in the case of cairo-run), and the builtin_runners and program_base from the CairoRunner. 
Iterates over the buitin_runners and calls `add_validation_rules` and `add_auto_deduction_rules` from each of them (these methods are not defined for SimpleBuiltinRunner)
Calls `validate_existing_memory()`. This methos calls validated_memory's `validate_existing_memory()`, which iterates over the memory and calls `validate_memory_cell` on each address and value pair. This function then proceeds to iterate over the validated_memory's validation rules and add each validated_address that the rules output to the validated_addresses.

## [`run_until_pc`](https://github.com/starkware-libs/cairo-lang/blob/b614d1867c64f3fb2cf4a4879348cfcf87c3a5a7/src/starkware/cairo/lang/vm/cairo_runner.py#L284)

*Its the main loop of the program, iterates over step().*
Iterates over `step()` until the pc reaches the predefined end.

## [`end_run`](https://github.com/starkware-libs/cairo-lang/blob/b614d1867c64f3fb2cf4a4879348cfcf87c3a5a7/src/starkware/cairo/lang/vm/cairo_runner.py#L328)

*Overall, this function relocates the accessed addresses and the vm's memory, verifies the validated_memory using the auto_deduction rules, freezes the vm's memory, and calculates the size of each memory segment*

Relocates each address (using the`relocate_value` function in memory_dict) in the accessed_addresses. This function will attempt to relocate the value according to the segment's relocation_rules.
Relocates the vm's memory using `relocate_memory`(this doesnt turn relocatables into ints).
Calls the vm's `end_run` function. This function will then call `verify_auto_deductions`, which will make sure that all assigned memory cells are consistent with their auto deduction rules. It achieves this by checking that each address in validated_memory is either non-relocatable or its value is equal to the one that can be obtained via the segment's auto_deduction_rules.
Freezes memory (so that no more changes can happen).
Computes the size of each segment (via `compute_effective_sizes()`, which deduces the size of each segment from its usage). **All addresses at this point should be Relocatable**, or else an exception will be raised.

## [`relocate`](https://github.com/starkware-libs/cairo-lang/blob/b614d1867c64f3fb2cf4a4879348cfcf87c3a5a7/src/starkware/cairo/lang/vm/cairo_runner.py#L581)

*Overall this function is in charge of relocating every memory address in the vm's memory, trace entry and addresses used by builtins*

First calls `relocate_segments`, which returns a dictionary mapping the segment indexes to to the cummulative sum of each segment size (taking the base as 1 (first address constant)) and stores it as segment_offsets.
Then creates an initializer, a map between maybe relocatable values which contains the relocation (using `relocate_value`) of each address and value in memory.
This initializer is then used to create a MemoryDict, which is stored as the CairoRunner's `relocated_memory`.
Then the trace is relocated by calling `relocate_trace` on the vm's trace, the segment offsets, and the program's prime. This returns a new trace entry with the relocated fp, pc, and ap of each trace entry.
Then iterates over the builtin runners, and relocates their internal values (if applicable).
After this relocation, each address whould be an Int instead of a RelocatableValue.

## [`print_output`](https://github.com/starkware-libs/cairo-lang/blob/b614d1867c64f3fb2cf4a4879348cfcf87c3a5a7/src/starkware/cairo/lang/vm/cairo_runner.py#L653)

*Overall, this function checks for an output_builtin, and if found uses its base and its segment size to fetch the values from memory and print them on the console*


If there is no output_builtin, does nothing.
Calls the function `get_used_cells_and_allocated_size` from this builtin. which calls `get_segment_used_size`, which returns the segment_used_size computed earlier in `compute_effective_sizes`. This would be the size of the output_builtin segment, which would correspond to the amount of values to be printed.
Iterates over the size returned by the previous function calls fetching values from memory and printing them(uses the address at the output_runner's base, and adds 1 to it on each iteration).

# How does the CairoVM manage Hints?

The following is a broad analysis of how hints are processed from the compiled json file to their execution. Below the explanation, there is an **Example data**  section that will illustrate each of these steps showing the actual vm data for each step for a simple cairo program.

### During initialization phase (before step)

On the compiled json file, there is a hint section that includes each hint's code, accesible scopes (includes main, current function, builtins, and import path if inside an imported function) flow-tracking data, and the reference_ids which would be the variables it can interact with (the ones available on the scope the moment the hint is "called").
Once we load the json file and create a [Program](https://github.com/starkware-libs/cairo-lang/blob/2abd303e1808612b724bc1412b2b5babd04bb4e7/src/starkware/cairo/lang/compiler/program.py#L83) object, it will have a field called hints, which will be a dictionary containing this same information we saw on the json file.
When the vm is initialized, and the program’s data is loaded through `load_program`, [`load_hints`](https://github.com/starkware-libs/cairo-lang/blob/b614d1867c64f3fb2cf4a4879348cfcf87c3a5a7/src/starkware/cairo/lang/vm/virtual_machine_base.py#L202) is called, which creates a dictionary that maps an address to a list of compiled hints (there may be more than one hint for an address) this is called hints, and also creates a map from an id (the hint’s id) to a pc (address) and an index (in case there is more than one hint on a particular address), this is called hint_pc_and_index.
The compiled hints are obtained by using python's [`compile`](https://github.com/starkware-libs/cairo-lang/blob/b614d1867c64f3fb2cf4a4879348cfcf87c3a5a7/src/starkware/cairo/lang/vm/virtual_machine_base.py#L278) function in exec mode using the hint's code.

### During execution phase (with each step iteration)
Hints are the first thing to be handled once a [`step`](https://github.com/starkware-libs/cairo-lang/blob/b614d1867c64f3fb2cf4a4879348cfcf87c3a5a7/src/starkware/cairo/lang/vm/vm_core.py#L443) starts.
The current steps are taken to execute hints with each iteration:
1. We obtain the list of the hints at our current pc (if any) and iterate over it
2. We create an `exec_locals` dictionary that will contain our program’s input, memory (validated_memory), current registers (ap, fp, pc), the current step, ids (which contains memory), ap, fp, and pc as constants, the functions load_program, enter_scope and exit_scope, and the static_locals. The static_locals add to the exec_locals the program’s prime, the MemorySegmentManager (for example, this is used by alloc to add a memory segment for an undetermined-length array), and basic operations such as fadd, fsub, fdiv, etc.
3. We [execute](https://github.com/starkware-libs/cairo-lang/blob/b614d1867c64f3fb2cf4a4879348cfcf87c3a5a7/src/starkware/cairo/lang/vm/virtual_machine_base.py#L285) the hint using python’s exec function, using exec_locals as globals.


## Example data 

### Simple program with simple hint code

Cairo source code:

```cairo
func main():
    let a: felt = 1
    let b: felt = 2
    %{  
        b = 4
    %}
    let c: felt = 1 + 2
    return ()
end
```

On the compiled json file:

```json=
"hints": {
        "0": [
            {
                "accessible_scopes": [
                    "__main__",
                    "__main__.main"
                ],
                "code": "b = 4",
                "flow_tracking_data": {
                    "ap_tracking": {
                        "group": 0,
                        "offset": 0
                    },
                    "reference_ids": {
                        "__main__.main.a": 0,
                        "__main__.main.b": 1
                    }
                }
            }
        ]
    },
```

The hints field on the Program object:
```bash
{0: [CairoHint(code='b = 4',
accessible_scopes=[ScopedName(path=('__main__',)),
ScopedName(path=('__main__', 'main'))],
flow_tracking_data=FlowTrackingDataActual(ap_tracking=RegTrackingData(group=0, offset=0),
reference_ids={ScopedName(path=('__main__', 'main', 'a')): 0,
ScopedName(path=('__main__', 'main', 'b')): 1}))]}
```
On the VirtualMachine structure we have the following fields:

hint_pc_and_index: 

```bash
{0: (RelocatableValue(segment_index=0, offset=0), 0)}
```
hints: 

```bash
{RelocatableValue(segment_index=0, offset=0): 
[CompiledHint(
compiled=<code object <module> at 0x106a51920,
file “<hint0>”, line 1>,
consts=<function VirtualMachineBase.load_hints.<locals>.<lambda> at 0x106a0ff70>)]}
```
exec_locals before executing our hint:

```bash
{'program_input': {},
'memory': <starkware.cairo.lang.vm.validated_memory_dict.ValidatedMemoryDict object at 0x1086ef040>,
'ap': RelocatableValue(segment_index=1, offset=2),
'fp': RelocatableValue(segment_index=1, offset=2),
'pc': RelocatableValue(segment_index=0, offset=0),
'current_step': 0, 'ids': <starkware.cairo.lang.vm.vm_consts.VmConsts object at 0x108744820>,
'vm_load_program': <bound method VirtualMachineBase.load_program of <starkware.cairo.lang.vm.vm_core.VirtualMachine object at 0x1086efa90>>,
'vm_enter_scope': <bound method VirtualMachineBase.enter_scope of <starkware.cairo.lang.vm.vm_core.VirtualMachine object at 0x1086efa90>>,
'vm_exit_scope': <bound method VirtualMachineBase.exit_scope of <starkware.cairo.lang.vm.vm_core.VirtualMachine object at 0x1086efa90>>,
'segments': <starkware.cairo.lang.vm.memory_segments.MemorySegmentManager object at 0x10871c6a0>,
'PRIME': 3618502788666131213697322783095070105623107215331596699973092056135872020481,
'fadd': <function VirtualMachineBase.__init__.<locals>.<lambda> at 0x10870d550>, 
'fsub': <function VirtualMachineBase.__init__.<locals>.<lambda> at 0x10870d3a0>, 
'fmul': <function VirtualMachineBase.__init__.<locals>.<lambda> at 0x10870d5e0>, 
'fdiv': <function VirtualMachineBase.__init__.<locals>.<lambda> at 0x10870d310>, 
'fpow': <function VirtualMachineBase.__init__.<locals>.<lambda> at 0x10870d670>, 
'fis_quad_residue': <function VirtualMachineBase.__init__.<locals>.<lambda> at 0x10870d280>, 
'fsqrt': <function VirtualMachineBase.__init__.<locals>.<lambda> at 0x10870d700>, 
'safe_div': <function safe_div at 0x1058ca0d0>, 
'to_felt_or_relocatable': <function RelocatableValue.to_felt_or_relocatable at 0x108534670>}
```
## Non-simple cases

### What happens when we import a function that contains a hint?
The hint's code will contain the hint on the library function's code, and the accessible_scopes will become the import path.
For example, when importing the function [alloc()](https://github.com/starkware-libs/cairo-lang/blob/b614d1867c64f3fb2cf4a4879348cfcf87c3a5a7/src/starkware/cairo/common/alloc.cairo#L2), our Program's hints field will look like this:

```bash
{0: [CairoHint(code=‘memory[ap] = segments.add()’,
accessible_scopes=[ScopedName(path=(‘starkware’, ‘cairo’, ‘common’, ‘alloc’)),
ScopedName(path=(‘starkware’, ‘cairo’, ‘common’, ‘alloc’, ‘alloc’))],
flow_tracking_data=FlowTrackingDataActual(ap_tracking=RegTrackingData(group=0, offset=0), reference_ids={}))]}
```
