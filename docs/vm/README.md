<!-- This documentation was taken from the https://github.com/lambdaclass/cairo-vm_in_go repository -->

# How does the Cairo VM work?

## High Level Overview

The Cairo virtual machine is meant to be used in the context of STARK validity proofs. What this means is that the point of Cairo is not just to execute some code and get a result, but to *prove* to someone else that said execution was done correctly, without them having to re-execute the entire thing. The rough flow for it looks like this:

- A user writes a Cairo program.
- The program is compiled into Cairo's VM bytecode.
- The VM executes said code and provides a *trace* of execution, i.e. a record of the state of the machine and its memory *at every step of the computation*.
- This trace is passed on to a STARK prover, which creates a cryptographic proof from it, attesting to the correct execution of the program.
- The proof is passed to a verifier, who checks that the proof is valid in a fraction of a second, without re-executing.

The main three components of this flow are:

- A Cairo compiler to turn a program written in the [Cairo programming language](https://www.cairo-lang.org/) into bytecode.
- A Cairo VM to then execute it and generate a trace.
- [A STARK prover and verifier](https://github.com/starkware-libs/stone-prover/tree/main) so one party can prove correct execution, while another can verify it.

While this repo is only concerned with the second component, it's important to keep in mind the other two; especially important are the prover and verifier that this VM feeds its trace to, as a lot of its design decisions come from them. This virtual machine is designed to make proving and verifying both feasible and fast, and that makes it quite different from most other VMs you are probably used to.

## Basic VM flow

Our virtual machine has a very simple flow:

- Take a compiled cairo program as input. You can check out an example program [here](https://github.com/lambdaclass/cairo-vm.go/blob/main/cairo_programs/fibonacci.cairo).
- Run the bytecode from the compiled program, doing the usual `fetch->decode->execute` loop, running until program termination.
- On every step of the execution, record the values of each register.
- Take the register values and memory at every step and write them to a file, called the `execution trace`.

Barring some simplifications we made, this is all the Cairo VM does. The two main things that stand out as radically different are the memory model and the use of `Field Elements` to perform arithmetic. Below we go into more detail on each step, and in the process explain the ommisions we made.

## Memory

The Cairo virtual machine uses a Von Neumann architecture with a Non-deterministic read-only memory. What this means, roughly, is that memory is immutable after you've written to it (i.e. you can only write to it once); this is to make the STARK proving easier, but we won't go into that here.

### Memory Segments and Relocation

The process of memory allocation in a contiguous write-once memory region can get pretty complicated. Imagine you want to have a regular call stack, with a stack pointer pointing to the top of it and allocation and deallocation of stack frames and local variables happening throughout execution. Because memory is immutable, this cannot be done the usual way; once you allocate a new stack frame that memory is set, it can't be reused for another one later on.

Because of this, memory in Cairo is divided into `segments`. This is just a way of organizing memory more conveniently for this write-once model. Each segment is nothing more than a contiguous memory region. Segments are identified by an `index`, an integer value that uniquely identifies them.

Memory `cells` (i.e. values in memory) are identified by the index of the segment they belong to and an `offset` into said segment. Thus, the memory cell `{2,0}` is the first cell of segment number `2`.

Even though this segment model is extremely convenient for the VM's execution, the STARK prover needs to have the memory as just one contiguous region. Because of this, once execution of a Cairo program finishes, all the memory segments are collapsed into one; this process is called `Relocation`. We will go into more detail on all of this below.

The first segment (index 0) is the program segment, which stores the instructions of a cairo program. The following segment (index 1) is the execution segment, which holds the values that are created along the execution of the vm, for example, when we call a function, a pointer to the next instruction after the call instruction will be stored in the execution segment which will then be used to find the next instruction after the function returns.

The following group of segments are the builtin segments, one for each builtin used by the program, and which hold values used by the builtin runners.

The last group of segments are the user segments, which represent data structures created by the user, for example, when creating an array on a cairo program, that array will be represented in memory as its own segment.

While memory is continous, some gaps may be present. These gaps can be created on purpose by the user, for example by executing the following CASM:

```text
[ap + 1] = 2;
```

These gaps may also be created indireclty by diverging branches, as for example one branch may declare a variable that the other branch doesn't, as memory needs to be allocated for both cases if the second case is ran then a gap is left where the variable should have been written.

### Felts

Felts, or Field Elements, are cairo's basic integer type. Every variable in a cairo vm that is not a pointer is a felt. From our point of view we could say a felt in cairo is an unsigned integer in the range [0, P), where P is a very large prime currently equal to `2^251+17*2^192+1`. This means that all operations are done modulo P.

### Relocatable

An address (or pointer) in cairo is represented as a `Relocatable` value, which is made up of a `segment_index` and an `offset`, the `segment_index` tells us which segment the value is stored in and the `offset` tells us how many values exist between the start of the segment and the value.

As the cairo memory can hold both felts and pointers, the basic memory unit is a `MaybeRelocatable`, a variable that can be either a `Relocatable` or a `Felt`.

### Memory Relocation

During execution, the memory consists of segments of varying length, and they can be accessed by indicating their segment index, and the offset within that segment. When the run is finished, a relocation process takes place, which transforms this segmented memory into a contiguous list of values. The relocation process works as follows:

1. The size of each segment is calculated as the highest offset within the segment + 1.
2. A base is assigned to each segment by accumulating the size of the previous segment. The first segment's base is set to 1.
3. All `relocatable` values are converted into a single integer by adding their `offset` value to their segment's base calculated in the previous step

For example, if we have this memory represented by address, value pairs:

```text
    0:0 -> 1
    0:1 -> 4
    0:2 -> 7
    1:0 -> 8
    1:1 -> 0:2
    1:4 -> 0:1
    2:0 -> 1
```

Then, to relocate:

1. Calculate segment sizes:
   ```text
       0 --(has size)--> 3
       1 --(has size)--> 5
       2 --(has size)--> 1
   ```

2. Assign a base to each segment:
   ```text
       0 --(has base value)--> 1
       1 --(has base value)--> 4 (that is: 1 + 3)
       2 --(has base value)--> 9 (that is: 4 + 5)
   ```

3. Convert relocatables to integers
   ```text
       1 (base[0] + 0) -> 1
       2 (base[0] + 1) -> 4
       3 (base[0] + 2) -> 7
       4 (base[1] + 0) -> 8
       5 (base[1] + 1) -> 3 (that is: base[0] + 2)
       .... (memory gaps)
       8 (base[1] + 4) -> 2 (that is: base[0] + 1)
       9 (base[2] + 0) -> 1
   ```

## Instruction Set

### Registers

There are only three registers in the Cairo VM:

- The program counter `pc`, which points to the next instruction to be executed.
- The allocation pointer `ap`, pointing to the next unused memory cell.
- The frame pointer `fp`, pointing to the base of the current stack frame. When a new function is called, `fp` is set to the current `ap`. When the function returns, `fp` goes back to its previous value. The VM creates new segments whenever dynamic allocation is needed, so for example the cairo analog to a Rust `Vec` will have its own segment. Relocation at the end meshes everything together.

### Instruction Format

CASM instruction have the following format. If the instruction uses an immediate operand, it's value is taken from the next cell of the program segment.

```
//  Structure of the 63-bit that form the first word of each instruction.
//  See Cairo whitepaper, page 32 - https://eprint.iacr.org/2021/1063.pdf.
┌─────────────────────────────────────────────────────────────────────────┐
│                     off_dst (biased representation)                     │
├─────────────────────────────────────────────────────────────────────────┤
│                     off_op0 (biased representation)                     │
├─────────────────────────────────────────────────────────────────────────┤
│                     off_op1 (biased representation)                     │
├─────┬─────┬───────┬───────┬───────────┬────────┬───────────────────┬────┤
│ dst │ op0 │  op1  │  res  │    pc     │   ap   │      opcode       │ 0  │
│ reg │ reg │  src  │ logic │  update   │ update │                   │    │
├─────┼─────┼───┬───┼───┬───┼───┬───┬───┼───┬────┼────┬────┬────┬────┼────┤
│  0  │  1  │ 2 │ 3 │ 4 │ 5 │ 6 │ 7 │ 8 │ 9 │ 10 │ 11 │ 12 │ 13 │ 14 │ 15 │
└─────┴─────┴───┴───┴───┴───┴───┴───┴───┴───┴────┴────┴────┴────┴────┴────┘
```

- The first 6 fields: `off_dst`, `off_op0`, `off_op1`, `dst_reg`, `op0_reg`, `op1_src` determine the memory locations of the operands, and the destination of the result (which is not always used).
  - `op1_src` occupies 2 bits, as it can also be an immediate, in which case the value will be taken from the next instruction (`[pc + 1]`).
- The `res_logic` field determines how to compute the result (op1, sum, multiplication). The usage of the result depends on the following fields.
- The `opcode` field describes which operation is been performed (noop, assert, call, ret). It modifies the meaning of the following fields.
- The `pc_update` field determines how to update the program counter (advance, jump, branch, etc.).
- The `ap_update` field determines how to update the allocation pointer.

For an in-depth explanation, you can see Cairo whitepaper, page 33 - https://eprint.iacr.org/2021/1063.pdf, or checkout [our implementation](/vm/src/vm/vm_core.rs).

Take, for example, the following instruction:

```
[ap + 1] = [fp + 2] + 3
```

The instruction (at `[pc]`) will be encoded as:

```
off_dst   = 1
off_op0   = 2
off_op1   = 1 # It's always 1 when op1 is an immediate
dst_reg   = 0 # AP
op0_reg   = 1 # FP
op1_src   = 1 # Immediate
res_logic = 1 # Add
pc_update = 0 # Regular (advance)
ap_update = 0 # Regular (no update)
opcode    = 4 # Assert
```

The next instruction (at `[pc + 1]`) will be the immediate, and it will have a value of `3`.

Given the following initial register values:
```
fp = 5
ap = 10
pc = 15
```
Then:
- `op1` is `[fp + 2]`, which is resolved to `[7]`.
- `op2` is `[pc + 1]`, which is resolved to `[16] == 3`.
- `dst` is `[ap + 1]`, which is resolved to `[11]`
- The result of `op1 + op2` is stored at `dst`
- The register `pc` is increased by 2, we skip the next instruction because it was the immediate.
- The register `fp` is not updated
- The register `ap` is not updated

## Hints

So far we have been thinking about the VM mostly abstracted from the prover and verifier it's meant to feed its results to. The last main feature we need to talk about, however, requires keeping this proving/verifying logic in mind.

As a reminder, the whole point of the Cairo VM is to output a trace/memory file so that a `prover` can then create a cryptographic proof that the execution of the program was done correctly. A `verifier` can then take that proof and verify it in much less time than it would have taken to re-execute the entire program.

In this model, the one actually using the VM to run a cairo program is *always the prover*. The verifier does not use the VM in any way, as that would defeat the entire purpose of validity proofs; they just get the program being run and the proof generated by the prover and run some cryptographic algorithm to check it.

While the verifier does not execute the code, they do *check it*. As an example, if a cairo program computes a fibonacci number like this:

```
func main() {
    // Call fib(1, 1, 10).
    let result: felt = fib(1, 1, 10);
}
```

the verifier won't *run* this, but they will reject any incorrect execution of the call to `fib`. The correct value for `result` in this case is `144` (it's the 10th fibonacci number); any attempt by the prover to convince the verifier that `result` is not `144` will fail, because the call to the `fib` function is *being proven* and thus *seen* by the verifier. 

A `Hint` is a piece of code that is not proven, and therefore not seen by the verifier. If `fib` above were a hint, then the prover could convince the verifier that `result` is $144$, $0$, $1000$ or any other number.

In cairo 0, hints are code written in `Python` and are surrounded by curly brackets. Here's an example from the `alloc` function, provided by the Cairo common library

```
func alloc() -> (ptr: felt*) {
    %{ memory[ap] = segments.add() %}
    ap += 1;
    return (ptr=cast([ap - 1], felt*));
}
```

The first line of the function,

```
%{ memory[ap] = segments.add() %}
```

is a hint called `ADD_SEGMENT`. All it does is create a new memory segment, then write its base to the current value of `ap`. This is python code that is being run in the context of the VM's execution; thus `memory` refers to the VM's current memory and `segments.add()` is just a function provided by the VM to allocate a new segment.

At this point you might be wondering: why run code that's not being proven? Isn't the whole point of Cairo to prove correct execution? There are (at least) two reasons for hints to exist.

### Nothing to prove

For some operations there's simply nothing to prove, as they are just convenient things one wants to do during execution. The `ADD_SEGMENT` hint shown above is a good example of that. When proving execution, the program's memory is presented as one relocated continuous segment, it does not matter at all which segment a cell was in, or when that segment was added. The verifier doesn't care.

Because of this, there's no reason to make `ADD_SEGMENT` a part of the cairo language and have an instruction for it.

### Optimization

Certain operations can be very expensive, in the sense that they might involve a huge amount of instructions or memory usage, and therefore contribute heavily to the proving time. For certain calculations, there are two ways to convince the verifier that it was done correctly:

- Write the entire calculation in Cairo/Cairo Assembly. This makes it show up in the trace and therefore get proven.
- *Present the result of the calculation to the verifier through a hint*, then show said result indeed satisfies the relevant condition that makes it the actual result.

To make this less abstract, let's show two examples.

#### Square root

Let's say the calculation in question is to compute the square root of a number `x`. The two ways to do it then become:

- Write the usual square root algorithm in Cairo to compute `sqrt(x)`.
- Write a hint that computes `sqrt(x)`, then immediately after calling the hint show __in Cairo__ that `(sqrt(x))^2 = x`.

The second approach is exactly what the `sqrt` function in the Cairo common library does:

```
// Returns the floor value of the square root of the given value.
// Assumptions: 0 <= value < 2**250.
func sqrt{range_check_ptr}(value) -> felt {
    alloc_locals;
    local root: felt;

    %{
        from starkware.python.math_utils import isqrt
        value = ids.value % PRIME
        assert value < 2 ** 250, f"value={value} is outside of the range [0, 2**250)."
        assert 2 ** 250 < PRIME
        ids.root = isqrt(value)
    %}

    assert_nn_le(root, 2 ** 125 - 1);
    tempvar root_plus_one = root + 1;
    assert_in_range(value, root * root, root_plus_one * root_plus_one);

    return root;
}
```

If you read it carefully, you'll see that the hint in this function computes the square root in python, then this line

```
assert_in_range(value, root * root, root_plus_one * root_plus_one);
```

asserts __in Cairo__ that `(sqrt(x))^2 = x`.

This is done this way because it is much cheaper, in terms of the generated trace (and thus proving time), to square a number than compute its square root.

Notice that the last assert is absolutely mandatory to make this safe. If you forget to write it, the square root calculation does not get proven, and anyone could convince the verifier that the result of `sqrt(x)` is any number they like.

#### Linear search turned into an O(1) lookup

This example is taken from the [Cairo documentation](https://docs.cairo-lang.org/0.12.0/hello_cairo/program_input.html).

Given a list of `(key, value)` pairs, if we want to write a `get_value_by_key` function that returns the value associated to a given key, there are two ways to do it:

- Write a linear search in Cairo, iterating over each key until you find the requested one.
- Do that exact same linear search *inside a hint*, find the result, then show that the result's key is the one requested.

Again, the second approach makes the resulting trace and proving much faster, because it's just a lookup; there's no linear search. Notice this only applies to proving, the VM has to execute the hint, so there's still a linear search when executing to generate the trace. In fact, the second approach is more expensive for the VM than the first one. It has to do both a linear search and a lookup. This is a tradeoff in favor of proving time.

Also note that, as in the square root example, when writing this logic you need to remember to show the hint's result is the correct one in Cairo. If you don't, your code is not being proven.

### Non-determinism

The Cairo paper and documentation refers to this second approach to calculating things through hints as *non-determinism*. The reason for this is that sometimes there is more than one result that satisfies a certain condition. This means that cairo execution becomes non deterministic; a hint could output multiple values, and in principle there is no way to know which one it's going to be. Running the same code multiple times could give different results.

The square root is an easy example of this. The condition `(sqrt(x))^2 = x` is not unique, there are two solutions to it. Without the hint, this is non-deterministic, `x` could have multiple values; the hint resolves that by choosing a specific value when being run.

### Common Library and Hints

As explained above, using hints in your code is highly unsafe. Forgetting to add a check after calling them can make your code vulnerable to any sorts of attacks, as your program will not prove what you think it proves.

Because of this, most hints in Cairo 0 are wrapped around or used by functions in the Cairo common library that do the checks for you, thus making them safe to use. Ideally, Cairo developers should not be using hints on their own; only transparently through Cairo library functions they call.

### Whitelisted Hints

In Cairo, a hint could be any Python code you like. In the context of it as just another language someone might want to use, this is fine. In the context of Cairo as a programming language used to write smart contracts deployed on a blockchain, it's not. Users could deploy contracts with hints that simply do

```python
while true:
    pass
```

and grind the network down to a halt, as nodes get stuck executing an infinite loop when calling the contract.

To address this, the starknet network maintains a list of *whitelisted* hints, which are the only ones that can be used in starknet contracts. These are the ones implemented in this VM.

## Builtins

A builtin is a low level optimization integrated into the core loop of the VM that allows otherwise expensive computation to be performed more efficiently. Builtins have two ways to operate: via validation rules and via auto-deduction rules.

- Validation rules are applied to every element that is inserted into a builtin's segment. For example, if I want to verify an ecdsa signature, I can insert it into the ecdsa builtin's segment and let a validation rule take care of verifying the signature.
- Auto-deduction rules take over during instruction execution, when we can't compute the value of an operand who's address belongs to a builtin segment, we can use that builtin's auto-deduction rule to calculate the value of the operand. For example, If I want to calculate the pedersen hash of two values, I can write the values into the pedersen builtin's segment and then ask for the next memory cell, without builtins, this instruction would have failed, as there is no value stored in that cell, but now we can use auto-deduction rules to calculate the hash and fill in that memory cell.

## Program parsing

The input of the Virtual Machine is a compiled Cairo program in Json format. The main parts of the file are listed below:

- **data:** List of hexadecimal values that represent the instructions and immediate values defined in the cairo program. Each hexadecimal value is stored as a maybe_relocatable element in memory, but they can only be felts because the decoder has to be able to get the instruction fields in its bit representation.
- **debug_info:** This field provides information about the instructions defined in the data list. Each one is identified with its index inside the data list. For each one it contains information about the cairo variables in scope, the hints executed before that instruction if any, and its location inside the cairo program.
- **hints:** All the hints used in the program, ordered by the pc offset at which they should be executed.
- **identifiers:** User-defined symbols in the Cairo code representing variables, functions, classes, etc. with unique names. The expected offset, type and its corresponding information is provided for each identifier

    For example, the identifier representing the main function (usually the entrypoint of the program) is of `function` type, and a list of decorators wrappers (if there are any) are provided as additional information.
    Another example is a user defined struct, is of `struct` type, it provides its size, the members it contains (with its information) and more.

- **main_scope:** Usually something like `__main__`. All the identifiers associated with main function will be identified as `__main__`.identifier_name. Useful to identify the entrypoint of the program.
- **prime:** The cairo prime in hexadecimal format. As explained above, all arithmetic operations are done over a base field, modulo this primer number.
- **reference_manager:** Contains information about cairo variables. This information is useful to access to variables when executing cairo hints.

# Code Walkthrough

If you want a step by step guide on how to implement your own VM you can read our [code walkthrough in Go](https://github.com/lambdaclass/cairo-vm_in_go?tab=readme-ov-file#code-walkthroughwrite-your-own-cairo-vm)
