use crate::bigint;
use crate::hint_processor::hint_processor_definition::HintProcessor;
use crate::hint_processor::hint_processor_definition::HintReference;
use crate::types::exec_scope::ExecutionScopes;
use crate::types::instruction::Register;
use crate::types::program::Program;
use crate::types::relocatable::{relocate_value, MaybeRelocatable, Relocatable};
use crate::utils::{is_subsequence, to_field_element};
use crate::vm::errors::memory_errors::MemoryError;
use crate::vm::errors::runner_errors::RunnerError;
use crate::vm::errors::trace_errors::TraceError;
use crate::vm::errors::vm_errors::VirtualMachineError;
use crate::vm::runners::builtin_runner::{
    BitwiseBuiltinRunner, BuiltinRunner, EcOpBuiltinRunner, HashBuiltinRunner, OutputBuiltinRunner,
    RangeCheckBuiltinRunner,
};
use crate::vm::trace::trace_entry::{relocate_trace_register, RelocatedTraceEntry};
use crate::vm::vm_core::VirtualMachine;
use num_bigint::BigInt;
use std::any::Any;
use std::collections::HashMap;
use std::io;

pub struct CairoRunner<'a> {
    program: Program,
    pub vm: VirtualMachine,
    _layout: String,
    final_pc: Option<Relocatable>,
    program_base: Option<Relocatable>,
    execution_base: Option<Relocatable>,
    initial_ap: Option<Relocatable>,
    initial_fp: Option<Relocatable>,
    initial_pc: Option<Relocatable>,
    pub relocated_memory: Vec<Option<BigInt>>,
    pub relocated_trace: Option<Vec<RelocatedTraceEntry>>,
    pub exec_scopes: ExecutionScopes,
    hint_executor: &'a dyn HintProcessor,
}

impl<'a> CairoRunner<'a> {
    pub fn new(
        program: &Program,
        trace_enabled: bool,
        hint_executor: &'a dyn HintProcessor,
    ) -> Result<CairoRunner<'a>, RunnerError> {
        let builtin_ordered_list = vec![
            String::from("output"),
            String::from("pedersen"),
            String::from("range_check"),
            String::from("ecdsa"),
            String::from("bitwise"),
            String::from("ec_op"),
        ];
        if !is_subsequence(&program.builtins, &builtin_ordered_list) {
            return Err(RunnerError::DisorderedBuiltins);
        };
        let mut builtin_runners = Vec::<(String, Box<dyn BuiltinRunner>)>::new();
        for builtin_name in program.builtins.iter() {
            if builtin_name == "output" {
                builtin_runners.push((builtin_name.clone(), Box::new(OutputBuiltinRunner::new())));
            }

            if builtin_name == "pedersen" {
                builtin_runners.push((builtin_name.clone(), Box::new(HashBuiltinRunner::new(8))));
            }

            if builtin_name == "range_check" {
                //Information for Buitin info taken from here https://github.com/starkware-libs/cairo-lang/blob/b614d1867c64f3fb2cf4a4879348cfcf87c3a5a7/src/starkware/cairo/lang/instances.py#L115
                builtin_runners.push((
                    builtin_name.clone(),
                    Box::new(RangeCheckBuiltinRunner::new(bigint!(8), 8)),
                ));
            }
            if builtin_name == "bitwise" {
                builtin_runners.push((
                    builtin_name.clone(),
                    Box::new(BitwiseBuiltinRunner::new(256)),
                ));
            }
            if builtin_name == "ec_op" {
                builtin_runners.push((builtin_name.clone(), Box::new(EcOpBuiltinRunner::new(256))));
            }
        }
        //Initialize a vm, with empty values, will later be filled with actual data in initialize_vm
        Ok(CairoRunner {
            program: program.clone(),
            _layout: String::from("plain"),
            vm: VirtualMachine::new(program.prime.clone(), builtin_runners, trace_enabled),
            final_pc: None,
            program_base: None,
            execution_base: None,
            initial_ap: None,
            initial_fp: None,
            initial_pc: None,
            relocated_memory: Vec::new(),
            relocated_trace: None,
            hint_executor,
            exec_scopes: ExecutionScopes::new(),
        })
    }
    ///Creates the necessary segments for the program, execution, and each builtin on the MemorySegmentManager and stores the first adress of each of this new segments as each owner's base
    pub fn initialize_segments(&mut self, program_base: Option<Relocatable>) {
        self.program_base = match program_base {
            Some(base) => Some(base),
            None => Some(self.vm.segments.add(&mut self.vm.memory)),
        };
        self.execution_base = Some(self.vm.segments.add(&mut self.vm.memory));
        for (_key, builtin_runner) in self.vm.builtin_runners.iter_mut() {
            builtin_runner.initialize_segments(&mut self.vm.segments, &mut self.vm.memory);
        }
    }

    fn initialize_state(
        &mut self,
        entrypoint: usize,
        stack: Vec<MaybeRelocatable>,
    ) -> Result<(), RunnerError> {
        if let Some(prog_base) = self.program_base.clone() {
            let initial_pc = Relocatable {
                segment_index: prog_base.segment_index,
                offset: prog_base.offset + entrypoint,
            };
            self.initial_pc = Some(initial_pc);
            self.vm
                .segments
                .load_data(
                    &mut self.vm.memory,
                    &MaybeRelocatable::RelocatableValue(prog_base),
                    self.program.data.clone(),
                )
                .map_err(RunnerError::MemoryInitializationError)?;
        }
        if let Some(exec_base) = &self.execution_base {
            self.vm
                .segments
                .load_data(
                    &mut self.vm.memory,
                    &MaybeRelocatable::RelocatableValue(exec_base.clone()),
                    stack,
                )
                .map_err(RunnerError::MemoryInitializationError)?;
        } else {
            return Err(RunnerError::NoProgBase);
        }
        Ok(())
    }

    fn initialize_function_entrypoint(
        &mut self,
        entrypoint: usize,
        mut stack: Vec<MaybeRelocatable>,
        return_fp: MaybeRelocatable,
    ) -> Result<MaybeRelocatable, RunnerError> {
        let end = self.vm.segments.add(&mut self.vm.memory);
        stack.append(&mut vec![
            return_fp,
            MaybeRelocatable::RelocatableValue(end.clone()),
        ]);
        if let Some(base) = &self.execution_base {
            self.initial_fp = Some(Relocatable {
                segment_index: base.segment_index,
                offset: base.offset + stack.len(),
            });
            self.initial_ap = self.initial_fp.clone();
        } else {
            return Err(RunnerError::NoExecBaseForEntrypoint);
        }
        self.initialize_state(entrypoint, stack)?;
        self.final_pc = Some(end.clone());
        Ok(MaybeRelocatable::RelocatableValue(end))
    }
    ///Initializes state for running a program from the main() entrypoint.
    ///If self.proof_mode == True, the execution starts from the start label rather then the main() function.
    ///Returns the value of the program counter after returning from main.
    pub fn initialize_main_entrypoint(&mut self) -> Result<MaybeRelocatable, RunnerError> {
        //self.execution_public_memory = Vec::new() -> Not used now
        let mut stack = Vec::new();
        for (_name, builtin_runner) in self.vm.builtin_runners.iter() {
            stack.append(&mut builtin_runner.initial_stack());
        }
        //Different process if proof_mode is enabled
        let return_fp = self.vm.segments.add(&mut self.vm.memory);
        if let Some(main) = &self.program.main {
            let main_clone = *main;
            Ok(self.initialize_function_entrypoint(
                main_clone,
                stack,
                MaybeRelocatable::RelocatableValue(return_fp),
            )?)
        } else {
            Err(RunnerError::MissingMain)
        }
    }

    pub fn initialize_vm(&mut self) -> Result<(), RunnerError> {
        self.vm.run_context.pc = self.initial_pc.as_ref().ok_or(RunnerError::NoPC)?.clone();
        self.vm.run_context.ap = self.initial_ap.as_ref().ok_or(RunnerError::NoAP)?.offset;
        self.vm.run_context.fp = self.initial_fp.as_ref().ok_or(RunnerError::NoFP)?.offset;
        self.vm._program_base = Some(MaybeRelocatable::from(
            self.program_base.as_ref().ok_or(RunnerError::NoProgBase)?,
        ));
        for (_, builtin) in self.vm.builtin_runners.iter() {
            builtin.add_validation_rule(&mut self.vm.memory);
        }
        self.vm
            .memory
            .validate_existing_memory()
            .map_err(RunnerError::MemoryValidationError)
    }

    fn get_reference_list(&self) -> HashMap<usize, HintReference> {
        let mut references = HashMap::<usize, HintReference>::new();

        for (i, reference) in self.program.reference_manager.references.iter().enumerate() {
            references.insert(
                i,
                HintReference {
                    register: reference.value_address.register.clone(),
                    offset1: reference.value_address.offset1,
                    offset2: reference.value_address.offset2,
                    inner_dereference: reference.value_address.inner_dereference,
                    dereference: reference.value_address.dereference,
                    immediate: reference.value_address.immediate.clone(),
                    // only store `ap` tracking data if the reference is referred to it
                    ap_tracking_data: if reference.value_address.register == Some(Register::FP) {
                        None
                    } else {
                        Some(reference.ap_tracking_data.clone())
                    },
                },
            );
        }
        references
    }

    //Gets the data used by the HintProcessor to execute each hint
    fn get_hint_data_dictionary(
        &self,
        references: &HashMap<usize, HintReference>,
    ) -> Result<HashMap<usize, Vec<Box<dyn Any>>>, VirtualMachineError> {
        let mut hint_data_dictionary = HashMap::<usize, Vec<Box<dyn Any>>>::new();
        for (hint_index, hints) in self.program.hints.iter() {
            for hint in hints {
                let hint_data = self.hint_executor.compile_hint(
                    &hint.code,
                    &hint.flow_tracking_data.ap_tracking,
                    &hint.flow_tracking_data.reference_ids,
                    references,
                );
                hint_data_dictionary
                    .entry(*hint_index)
                    .or_insert(vec![])
                    .push(
                        hint_data
                            .map_err(|_| VirtualMachineError::CompileHintFail(hint.code.clone()))?,
                    );
            }
        }
        Ok(hint_data_dictionary)
    }

    pub fn run_until_pc(&mut self, address: MaybeRelocatable) -> Result<(), VirtualMachineError> {
        let references = self.get_reference_list();
        let hint_data_dictionary = self.get_hint_data_dictionary(&references)?;
        while self.vm.run_context.get_pc() != address {
            self.vm.step(
                self.hint_executor,
                &mut self.exec_scopes,
                &hint_data_dictionary,
            )?;
        }
        Ok(())
    }

    ///Relocates the VM's memory, turning bidimensional indexes into contiguous numbers, and values into BigInts
    /// Uses the relocation_table to asign each index a number according to the value on its segment number
    fn relocate_memory(&mut self, relocation_table: &Vec<usize>) -> Result<(), MemoryError> {
        if !(self.relocated_memory.is_empty()) {
            return Err(MemoryError::Relocation);
        }
        //Relocated addresses start at 1
        self.relocated_memory.push(None);
        for (index, segment) in self.vm.memory.data.iter().enumerate() {
            if self.relocated_memory.len() != relocation_table[index] {
                return Err(MemoryError::Relocation);
            }

            for element in segment {
                match element {
                    Some(elem) => self
                        .relocated_memory
                        .push(Some(relocate_value(elem.clone(), relocation_table)?)),
                    None => self.relocated_memory.push(None),
                }
            }
        }
        Ok(())
    }

    ///Relocates the VM's trace, turning relocatable registers to numbered ones
    fn relocate_trace(&mut self, relocation_table: &Vec<usize>) -> Result<(), TraceError> {
        if self.relocated_trace.is_some() {
            return Err(TraceError::AlreadyRelocated);
        }

        let trace = self
            .vm
            .trace
            .as_ref()
            .ok_or(TraceError::TraceNotEnabled)?
            .iter();
        let mut relocated_trace = Vec::<RelocatedTraceEntry>::with_capacity(trace.len());
        for entry in trace {
            relocated_trace.push(RelocatedTraceEntry {
                pc: relocate_trace_register(&entry.pc, relocation_table)?,
                ap: relocate_trace_register(&entry.ap, relocation_table)?,
                fp: relocate_trace_register(&entry.fp, relocation_table)?,
            })
        }
        self.relocated_trace = Some(relocated_trace);
        Ok(())
    }

    pub fn relocate(&mut self) -> Result<(), TraceError> {
        self.vm.segments.compute_effective_sizes(&self.vm.memory);
        // relocate_segments can fail if compute_effective_sizes is not called before.
        // The expect should be unreachable.
        let relocation_table = self
            .vm
            .segments
            .relocate_segments()
            .expect("compute_effective_sizes called but relocate_memory still returned error");
        if let Err(memory_error) = self.relocate_memory(&relocation_table) {
            return Err(TraceError::MemoryError(memory_error));
        }
        if self.vm.trace.is_some() {
            self.relocate_trace(&relocation_table)?;
        }
        Ok(())
    }

    pub fn get_output(&mut self) -> Result<Option<String>, RunnerError> {
        let mut output = Vec::<u8>::new();
        self.write_output(&mut output)?;
        let output = String::from_utf8(output).map_err(|_| RunnerError::FailedStringConversion)?;
        Ok(Some(output))
    }

    ///Writes the values hosted in the output builtin's segment
    /// Does nothing if the output builtin is not present in the program
    pub fn write_output(&mut self, stdout: &mut dyn io::Write) -> Result<(), RunnerError> {
        //If the output builtin is present it will always be the first one
        if !self.vm.builtin_runners.is_empty() && self.vm.builtin_runners[0].0 == *"output" {
            let builtin = &self.vm.builtin_runners[0].1;
            self.vm.segments.compute_effective_sizes(&self.vm.memory);

            let base = builtin.base();

            // After this if block,
            // segment_used_sizes is always Some(_)
            if self.vm.segments.segment_used_sizes == None {
                self.vm.segments.compute_effective_sizes(&self.vm.memory);
            }
            // See previous comment, the unwrap below is safe.
            for i in 0..self.vm.segments.segment_used_sizes.as_ref().unwrap()[base.segment_index] {
                let value = self
                    .vm
                    .memory
                    .get_integer(&(base.clone() + i))
                    .map_err(|_| {
                        RunnerError::MemoryGet(MaybeRelocatable::from(base.clone() + i))
                    })?;
                writeln!(
                    stdout,
                    "{}",
                    to_field_element(value.clone(), self.vm.prime.clone())
                )
                .map_err(|_| RunnerError::WriteFail)?;
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use num_bigint::Sign;
    use num_traits::FromPrimitive;

    use super::*;
    use crate::hint_processor::builtin_hint_processor::builtin_hint_processor_definition::BuiltinHintProcessor;
    use crate::serde::deserialize_program::ReferenceManager;
    use crate::vm::trace::trace_entry::TraceEntry;
    use crate::{bigint_str, relocatable};
    use std::collections::HashMap;

    #[test]
    fn create_cairo_runner_with_disordered_builtins() {
        //This test works with basic Program definition, will later be updated to use Program::new() when fully defined
        let program = Program {
            builtins: vec![String::from("range_check"), String::from("output")],
            prime: bigint!(17),
            data: Vec::new(),
            main: None,
            hints: HashMap::new(),
            reference_manager: ReferenceManager {
                references: Vec::new(),
            },
        };
        let hint_processor = BuiltinHintProcessor::new_empty();
        let cairo_runner = CairoRunner::new(&program, false, &hint_processor);
        assert!(cairo_runner.is_err());
    }

    #[test]
    fn create_cairo_runner_with_ordered_but_missing_builtins() {
        //This test works with basic Program definition, will later be updated to use Program::new() when fully defined
        let program = Program {
            builtins: vec![String::from("output"), String::from("ecdsa")],
            prime: bigint!(17),
            data: Vec::new(),
            main: None,
            hints: HashMap::new(),
            reference_manager: ReferenceManager {
                references: Vec::new(),
            },
        };
        let hint_processor = BuiltinHintProcessor::new_empty();
        //We only check that the creation doesnt panic
        let _cairo_runner = CairoRunner::new(&program, false, &hint_processor);
    }

    #[test]
    fn initialize_segments_with_base() {
        //This test works with basic Program definition, will later be updated to use Program::new() when fully defined
        let program = Program {
            builtins: vec![String::from("output")],
            prime: bigint!(17),
            data: Vec::new(),
            main: None,
            hints: HashMap::new(),
            reference_manager: ReferenceManager {
                references: Vec::new(),
            },
        };
        let hint_processor = BuiltinHintProcessor::new_empty();
        let mut cairo_runner = CairoRunner::new(&program, false, &hint_processor).unwrap();
        let program_base = Some(Relocatable {
            segment_index: 5,
            offset: 9,
        });
        cairo_runner.vm.segments.num_segments = 6;
        cairo_runner.initialize_segments(program_base);
        assert_eq!(
            cairo_runner.program_base,
            Some(Relocatable {
                segment_index: 5,
                offset: 9,
            })
        );
        assert_eq!(
            cairo_runner.execution_base,
            Some(Relocatable {
                segment_index: 6,
                offset: 0,
            })
        );
        assert_eq!(cairo_runner.vm.builtin_runners[0].0, String::from("output"));
        assert_eq!(
            cairo_runner.vm.builtin_runners[0].1.base(),
            Relocatable::from((7, 0))
        );

        assert_eq!(cairo_runner.vm.segments.num_segments, 8);
    }

    #[test]
    fn initialize_segments_no_base() {
        //This test works with basic Program definition, will later be updated to use Program::new() when fully defined
        let program = Program {
            builtins: vec![String::from("output")],
            prime: bigint!(17),
            data: Vec::new(),
            main: None,
            hints: HashMap::new(),
            reference_manager: ReferenceManager {
                references: Vec::new(),
            },
        };
        let hint_processor = BuiltinHintProcessor::new_empty();
        let mut cairo_runner = CairoRunner::new(&program, false, &hint_processor).unwrap();
        cairo_runner.initialize_segments(None);
        assert_eq!(
            cairo_runner.program_base,
            Some(Relocatable {
                segment_index: 0,
                offset: 0
            })
        );
        assert_eq!(
            cairo_runner.execution_base,
            Some(Relocatable {
                segment_index: 1,
                offset: 0
            })
        );
        assert_eq!(cairo_runner.vm.builtin_runners[0].0, String::from("output"));
        assert_eq!(
            cairo_runner.vm.builtin_runners[0].1.base(),
            Relocatable::from((2, 0))
        );

        assert_eq!(cairo_runner.vm.segments.num_segments, 3);
    }

    #[test]
    fn initialize_state_empty_data_and_stack() {
        //This test works with basic Program definition, will later be updated to use Program::new() when fully defined
        let program = Program {
            builtins: vec![String::from("output")],
            prime: bigint!(17),
            data: Vec::new(),
            main: None,
            hints: HashMap::new(),
            reference_manager: ReferenceManager {
                references: Vec::new(),
            },
        };
        let hint_processor = BuiltinHintProcessor::new_empty();
        let mut cairo_runner = CairoRunner::new(&program, false, &hint_processor).unwrap();
        cairo_runner.program_base = Some(relocatable!(1, 0));
        cairo_runner.execution_base = Some(relocatable!(2, 0));
        let stack = Vec::new();
        cairo_runner.initialize_state(1, stack).unwrap();
        assert_eq!(
            cairo_runner.initial_pc,
            Some(Relocatable {
                segment_index: 1,
                offset: 1
            })
        );
    }

    #[test]
    fn initialize_state_some_data_empty_stack() {
        //This test works with basic Program definition, will later be updated to use Program::new() when fully defined
        let program = Program {
            builtins: vec![String::from("output")],
            prime: bigint!(17),
            data: vec![
                MaybeRelocatable::from(bigint!(4)),
                MaybeRelocatable::from(bigint!(6)),
            ],
            main: None,
            hints: HashMap::new(),
            reference_manager: ReferenceManager {
                references: Vec::new(),
            },
        };
        let hint_processor = BuiltinHintProcessor::new_empty();
        let mut cairo_runner = CairoRunner::new(&program, false, &hint_processor).unwrap();
        for _ in 0..2 {
            cairo_runner.vm.segments.add(&mut cairo_runner.vm.memory);
        }
        cairo_runner.program_base = Some(Relocatable {
            segment_index: 1,
            offset: 0,
        });
        cairo_runner.execution_base = Some(relocatable!(2, 0));
        let stack = Vec::new();
        cairo_runner.initialize_state(1, stack).unwrap();
        assert_eq!(
            cairo_runner
                .vm
                .memory
                .get(&MaybeRelocatable::RelocatableValue(
                    cairo_runner.program_base.unwrap()
                ))
                .unwrap(),
            Some(&MaybeRelocatable::from(bigint!(4)))
        );
        assert_eq!(
            cairo_runner
                .vm
                .memory
                .get(&MaybeRelocatable::from((1, 1)))
                .unwrap(),
            Some(&MaybeRelocatable::from(bigint!(6)))
        );
    }

    #[test]
    fn initialize_state_empty_data_some_stack() {
        //This test works with basic Program definition, will later be updated to use Program::new() when fully defined
        let program = Program {
            builtins: vec![String::from("output")],
            prime: bigint!(17),
            data: Vec::new(),
            main: None,
            hints: HashMap::new(),
            reference_manager: ReferenceManager {
                references: Vec::new(),
            },
        };
        let hint_processor = BuiltinHintProcessor::new_empty();
        let mut cairo_runner = CairoRunner::new(&program, false, &hint_processor).unwrap();
        for _ in 0..3 {
            cairo_runner.vm.segments.add(&mut cairo_runner.vm.memory);
        }
        cairo_runner.program_base = Some(relocatable!(1, 0));
        cairo_runner.execution_base = Some(relocatable!(2, 0));
        let stack = vec![
            MaybeRelocatable::from(bigint!(4)),
            MaybeRelocatable::from(bigint!(6)),
        ];
        cairo_runner.initialize_state(1, stack).unwrap();
        assert_eq!(
            cairo_runner
                .vm
                .memory
                .get(&MaybeRelocatable::RelocatableValue(
                    cairo_runner.execution_base.unwrap()
                ))
                .unwrap(),
            Some(&MaybeRelocatable::from(bigint!(4)))
        );
        assert_eq!(
            cairo_runner
                .vm
                .memory
                .get(&MaybeRelocatable::from((2, 1)))
                .unwrap(),
            Some(&MaybeRelocatable::from(bigint!(6)))
        );
    }

    #[test]
    fn initialize_state_no_program_base() {
        //This test works with basic Program definition, will later be updated to use Program::new() when fully defined
        let program = Program {
            builtins: vec![String::from("output")],
            prime: bigint!(17),
            data: Vec::new(),
            main: None,
            hints: HashMap::new(),
            reference_manager: ReferenceManager {
                references: Vec::new(),
            },
        };
        let hint_processor = BuiltinHintProcessor::new_empty();
        let mut cairo_runner = CairoRunner::new(&program, false, &hint_processor).unwrap();
        for _ in 0..2 {
            cairo_runner.vm.segments.add(&mut cairo_runner.vm.memory);
        }
        cairo_runner.execution_base = Some(Relocatable {
            segment_index: 2,
            offset: 0,
        });
        let stack = vec![
            MaybeRelocatable::from(bigint!(4)),
            MaybeRelocatable::from(bigint!(6)),
        ];
        assert!(cairo_runner.initialize_state(1, stack).is_err());
    }

    #[test]
    #[should_panic]
    fn initialize_state_no_execution_base() {
        //This test works with basic Program definition, will later be updated to use Program::new() when fully defined
        let program = Program {
            builtins: vec![String::from("output")],
            prime: bigint!(17),
            data: Vec::new(),
            main: None,
            hints: HashMap::new(),
            reference_manager: ReferenceManager {
                references: Vec::new(),
            },
        };
        let hint_processor = BuiltinHintProcessor::new_empty();
        let mut cairo_runner = CairoRunner::new(&program, false, &hint_processor).unwrap();
        for _ in 0..2 {
            cairo_runner.vm.segments.add(&mut cairo_runner.vm.memory);
        }
        cairo_runner.program_base = Some(relocatable!(1, 0));
        let stack = vec![
            MaybeRelocatable::from(bigint!(4)),
            MaybeRelocatable::from(bigint!(6)),
        ];
        cairo_runner.initialize_state(1, stack).unwrap();
    }

    #[test]
    fn initialize_function_entrypoint_empty_stack() {
        //This test works with basic Program definition, will later be updated to use Program::new() when fully defined
        let program = Program {
            builtins: vec![String::from("output")],
            prime: bigint!(17),
            data: Vec::new(),
            main: None,
            hints: HashMap::new(),
            reference_manager: ReferenceManager {
                references: Vec::new(),
            },
        };
        let hint_processor = BuiltinHintProcessor::new_empty();
        let mut cairo_runner = CairoRunner::new(&program, false, &hint_processor).unwrap();
        for _ in 0..2 {
            cairo_runner.vm.segments.add(&mut cairo_runner.vm.memory);
        }
        cairo_runner.program_base = Some(relocatable!(0, 0));
        cairo_runner.execution_base = Some(relocatable!(1, 0));
        let stack = Vec::new();
        let return_fp = MaybeRelocatable::from(bigint!(9));
        cairo_runner
            .initialize_function_entrypoint(0, stack, return_fp)
            .unwrap();
        assert_eq!(cairo_runner.initial_fp, cairo_runner.initial_ap);
        assert_eq!(cairo_runner.initial_fp, Some(relocatable!(1, 2)));
        assert_eq!(
            cairo_runner
                .vm
                .memory
                .get(&MaybeRelocatable::from((1, 0)))
                .unwrap(),
            Some(&MaybeRelocatable::from(bigint!(9)))
        );
        assert_eq!(
            cairo_runner
                .vm
                .memory
                .get(&MaybeRelocatable::from((1, 1)))
                .unwrap(),
            Some(&MaybeRelocatable::from((2, 0)))
        );
    }

    #[test]
    fn initialize_function_entrypoint_some_stack() {
        //This test works with basic Program definition, will later be updated to use Program::new() when fully defined
        let program = Program {
            builtins: vec![String::from("output")],
            prime: bigint!(17),
            data: Vec::new(),
            main: None,
            hints: HashMap::new(),
            reference_manager: ReferenceManager {
                references: Vec::new(),
            },
        };
        let hint_processor = BuiltinHintProcessor::new_empty();
        let mut cairo_runner = CairoRunner::new(&program, false, &hint_processor).unwrap();
        for _ in 0..2 {
            cairo_runner.vm.segments.add(&mut cairo_runner.vm.memory);
        }
        cairo_runner.program_base = Some(relocatable!(0, 0));
        cairo_runner.execution_base = Some(relocatable!(1, 0));
        let stack = vec![MaybeRelocatable::from(bigint!(7))];
        let return_fp = MaybeRelocatable::from(bigint!(9));
        cairo_runner
            .initialize_function_entrypoint(1, stack, return_fp)
            .unwrap();
        assert_eq!(cairo_runner.initial_fp, cairo_runner.initial_ap);
        assert_eq!(cairo_runner.initial_fp, Some(relocatable!(1, 3)));
        assert_eq!(
            cairo_runner
                .vm
                .memory
                .get(&MaybeRelocatable::from((1, 0)))
                .unwrap(),
            Some(&MaybeRelocatable::from(bigint!(7)))
        );
        assert_eq!(
            cairo_runner
                .vm
                .memory
                .get(&MaybeRelocatable::from((1, 1)))
                .unwrap(),
            Some(&MaybeRelocatable::from(bigint!(9)))
        );
        assert_eq!(
            cairo_runner
                .vm
                .memory
                .get(&MaybeRelocatable::from((1, 2)))
                .unwrap(),
            Some(&MaybeRelocatable::from((2, 0)))
        );
    }

    #[test]
    #[should_panic]
    fn initialize_function_entrypoint_no_execution_base() {
        //This test works with basic Program definition, will later be updated to use Program::new() when fully defined
        let program = Program {
            builtins: vec![String::from("output")],
            prime: bigint!(17),
            data: Vec::new(),
            main: None,
            hints: HashMap::new(),
            reference_manager: ReferenceManager {
                references: Vec::new(),
            },
        };
        let hint_processor = BuiltinHintProcessor::new_empty();
        let mut cairo_runner = CairoRunner::new(&program, false, &hint_processor).unwrap();
        let stack = vec![MaybeRelocatable::from(bigint!(7))];
        let return_fp = MaybeRelocatable::from(bigint!(9));
        cairo_runner
            .initialize_function_entrypoint(1, stack, return_fp)
            .unwrap();
    }

    #[test]
    #[should_panic]
    fn initialize_main_entrypoint_no_main() {
        //This test works with basic Program definition, will later be updated to use Program::new() when fully defined
        let program = Program {
            builtins: vec![String::from("output")],
            prime: bigint!(17),
            data: Vec::new(),
            main: None,
            hints: HashMap::new(),
            reference_manager: ReferenceManager {
                references: Vec::new(),
            },
        };
        let hint_processor = BuiltinHintProcessor::new_empty();
        let mut cairo_runner = CairoRunner::new(&program, false, &hint_processor).unwrap();
        cairo_runner.initialize_main_entrypoint().unwrap();
    }

    #[test]
    fn initialize_main_entrypoint() {
        //This test works with basic Program definition, will later be updated to use Program::new() when fully defined
        let program = Program {
            builtins: Vec::new(),
            prime: bigint!(17),
            data: Vec::new(),
            main: Some(1),
            hints: HashMap::new(),
            reference_manager: ReferenceManager {
                references: Vec::new(),
            },
        };
        let hint_processor = BuiltinHintProcessor::new_empty();
        let mut cairo_runner = CairoRunner::new(&program, false, &hint_processor).unwrap();
        cairo_runner.program_base = Some(relocatable!(0, 0));
        cairo_runner.execution_base = Some(relocatable!(0, 0));
        let return_pc = cairo_runner.initialize_main_entrypoint().unwrap();
        assert_eq!(return_pc, MaybeRelocatable::from((1, 0)));
    }

    #[test]
    fn initialize_vm_no_builtins() {
        //This test works with basic Program definition, will later be updated to use Program::new() when fully defined
        let program = Program {
            builtins: Vec::new(),
            prime: bigint!(17),
            data: Vec::new(),
            main: Some(1),
            hints: HashMap::new(),
            reference_manager: ReferenceManager {
                references: Vec::new(),
            },
        };
        let hint_processor = BuiltinHintProcessor::new_empty();
        let mut cairo_runner = CairoRunner::new(&program, false, &hint_processor).unwrap();
        cairo_runner.program_base = Some(relocatable!(0, 0));
        cairo_runner.initial_pc = Some(relocatable!(0, 1));
        cairo_runner.initial_ap = Some(relocatable!(1, 2));
        cairo_runner.initial_fp = Some(relocatable!(1, 2));
        cairo_runner.initialize_vm().unwrap();
        assert_eq!(cairo_runner.vm.run_context.pc, relocatable!(0, 1));
        assert_eq!(cairo_runner.vm.run_context.ap, 2);
        assert_eq!(cairo_runner.vm.run_context.fp, 2);
        assert_eq!(
            cairo_runner.vm._program_base,
            Some(MaybeRelocatable::from((0, 0)))
        );
    }

    #[test]
    fn initialize_vm_with_range_check_valid() {
        //This test works with basic Program definition, will later be updated to use Program::new() when fully defined
        let program = Program {
            builtins: vec![String::from("range_check")],
            prime: bigint!(17),
            data: Vec::new(),
            main: Some(1),
            hints: HashMap::new(),
            reference_manager: ReferenceManager {
                references: Vec::new(),
            },
        };
        let hint_processor = BuiltinHintProcessor::new_empty();
        let mut cairo_runner = CairoRunner::new(&program, false, &hint_processor).unwrap();
        cairo_runner.initial_pc = Some(relocatable!(0, 1));
        cairo_runner.initial_ap = Some(relocatable!(1, 2));
        cairo_runner.initial_fp = Some(relocatable!(1, 2));
        cairo_runner.initialize_segments(None);
        cairo_runner
            .vm
            .memory
            .insert(
                &MaybeRelocatable::from((2, 0)),
                &MaybeRelocatable::from(bigint!(23)),
            )
            .unwrap();
        cairo_runner
            .vm
            .memory
            .insert(
                &MaybeRelocatable::from((2, 1)),
                &MaybeRelocatable::from(bigint!(233)),
            )
            .unwrap();
        assert_eq!(
            cairo_runner.vm.builtin_runners[0].0,
            String::from("range_check")
        );
        assert_eq!(
            cairo_runner.vm.builtin_runners[0].1.base(),
            relocatable!(2, 0)
        );
        cairo_runner.initialize_vm().unwrap();
        assert!(cairo_runner
            .vm
            .memory
            .validated_addresses
            .contains(&MaybeRelocatable::from((2, 0))));
        assert!(cairo_runner
            .vm
            .memory
            .validated_addresses
            .contains(&MaybeRelocatable::from((2, 1))));
        assert_eq!(cairo_runner.vm.memory.validated_addresses.len(), 2);
    }

    #[test]
    #[should_panic]
    fn initialize_vm_with_range_check_invalid() {
        //This test works with basic Program definition, will later be updated to use Program::new() when fully defined
        let program = Program {
            builtins: vec![String::from("range_check")],
            prime: bigint!(17),
            data: Vec::new(),
            main: Some(1),
            hints: HashMap::new(),
            reference_manager: ReferenceManager {
                references: Vec::new(),
            },
        };
        let hint_processor = BuiltinHintProcessor::new_empty();
        let mut cairo_runner = CairoRunner::new(&program, false, &hint_processor).unwrap();
        cairo_runner.initial_pc = Some(relocatable!(0, 1));
        cairo_runner.initial_ap = Some(relocatable!(1, 2));
        cairo_runner.initial_fp = Some(relocatable!(1, 2));
        cairo_runner.initialize_segments(None);
        cairo_runner
            .vm
            .memory
            .insert(
                &MaybeRelocatable::from((2, 1)),
                &MaybeRelocatable::from(bigint!(23)),
            )
            .unwrap();
        cairo_runner
            .vm
            .memory
            .insert(
                &MaybeRelocatable::from((2, 4)),
                &MaybeRelocatable::from(bigint!(-1)),
            )
            .unwrap();
        cairo_runner.initialize_vm().unwrap();
    }

    //Integration tests for initialization phase

    #[test]
    /* Program used:
    func myfunc(a: felt) -> (r: felt):
        let b = a * 2
        return(b)
    end

    func main():
        let a = 1
        let b = myfunc(a)
        return()
    end

    main = 3
    data = [5207990763031199744, 2, 2345108766317314046, 5189976364521848832, 1, 1226245742482522112, 3618502788666131213697322783095070105623107215331596699973092056135872020476, 2345108766317314046]
    */
    fn initialization_phase_no_builtins() {
        let program = Program {
            builtins: vec![],
            prime: bigint!(17),
            data: vec![
                MaybeRelocatable::from(BigInt::from_i64(5207990763031199744).unwrap()),
                MaybeRelocatable::from(bigint!(2)),
                MaybeRelocatable::from(BigInt::from_i64(2345108766317314046).unwrap()),
                MaybeRelocatable::from(BigInt::from_i64(5189976364521848832).unwrap()),
                MaybeRelocatable::from(bigint!(1)),
                MaybeRelocatable::from(BigInt::from_i64(1226245742482522112).unwrap()),
                MaybeRelocatable::Int(BigInt::new(
                    Sign::Plus,
                    vec![
                        4294967292, 4294967295, 4294967295, 4294967295, 4294967295, 4294967295, 16,
                        134217728,
                    ],
                )),
                MaybeRelocatable::from(BigInt::from_i64(2345108766317314046).unwrap()),
            ],
            main: Some(3),
            hints: HashMap::new(),
            reference_manager: ReferenceManager {
                references: Vec::new(),
            },
        };
        let hint_processor = BuiltinHintProcessor::new_empty();
        let mut cairo_runner = CairoRunner::new(&program, false, &hint_processor).unwrap();
        cairo_runner.initialize_segments(None);
        cairo_runner.initialize_main_entrypoint().unwrap();
        cairo_runner.initialize_vm().unwrap();

        assert_eq!(cairo_runner.program_base, Some(relocatable!(0, 0)));
        assert_eq!(cairo_runner.execution_base, Some(relocatable!(1, 0)));
        assert_eq!(cairo_runner.final_pc, Some(relocatable!(3, 0)));

        //RunContext check
        //Registers
        assert_eq!(cairo_runner.vm.run_context.pc, relocatable!(0, 3));
        assert_eq!(cairo_runner.vm.run_context.ap, 2);
        assert_eq!(cairo_runner.vm.run_context.fp, 2);
        //Memory
        assert_eq!(
            cairo_runner
                .vm
                .memory
                .get(&MaybeRelocatable::from((0, 0)))
                .unwrap(),
            Some(&MaybeRelocatable::from(
                BigInt::from_i64(5207990763031199744).unwrap()
            ))
        );
        assert_eq!(
            cairo_runner
                .vm
                .memory
                .get(&MaybeRelocatable::from((0, 1)))
                .unwrap(),
            Some(&MaybeRelocatable::from(bigint!(2)))
        );
        assert_eq!(
            cairo_runner
                .vm
                .memory
                .get(&MaybeRelocatable::from((0, 2)))
                .unwrap(),
            Some(&MaybeRelocatable::from(
                BigInt::from_i64(2345108766317314046).unwrap()
            ))
        );
        assert_eq!(
            cairo_runner
                .vm
                .memory
                .get(&MaybeRelocatable::from((0, 3)))
                .unwrap(),
            Some(&MaybeRelocatable::from(
                BigInt::from_i64(5189976364521848832).unwrap()
            ))
        );
        assert_eq!(
            cairo_runner
                .vm
                .memory
                .get(&MaybeRelocatable::from((0, 4)))
                .unwrap(),
            Some(&MaybeRelocatable::from(bigint!(1)))
        );
        assert_eq!(
            cairo_runner
                .vm
                .memory
                .get(&MaybeRelocatable::from((0, 5)))
                .unwrap(),
            Some(&MaybeRelocatable::from(
                BigInt::from_i64(1226245742482522112).unwrap()
            ))
        );
        assert_eq!(
            cairo_runner
                .vm
                .memory
                .get(&MaybeRelocatable::from((0, 6)))
                .unwrap(),
            Some(&MaybeRelocatable::Int(BigInt::new(
                Sign::Plus,
                vec![
                    4294967292, 4294967295, 4294967295, 4294967295, 4294967295, 4294967295, 16,
                    134217728,
                ],
            )))
        );
        assert_eq!(
            cairo_runner
                .vm
                .memory
                .get(&MaybeRelocatable::from((0, 7)))
                .unwrap(),
            Some(&MaybeRelocatable::from(
                BigInt::from_i64(2345108766317314046).unwrap()
            ))
        );
        assert_eq!(
            cairo_runner
                .vm
                .memory
                .get(&MaybeRelocatable::from((1, 0)))
                .unwrap(),
            Some(&MaybeRelocatable::from((2, 0)))
        );
        assert_eq!(
            cairo_runner
                .vm
                .memory
                .get(&MaybeRelocatable::from((1, 1)))
                .unwrap(),
            Some(&MaybeRelocatable::from((3, 0)))
        );
    }

    #[test]
    /*Program used:
    %builtins output

    from starkware.cairo.common.serialize import serialize_word

    func main{output_ptr: felt*}():
        let a = 1
        serialize_word(a)
        return()
    end

    main = 4
    data = [4612671182993129469, 5198983563776393216, 1, 2345108766317314046, 5191102247248822272, 5189976364521848832, 1, 1226245742482522112, 3618502788666131213697322783095070105623107215331596699973092056135872020474, 2345108766317314046]
    */
    fn initialization_phase_output_builtin() {
        let program = Program {
            builtins: vec![String::from("output")],
            prime: bigint!(17),
            data: vec![
                MaybeRelocatable::from(BigInt::from_i64(4612671182993129469).unwrap()),
                MaybeRelocatable::from(BigInt::from_i64(5198983563776393216).unwrap()),
                MaybeRelocatable::from(bigint!(1)),
                MaybeRelocatable::from(BigInt::from_i64(2345108766317314046).unwrap()),
                MaybeRelocatable::from(BigInt::from_i64(5191102247248822272).unwrap()),
                MaybeRelocatable::from(BigInt::from_i64(5189976364521848832).unwrap()),
                MaybeRelocatable::from(bigint!(1)),
                MaybeRelocatable::from(BigInt::from_i64(1226245742482522112).unwrap()),
                MaybeRelocatable::Int(BigInt::new(
                    Sign::Plus,
                    vec![
                        4294967290, 4294967295, 4294967295, 4294967295, 4294967295, 4294967295, 16,
                        134217728,
                    ],
                )),
                MaybeRelocatable::from(BigInt::from_i64(2345108766317314046).unwrap()),
            ],
            main: Some(4),
            hints: HashMap::new(),
            reference_manager: ReferenceManager {
                references: Vec::new(),
            },
        };
        let hint_processor = BuiltinHintProcessor::new_empty();
        let mut cairo_runner = CairoRunner::new(&program, false, &hint_processor).unwrap();
        cairo_runner.initialize_segments(None);
        cairo_runner.initialize_main_entrypoint().unwrap();
        cairo_runner.initialize_vm().unwrap();

        assert_eq!(cairo_runner.program_base, Some(relocatable!(0, 0)));
        assert_eq!(cairo_runner.execution_base, Some(relocatable!(1, 0)));
        assert_eq!(cairo_runner.final_pc, Some(relocatable!(4, 0)));

        //RunContext check
        //Registers
        assert_eq!(cairo_runner.vm.run_context.pc, relocatable!(0, 4));
        assert_eq!(cairo_runner.vm.run_context.ap, 3);
        assert_eq!(cairo_runner.vm.run_context.fp, 3);
        //Memory
        assert_eq!(
            cairo_runner
                .vm
                .memory
                .get(&MaybeRelocatable::from((0, 0)))
                .unwrap(),
            Some(&MaybeRelocatable::from(
                BigInt::from_i64(4612671182993129469).unwrap()
            ))
        );
        assert_eq!(
            cairo_runner
                .vm
                .memory
                .get(&MaybeRelocatable::from((0, 1)))
                .unwrap(),
            Some(&MaybeRelocatable::from(
                BigInt::from_i64(5198983563776393216).unwrap()
            ))
        );
        assert_eq!(
            cairo_runner
                .vm
                .memory
                .get(&MaybeRelocatable::from((0, 2)))
                .unwrap(),
            Some(&MaybeRelocatable::from(bigint!(1)))
        );
        assert_eq!(
            cairo_runner
                .vm
                .memory
                .get(&MaybeRelocatable::from((0, 3)))
                .unwrap(),
            Some(&MaybeRelocatable::from(
                BigInt::from_i64(2345108766317314046).unwrap()
            ))
        );
        assert_eq!(
            cairo_runner
                .vm
                .memory
                .get(&MaybeRelocatable::from((0, 4)))
                .unwrap(),
            Some(&MaybeRelocatable::from(
                BigInt::from_i64(5191102247248822272).unwrap()
            ))
        );
        assert_eq!(
            cairo_runner
                .vm
                .memory
                .get(&MaybeRelocatable::from((0, 5)))
                .unwrap(),
            Some(&MaybeRelocatable::from(
                BigInt::from_i64(5189976364521848832).unwrap()
            ))
        );
        assert_eq!(
            cairo_runner
                .vm
                .memory
                .get(&MaybeRelocatable::from((0, 6)))
                .unwrap(),
            Some(&MaybeRelocatable::from(bigint!(1)))
        );
        assert_eq!(
            cairo_runner
                .vm
                .memory
                .get(&MaybeRelocatable::from((0, 7)))
                .unwrap(),
            Some(&MaybeRelocatable::from(
                BigInt::from_i64(1226245742482522112).unwrap()
            ))
        );
        assert_eq!(
            cairo_runner
                .vm
                .memory
                .get(&MaybeRelocatable::from((0, 8)))
                .unwrap(),
            Some(&MaybeRelocatable::Int(BigInt::new(
                Sign::Plus,
                vec![
                    4294967290, 4294967295, 4294967295, 4294967295, 4294967295, 4294967295, 16,
                    134217728
                ]
            )))
        );
        assert_eq!(
            cairo_runner
                .vm
                .memory
                .get(&MaybeRelocatable::from((0, 9)))
                .unwrap(),
            Some(&MaybeRelocatable::from(
                BigInt::from_i64(2345108766317314046).unwrap()
            ))
        );
        assert_eq!(
            cairo_runner
                .vm
                .memory
                .get(&MaybeRelocatable::from((1, 0)))
                .unwrap(),
            Some(&MaybeRelocatable::from((2, 0)))
        );
        assert_eq!(
            cairo_runner
                .vm
                .memory
                .get(&MaybeRelocatable::from((1, 1)))
                .unwrap(),
            Some(&MaybeRelocatable::from((3, 0)))
        );
        assert_eq!(
            cairo_runner
                .vm
                .memory
                .get(&MaybeRelocatable::from((1, 2)))
                .unwrap(),
            Some(&MaybeRelocatable::from((4, 0)))
        );
    }

    #[test]
    /*Program used:
    %builtins range_check

    func check_range{range_check_ptr}(num):

        # Check that 0 <= num < 2**64.
        [range_check_ptr] = num
        assert [range_check_ptr + 1] = 2 ** 64 - 1 - num
        let range_check_ptr = range_check_ptr + 2
        return()
    end

    func main{range_check_ptr}():
        check_range(7)
        return()
    end

    main = 8
    data = [4612671182993129469, 5189976364521848832, 18446744073709551615, 5199546496550207487, 4612389712311386111, 5198983563776393216, 2, 2345108766317314046, 5191102247248822272, 5189976364521848832, 7, 1226245742482522112, 3618502788666131213697322783095070105623107215331596699973092056135872020470, 2345108766317314046]
    */
    fn initialization_phase_range_check_builtin() {
        let program = Program {
            builtins: vec![String::from("range_check")],
            prime: bigint!(17),
            data: vec![
                MaybeRelocatable::from(BigInt::from_i64(4612671182993129469).unwrap()),
                MaybeRelocatable::from(BigInt::from_i64(5189976364521848832).unwrap()),
                MaybeRelocatable::Int(BigInt::from_i128(18446744073709551615).unwrap()),
                MaybeRelocatable::from(BigInt::from_i64(5199546496550207487).unwrap()),
                MaybeRelocatable::from(BigInt::from_i64(4612389712311386111).unwrap()),
                MaybeRelocatable::from(BigInt::from_i64(5198983563776393216).unwrap()),
                MaybeRelocatable::from(bigint!(2)),
                MaybeRelocatable::from(BigInt::from_i64(2345108766317314046).unwrap()),
                MaybeRelocatable::from(BigInt::from_i64(5191102247248822272).unwrap()),
                MaybeRelocatable::from(BigInt::from_i64(5189976364521848832).unwrap()),
                MaybeRelocatable::from(bigint!(7)),
                MaybeRelocatable::from(BigInt::from_i64(1226245742482522112).unwrap()),
                MaybeRelocatable::Int(BigInt::new(
                    Sign::Plus,
                    vec![
                        4294967286, 4294967295, 4294967295, 4294967295, 4294967295, 4294967295, 16,
                        134217728,
                    ],
                )),
                MaybeRelocatable::from(BigInt::from_i64(2345108766317314046).unwrap()),
            ],
            main: Some(8),
            hints: HashMap::new(),
            reference_manager: ReferenceManager {
                references: Vec::new(),
            },
        };
        let hint_processor = BuiltinHintProcessor::new_empty();
        let mut cairo_runner = CairoRunner::new(&program, false, &hint_processor).unwrap();
        cairo_runner.initialize_segments(None);
        cairo_runner.initialize_main_entrypoint().unwrap();
        cairo_runner.initialize_vm().unwrap();

        assert_eq!(cairo_runner.program_base, Some(relocatable!(0, 0)));
        assert_eq!(cairo_runner.execution_base, Some(relocatable!(1, 0)));
        assert_eq!(cairo_runner.final_pc, Some(relocatable!(4, 0)));

        //RunContext check
        //Registers
        assert_eq!(cairo_runner.vm.run_context.pc, relocatable!(0, 8));
        assert_eq!(cairo_runner.vm.run_context.ap, 3);
        assert_eq!(cairo_runner.vm.run_context.fp, 3);
        //Memory
        assert_eq!(
            cairo_runner
                .vm
                .memory
                .get(&MaybeRelocatable::from((0, 0)))
                .unwrap(),
            Some(&MaybeRelocatable::from(
                BigInt::from_i64(4612671182993129469).unwrap()
            ))
        );
        assert_eq!(
            cairo_runner
                .vm
                .memory
                .get(&MaybeRelocatable::from((0, 1)))
                .unwrap(),
            Some(&MaybeRelocatable::from(
                BigInt::from_i64(5189976364521848832).unwrap()
            ))
        );
        assert_eq!(
            cairo_runner
                .vm
                .memory
                .get(&MaybeRelocatable::from((0, 2)))
                .unwrap(),
            Some(&MaybeRelocatable::Int(
                BigInt::from_i128(18446744073709551615).unwrap()
            ))
        );
        assert_eq!(
            cairo_runner
                .vm
                .memory
                .get(&MaybeRelocatable::from((0, 3)))
                .unwrap(),
            Some(&MaybeRelocatable::from(
                BigInt::from_i64(5199546496550207487).unwrap()
            ))
        );
        assert_eq!(
            cairo_runner
                .vm
                .memory
                .get(&MaybeRelocatable::from((0, 4)))
                .unwrap(),
            Some(&MaybeRelocatable::from(
                BigInt::from_i64(4612389712311386111).unwrap()
            ))
        );
        assert_eq!(
            cairo_runner
                .vm
                .memory
                .get(&MaybeRelocatable::from((0, 5)))
                .unwrap(),
            Some(&MaybeRelocatable::from(
                BigInt::from_i64(5198983563776393216).unwrap()
            ))
        );
        assert_eq!(
            cairo_runner
                .vm
                .memory
                .get(&MaybeRelocatable::from((0, 6)))
                .unwrap(),
            Some(&MaybeRelocatable::from(bigint!(2)))
        );
        assert_eq!(
            cairo_runner
                .vm
                .memory
                .get(&MaybeRelocatable::from((0, 7)))
                .unwrap(),
            Some(&MaybeRelocatable::from(
                BigInt::from_i64(2345108766317314046).unwrap()
            ))
        );
        assert_eq!(
            cairo_runner
                .vm
                .memory
                .get(&MaybeRelocatable::from((0, 8)))
                .unwrap(),
            Some(&MaybeRelocatable::from(
                BigInt::from_i64(5191102247248822272).unwrap()
            ))
        );
        assert_eq!(
            cairo_runner
                .vm
                .memory
                .get(&MaybeRelocatable::from((0, 9)))
                .unwrap(),
            Some(&MaybeRelocatable::from(
                BigInt::from_i64(5189976364521848832).unwrap()
            ))
        );
        assert_eq!(
            cairo_runner
                .vm
                .memory
                .get(&MaybeRelocatable::from((0, 10)))
                .unwrap(),
            Some(&MaybeRelocatable::from(bigint!(7)))
        );
        assert_eq!(
            cairo_runner
                .vm
                .memory
                .get(&MaybeRelocatable::from((0, 11)))
                .unwrap(),
            Some(&MaybeRelocatable::from(
                BigInt::from_i64(1226245742482522112).unwrap()
            ))
        );
        assert_eq!(
            cairo_runner
                .vm
                .memory
                .get(&MaybeRelocatable::from((0, 12)))
                .unwrap(),
            Some(&MaybeRelocatable::Int(BigInt::new(
                Sign::Plus,
                vec![
                    4294967286, 4294967295, 4294967295, 4294967295, 4294967295, 4294967295, 16,
                    134217728
                ]
            )))
        );
        assert_eq!(
            cairo_runner
                .vm
                .memory
                .get(&MaybeRelocatable::from((0, 13)))
                .unwrap(),
            Some(&MaybeRelocatable::from(
                BigInt::from_i64(2345108766317314046).unwrap()
            ))
        );
        assert_eq!(
            cairo_runner
                .vm
                .memory
                .get(&MaybeRelocatable::from((1, 0)))
                .unwrap(),
            Some(&MaybeRelocatable::from((2, 0)))
        );
        assert_eq!(
            cairo_runner
                .vm
                .memory
                .get(&MaybeRelocatable::from((1, 1)))
                .unwrap(),
            Some(&MaybeRelocatable::from((3, 0)))
        );
        assert_eq!(
            cairo_runner
                .vm
                .memory
                .get(&MaybeRelocatable::from((1, 2)))
                .unwrap(),
            Some(&MaybeRelocatable::from((4, 0)))
        );
    }

    //Integration tests for initialization + execution phase

    #[test]
    /*Program used:
    func myfunc(a: felt) -> (r: felt):
        let b = a * 2
        return(b)
    end

    func main():
        let a = 1
        let b = myfunc(a)
        return()
    end

    main = 3
    data = [5207990763031199744, 2, 2345108766317314046, 5189976364521848832, 1, 1226245742482522112, 3618502788666131213697322783095070105623107215331596699973092056135872020476, 2345108766317314046]
    */
    fn initialize_and_run_function_call() {
        //Initialization Phase
        let program = Program {
            builtins: vec![],
            prime: BigInt::new(Sign::Plus, vec![1, 0, 0, 0, 0, 0, 17, 134217728]),
            data: vec![
                MaybeRelocatable::from(BigInt::from_i64(5207990763031199744).unwrap()),
                MaybeRelocatable::from(bigint!(2)),
                MaybeRelocatable::from(BigInt::from_i64(2345108766317314046).unwrap()),
                MaybeRelocatable::from(BigInt::from_i64(5189976364521848832).unwrap()),
                MaybeRelocatable::from(bigint!(1)),
                MaybeRelocatable::from(BigInt::from_i64(1226245742482522112).unwrap()),
                MaybeRelocatable::Int(BigInt::new(
                    Sign::Plus,
                    vec![
                        4294967292, 4294967295, 4294967295, 4294967295, 4294967295, 4294967295, 16,
                        134217728,
                    ],
                )),
                MaybeRelocatable::from(BigInt::from_i64(2345108766317314046).unwrap()),
            ],
            main: Some(3),
            hints: HashMap::new(),
            reference_manager: ReferenceManager {
                references: Vec::new(),
            },
        };
        let hint_processor = BuiltinHintProcessor::new_empty();
        let mut cairo_runner = CairoRunner::new(&program, true, &hint_processor).unwrap();
        cairo_runner.initialize_segments(None);
        let end = cairo_runner.initialize_main_entrypoint().unwrap();
        assert_eq!(end, MaybeRelocatable::from((3, 0)));
        cairo_runner.initialize_vm().unwrap();
        //Execution Phase
        assert_eq!(cairo_runner.run_until_pc(end), Ok(()));
        //Check final values against Python VM
        //Check final register values
        assert_eq!(cairo_runner.vm.run_context.pc, Relocatable::from((3, 0)));

        assert_eq!(cairo_runner.vm.run_context.ap, 6);

        assert_eq!(cairo_runner.vm.run_context.fp, 0);

        //Check each TraceEntry in trace
        let trace = cairo_runner.vm.trace.unwrap();
        assert_eq!(trace.len(), 5);
        assert_eq!(
            trace[0],
            TraceEntry {
                pc: Relocatable {
                    segment_index: 0,
                    offset: 3
                },
                ap: Relocatable {
                    segment_index: 1,
                    offset: 2
                },
                fp: Relocatable {
                    segment_index: 1,
                    offset: 2
                },
            }
        );
        assert_eq!(
            trace[1],
            TraceEntry {
                pc: Relocatable {
                    segment_index: 0,
                    offset: 5
                },
                ap: Relocatable {
                    segment_index: 1,
                    offset: 3
                },
                fp: Relocatable {
                    segment_index: 1,
                    offset: 2
                },
            }
        );
        assert_eq!(
            trace[2],
            TraceEntry {
                pc: Relocatable {
                    segment_index: 0,
                    offset: 0
                },
                ap: Relocatable {
                    segment_index: 1,
                    offset: 5
                },
                fp: Relocatable {
                    segment_index: 1,
                    offset: 5
                },
            }
        );
        assert_eq!(
            trace[3],
            TraceEntry {
                pc: Relocatable {
                    segment_index: 0,
                    offset: 2
                },
                ap: Relocatable {
                    segment_index: 1,
                    offset: 6
                },
                fp: Relocatable {
                    segment_index: 1,
                    offset: 5
                },
            }
        );
        assert_eq!(
            trace[4],
            TraceEntry {
                pc: Relocatable {
                    segment_index: 0,
                    offset: 7
                },
                ap: Relocatable {
                    segment_index: 1,
                    offset: 6
                },
                fp: Relocatable {
                    segment_index: 1,
                    offset: 2
                },
            }
        );
    }

    #[test]
    /*Program used:
    %builtins range_check

    func check_range{range_check_ptr}(num):

        # Check that 0 <= num < 2**64.
        [range_check_ptr] = num
        assert [range_check_ptr + 1] = 2 ** 64 - 1 - num
        let range_check_ptr = range_check_ptr + 2
        return()
    end

    func main{range_check_ptr}():
        check_range(7)
        return()
    end

    main = 8
    data = [4612671182993129469, 5189976364521848832, 18446744073709551615, 5199546496550207487, 4612389712311386111, 5198983563776393216, 2, 2345108766317314046, 5191102247248822272, 5189976364521848832, 7, 1226245742482522112, 3618502788666131213697322783095070105623107215331596699973092056135872020470, 2345108766317314046]
    */
    fn initialize_and_run_range_check_builtin() {
        //Initialization Phase
        let program = Program {
            builtins: vec![String::from("range_check")],
            prime: BigInt::new(Sign::Plus, vec![1, 0, 0, 0, 0, 0, 17, 134217728]),
            data: vec![
                MaybeRelocatable::from(BigInt::from_i64(4612671182993129469).unwrap()),
                MaybeRelocatable::from(BigInt::from_i64(5189976364521848832).unwrap()),
                MaybeRelocatable::Int(BigInt::from_i128(18446744073709551615).unwrap()),
                MaybeRelocatable::from(BigInt::from_i64(5199546496550207487).unwrap()),
                MaybeRelocatable::from(BigInt::from_i64(4612389712311386111).unwrap()),
                MaybeRelocatable::from(BigInt::from_i64(5198983563776393216).unwrap()),
                MaybeRelocatable::from(bigint!(2)),
                MaybeRelocatable::from(BigInt::from_i64(2345108766317314046).unwrap()),
                MaybeRelocatable::from(BigInt::from_i64(5191102247248822272).unwrap()),
                MaybeRelocatable::from(BigInt::from_i64(5189976364521848832).unwrap()),
                MaybeRelocatable::from(bigint!(7)),
                MaybeRelocatable::from(BigInt::from_i64(1226245742482522112).unwrap()),
                MaybeRelocatable::Int(BigInt::new(
                    Sign::Plus,
                    vec![
                        4294967286, 4294967295, 4294967295, 4294967295, 4294967295, 4294967295, 16,
                        134217728,
                    ],
                )),
                MaybeRelocatable::from(BigInt::from_i64(2345108766317314046).unwrap()),
            ],
            main: Some(8),
            hints: HashMap::new(),
            reference_manager: ReferenceManager {
                references: Vec::new(),
            },
        };
        let hint_processor = BuiltinHintProcessor::new_empty();
        let mut cairo_runner = CairoRunner::new(&program, true, &hint_processor).unwrap();
        cairo_runner.initialize_segments(None);
        let end = cairo_runner.initialize_main_entrypoint().unwrap();
        cairo_runner.initialize_vm().unwrap();
        //Execution Phase
        assert_eq!(cairo_runner.run_until_pc(end), Ok(()));
        //Check final values against Python VM
        //Check final register values
        assert_eq!(cairo_runner.vm.run_context.pc, Relocatable::from((4, 0)));

        assert_eq!(cairo_runner.vm.run_context.ap, 10);

        assert_eq!(cairo_runner.vm.run_context.fp, 0);

        //Check each TraceEntry in trace
        let trace = cairo_runner.vm.trace.unwrap();
        assert_eq!(trace.len(), 10);
        assert_eq!(
            trace[0],
            TraceEntry {
                pc: Relocatable {
                    segment_index: 0,
                    offset: 8
                },
                ap: Relocatable {
                    segment_index: 1,
                    offset: 3
                },
                fp: Relocatable {
                    segment_index: 1,
                    offset: 3
                },
            }
        );
        assert_eq!(
            trace[1],
            TraceEntry {
                pc: Relocatable {
                    segment_index: 0,
                    offset: 9
                },
                ap: Relocatable {
                    segment_index: 1,
                    offset: 4
                },
                fp: Relocatable {
                    segment_index: 1,
                    offset: 3
                },
            }
        );
        assert_eq!(
            trace[2],
            TraceEntry {
                pc: Relocatable {
                    segment_index: 0,
                    offset: 11
                },
                ap: Relocatable {
                    segment_index: 1,
                    offset: 5
                },
                fp: Relocatable {
                    segment_index: 1,
                    offset: 3
                },
            }
        );
        assert_eq!(
            trace[3],
            TraceEntry {
                pc: Relocatable {
                    segment_index: 0,
                    offset: 0
                },
                ap: Relocatable {
                    segment_index: 1,
                    offset: 7
                },
                fp: Relocatable {
                    segment_index: 1,
                    offset: 7
                },
            }
        );
        assert_eq!(
            trace[4],
            TraceEntry {
                pc: Relocatable {
                    segment_index: 0,
                    offset: 1
                },
                ap: Relocatable {
                    segment_index: 1,
                    offset: 7
                },
                fp: Relocatable {
                    segment_index: 1,
                    offset: 7
                },
            }
        );
        assert_eq!(
            trace[5],
            TraceEntry {
                pc: Relocatable {
                    segment_index: 0,
                    offset: 3
                },
                ap: Relocatable {
                    segment_index: 1,
                    offset: 8
                },
                fp: Relocatable {
                    segment_index: 1,
                    offset: 7
                },
            }
        );
        assert_eq!(
            trace[6],
            TraceEntry {
                pc: Relocatable {
                    segment_index: 0,
                    offset: 4
                },
                ap: Relocatable {
                    segment_index: 1,
                    offset: 9
                },
                fp: Relocatable {
                    segment_index: 1,
                    offset: 7
                },
            }
        );
        assert_eq!(
            trace[7],
            TraceEntry {
                pc: Relocatable {
                    segment_index: 0,
                    offset: 5
                },
                ap: Relocatable {
                    segment_index: 1,
                    offset: 9
                },
                fp: Relocatable {
                    segment_index: 1,
                    offset: 7
                },
            }
        );
        assert_eq!(
            trace[8],
            TraceEntry {
                pc: Relocatable {
                    segment_index: 0,
                    offset: 7
                },
                ap: Relocatable {
                    segment_index: 1,
                    offset: 10
                },
                fp: Relocatable {
                    segment_index: 1,
                    offset: 7
                },
            }
        );
        assert_eq!(
            trace[9],
            TraceEntry {
                pc: Relocatable {
                    segment_index: 0,
                    offset: 13
                },
                ap: Relocatable {
                    segment_index: 1,
                    offset: 10
                },
                fp: Relocatable {
                    segment_index: 1,
                    offset: 3
                },
            }
        );
        //Check the range_check builtin segment
        assert_eq!(
            cairo_runner.vm.builtin_runners[0].0,
            String::from("range_check")
        );
        assert_eq!(
            cairo_runner.vm.builtin_runners[0].1.base(),
            relocatable!(2, 0)
        );
        assert_eq!(
            cairo_runner
                .vm
                .memory
                .get(&MaybeRelocatable::from((2, 0)))
                .unwrap(),
            Some(&MaybeRelocatable::from(bigint!(7)))
        );
        assert_eq!(
            cairo_runner
                .vm
                .memory
                .get(&MaybeRelocatable::from((2, 1)))
                .unwrap(),
            Some(&MaybeRelocatable::from(bigint!(2).pow(64) - bigint!(8)))
        );
        assert_eq!(
            cairo_runner
                .vm
                .memory
                .get(&MaybeRelocatable::from((2, 2)))
                .unwrap(),
            None
        );
    }

    #[test]
    /*Program used:
    %builtins output

    from starkware.cairo.common.serialize import serialize_word

    func main{output_ptr: felt*}():
        let a = 1
        serialize_word(a)
        let b = 17 * a
        serialize_word(b)
        return()
    end

    main = 4
    data = [
    4612671182993129469,
    5198983563776393216,
    1,
    2345108766317314046,
    5191102247248822272,
    5189976364521848832,
    1,
    1226245742482522112,
    3618502788666131213697322783095070105623107215331596699973092056135872020474,
    5189976364521848832,
    17,
    1226245742482522112,
    3618502788666131213697322783095070105623107215331596699973092056135872020470,
    2345108766317314046
    ]
    */
    fn initialize_and_run_output_builtin() {
        //Initialization Phase
        let program = Program {
            builtins: vec![String::from("output")],
            prime: BigInt::new(Sign::Plus, vec![1, 0, 0, 0, 0, 0, 17, 134217728]),
            data: vec![
                MaybeRelocatable::from(BigInt::from_i64(4612671182993129469).unwrap()),
                MaybeRelocatable::from(BigInt::from_i64(5198983563776393216).unwrap()),
                MaybeRelocatable::from(bigint!(1)),
                MaybeRelocatable::from(BigInt::from_i64(2345108766317314046).unwrap()),
                MaybeRelocatable::from(BigInt::from_i64(5191102247248822272).unwrap()),
                MaybeRelocatable::from(BigInt::from_i64(5189976364521848832).unwrap()),
                MaybeRelocatable::from(bigint!(1)),
                MaybeRelocatable::from(BigInt::from_i64(1226245742482522112).unwrap()),
                MaybeRelocatable::from(bigint_str!(
                    b"3618502788666131213697322783095070105623107215331596699973092056135872020474"
                )),
                MaybeRelocatable::from(BigInt::from_i64(5189976364521848832).unwrap()),
                MaybeRelocatable::from(bigint!(17)),
                MaybeRelocatable::from(BigInt::from_i64(1226245742482522112).unwrap()),
                MaybeRelocatable::from(bigint_str!(
                    b"3618502788666131213697322783095070105623107215331596699973092056135872020470"
                )),
                MaybeRelocatable::from(BigInt::from_i64(2345108766317314046).unwrap()),
            ],
            main: Some(4),
            hints: HashMap::new(),
            reference_manager: ReferenceManager {
                references: Vec::new(),
            },
        };
        let hint_processor = BuiltinHintProcessor::new_empty();
        let mut cairo_runner = CairoRunner::new(&program, true, &hint_processor).unwrap();
        cairo_runner.initialize_segments(None);
        let end = cairo_runner.initialize_main_entrypoint().unwrap();
        cairo_runner.initialize_vm().unwrap();
        //Execution Phase
        assert_eq!(cairo_runner.run_until_pc(end), Ok(()));
        //Check final values against Python VM
        //Check final register values
        //todo
        assert_eq!(cairo_runner.vm.run_context.pc, Relocatable::from((4, 0)));

        assert_eq!(cairo_runner.vm.run_context.ap, 12);

        assert_eq!(cairo_runner.vm.run_context.fp, 0);

        //Check each TraceEntry in trace
        let trace = cairo_runner.vm.trace.unwrap();
        assert_eq!(trace.len(), 12);
        assert_eq!(
            trace[0],
            TraceEntry {
                pc: Relocatable {
                    segment_index: 0,
                    offset: 4
                },
                ap: Relocatable {
                    segment_index: 1,
                    offset: 3
                },
                fp: Relocatable {
                    segment_index: 1,
                    offset: 3
                },
            }
        );
        assert_eq!(
            trace[1],
            TraceEntry {
                pc: Relocatable {
                    segment_index: 0,
                    offset: 5
                },
                ap: Relocatable {
                    segment_index: 1,
                    offset: 4
                },
                fp: Relocatable {
                    segment_index: 1,
                    offset: 3
                },
            }
        );
        assert_eq!(
            trace[2],
            TraceEntry {
                pc: Relocatable {
                    segment_index: 0,
                    offset: 7
                },
                ap: Relocatable {
                    segment_index: 1,
                    offset: 5
                },
                fp: Relocatable {
                    segment_index: 1,
                    offset: 3
                },
            }
        );
        assert_eq!(
            trace[3],
            TraceEntry {
                pc: Relocatable {
                    segment_index: 0,
                    offset: 0
                },
                ap: Relocatable {
                    segment_index: 1,
                    offset: 7
                },
                fp: Relocatable {
                    segment_index: 1,
                    offset: 7
                },
            }
        );
        assert_eq!(
            trace[4],
            TraceEntry {
                pc: Relocatable {
                    segment_index: 0,
                    offset: 1
                },
                ap: Relocatable {
                    segment_index: 1,
                    offset: 7
                },
                fp: Relocatable {
                    segment_index: 1,
                    offset: 7
                },
            }
        );
        assert_eq!(
            trace[5],
            TraceEntry {
                pc: Relocatable {
                    segment_index: 0,
                    offset: 3
                },
                ap: Relocatable {
                    segment_index: 1,
                    offset: 8
                },
                fp: Relocatable {
                    segment_index: 1,
                    offset: 7
                },
            }
        );
        assert_eq!(
            trace[6],
            TraceEntry {
                pc: Relocatable {
                    segment_index: 0,
                    offset: 9
                },
                ap: Relocatable {
                    segment_index: 1,
                    offset: 8
                },
                fp: Relocatable {
                    segment_index: 1,
                    offset: 3
                },
            }
        );
        assert_eq!(
            trace[7],
            TraceEntry {
                pc: Relocatable {
                    segment_index: 0,
                    offset: 11
                },
                ap: Relocatable {
                    segment_index: 1,
                    offset: 9
                },
                fp: Relocatable {
                    segment_index: 1,
                    offset: 3
                },
            }
        );
        assert_eq!(
            trace[8],
            TraceEntry {
                pc: Relocatable {
                    segment_index: 0,
                    offset: 0
                },
                ap: Relocatable {
                    segment_index: 1,
                    offset: 11
                },
                fp: Relocatable {
                    segment_index: 1,
                    offset: 11
                },
            }
        );
        assert_eq!(
            trace[9],
            TraceEntry {
                pc: Relocatable {
                    segment_index: 0,
                    offset: 1
                },
                ap: Relocatable {
                    segment_index: 1,
                    offset: 11
                },
                fp: Relocatable {
                    segment_index: 1,
                    offset: 11
                },
            }
        );
        assert_eq!(
            trace[10],
            TraceEntry {
                pc: Relocatable {
                    segment_index: 0,
                    offset: 3
                },
                ap: Relocatable {
                    segment_index: 1,
                    offset: 12
                },
                fp: Relocatable {
                    segment_index: 1,
                    offset: 11
                },
            }
        );
        assert_eq!(
            trace[11],
            TraceEntry {
                pc: Relocatable {
                    segment_index: 0,
                    offset: 13
                },
                ap: Relocatable {
                    segment_index: 1,
                    offset: 12
                },
                fp: Relocatable {
                    segment_index: 1,
                    offset: 3
                },
            }
        );
        //Check that the output to be printed is correct
        assert_eq!(cairo_runner.vm.builtin_runners[0].0, String::from("output"));
        assert_eq!(
            cairo_runner.vm.builtin_runners[0].1.base(),
            relocatable!(2, 0)
        );
        assert_eq!(
            cairo_runner
                .vm
                .memory
                .get(&MaybeRelocatable::from((2, 0)))
                .unwrap(),
            Some(&MaybeRelocatable::from(bigint!(1)))
        );
        assert_eq!(
            cairo_runner
                .vm
                .memory
                .get(&MaybeRelocatable::from((2, 1)))
                .unwrap(),
            Some(&MaybeRelocatable::from(bigint!(17)))
        );
        assert_eq!(
            cairo_runner
                .vm
                .memory
                .get(&MaybeRelocatable::from((2, 2)))
                .unwrap(),
            None
        );
    }

    #[test]
    /*Program used:
    %builtins output range_check

    from starkware.cairo.common.serialize import serialize_word

    func check_range{range_check_ptr}(num) -> (num : felt):

        # Check that 0 <= num < 2**64.
        [range_check_ptr] = num
        assert [range_check_ptr + 1] = 2 ** 64 - 1 - num
        let range_check_ptr = range_check_ptr + 2
        return(num)
    end

    func main{output_ptr: felt*, range_check_ptr: felt}():
        let num: felt = check_range(7)
        serialize_word(num)
        return()
    end

    main = 13
    data = [
    4612671182993129469,
    5198983563776393216,
    1,
    2345108766317314046,
    4612671182993129469,
    5189976364521848832,
    18446744073709551615,
    5199546496550207487,
    4612389712311386111,
    5198983563776393216,
    2,
    5191102247248822272,
    2345108766317314046,
    5191102247248822272,
    5189976364521848832,
    7,
    1226245742482522112,
    3618502788666131213697322783095070105623107215331596699973092056135872020469,
    5191102242953854976,
    5193354051357474816,
    1226245742482522112,
    3618502788666131213697322783095070105623107215331596699973092056135872020461,
    5193354029882638336,
    2345108766317314046]
    */
    fn initialize_and_run_output_range_check_builtin() {
        //Initialization Phase
        let program = Program {
            builtins: vec![String::from("output"), String::from("range_check")],
            prime: BigInt::new(Sign::Plus, vec![1, 0, 0, 0, 0, 0, 17, 134217728]),
            data: vec![
                MaybeRelocatable::from(BigInt::from_i64(4612671182993129469).unwrap()),
                MaybeRelocatable::from(BigInt::from_i64(5198983563776393216).unwrap()),
                MaybeRelocatable::from(bigint!(1)),
                MaybeRelocatable::from(BigInt::from_i64(2345108766317314046).unwrap()),
                MaybeRelocatable::from(BigInt::from_i64(4612671182993129469).unwrap()),
                MaybeRelocatable::from(BigInt::from_i64(5189976364521848832).unwrap()),
                MaybeRelocatable::Int(BigInt::from_i128(18446744073709551615).unwrap()),
                MaybeRelocatable::from(BigInt::from_i64(5199546496550207487).unwrap()),
                MaybeRelocatable::from(BigInt::from_i64(4612389712311386111).unwrap()),
                MaybeRelocatable::from(BigInt::from_i64(5198983563776393216).unwrap()),
                MaybeRelocatable::from(bigint!(2)),
                MaybeRelocatable::from(BigInt::from_i64(5191102247248822272).unwrap()),
                MaybeRelocatable::from(BigInt::from_i64(2345108766317314046).unwrap()),
                MaybeRelocatable::from(BigInt::from_i64(5191102247248822272).unwrap()),
                MaybeRelocatable::from(BigInt::from_i64(5189976364521848832).unwrap()),
                MaybeRelocatable::from(bigint!(7)),
                MaybeRelocatable::from(BigInt::from_i64(1226245742482522112).unwrap()),
                MaybeRelocatable::from(bigint_str!(
                    b"3618502788666131213697322783095070105623107215331596699973092056135872020469"
                )),
                MaybeRelocatable::from(BigInt::from_i64(5191102242953854976).unwrap()),
                MaybeRelocatable::from(BigInt::from_i64(5193354051357474816).unwrap()),
                MaybeRelocatable::from(BigInt::from_i64(1226245742482522112).unwrap()),
                MaybeRelocatable::from(bigint_str!(
                    b"3618502788666131213697322783095070105623107215331596699973092056135872020461"
                )),
                MaybeRelocatable::from(BigInt::from_i64(5193354029882638336).unwrap()),
                MaybeRelocatable::from(BigInt::from_i64(2345108766317314046).unwrap()),
            ],
            main: Some(13),
            hints: HashMap::new(),
            reference_manager: ReferenceManager {
                references: Vec::new(),
            },
        };
        let hint_processor = BuiltinHintProcessor::new_empty();
        let mut cairo_runner = CairoRunner::new(&program, true, &hint_processor).unwrap();
        cairo_runner.initialize_segments(None);
        let end = cairo_runner.initialize_main_entrypoint().unwrap();
        cairo_runner.initialize_vm().unwrap();
        //Execution Phase
        assert_eq!(cairo_runner.run_until_pc(end), Ok(()));
        //Check final values against Python VM
        //Check final register values
        assert_eq!(cairo_runner.vm.run_context.pc, Relocatable::from((5, 0)));

        assert_eq!(cairo_runner.vm.run_context.ap, 18);

        assert_eq!(cairo_runner.vm.run_context.fp, 0);

        //Check each TraceEntry in trace
        let trace = cairo_runner.vm.trace.unwrap();
        assert_eq!(trace.len(), 18);
        assert_eq!(
            trace[0],
            TraceEntry {
                pc: Relocatable {
                    segment_index: 0,
                    offset: 13
                },
                ap: Relocatable {
                    segment_index: 1,
                    offset: 4
                },
                fp: Relocatable {
                    segment_index: 1,
                    offset: 4
                },
            }
        );
        assert_eq!(
            trace[1],
            TraceEntry {
                pc: Relocatable {
                    segment_index: 0,
                    offset: 14
                },
                ap: Relocatable {
                    segment_index: 1,
                    offset: 5
                },
                fp: Relocatable {
                    segment_index: 1,
                    offset: 4
                },
            }
        );
        assert_eq!(
            trace[2],
            TraceEntry {
                pc: Relocatable {
                    segment_index: 0,
                    offset: 16
                },
                ap: Relocatable {
                    segment_index: 1,
                    offset: 6
                },
                fp: Relocatable {
                    segment_index: 1,
                    offset: 4
                },
            }
        );
        assert_eq!(
            trace[3],
            TraceEntry {
                pc: Relocatable {
                    segment_index: 0,
                    offset: 4
                },
                ap: Relocatable {
                    segment_index: 1,
                    offset: 8
                },
                fp: Relocatable {
                    segment_index: 1,
                    offset: 8
                },
            }
        );
        assert_eq!(
            trace[4],
            TraceEntry {
                pc: Relocatable {
                    segment_index: 0,
                    offset: 5
                },
                ap: Relocatable {
                    segment_index: 1,
                    offset: 8
                },
                fp: Relocatable {
                    segment_index: 1,
                    offset: 8
                },
            }
        );
        assert_eq!(
            trace[5],
            TraceEntry {
                pc: Relocatable {
                    segment_index: 0,
                    offset: 7
                },
                ap: Relocatable {
                    segment_index: 1,
                    offset: 9
                },
                fp: Relocatable {
                    segment_index: 1,
                    offset: 8
                },
            }
        );
        assert_eq!(
            trace[6],
            TraceEntry {
                pc: Relocatable {
                    segment_index: 0,
                    offset: 8
                },
                ap: Relocatable {
                    segment_index: 1,
                    offset: 10
                },
                fp: Relocatable {
                    segment_index: 1,
                    offset: 8
                },
            }
        );
        assert_eq!(
            trace[7],
            TraceEntry {
                pc: Relocatable {
                    segment_index: 0,
                    offset: 9
                },
                ap: Relocatable {
                    segment_index: 1,
                    offset: 10
                },
                fp: Relocatable {
                    segment_index: 1,
                    offset: 8
                },
            }
        );
        assert_eq!(
            trace[8],
            TraceEntry {
                pc: Relocatable {
                    segment_index: 0,
                    offset: 11
                },
                ap: Relocatable {
                    segment_index: 1,
                    offset: 11
                },
                fp: Relocatable {
                    segment_index: 1,
                    offset: 8
                },
            }
        );
        assert_eq!(
            trace[9],
            TraceEntry {
                pc: Relocatable {
                    segment_index: 0,
                    offset: 12
                },
                ap: Relocatable {
                    segment_index: 1,
                    offset: 12
                },
                fp: Relocatable {
                    segment_index: 1,
                    offset: 8
                },
            }
        );
        assert_eq!(
            trace[10],
            TraceEntry {
                pc: Relocatable {
                    segment_index: 0,
                    offset: 18
                },
                ap: Relocatable {
                    segment_index: 1,
                    offset: 12
                },
                fp: Relocatable {
                    segment_index: 1,
                    offset: 4
                },
            }
        );
        assert_eq!(
            trace[11],
            TraceEntry {
                pc: Relocatable {
                    segment_index: 0,
                    offset: 19
                },
                ap: Relocatable {
                    segment_index: 1,
                    offset: 13
                },
                fp: Relocatable {
                    segment_index: 1,
                    offset: 4
                },
            }
        );
        assert_eq!(
            trace[12],
            TraceEntry {
                pc: Relocatable {
                    segment_index: 0,
                    offset: 20
                },
                ap: Relocatable {
                    segment_index: 1,
                    offset: 14
                },
                fp: Relocatable {
                    segment_index: 1,
                    offset: 4
                },
            }
        );
        assert_eq!(
            trace[13],
            TraceEntry {
                pc: Relocatable {
                    segment_index: 0,
                    offset: 0
                },
                ap: Relocatable {
                    segment_index: 1,
                    offset: 16
                },
                fp: Relocatable {
                    segment_index: 1,
                    offset: 16
                },
            }
        );
        assert_eq!(
            trace[14],
            TraceEntry {
                pc: Relocatable {
                    segment_index: 0,
                    offset: 1
                },
                ap: Relocatable {
                    segment_index: 1,
                    offset: 16
                },
                fp: Relocatable {
                    segment_index: 1,
                    offset: 16
                },
            }
        );
        assert_eq!(
            trace[15],
            TraceEntry {
                pc: Relocatable {
                    segment_index: 0,
                    offset: 3
                },
                ap: Relocatable {
                    segment_index: 1,
                    offset: 17
                },
                fp: Relocatable {
                    segment_index: 1,
                    offset: 16
                },
            }
        );
        assert_eq!(
            trace[16],
            TraceEntry {
                pc: Relocatable {
                    segment_index: 0,
                    offset: 22
                },
                ap: Relocatable {
                    segment_index: 1,
                    offset: 17
                },
                fp: Relocatable {
                    segment_index: 1,
                    offset: 4
                },
            }
        );
        assert_eq!(
            trace[17],
            TraceEntry {
                pc: Relocatable {
                    segment_index: 0,
                    offset: 23
                },
                ap: Relocatable {
                    segment_index: 1,
                    offset: 18
                },
                fp: Relocatable {
                    segment_index: 1,
                    offset: 4
                },
            }
        );
        //Check the range_check builtin segment
        assert_eq!(
            cairo_runner.vm.builtin_runners[1].0,
            String::from("range_check")
        );
        assert_eq!(
            cairo_runner.vm.builtin_runners[1].1.base(),
            relocatable!(3, 0)
        );
        assert_eq!(
            cairo_runner
                .vm
                .memory
                .get(&MaybeRelocatable::from((3, 0)))
                .unwrap(),
            Some(&MaybeRelocatable::from(bigint!(7)))
        );
        assert_eq!(
            cairo_runner
                .vm
                .memory
                .get(&MaybeRelocatable::from((3, 1)))
                .unwrap(),
            Some(&MaybeRelocatable::from(bigint!(2).pow(64) - bigint!(8)))
        );
        assert_eq!(
            cairo_runner
                .vm
                .memory
                .get(&MaybeRelocatable::from((2, 2)))
                .unwrap(),
            None
        );

        //Check the output segment
        assert_eq!(cairo_runner.vm.builtin_runners[0].0, String::from("output"));
        assert_eq!(
            cairo_runner.vm.builtin_runners[0].1.base(),
            relocatable!(2, 0)
        );

        assert_eq!(
            cairo_runner
                .vm
                .memory
                .get(&(MaybeRelocatable::from((2, 0))))
                .unwrap(),
            Some(&MaybeRelocatable::from(bigint!(7)))
        );
        assert_eq!(
            cairo_runner
                .vm
                .memory
                .get(&(MaybeRelocatable::from((2, 1))))
                .unwrap(),
            None
        );
    }

    #[test]
    /*Memory from this test is taken from a cairo program execution
    Program used:
        func main():
        let a = 1
        [ap + 3] = 5
        return()

    end
    Final Memory:
    {RelocatableValue(segment_index=0, offset=0): 4613515612218425347,
     RelocatableValue(segment_index=0, offset=1): 5,
     RelocatableValue(segment_index=0, offset=2): 2345108766317314046,
     RelocatableValue(segment_index=1, offset=0): RelocatableValue(segment_index=2, offset=0),
     RelocatableValue(segment_index=1, offset=1): RelocatableValue(segment_index=3, offset=0),
     RelocatableValue(segment_index=1, offset=5): 5}
    Relocated Memory:
        1     4613515612218425347
        2     5
        3     2345108766317314046
        4     10
        5     10
        ⋮
        9     5
    */
    fn relocate_memory_with_gap() {
        let program = Program {
            builtins: Vec::new(),
            prime: bigint!(17),
            data: Vec::new(),
            main: None,
            hints: HashMap::new(),
            reference_manager: ReferenceManager {
                references: Vec::new(),
            },
        };
        let hint_processor = BuiltinHintProcessor::new_empty();
        let mut cairo_runner = CairoRunner::new(&program, true, &hint_processor).unwrap();
        for _ in 0..4 {
            cairo_runner.vm.segments.add(&mut cairo_runner.vm.memory);
        }
        cairo_runner
            .vm
            .memory
            .insert(
                &MaybeRelocatable::from((0, 0)),
                &MaybeRelocatable::from(bigint!(4613515612218425347_i64)),
            )
            .unwrap();
        cairo_runner
            .vm
            .memory
            .insert(
                &MaybeRelocatable::from((0, 1)),
                &MaybeRelocatable::from(bigint!(5)),
            )
            .unwrap();
        cairo_runner
            .vm
            .memory
            .insert(
                &MaybeRelocatable::from((0, 2)),
                &MaybeRelocatable::from(bigint!(2345108766317314046_i64)),
            )
            .unwrap();
        cairo_runner
            .vm
            .memory
            .insert(
                &MaybeRelocatable::from((1, 0)),
                &MaybeRelocatable::from((2, 0)),
            )
            .unwrap();
        cairo_runner
            .vm
            .memory
            .insert(
                &MaybeRelocatable::from((1, 1)),
                &MaybeRelocatable::from((3, 0)),
            )
            .unwrap();
        cairo_runner
            .vm
            .memory
            .insert(
                &MaybeRelocatable::from((1, 5)),
                &MaybeRelocatable::from(bigint!(5)),
            )
            .unwrap();
        cairo_runner
            .vm
            .segments
            .compute_effective_sizes(&cairo_runner.vm.memory);
        let rel_table = cairo_runner
            .vm
            .segments
            .relocate_segments()
            .expect("Couldn't relocate after compute effective sizes");
        assert_eq!(cairo_runner.relocate_memory(&rel_table), Ok(()));
        assert_eq!(cairo_runner.relocated_memory[0], None);
        assert_eq!(
            cairo_runner.relocated_memory[1],
            Some(bigint!(4613515612218425347_i64))
        );
        assert_eq!(cairo_runner.relocated_memory[2], Some(bigint!(5)));
        assert_eq!(
            cairo_runner.relocated_memory[3],
            Some(bigint!(2345108766317314046_i64))
        );
        assert_eq!(cairo_runner.relocated_memory[4], Some(bigint!(10)));
        assert_eq!(cairo_runner.relocated_memory[5], Some(bigint!(10)));
        assert_eq!(cairo_runner.relocated_memory[6], None);
        assert_eq!(cairo_runner.relocated_memory[7], None);
        assert_eq!(cairo_runner.relocated_memory[8], None);
        assert_eq!(cairo_runner.relocated_memory[9], Some(bigint!(5)));
    }

    #[test]
    /* Program used:
    %builtins output

    from starkware.cairo.common.serialize import serialize_word

    func main{output_ptr: felt*}():
        let a = 1
        serialize_word(a)
        let b = 17 * a
        serialize_word(b)
        return()
    end
    Relocated Memory:
        1     4612671182993129469
        2     5198983563776393216
        3     1
        4     2345108766317314046
        5     5191102247248822272
        6     5189976364521848832
        7     1
        8     1226245742482522112
        9     -7
        10    5189976364521848832
        11    17
        12    1226245742482522112
        13    -11
        14    2345108766317314046
        15    27
        16    29
        17    29
        18    27
        19    1
        20    18
        21    10
        22    28
        23    17
        24    18
        25    14
        26    29
        27    1
        28    17
     */
    fn initialize_run_and_relocate_output_builtin() {
        let program = Program {
            builtins: vec![String::from("output")],
            prime: BigInt::new(Sign::Plus, vec![1, 0, 0, 0, 0, 0, 17, 134217728]),
            data: vec![
                MaybeRelocatable::from(bigint!(4612671182993129469_i64)),
                MaybeRelocatable::from(bigint!(5198983563776393216_i64)),
                MaybeRelocatable::from(bigint!(1)),
                MaybeRelocatable::from(bigint!(2345108766317314046_i64)),
                MaybeRelocatable::from(bigint!(5191102247248822272_i64)),
                MaybeRelocatable::from(bigint!(5189976364521848832_i64)),
                MaybeRelocatable::from(bigint!(1)),
                MaybeRelocatable::from(bigint!(1226245742482522112_i64)),
                MaybeRelocatable::from(bigint_str!(
                    b"3618502788666131213697322783095070105623107215331596699973092056135872020474"
                )),
                MaybeRelocatable::from(bigint!(5189976364521848832_i64)),
                MaybeRelocatable::from(bigint!(17)),
                MaybeRelocatable::from(bigint!(1226245742482522112_i64)),
                MaybeRelocatable::from(bigint_str!(
                    b"3618502788666131213697322783095070105623107215331596699973092056135872020470"
                )),
                MaybeRelocatable::from(bigint!(2345108766317314046_i64)),
            ],
            main: Some(4),
            hints: HashMap::new(),
            reference_manager: ReferenceManager {
                references: Vec::new(),
            },
        };
        let hint_processor = BuiltinHintProcessor::new_empty();
        let mut cairo_runner = CairoRunner::new(&program, false, &hint_processor).unwrap();
        cairo_runner.initialize_segments(None);
        let end = cairo_runner.initialize_main_entrypoint().unwrap();
        cairo_runner.initialize_vm().unwrap();
        assert_eq!(cairo_runner.run_until_pc(end), Ok(()));
        cairo_runner
            .vm
            .segments
            .compute_effective_sizes(&cairo_runner.vm.memory);
        let rel_table = cairo_runner
            .vm
            .segments
            .relocate_segments()
            .expect("Couldn't relocate after compute effective sizes");
        assert_eq!(cairo_runner.relocate_memory(&rel_table), Ok(()));
        assert_eq!(cairo_runner.relocated_memory[0], None);
        assert_eq!(
            cairo_runner.relocated_memory[1],
            Some(bigint!(4612671182993129469_i64))
        );
        assert_eq!(
            cairo_runner.relocated_memory[2],
            Some(bigint!(5198983563776393216_i64))
        );
        assert_eq!(cairo_runner.relocated_memory[3], Some(bigint!(1)));
        assert_eq!(
            cairo_runner.relocated_memory[4],
            Some(bigint!(2345108766317314046_i64))
        );
        assert_eq!(
            cairo_runner.relocated_memory[5],
            Some(bigint!(5191102247248822272_i64))
        );
        assert_eq!(
            cairo_runner.relocated_memory[6],
            Some(bigint!(5189976364521848832_i64))
        );
        assert_eq!(cairo_runner.relocated_memory[7], Some(bigint!(1)));
        assert_eq!(
            cairo_runner.relocated_memory[8],
            Some(bigint!(1226245742482522112_i64))
        );
        assert_eq!(
            cairo_runner.relocated_memory[9],
            Some(bigint_str!(
                b"3618502788666131213697322783095070105623107215331596699973092056135872020474"
            ))
        );
        assert_eq!(
            cairo_runner.relocated_memory[10],
            Some(bigint!(5189976364521848832_i64))
        );
        assert_eq!(cairo_runner.relocated_memory[11], Some(bigint!(17)));
        assert_eq!(
            cairo_runner.relocated_memory[12],
            Some(bigint!(1226245742482522112_i64))
        );
        assert_eq!(
            cairo_runner.relocated_memory[13],
            Some(bigint_str!(
                b"3618502788666131213697322783095070105623107215331596699973092056135872020470"
            ))
        );
        assert_eq!(
            cairo_runner.relocated_memory[14],
            Some(bigint!(2345108766317314046_i64))
        );
        assert_eq!(cairo_runner.relocated_memory[15], Some(bigint!(27)));
        assert_eq!(cairo_runner.relocated_memory[16], Some(bigint!(29)));
        assert_eq!(cairo_runner.relocated_memory[17], Some(bigint!(29)));
        assert_eq!(cairo_runner.relocated_memory[18], Some(bigint!(27)));
        assert_eq!(cairo_runner.relocated_memory[19], Some(bigint!(1)));
        assert_eq!(cairo_runner.relocated_memory[20], Some(bigint!(18)));
        assert_eq!(cairo_runner.relocated_memory[21], Some(bigint!(10)));
        assert_eq!(cairo_runner.relocated_memory[22], Some(bigint!(28)));
        assert_eq!(cairo_runner.relocated_memory[23], Some(bigint!(17)));
        assert_eq!(cairo_runner.relocated_memory[24], Some(bigint!(18)));
        assert_eq!(cairo_runner.relocated_memory[25], Some(bigint!(14)));
        assert_eq!(cairo_runner.relocated_memory[26], Some(bigint!(29)));
        assert_eq!(cairo_runner.relocated_memory[27], Some(bigint!(1)));
        assert_eq!(cairo_runner.relocated_memory[28], Some(bigint!(17)));
    }

    #[test]
    /* Program used:
    %builtins output

    from starkware.cairo.common.serialize import serialize_word

    func main{output_ptr: felt*}():
        let a = 1
        serialize_word(a)
        let b = 17 * a
        serialize_word(b)
        return()
    end

    Relocated Trace:
    [TraceEntry(pc=5, ap=18, fp=18),
     TraceEntry(pc=6, ap=19, fp=18),
     TraceEntry(pc=8, ap=20, fp=18),
     TraceEntry(pc=1, ap=22, fp=22),
     TraceEntry(pc=2, ap=22, fp=22),
     TraceEntry(pc=4, ap=23, fp=22),
     TraceEntry(pc=10, ap=23, fp=18),
    */
    fn relocate_trace_output_builtin() {
        let program = Program {
            builtins: vec![String::from("output")],
            prime: BigInt::new(Sign::Plus, vec![1, 0, 0, 0, 0, 0, 17, 134217728]),
            data: vec![
                MaybeRelocatable::from(bigint!(4612671182993129469_i64)),
                MaybeRelocatable::from(bigint!(5198983563776393216_i64)),
                MaybeRelocatable::from(bigint!(1)),
                MaybeRelocatable::from(bigint!(2345108766317314046_i64)),
                MaybeRelocatable::from(bigint!(5191102247248822272_i64)),
                MaybeRelocatable::from(bigint!(5189976364521848832_i64)),
                MaybeRelocatable::from(bigint!(1)),
                MaybeRelocatable::from(bigint!(1226245742482522112_i64)),
                MaybeRelocatable::from(bigint_str!(
                    b"3618502788666131213697322783095070105623107215331596699973092056135872020474"
                )),
                MaybeRelocatable::from(bigint!(5189976364521848832_i64)),
                MaybeRelocatable::from(bigint!(17)),
                MaybeRelocatable::from(bigint!(1226245742482522112_i64)),
                MaybeRelocatable::from(bigint_str!(
                    b"3618502788666131213697322783095070105623107215331596699973092056135872020470"
                )),
                MaybeRelocatable::from(bigint!(2345108766317314046_i64)),
            ],
            main: Some(4),
            hints: HashMap::new(),
            reference_manager: ReferenceManager {
                references: Vec::new(),
            },
        };
        let hint_processor = BuiltinHintProcessor::new_empty();
        let mut cairo_runner = CairoRunner::new(&program, true, &hint_processor).unwrap();
        cairo_runner.initialize_segments(None);
        let end = cairo_runner.initialize_main_entrypoint().unwrap();
        cairo_runner.initialize_vm().unwrap();
        assert_eq!(cairo_runner.run_until_pc(end), Ok(()));
        cairo_runner
            .vm
            .segments
            .compute_effective_sizes(&cairo_runner.vm.memory);
        let rel_table = cairo_runner
            .vm
            .segments
            .relocate_segments()
            .expect("Couldn't relocate after compute effective sizes");
        cairo_runner.relocate_trace(&rel_table).unwrap();
        let relocated_trace = cairo_runner.relocated_trace.unwrap();
        assert_eq!(relocated_trace.len(), 12);
        assert_eq!(
            relocated_trace[0],
            RelocatedTraceEntry {
                pc: 5,
                ap: 18,
                fp: 18
            }
        );
        assert_eq!(
            relocated_trace[1],
            RelocatedTraceEntry {
                pc: 6,
                ap: 19,
                fp: 18
            }
        );
        assert_eq!(
            relocated_trace[2],
            RelocatedTraceEntry {
                pc: 8,
                ap: 20,
                fp: 18
            }
        );
        assert_eq!(
            relocated_trace[3],
            RelocatedTraceEntry {
                pc: 1,
                ap: 22,
                fp: 22
            }
        );
        assert_eq!(
            relocated_trace[4],
            RelocatedTraceEntry {
                pc: 2,
                ap: 22,
                fp: 22
            }
        );
        assert_eq!(
            relocated_trace[5],
            RelocatedTraceEntry {
                pc: 4,
                ap: 23,
                fp: 22
            }
        );
        assert_eq!(
            relocated_trace[6],
            RelocatedTraceEntry {
                pc: 10,
                ap: 23,
                fp: 18
            }
        );
        assert_eq!(
            relocated_trace[7],
            RelocatedTraceEntry {
                pc: 12,
                ap: 24,
                fp: 18
            }
        );
        assert_eq!(
            relocated_trace[8],
            RelocatedTraceEntry {
                pc: 1,
                ap: 26,
                fp: 26
            }
        );
        assert_eq!(
            relocated_trace[9],
            RelocatedTraceEntry {
                pc: 2,
                ap: 26,
                fp: 26
            }
        );
        assert_eq!(
            relocated_trace[10],
            RelocatedTraceEntry {
                pc: 4,
                ap: 27,
                fp: 26
            }
        );
        assert_eq!(
            relocated_trace[11],
            RelocatedTraceEntry {
                pc: 14,
                ap: 27,
                fp: 18
            }
        );
    }

    #[test]
    fn write_output_from_preset_memory() {
        let program = Program {
            builtins: vec![String::from("output")],
            prime: bigint!(17),
            data: Vec::new(),
            main: None,
            hints: HashMap::new(),
            reference_manager: ReferenceManager {
                references: Vec::new(),
            },
        };
        let hint_processor = BuiltinHintProcessor::new_empty();
        let mut cairo_runner = CairoRunner::new(&program, false, &hint_processor).unwrap();
        cairo_runner.initialize_segments(None);
        assert_eq!(cairo_runner.vm.builtin_runners[0].0, String::from("output"));
        assert_eq!(
            cairo_runner.vm.builtin_runners[0].1.base(),
            relocatable!(2, 0)
        );
        cairo_runner
            .vm
            .memory
            .insert(
                &MaybeRelocatable::from((2, 0)),
                &MaybeRelocatable::from(bigint!(1)),
            )
            .unwrap();
        cairo_runner
            .vm
            .memory
            .insert(
                &MaybeRelocatable::from((2, 1)),
                &MaybeRelocatable::from(bigint!(2)),
            )
            .unwrap();
        cairo_runner.vm.segments.segment_used_sizes = Some(vec![0, 0, 2]);
        let mut stdout = Vec::<u8>::new();
        cairo_runner.write_output(&mut stdout).unwrap();
        assert_eq!(String::from_utf8(stdout), Ok(String::from("1\n2\n")));
    }

    #[test]
    /*Program used:
    %builtins output

    from starkware.cairo.common.serialize import serialize_word

    func main{output_ptr: felt*}():
        let a = 1
        serialize_word(a)
        return()
    end */
    fn write_output_from_program() {
        //Initialization Phase
        let program = Program {
            builtins: vec![String::from("output")],
            prime: BigInt::new(Sign::Plus, vec![1, 0, 0, 0, 0, 0, 17, 134217728]),
            data: vec![
                MaybeRelocatable::from(BigInt::from_i64(4612671182993129469).unwrap()),
                MaybeRelocatable::from(BigInt::from_i64(5198983563776393216).unwrap()),
                MaybeRelocatable::from(bigint!(1)),
                MaybeRelocatable::from(BigInt::from_i64(2345108766317314046).unwrap()),
                MaybeRelocatable::from(BigInt::from_i64(5191102247248822272).unwrap()),
                MaybeRelocatable::from(BigInt::from_i64(5189976364521848832).unwrap()),
                MaybeRelocatable::from(bigint!(1)),
                MaybeRelocatable::from(BigInt::from_i64(1226245742482522112).unwrap()),
                MaybeRelocatable::from(bigint_str!(
                    b"3618502788666131213697322783095070105623107215331596699973092056135872020474"
                )),
                MaybeRelocatable::from(BigInt::from_i64(5189976364521848832).unwrap()),
                MaybeRelocatable::from(bigint!(17)),
                MaybeRelocatable::from(BigInt::from_i64(1226245742482522112).unwrap()),
                MaybeRelocatable::from(bigint_str!(
                    b"3618502788666131213697322783095070105623107215331596699973092056135872020470"
                )),
                MaybeRelocatable::from(BigInt::from_i64(2345108766317314046).unwrap()),
            ],
            main: Some(4),
            hints: HashMap::new(),
            reference_manager: ReferenceManager {
                references: Vec::new(),
            },
        };
        let hint_processor = BuiltinHintProcessor::new_empty();
        let mut cairo_runner = CairoRunner::new(&program, false, &hint_processor).unwrap();
        cairo_runner.initialize_segments(None);
        let end = cairo_runner.initialize_main_entrypoint().unwrap();
        cairo_runner.initialize_vm().unwrap();
        //Execution Phase
        assert_eq!(cairo_runner.run_until_pc(end), Ok(()));
        let mut stdout = Vec::<u8>::new();
        cairo_runner.write_output(&mut stdout).unwrap();
        assert_eq!(String::from_utf8(stdout), Ok(String::from("1\n17\n")));
    }

    #[test]
    fn write_output_from_preset_memory_neg_output() {
        let program = Program {
            builtins: vec![String::from("output")],
            prime: bigint_str!(
                b"3618502788666131213697322783095070105623107215331596699973092056135872020481"
            ),
            data: Vec::new(),
            main: None,
            hints: HashMap::new(),
            reference_manager: ReferenceManager {
                references: Vec::new(),
            },
        };
        let hint_processor = BuiltinHintProcessor::new_empty();
        let mut cairo_runner = CairoRunner::new(&program, false, &hint_processor).unwrap();
        cairo_runner.initialize_segments(None);
        assert_eq!(cairo_runner.vm.builtin_runners[0].0, String::from("output"));
        assert_eq!(
            cairo_runner.vm.builtin_runners[0].1.base(),
            relocatable!(2, 0)
        );
        cairo_runner
            .vm
            .memory
            .insert(
                &MaybeRelocatable::from((2, 0)),
                &MaybeRelocatable::from(bigint_str!(
                    b"3270867057177188607814717243084834301278723532952411121381966378910183338911"
                )),
            )
            .unwrap();
        cairo_runner.vm.segments.segment_used_sizes = Some(vec![0, 0, 1]);
        let mut stdout = Vec::<u8>::new();
        cairo_runner.write_output(&mut stdout).unwrap();
        assert_eq!(
            String::from_utf8(stdout),
            Ok(String::from(
                "-347635731488942605882605540010235804344383682379185578591125677225688681570\n"
            ))
        );
    }

    #[test]
    fn insert_all_builtins_in_order() {
        let program = Program {
            builtins: vec![
                String::from("output"),
                String::from("pedersen"),
                String::from("range_check"),
                String::from("bitwise"),
                String::from("ec_op"),
            ],
            prime: bigint_str!(
                b"3618502788666131213697322783095070105623107215331596699973092056135872020481"
            ),
            data: Vec::new(),
            main: None,
            hints: HashMap::new(),
            reference_manager: ReferenceManager {
                references: Vec::new(),
            },
        };
        let hint_processor = BuiltinHintProcessor::new_empty();
        let cairo_runner = CairoRunner::new(&program, false, &hint_processor).unwrap();
        assert_eq!(cairo_runner.vm.builtin_runners[0].0, String::from("output"));
        assert_eq!(
            cairo_runner.vm.builtin_runners[1].0,
            String::from("pedersen")
        );
        assert_eq!(
            cairo_runner.vm.builtin_runners[2].0,
            String::from("range_check")
        );
        assert_eq!(
            cairo_runner.vm.builtin_runners[3].0,
            String::from("bitwise")
        );
        assert_eq!(cairo_runner.vm.builtin_runners[4].0, String::from("ec_op"));
    }
}
