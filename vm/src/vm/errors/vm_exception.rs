use crate::{
    stdlib::{
        fmt::{self, Display},
        prelude::*,
        str,
    },
    types::relocatable::Relocatable,
};

use thiserror_no_std::Error;

use crate::{
    hint_processor::hint_processor_utils::get_maybe_relocatable_from_reference,
    serde::deserialize_program::{ApTracking, Attribute, Location, OffsetValue},
    types::{instruction::Register, relocatable::MaybeRelocatable},
    vm::runners::cairo_runner::CairoRunner,
};

use super::vm_errors::VirtualMachineError;
#[derive(Debug, Error)]
pub struct VmException {
    pub pc: Relocatable,
    pub inst_location: Option<Location>,
    pub inner_exc: VirtualMachineError,
    pub error_attr_value: Option<String>,
    pub traceback: Option<String>,
}

impl VmException {
    pub fn from_vm_error(runner: &CairoRunner, error: VirtualMachineError) -> Self {
        let pc = runner.vm.run_context.pc;
        let error_attr_value = if pc.segment_index == 0 {
            get_error_attr_value(pc.offset, runner)
        } else {
            None
        };
        let hint_index = if let VirtualMachineError::Hint(ref bx) = error {
            Some(bx.0)
        } else {
            None
        };
        VmException {
            pc,
            inst_location: if pc.segment_index == 0 {
                get_location(pc.offset, runner, hint_index)
            } else {
                None
            },
            inner_exc: error,
            error_attr_value,
            traceback: get_traceback(runner),
        }
    }
}

pub fn get_error_attr_value(pc: usize, runner: &CairoRunner) -> Option<String> {
    let mut errors = String::new();
    for attribute in &runner.program.shared_program_data.error_message_attributes {
        if attribute.start_pc <= pc && attribute.end_pc > pc {
            errors.push_str(&format!(
                "Error message: {}\n",
                substitute_error_message_references(attribute, runner)
            ));
        }
    }
    if errors.is_empty() {
        None
    } else {
        Some(errors)
    }
}

pub fn get_location(
    pc: usize,
    runner: &CairoRunner,
    hint_index: Option<usize>,
) -> Option<Location> {
    let instruction_location = runner
        .program
        .shared_program_data
        .instruction_locations
        .as_ref()?
        .get(&pc)?;
    if let Some(index) = hint_index {
        instruction_location
            .hints
            .get(index)
            .map(|hint_location| hint_location.location.clone())
    } else {
        Some(instruction_location.inst.clone())
    }
}

// Returns the traceback at the current pc.
pub fn get_traceback(runner: &CairoRunner) -> Option<String> {
    let mut traceback = String::new();
    for (_fp, traceback_pc) in runner.vm.get_traceback_entries() {
        if let (0, Some(ref attr)) = (
            traceback_pc.segment_index,
            get_error_attr_value(traceback_pc.offset, runner),
        ) {
            traceback.push_str(attr)
        }
        match (
            traceback_pc.segment_index,
            get_location(traceback_pc.offset, runner, None),
        ) {
            (0, Some(location)) => traceback.push_str(&format!(
                "{}\n",
                location.to_string_with_content(&format!("(pc={})", traceback_pc))
            )),
            _ => traceback.push_str(&format!("Unknown location (pc={})\n", traceback_pc)),
        }
    }
    (!traceback.is_empty())
        .then(|| format!("Cairo traceback (most recent call last):\n{traceback}"))
}

// Substitutes references in the given error_message attribute with their actual value.
// References are defined with '{}'. E.g., 'x must be positive. Got: {x}'.
fn substitute_error_message_references(
    error_message_attr: &Attribute,
    runner: &CairoRunner,
) -> String {
    let mut error_msg = error_message_attr.value.clone();
    if let Some(tracking_data) = &error_message_attr.flow_tracking_data {
        let mut invalid_references = Vec::<String>::new();
        // Iterate over the available references and check if one of them is addressed in the error message
        for (cairo_variable_path, ref_id) in &tracking_data.reference_ids {
            // Get the cairo variable name from its path ie: __main__.main.x -> x
            let cairo_variable_name = match cairo_variable_path.rsplit('.').next() {
                Some(string) => string,
                None => continue,
            };
            // Format the variable name to make it easier to search for and replace in the error message
            // ie: x -> {x}
            let formated_variable_name = format!("{{{cairo_variable_name}}}");
            // Look for the formated name inside the error message
            if error_msg.contains(&formated_variable_name) {
                // Get the value of the cairo variable from its reference id
                match get_value_from_simple_reference(*ref_id, &tracking_data.ap_tracking, runner) {
                    Some(cairo_variable) => {
                        // Replace the value in the error message
                        error_msg =
                            error_msg.replace(&formated_variable_name, &format!("{cairo_variable}"))
                    }
                    None => {
                        // If the reference is too complex or ap-based it might lead to a wrong value
                        // So we append the variable's name to the list of invalid reference
                        invalid_references.push(cairo_variable_name.to_string());
                    }
                }
            }
        }
        if !invalid_references.is_empty() {
            // Add the invalid references (if any) to the error_msg
            error_msg.push_str(&format!(
                " (Cannot evaluate ap-based or complex references: [{}])",
                invalid_references
                    .iter()
                    .fold(String::new(), |acc, arg| acc + &format!("'{arg}'"))
            ));
        }
    }
    error_msg
}

fn get_value_from_simple_reference(
    ref_id: usize,
    ap_tracking: &ApTracking,
    runner: &CairoRunner,
) -> Option<MaybeRelocatable> {
    let reference = runner
        .program
        .shared_program_data
        .reference_manager
        .get(ref_id)?;
    // Filter ap-based references
    match reference.offset1 {
        OffsetValue::Reference(Register::AP, _, _, _) => None,
        _ => {
            // Filer complex types (only felt/felt pointers)
            match reference.cairo_type {
                Some(ref cairo_type) if cairo_type.contains("felt") => Some(
                    get_maybe_relocatable_from_reference(&runner.vm, reference, ap_tracking)?,
                ),
                _ => None,
            }
        }
    }
}

impl Display for VmException {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        // Build initial message
        let message = format!("Error at pc={}:\n{}", self.pc, self.inner_exc);
        let mut error_msg = String::new();
        // Add error attribute value
        if let Some(ref string) = self.error_attr_value {
            error_msg.push_str(string)
        }
        // Add location information
        if let Some(ref location) = self.inst_location {
            let mut location_msg = String::new();
            let (mut location, mut message) = (location, &message);
            loop {
                location_msg = format!(
                    "{}\n{}",
                    location.to_string_with_content(message),
                    location_msg
                );
                // Add parent location info
                if let Some(parent) = &location.parent_location {
                    (location, message) = (&parent.0, &parent.1)
                } else {
                    break;
                }
            }
            error_msg.push_str(&location_msg);
        } else {
            error_msg.push_str(&format!("{message}\n"));
        }
        if let Some(ref string) = self.traceback {
            error_msg.push_str(string);
        }
        // Write error message
        write!(f, "{error_msg}")
    }
}

impl Location {
    ///  Prints the location with the passed message.
    pub fn to_string(&self, message: &str) -> String {
        let msg_prefix = if message.is_empty() { "" } else { ": " };
        format!(
            "{}:{}:{}{}{}",
            self.input_file.filename, self.start_line, self.start_col, msg_prefix, message
        )
    }

    #[cfg(not(feature = "std"))]
    pub fn to_string_with_content(&self, message: &str) -> String {
        self.to_string(message)
    }

    #[cfg(feature = "std")]
    pub fn to_string_with_content(&self, message: &str) -> String {
        let mut string = self.to_string(message);
        let input_file_path = std::path::Path::new(&self.input_file.filename);
        #[cfg(test)]
        let input_file_path = {
            use std::path::PathBuf;
            let current_dir = std::env::current_dir().expect("should return the current directory");
            let mut parent_dir: PathBuf = current_dir
                .parent()
                .expect("should have a parent directory")
                .into();
            parent_dir.push(input_file_path);
            parent_dir
        };
        if let Ok(file_content) = std::fs::read(input_file_path) {
            string.push_str(&format!("\n{}", self.get_location_marks(&file_content)));
        }
        string
    }

    pub fn get_location_marks(&self, file_contents: &[u8]) -> String {
        let mut contents = String::new();
        if let Ok(content) = str::from_utf8(file_contents) {
            contents.push_str(content);
        }
        let split_lines: Vec<&str> = contents.split('\n').collect();
        if !(0 < self.start_line && ((self.start_line - 1) as usize) < split_lines.len()) {
            return String::new();
        }
        let start_line = split_lines[(self.start_line - 1) as usize];
        let start_col = self.start_col as usize;
        let mut result = format!("{start_line}\n");
        let end_col = if self.start_line == self.end_line {
            self.end_col as usize
        } else {
            start_line.len() + 1
        };
        let left_margin: String = vec![' '; start_col - 1].into_iter().collect();
        if end_col > start_col + 1 {
            let highlight: String = vec!['*'; end_col - start_col - 2].into_iter().collect();
            result.push_str(&format!("{left_margin}^{highlight}^"));
        } else {
            result.push_str(&format!("{left_margin}^"))
        }
        result
    }
}
#[cfg(test)]
mod test {
    use crate::stdlib::{boxed::Box, collections::HashMap};
    use crate::types::layout_name::LayoutName;
    use assert_matches::assert_matches;
    #[cfg(feature = "std")]
    use std::path::Path;

    use crate::hint_processor::builtin_hint_processor::builtin_hint_processor_definition::BuiltinHintProcessor;
    use crate::serde::deserialize_program::{
        Attribute, HintLocation, InputFile, InstructionLocation,
    };
    use crate::types::program::Program;
    use crate::types::relocatable::Relocatable;
    use crate::utils::test_utils::*;

    #[cfg(target_arch = "wasm32")]
    use wasm_bindgen_test::*;

    use super::*;
    #[test]
    #[cfg_attr(target_arch = "wasm32", wasm_bindgen_test)]
    fn get_vm_exception_from_vm_error() {
        let pc: Relocatable = (0, 0).into();
        let location = Location {
            end_line: 2,
            end_col: 2,
            input_file: InputFile {
                filename: String::from("Folder/file.cairo"),
            },
            parent_location: None,
            start_line: 1,
            start_col: 1,
        };
        let instruction_location = InstructionLocation {
            inst: location.clone(),
            hints: vec![],
        };
        let program = program!(
            instruction_locations = Some(HashMap::from([(pc.offset, instruction_location)])),
        );
        let runner = cairo_runner!(program);
        assert_matches!(
            VmException::from_vm_error(&runner, VirtualMachineError::NoImm,),
            VmException {
                pc: x,
                inst_location: Some(y),
                inner_exc: VirtualMachineError::NoImm,
                error_attr_value: None,
                traceback: None,
            } if x == pc && y == location
        )
    }

    #[test]
    #[cfg_attr(target_arch = "wasm32", wasm_bindgen_test)]
    fn location_to_string_no_message() {
        let location = Location {
            end_line: 2,
            end_col: 2,
            input_file: InputFile {
                filename: String::from("Folder/file.cairo"),
            },
            parent_location: None,
            start_line: 1,
            start_col: 1,
        };
        let message = String::new();
        assert_eq!(
            location.to_string(&message),
            String::from("Folder/file.cairo:1:1")
        )
    }

    #[test]
    #[cfg_attr(target_arch = "wasm32", wasm_bindgen_test)]
    fn location_to_string_with_message() {
        let location = Location {
            end_line: 2,
            end_col: 2,
            input_file: InputFile {
                filename: String::from("Folder/file.cairo"),
            },
            parent_location: None,
            start_line: 1,
            start_col: 1,
        };
        let message = String::from("While expanding the reference");
        assert_eq!(
            location.to_string(&message),
            String::from("Folder/file.cairo:1:1: While expanding the reference")
        )
    }

    #[test]
    #[cfg_attr(target_arch = "wasm32", wasm_bindgen_test)]
    fn vm_exception_display_instruction_no_location_no_attributes() {
        let vm_excep = VmException {
            pc: (0, 2).into(),
            inst_location: None,
            inner_exc: VirtualMachineError::FailedToComputeOperands(Box::new((
                "op0".to_string(),
                Relocatable::from((0, 4)),
            ))),
            error_attr_value: None,
            traceback: None,
        };
        assert_eq!(
            vm_excep.to_string(),
            format!(
                "Error at pc=0:2:\n{}\n",
                VirtualMachineError::FailedToComputeOperands(Box::new((
                    "op0".to_string(),
                    Relocatable::from((0, 4))
                )))
            )
        )
    }

    #[test]
    #[cfg_attr(target_arch = "wasm32", wasm_bindgen_test)]
    fn vm_exception_display_instruction_no_location_with_attributes() {
        let vm_excep = VmException {
            pc: (0, 2).into(),
            inst_location: None,
            inner_exc: VirtualMachineError::FailedToComputeOperands(Box::new((
                "op0".to_string(),
                Relocatable::from((0, 4)),
            ))),
            error_attr_value: Some(String::from("Error message: Block may fail\n")),
            traceback: None,
        };
        assert_eq!(
            vm_excep.to_string(),
            format!(
                "Error message: Block may fail\nError at pc=0:2:\n{}\n",
                VirtualMachineError::FailedToComputeOperands(Box::new((
                    "op0".to_string(),
                    Relocatable::from((0, 4))
                )))
            )
        )
    }

    #[test]
    #[cfg_attr(target_arch = "wasm32", wasm_bindgen_test)]
    fn vm_exception_display_instruction_no_attributes_no_parent() {
        let location = Location {
            end_line: 2,
            end_col: 2,
            input_file: InputFile {
                filename: String::from("Folder/file.cairo"),
            },
            parent_location: None,
            start_line: 1,
            start_col: 1,
        };
        let vm_excep = VmException {
            pc: (0, 2).into(),
            inst_location: Some(location),
            inner_exc: VirtualMachineError::FailedToComputeOperands(Box::new((
                "op0".to_string(),
                Relocatable::from((0, 4)),
            ))),
            error_attr_value: None,
            traceback: None,
        };
        assert_eq!(
            vm_excep.to_string(),
            format!(
                "Folder/file.cairo:1:1: Error at pc=0:2:\n{}\n",
                VirtualMachineError::FailedToComputeOperands(Box::new((
                    "op0".to_string(),
                    Relocatable::from((0, 4))
                )))
            )
        )
    }

    #[test]
    #[cfg_attr(target_arch = "wasm32", wasm_bindgen_test)]
    fn vm_exception_display_instruction_no_attributes_with_parent() {
        let location = Location {
            end_line: 2,
            end_col: 2,
            input_file: InputFile {
                filename: String::from("Folder/file.cairo"),
            },
            parent_location: Some((
                Box::new(Location {
                    end_line: 3,
                    end_col: 3,
                    input_file: InputFile {
                        filename: String::from("Folder/file_b.cairo"),
                    },
                    parent_location: None,
                    start_line: 2,
                    start_col: 2,
                }),
                String::from("While expanding the reference:"),
            )),
            start_line: 1,
            start_col: 1,
        };
        let vm_excep = VmException {
            pc: (0, 2).into(),
            inst_location: Some(location),
            inner_exc: VirtualMachineError::FailedToComputeOperands(Box::new((
                "op0".to_string(),
                Relocatable::from((0, 4)),
            ))),
            error_attr_value: None,
            traceback: None,
        };
        assert_eq!(
            vm_excep.to_string(),
            format!(
                "Folder/file_b.cairo:2:2: While expanding the reference:\nFolder/file.cairo:1:1: Error at pc=0:2:\n{}\n",
                VirtualMachineError::FailedToComputeOperands(Box::new(("op0".to_string(), Relocatable::from((0, 4)))))
            )
        )
    }

    #[test]
    #[cfg_attr(target_arch = "wasm32", wasm_bindgen_test)]
    fn get_error_attr_value_some() {
        let attributes = vec![Attribute {
            name: String::from("Error message"),
            start_pc: 1,
            end_pc: 5,
            value: String::from("Invalid hash"),
            flow_tracking_data: None,
        }];
        let program = program!(error_message_attributes = attributes,);
        let runner = cairo_runner!(program);
        assert_eq!(
            get_error_attr_value(2, &runner),
            Some(String::from("Error message: Invalid hash\n"))
        );
    }

    #[test]
    #[cfg_attr(target_arch = "wasm32", wasm_bindgen_test)]
    fn get_error_attr_value_none() {
        let attributes = vec![Attribute {
            name: String::from("Error message"),
            start_pc: 1,
            end_pc: 5,
            value: String::from("Invalid hash"),
            flow_tracking_data: None,
        }];
        let program = program!(error_message_attributes = attributes,);
        let runner = cairo_runner!(program);
        assert_eq!(get_error_attr_value(5, &runner), None);
    }

    #[test]
    #[cfg_attr(target_arch = "wasm32", wasm_bindgen_test)]
    fn get_location_some() {
        let location = Location {
            end_line: 2,
            end_col: 2,
            input_file: InputFile {
                filename: String::from("Folder/file.cairo"),
            },
            parent_location: None,
            start_line: 1,
            start_col: 1,
        };
        let instruction_location = InstructionLocation {
            inst: location.clone(),
            hints: vec![],
        };
        let program =
            program!(instruction_locations = Some(HashMap::from([(2, instruction_location)])),);
        let runner = cairo_runner!(program);
        assert_eq!(get_location(2, &runner, None), Some(location));
    }

    #[test]
    #[cfg_attr(target_arch = "wasm32", wasm_bindgen_test)]
    fn get_location_none() {
        let location = Location {
            end_line: 2,
            end_col: 2,
            input_file: InputFile {
                filename: String::from("Folder/file.cairo"),
            },
            parent_location: None,
            start_line: 1,
            start_col: 1,
        };
        let instruction_location = InstructionLocation {
            inst: location,
            hints: vec![],
        };
        let program =
            program!(instruction_locations = Some(HashMap::from([(2, instruction_location)])),);
        let runner = cairo_runner!(program);
        assert_eq!(get_location(3, &runner, None), None);
    }

    #[test]
    #[cfg_attr(target_arch = "wasm32", wasm_bindgen_test)]
    fn get_location_some_hint_index() {
        let location_a = Location {
            end_line: 2,
            end_col: 2,
            input_file: InputFile {
                filename: String::from("Folder/file_a.cairo"),
            },
            parent_location: None,
            start_line: 1,
            start_col: 1,
        };
        let location_b = Location {
            end_line: 3,
            end_col: 2,
            input_file: InputFile {
                filename: String::from("Folder/file_b.cairo"),
            },
            parent_location: None,
            start_line: 1,
            start_col: 5,
        };
        let hint_location = HintLocation {
            location: location_b.clone(),
            n_prefix_newlines: 2,
        };
        let instruction_location = InstructionLocation {
            inst: location_a,
            hints: vec![hint_location],
        };
        let program =
            program!(instruction_locations = Some(HashMap::from([(2, instruction_location)])),);
        let runner = cairo_runner!(program);
        assert_eq!(get_location(2, &runner, Some(0)), Some(location_b));
    }

    #[test]
    #[cfg_attr(target_arch = "wasm32", wasm_bindgen_test)]
    fn get_traceback_bad_dict_update() {
        let program = Program::from_bytes(
            include_bytes!("../../../../cairo_programs/bad_programs/bad_dict_update.json"),
            Some("main"),
        )
        .expect("Call to `Program::from_file()` failed.");

        let mut hint_processor = BuiltinHintProcessor::new_empty();
        let mut cairo_runner = cairo_runner!(program, LayoutName::all_cairo, false);

        let end = cairo_runner.initialize(false).unwrap();
        assert!(cairo_runner.run_until_pc(end, &mut hint_processor).is_err());

        #[cfg(feature = "std")]
        let expected_traceback = String::from("Cairo traceback (most recent call last):\ncairo_programs/bad_programs/bad_dict_update.cairo:10:5: (pc=0:34)\n    dict_update{dict_ptr=my_dict}(key=2, prev_value=3, new_value=4);\n    ^*************************************************************^\n");
        #[cfg(not(feature = "std"))]
        let expected_traceback = String::from("Cairo traceback (most recent call last):\ncairo_programs/bad_programs/bad_dict_update.cairo:10:5: (pc=0:34)\n");

        let mut hint_processor = BuiltinHintProcessor::new_empty();
        let mut cairo_runner = cairo_runner!(program, LayoutName::all_cairo, false);

        let end = cairo_runner.initialize(false).unwrap();
        assert!(cairo_runner.run_until_pc(end, &mut hint_processor).is_err());
        assert_eq!(get_traceback(&cairo_runner), Some(expected_traceback));
    }

    #[test]
    #[cfg_attr(target_arch = "wasm32", wasm_bindgen_test)]
    fn get_traceback_bad_usort() {
        let program = Program::from_bytes(
            include_bytes!("../../../../cairo_programs/bad_programs/bad_usort.json"),
            Some("main"),
        )
        .unwrap();
        #[cfg(feature = "std")]
        let expected_traceback = r"Cairo traceback (most recent call last):
cairo_programs/bad_programs/bad_usort.cairo:91:48: (pc=0:97)
    let (output_len, output, multiplicities) = usort(input_len=3, input=input_array);
                                               ^***********************************^
cairo_programs/bad_programs/bad_usort.cairo:36:5: (pc=0:30)
    verify_usort{output=output}(
    ^**************************^
cairo_programs/bad_programs/bad_usort.cairo:64:5: (pc=0:60)
    verify_multiplicity(multiplicity=multiplicity, input_len=input_len, input=input, value=value);
    ^*******************************************************************************************^
";
        #[cfg(not(feature = "std"))]
        let expected_traceback = r"Cairo traceback (most recent call last):
cairo_programs/bad_programs/bad_usort.cairo:91:48: (pc=0:97)
cairo_programs/bad_programs/bad_usort.cairo:36:5: (pc=0:30)
cairo_programs/bad_programs/bad_usort.cairo:64:5: (pc=0:60)
";

        let mut hint_processor = BuiltinHintProcessor::new_empty();
        let mut cairo_runner = cairo_runner!(program, LayoutName::all_cairo, false);

        let end = cairo_runner.initialize(false).unwrap();
        assert!(cairo_runner.run_until_pc(end, &mut hint_processor).is_err());
        assert_eq!(
            get_traceback(&cairo_runner),
            Some(expected_traceback.to_string())
        );
    }

    #[test]
    #[cfg_attr(target_arch = "wasm32", wasm_bindgen_test)]
    fn location_to_string_with_contents_no_contents() {
        let location = Location {
            end_line: 2,
            end_col: 2,
            input_file: InputFile {
                filename: String::from("Folder/file.cairo"),
            },
            parent_location: None,
            start_line: 1,
            start_col: 1,
        };
        let message = String::from("While expanding the reference");
        assert_eq!(
            location.to_string_with_content(&message),
            String::from("Folder/file.cairo:1:1: While expanding the reference")
        )
    }

    #[test]
    #[cfg_attr(target_arch = "wasm32", wasm_bindgen_test)]
    fn location_to_string_with_contents() {
        let location = Location {
            end_line: 5,
            end_col: 2,
            input_file: InputFile {
                filename: String::from("cairo_programs/bad_programs/bad_usort.cairo"),
            },
            parent_location: None,
            start_line: 5,
            start_col: 1,
        };
        let message = String::from("Error at pc=0:75:");

        #[cfg(feature = "std")]
        let expected_message = "cairo_programs/bad_programs/bad_usort.cairo:5:1: Error at pc=0:75:\nfunc usort{range_check_ptr}(input_len: felt, input: felt*) -> (\n^";
        #[cfg(not(feature = "std"))]
        let expected_message = "cairo_programs/bad_programs/bad_usort.cairo:5:1: Error at pc=0:75:";

        assert_eq!(
            location.to_string_with_content(&message),
            expected_message.to_string()
        )
    }

    #[test]
    #[cfg_attr(target_arch = "wasm32", wasm_bindgen_test)]
    fn location_to_string_with_contents_no_file() {
        let location = Location {
            end_line: 5,
            end_col: 2,
            input_file: InputFile {
                filename: String::from("cairo_programs/bad_prtypoograms/bad_usort.cairo"),
            },
            parent_location: None,
            start_line: 5,
            start_col: 1,
        };
        let message = String::from("Error at pc=0:75:\n");
        assert_eq!(
            location.to_string_with_content(&message),
            String::from(
                "cairo_programs/bad_prtypoograms/bad_usort.cairo:5:1: Error at pc=0:75:\n"
            )
        )
    }

    #[test]
    #[cfg(feature = "std")]
    fn location_get_location_marks() {
        let location = Location {
            end_line: 5,
            end_col: 2,
            input_file: InputFile {
                filename: String::from("../cairo_programs/bad_programs/bad_usort.cairo"),
            },
            parent_location: None,
            start_line: 5,
            start_col: 1,
        };
        let input_file_path = Path::new(&location.input_file.filename);
        let file_content = std::fs::read(input_file_path).expect("Failed to open file");
        assert_eq!(
            location.get_location_marks(&file_content),
            String::from("func usort{range_check_ptr}(input_len: felt, input: felt*) -> (\n^")
        )
    }

    #[test]
    #[cfg_attr(target_arch = "wasm32", wasm_bindgen_test)]
    fn location_get_location_marks_empty_file() {
        let location = Location {
            end_line: 5,
            end_col: 2,
            input_file: InputFile {
                filename: String::from("cairo_programs/bad_programs/bad_usort.cairo"),
            },
            parent_location: None,
            start_line: 5,
            start_col: 1,
        };
        let reader: &[u8] = &[];
        assert_eq!(location.get_location_marks(reader), String::from(""))
    }

    #[test]
    #[cfg_attr(target_arch = "wasm32", wasm_bindgen_test)]
    fn run_bad_range_check_and_check_error_displayed() {
        #[cfg(feature = "std")]
        let expected_error_string = r#"Error message: Failed range-check
cairo_programs/bad_programs/bad_range_check.cairo:5:9: Error at pc=0:0:
An ASSERT_EQ instruction failed: 4 != 5.
        [range_check_ptr] = num;
        ^*********************^
Cairo traceback (most recent call last):
cairo_programs/bad_programs/bad_range_check.cairo:23:5: (pc=0:29)
    sub_by_1_check_range(6, 7);
    ^************************^
cairo_programs/bad_programs/bad_range_check.cairo:19:12: (pc=0:21)
    return sub_by_1_check_range(sub_1_check_range(num), sub_amount -1);
           ^*********************************************************^
cairo_programs/bad_programs/bad_range_check.cairo:19:33: (pc=0:17)
    return sub_by_1_check_range(sub_1_check_range(num), sub_amount -1);
                                ^********************^
cairo_programs/bad_programs/bad_range_check.cairo:11:5: (pc=0:6)
    check_range(num - 1);
    ^******************^
"#;
        #[cfg(not(feature = "std"))]
        let expected_error_string = r#"Error message: Failed range-check
cairo_programs/bad_programs/bad_range_check.cairo:5:9: Error at pc=0:0:
An ASSERT_EQ instruction failed: 4 != 5.
Cairo traceback (most recent call last):
cairo_programs/bad_programs/bad_range_check.cairo:23:5: (pc=0:29)
cairo_programs/bad_programs/bad_range_check.cairo:19:12: (pc=0:21)
cairo_programs/bad_programs/bad_range_check.cairo:19:33: (pc=0:17)
cairo_programs/bad_programs/bad_range_check.cairo:11:5: (pc=0:6)
"#;
        let program = Program::from_bytes(
            include_bytes!("../../../../cairo_programs/bad_programs/bad_range_check.json"),
            Some("main"),
        )
        .unwrap();

        let mut hint_processor = BuiltinHintProcessor::new_empty();
        let mut cairo_runner = cairo_runner!(program, LayoutName::all_cairo, false);

        let end = cairo_runner.initialize(false).unwrap();
        let error = cairo_runner
            .run_until_pc(end, &mut hint_processor)
            .unwrap_err();
        let vm_excepction = VmException::from_vm_error(&cairo_runner, error);
        assert_eq!(vm_excepction.to_string(), expected_error_string);
    }

    #[test]
    #[cfg_attr(target_arch = "wasm32", wasm_bindgen_test)]
    fn run_bad_usort_and_check_error_displayed() {
        #[cfg(feature = "std")]
        let expected_error_string = r#"cairo_programs/bad_programs/bad_usort.cairo:79:5: Error at pc=0:75:
Got an exception while executing a hint: unexpected verify multiplicity fail: positions length != 0
    %{ assert len(positions) == 0 %}
    ^******************************^
Cairo traceback (most recent call last):
cairo_programs/bad_programs/bad_usort.cairo:91:48: (pc=0:97)
    let (output_len, output, multiplicities) = usort(input_len=3, input=input_array);
                                               ^***********************************^
cairo_programs/bad_programs/bad_usort.cairo:36:5: (pc=0:30)
    verify_usort{output=output}(
    ^**************************^
cairo_programs/bad_programs/bad_usort.cairo:64:5: (pc=0:60)
    verify_multiplicity(multiplicity=multiplicity, input_len=input_len, input=input, value=value);
    ^*******************************************************************************************^
"#;
        #[cfg(not(feature = "std"))]
        let expected_error_string = r#"cairo_programs/bad_programs/bad_usort.cairo:79:5: Error at pc=0:75:
Got an exception while executing a hint: unexpected verify multiplicity fail: positions length != 0
Cairo traceback (most recent call last):
cairo_programs/bad_programs/bad_usort.cairo:91:48: (pc=0:97)
cairo_programs/bad_programs/bad_usort.cairo:36:5: (pc=0:30)
cairo_programs/bad_programs/bad_usort.cairo:64:5: (pc=0:60)
"#;
        let program = Program::from_bytes(
            include_bytes!("../../../../cairo_programs/bad_programs/bad_usort.json"),
            Some("main"),
        )
        .unwrap();

        let mut hint_processor = BuiltinHintProcessor::new_empty();
        let mut cairo_runner = cairo_runner!(program, LayoutName::all_cairo, false);

        let end = cairo_runner.initialize(false).unwrap();
        let error = cairo_runner
            .run_until_pc(end, &mut hint_processor)
            .unwrap_err();
        let vm_excepction = VmException::from_vm_error(&cairo_runner, error);
        assert_eq!(vm_excepction.to_string(), expected_error_string);
    }

    #[test]
    #[cfg_attr(target_arch = "wasm32", wasm_bindgen_test)]
    fn run_bad_ec_recover_product_mod() {
        #[cfg(feature = "std")]
        let expected_error_string = r#"cairo_programs/bad_programs/ec_recover_product_mod_m_zero.cairo:16:5: Error at pc=0:21:
Got an exception while executing a hint: Attempted to divide by zero
    %{
    ^^
Cairo traceback (most recent call last):
cairo_programs/bad_programs/ec_recover_product_mod_m_zero.cairo:11:5: (pc=0:18)
    ec_recover_product(a, b, m);
    ^*************************^
"#;
        #[cfg(not(feature = "std"))]
        let expected_error_string = r#"cairo_programs/bad_programs/ec_recover_product_mod_m_zero.cairo:16:5: Error at pc=0:21:
Got an exception while executing a hint: Attempted to divide by zero
Cairo traceback (most recent call last):
cairo_programs/bad_programs/ec_recover_product_mod_m_zero.cairo:11:5: (pc=0:18)
"#;
        let program = Program::from_bytes(
            include_bytes!(
                "../../../../cairo_programs/bad_programs/ec_recover_product_mod_m_zero.json"
            ),
            Some("main"),
        )
        .unwrap();

        let mut hint_processor = BuiltinHintProcessor::new_empty();
        let mut cairo_runner = cairo_runner!(program, LayoutName::all_cairo, false);

        let end = cairo_runner.initialize(false).unwrap();
        let error = cairo_runner
            .run_until_pc(end, &mut hint_processor)
            .unwrap_err();
        let vm_excepction = VmException::from_vm_error(&cairo_runner, error);
        assert_eq!(vm_excepction.to_string(), expected_error_string);
    }

    #[test]
    #[cfg_attr(target_arch = "wasm32", wasm_bindgen_test)]
    fn run_bad_ec_recover_div_mod_n_packed_n_zero() {
        #[cfg(feature = "std")]
        let expected_error_string = r#"cairo_programs/bad_programs/ec_recover_div_mod_n_packed_n_zero.cairo:16:5: Error at pc=0:21:
Got an exception while executing a hint: Attempted to divide by zero
    %{
    ^^
Cairo traceback (most recent call last):
cairo_programs/bad_programs/ec_recover_div_mod_n_packed_n_zero.cairo:11:5: (pc=0:18)
    ec_recover_product(x, s, n);
    ^*************************^
"#;
        #[cfg(not(feature = "std"))]
        let expected_error_string = r#"cairo_programs/bad_programs/ec_recover_div_mod_n_packed_n_zero.cairo:16:5: Error at pc=0:21:
Got an exception while executing a hint: Attempted to divide by zero
Cairo traceback (most recent call last):
cairo_programs/bad_programs/ec_recover_div_mod_n_packed_n_zero.cairo:11:5: (pc=0:18)
"#;
        let program = Program::from_bytes(
            include_bytes!(
                "../../../../cairo_programs/bad_programs/ec_recover_div_mod_n_packed_n_zero.json"
            ),
            Some("main"),
        )
        .unwrap();

        let mut hint_processor = BuiltinHintProcessor::new_empty();
        let mut cairo_runner = cairo_runner!(program, LayoutName::all_cairo, false);

        let end = cairo_runner.initialize(false).unwrap();
        let error = cairo_runner
            .run_until_pc(end, &mut hint_processor)
            .unwrap_err();
        let vm_excepction = VmException::from_vm_error(&cairo_runner, error);
        assert_eq!(vm_excepction.to_string(), expected_error_string);
    }

    #[test]
    #[cfg_attr(target_arch = "wasm32", wasm_bindgen_test)]
    fn run_bad_uint512_unsigned_div_rem() {
        #[cfg(feature = "std")]
        let expected_error_string = r#"cairo_programs/bad_programs/uint512_unsigned_div_rem_div_is_zero.cairo:24:1: Error at pc=0:17:
Got an exception while executing a hint: Attempted to divide by zero
%{
^^
Cairo traceback (most recent call last):
cairo_programs/bad_programs/uint512_unsigned_div_rem_div_is_zero.cairo:15:2: (pc=0:12)
	hint_func(x, div);
 ^***************^
"#;
        #[cfg(not(feature = "std"))]
        let expected_error_string = r#"cairo_programs/bad_programs/uint512_unsigned_div_rem_div_is_zero.cairo:24:1: Error at pc=0:17:
Got an exception while executing a hint: Attempted to divide by zero
Cairo traceback (most recent call last):
cairo_programs/bad_programs/uint512_unsigned_div_rem_div_is_zero.cairo:15:2: (pc=0:12)
"#;
        let program = Program::from_bytes(
            include_bytes!(
                "../../../../cairo_programs/bad_programs/uint512_unsigned_div_rem_div_is_zero.json"
            ),
            Some("main"),
        )
        .unwrap();

        let mut hint_processor = BuiltinHintProcessor::new_empty();
        let mut cairo_runner = cairo_runner!(program, LayoutName::all_cairo, false);

        let end = cairo_runner.initialize(false).unwrap();
        let error = cairo_runner
            .run_until_pc(end, &mut hint_processor)
            .unwrap_err();
        let vm_excepction = VmException::from_vm_error(&cairo_runner, error);
        assert_eq!(vm_excepction.to_string(), expected_error_string);
    }

    #[test]
    #[cfg_attr(target_arch = "wasm32", wasm_bindgen_test)]
    fn run_bad_uint256_sub_check_error_displayed() {
        #[cfg(feature = "std")]
        let expected_error_string = r#"cairo_programs/bad_programs/uint256_sub_b_gt_256.cairo:17:1: Error at pc=0:17:
Got an exception while executing a hint: Inconsistent memory assignment at address Relocatable { segment_index: 1, offset: 6 }. Int(1) != Int(41367660292349381832802403122744918015)
%{
^^
Cairo traceback (most recent call last):
cairo_programs/bad_programs/uint256_sub_b_gt_256.cairo:10:2: (pc=0:12)
	hint_func(a, b, res);
 ^******************^
"#;
        #[cfg(not(feature = "std"))]
        let expected_error_string = r#"cairo_programs/bad_programs/uint256_sub_b_gt_256.cairo:17:1: Error at pc=0:17:
Got an exception while executing a hint: Inconsistent memory assignment at address Relocatable { segment_index: 1, offset: 6 }. Int(1) != Int(41367660292349381832802403122744918015)
Cairo traceback (most recent call last):
cairo_programs/bad_programs/uint256_sub_b_gt_256.cairo:10:2: (pc=0:12)
"#;
        let program = Program::from_bytes(
            include_bytes!("../../../../cairo_programs/bad_programs/uint256_sub_b_gt_256.json"),
            Some("main"),
        )
        .unwrap();

        let mut hint_processor = BuiltinHintProcessor::new_empty();
        let mut cairo_runner = cairo_runner!(program, LayoutName::all_cairo, false);

        let end = cairo_runner.initialize(false).unwrap();
        let error = cairo_runner
            .run_until_pc(end, &mut hint_processor)
            .unwrap_err();
        let vm_excepction = VmException::from_vm_error(&cairo_runner, error);
        assert_eq!(vm_excepction.to_string(), expected_error_string);
    }

    #[test]
    #[cfg_attr(target_arch = "wasm32", wasm_bindgen_test)]
    fn get_value_from_simple_reference_ap_based() {
        let program = Program::from_bytes(
            include_bytes!("../../../../cairo_programs/bad_programs/error_msg_attr_tempvar.json"),
            Some("main"),
        )
        .unwrap();
        // This program uses a tempvar inside an error attribute
        // This reference should be rejected when substituting the error attribute references
        let runner = cairo_runner!(program);
        // Ref id 0 corresponds to __main__.main.x, our tempvar
        assert_eq!(
            get_value_from_simple_reference(0, &ApTracking::default(), &runner),
            None
        )
    }

    #[test]
    #[cfg_attr(target_arch = "wasm32", wasm_bindgen_test)]
    fn substitute_error_message_references_ap_based() {
        let program = Program::from_bytes(
            include_bytes!("../../../../cairo_programs/bad_programs/error_msg_attr_tempvar.json"),
            Some("main"),
        )
        .unwrap();
        // This program uses a tempvar inside an error attribute
        // This reference should be rejected when substituting the error attribute references
        let runner = cairo_runner!(program);
        let attribute = &program.shared_program_data.error_message_attributes[0];
        assert_eq!(
            substitute_error_message_references(attribute, &runner),
            format!(
                "{} (Cannot evaluate ap-based or complex references: ['x'])",
                attribute.value
            )
        );
    }

    #[test]
    #[cfg_attr(target_arch = "wasm32", wasm_bindgen_test)]
    fn get_value_from_simple_reference_complex() {
        let program = Program::from_bytes(
            include_bytes!("../../../../cairo_programs/bad_programs/error_msg_attr_struct.json"),
            Some("main"),
        )
        .unwrap();
        // This program uses a struct inside an error attribute
        // This reference should be rejected when substituting the error attribute references
        let runner = cairo_runner!(program);
        // Ref id 0 corresponds to __main__.main.cat, our struct
        assert_eq!(
            get_value_from_simple_reference(0, &ApTracking::default(), &runner),
            None
        )
    }

    #[test]
    #[cfg_attr(target_arch = "wasm32", wasm_bindgen_test)]
    fn substitute_error_message_references_complex() {
        let program = Program::from_bytes(
            include_bytes!("../../../../cairo_programs/bad_programs/error_msg_attr_struct.json"),
            Some("main"),
        )
        .unwrap();
        // This program uses a struct inside an error attribute
        // This reference should be rejected when substituting the error attribute references
        let runner = cairo_runner!(program);
        let attribute = &program.shared_program_data.error_message_attributes[0];
        assert_eq!(
            substitute_error_message_references(attribute, &runner),
            format!(
                "{} (Cannot evaluate ap-based or complex references: ['cat'])",
                attribute.value
            )
        );
    }

    #[test]
    #[cfg_attr(target_arch = "wasm32", wasm_bindgen_test)]
    fn get_vm_exception_from_vm_error_pc_not_program_segment() {
        let pc = (9, 5).into();
        let location = Location {
            end_line: 2,
            end_col: 2,
            input_file: InputFile {
                filename: String::from("Folder/file.cairo"),
            },
            parent_location: None,
            start_line: 1,
            start_col: 1,
        };
        let instruction_location = InstructionLocation {
            inst: location,
            hints: vec![],
        };
        let program =
            program!(instruction_locations = Some(HashMap::from([(5, instruction_location)])),);
        let mut runner = cairo_runner!(program);
        runner.vm.set_pc(pc);
        assert_matches!(
            VmException::from_vm_error(&runner, VirtualMachineError::NoImm,),
            VmException {
                pc: x,
                inst_location: None,
                inner_exc: VirtualMachineError::NoImm,
                error_attr_value: None,
                traceback: None,
            } if x == pc
        )
    }
}
