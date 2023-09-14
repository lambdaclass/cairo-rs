//! # Program deserialization
//!
//! This module contains the logic for [`Program`] deserialization.
//! Users shouldn't need to use it directly (except for [`BuiltinName`]).
//!
//! To generate a [`Program`] from a JSON string, see [`Program::from_bytes()`].
//! To do the same from a JSON file, see [`Program::from_file()`].

use crate::stdlib::{
    collections::{BTreeMap, HashMap},
    fmt,
    prelude::*,
    sync::Arc,
};

use crate::vm::runners::builtin_runner::SEGMENT_ARENA_BUILTIN_NAME;
use crate::{
    serde::deserialize_utils,
    types::{
        errors::program_errors::ProgramError,
        instruction::Register,
        program::{HintsCollection, Program, SharedProgramData},
        relocatable::MaybeRelocatable,
    },
    vm::runners::builtin_runner::{
        BITWISE_BUILTIN_NAME, EC_OP_BUILTIN_NAME, HASH_BUILTIN_NAME, KECCAK_BUILTIN_NAME,
        OUTPUT_BUILTIN_NAME, POSEIDON_BUILTIN_NAME, RANGE_CHECK_BUILTIN_NAME,
        SIGNATURE_BUILTIN_NAME,
    },
};
use felt::{Felt252, PRIME_STR};
use num_traits::float::FloatCore;
use num_traits::{Num, Pow};
use serde::{de, de::MapAccess, de::SeqAccess, Deserialize, Deserializer, Serialize};
use serde_json::Number;

#[cfg(all(feature = "arbitrary", feature = "std"))]
use arbitrary::{self, Arbitrary, Unstructured};

// This enum is used to deserialize program builtins into &str and catch non-valid names
#[cfg_attr(all(feature = "arbitrary", feature = "std"), derive(Arbitrary))]
#[derive(Serialize, Deserialize, Debug, PartialEq, Copy, Clone, Eq, Hash)]
#[allow(non_camel_case_types)]
pub enum BuiltinName {
    output,
    range_check,
    pedersen,
    ecdsa,
    keccak,
    bitwise,
    ec_op,
    poseidon,
    segment_arena,
}

impl BuiltinName {
    pub fn name(&self) -> &'static str {
        match self {
            BuiltinName::output => OUTPUT_BUILTIN_NAME,
            BuiltinName::range_check => RANGE_CHECK_BUILTIN_NAME,
            BuiltinName::pedersen => HASH_BUILTIN_NAME,
            BuiltinName::ecdsa => SIGNATURE_BUILTIN_NAME,
            BuiltinName::keccak => KECCAK_BUILTIN_NAME,
            BuiltinName::bitwise => BITWISE_BUILTIN_NAME,
            BuiltinName::ec_op => EC_OP_BUILTIN_NAME,
            BuiltinName::poseidon => POSEIDON_BUILTIN_NAME,
            BuiltinName::segment_arena => SEGMENT_ARENA_BUILTIN_NAME,
        }
    }
}

#[cfg_attr(all(feature = "arbitrary", feature = "std"), derive(Arbitrary, Clone))]
#[derive(Deserialize, Debug)]
pub struct ProgramJson {
    pub prime: String,
    pub builtins: Vec<BuiltinName>,
    #[serde(deserialize_with = "deserialize_array_of_bigint_hex")]
    pub data: Vec<MaybeRelocatable>,
    pub identifiers: HashMap<String, Identifier>,
    pub hints: BTreeMap<usize, Vec<HintParams>>,
    pub reference_manager: ReferenceManager,
    pub attributes: Vec<Attribute>,
    pub debug_info: Option<DebugInfo>,
}

#[cfg_attr(all(feature = "arbitrary", feature = "std"), derive(Arbitrary))]
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
pub struct HintParams {
    pub code: String,
    pub accessible_scopes: Vec<String>,
    pub flow_tracking_data: FlowTrackingData,
}

#[cfg_attr(all(feature = "arbitrary", feature = "std"), derive(Arbitrary))]
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
pub struct FlowTrackingData {
    pub ap_tracking: ApTracking,
    #[serde(deserialize_with = "deserialize_map_to_string_and_usize_hashmap")]
    pub reference_ids: HashMap<String, usize>,
}

#[cfg_attr(all(feature = "arbitrary", feature = "std"), derive(Arbitrary))]
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
pub struct ApTracking {
    pub group: usize,
    pub offset: usize,
}

impl ApTracking {
    pub fn new() -> ApTracking {
        ApTracking {
            group: 0,
            offset: 0,
        }
    }
}

impl Default for ApTracking {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg_attr(all(feature = "arbitrary", feature = "std"), derive(Arbitrary))]
#[derive(Serialize, Deserialize, Debug, PartialEq, Eq, Clone)]
pub struct Identifier {
    pub pc: Option<usize>,
    #[serde(rename(deserialize = "type"))]
    pub type_: Option<String>,
    #[serde(default)]
    #[serde(deserialize_with = "felt_from_number")]
    pub value: Option<Felt252>,

    pub full_name: Option<String>,
    pub members: Option<HashMap<String, Member>>,
    pub cairo_type: Option<String>,
}

#[cfg_attr(all(feature = "arbitrary", feature = "std"), derive(Arbitrary))]
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct Member {
    pub cairo_type: String,
    pub offset: usize,
}

#[cfg_attr(all(feature = "arbitrary", feature = "std"), derive(Arbitrary))]
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct Attribute {
    pub name: String,
    pub start_pc: usize,
    pub end_pc: usize,
    pub value: String,
    #[cfg_attr(
        all(feature = "arbitrary", feature = "std"),
        serde(skip_serializing_if = "Option::is_none")
    )]
    pub flow_tracking_data: Option<FlowTrackingData>,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
pub struct Location {
    pub end_line: u32,
    pub end_col: u32,
    pub input_file: InputFile,
    pub parent_location: Option<(Box<Location>, String)>,
    pub start_line: u32,
    pub start_col: u32,
}

#[cfg(all(feature = "arbitrary", feature = "std"))]
impl<'a> Arbitrary<'a> for Location {
    fn arbitrary(u: &mut Unstructured<'a>) -> arbitrary::Result<Self> {
        arbitrary_parent_location(u, 20)
    }
}

#[cfg(all(feature = "arbitrary", feature = "std"))]
fn arbitrary_parent_location(u: &mut Unstructured, depth: u8) -> arbitrary::Result<Location> {
    let parent_location = if depth > 0 {
        Some((
            Box::new(arbitrary_parent_location(u, depth - 1)?),
            String::arbitrary(u)?,
        ))
    } else {
        None
    };
    Ok(Location {
        end_line: u32::arbitrary(u)?,
        end_col: u32::arbitrary(u)?,
        input_file: InputFile::arbitrary(u)?,
        parent_location,
        start_line: u32::arbitrary(u)?,
        start_col: u32::arbitrary(u)?,
    })
}

#[cfg_attr(
    all(feature = "arbitrary", feature = "std"),
    derive(Arbitrary, Clone, Serialize)
)]
#[derive(Deserialize, Debug, PartialEq, Eq)]
pub struct DebugInfo {
    instruction_locations: HashMap<usize, InstructionLocation>,
}

#[cfg_attr(all(feature = "arbitrary", feature = "std"), derive(Arbitrary))]
#[derive(Serialize, Deserialize, Debug, PartialEq, Eq, Clone)]
pub struct InstructionLocation {
    pub inst: Location,
    pub hints: Vec<HintLocation>,
}

#[cfg_attr(all(feature = "arbitrary", feature = "std"), derive(Arbitrary))]
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
pub struct InputFile {
    pub filename: String,
}

#[cfg_attr(all(feature = "arbitrary", feature = "std"), derive(Arbitrary))]
#[derive(Serialize, Deserialize, Debug, PartialEq, Eq, Clone)]
pub struct HintLocation {
    pub location: Location,
    pub n_prefix_newlines: u32,
}

fn felt_from_number<'de, D>(deserializer: D) -> Result<Option<Felt252>, D::Error>
where
    D: Deserializer<'de>,
{
    let n = Number::deserialize(deserializer)?;
    match Felt252::parse_bytes(n.to_string().as_bytes(), 10) {
        Some(x) => Ok(Some(x)),
        None => {
            // Handle de Number with scientific notation cases
            // e.g.: n = Number(1e27)
            let felt = deserialize_scientific_notation(n);
            if felt.is_some() {
                return Ok(felt);
            }

            Err(de::Error::custom(String::from(
                "felt_from_number parse error",
            )))
        }
    }
}

fn deserialize_scientific_notation(n: Number) -> Option<Felt252> {
    match n.as_f64() {
        None => {
            let str = n.to_string();
            let list: [&str; 2] = str.split('e').collect::<Vec<&str>>().try_into().ok()?;

            let exponent = list[1].parse::<u32>().ok()?;
            let base = Felt252::parse_bytes(list[0].to_string().as_bytes(), 10)?;
            Some(base * Felt252::from(10).pow(exponent))
        }
        Some(float) => Felt252::parse_bytes(FloatCore::round(float).to_string().as_bytes(), 10),
    }
}

#[cfg_attr(all(feature = "arbitrary", feature = "std"), derive(Arbitrary))]
#[derive(Serialize, Deserialize, Debug, PartialEq, Eq, Clone, Default)]
pub struct ReferenceManager {
    pub references: Vec<Reference>,
}

#[cfg_attr(all(feature = "arbitrary", feature = "std"), derive(Arbitrary))]
#[derive(Serialize, Deserialize, Debug, PartialEq, Eq, Clone)]
pub struct Reference {
    pub ap_tracking_data: ApTracking,
    pub pc: Option<usize>,
    #[serde(deserialize_with = "deserialize_value_address")]
    #[serde(rename(deserialize = "value"))]
    pub value_address: ValueAddress,
}

#[cfg_attr(all(feature = "arbitrary", feature = "std"), derive(Arbitrary))]
#[derive(Serialize, Deserialize, Debug, PartialEq, Eq, Clone)]
pub enum OffsetValue {
    Immediate(Felt252),
    Value(i32),
    Reference(Register, i32, bool),
}

#[cfg_attr(all(feature = "arbitrary", feature = "std"), derive(Arbitrary))]
#[derive(Serialize, Deserialize, Debug, PartialEq, Eq, Clone)]
pub struct ValueAddress {
    pub offset1: OffsetValue,
    pub offset2: OffsetValue,
    pub dereference: bool,
    pub value_type: String,
}

impl ValueAddress {
    // The parsing functionality is focused on the string formats that appear in the
    // references used by hints. Errors may occur when parsing references not used by hints.
    // When this happens, this default ValueAddress is returned to make explicit that the value was not
    // parsed correctly.
    // In case an incorrectly parsed reference is used by a hint, an error will be raised (IdNotFound) in the
    // get_address_from_reference function call to notify this, and the parsing functionality should be
    // extended to contemplate this new case.
    pub fn no_hint_reference_default() -> ValueAddress {
        ValueAddress {
            offset1: OffsetValue::Value(99),
            offset2: OffsetValue::Value(99),
            dereference: false,
            value_type: String::from("felt"),
        }
    }
}

struct Felt252Visitor;

impl<'de> de::Visitor<'de> for Felt252Visitor {
    type Value = Felt252;

    fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
        formatter.write_str("Could not deserialize hexadecimal string")
    }

    fn visit_str<E>(self, value: &str) -> Result<Self::Value, E>
    where
        E: de::Error,
    {
        // Strip the '0x' prefix from the encoded hex string
        if let Some(no_prefix_hex) = value.strip_prefix("0x") {
            // Add padding if necessary
            let no_prefix_hex = deserialize_utils::maybe_add_padding(no_prefix_hex.to_string());
            Ok(Felt252::from_str_radix(&no_prefix_hex, 16).map_err(de::Error::custom)?)
        } else {
            Err(String::from("hex prefix error")).map_err(de::Error::custom)
        }
    }
}

struct MaybeRelocatableVisitor;

impl<'de> de::Visitor<'de> for MaybeRelocatableVisitor {
    type Value = Vec<MaybeRelocatable>;

    fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
        formatter.write_str("Could not deserialize array of hexadecimal")
    }

    fn visit_seq<A>(self, mut seq: A) -> Result<Self::Value, A::Error>
    where
        A: SeqAccess<'de>,
    {
        let mut data: Vec<MaybeRelocatable> = vec![];

        while let Some(value) = seq.next_element::<String>()? {
            if let Some(no_prefix_hex) = value.strip_prefix("0x") {
                // Add padding if necessary
                let no_prefix_hex = deserialize_utils::maybe_add_padding(no_prefix_hex.to_string());
                data.push(MaybeRelocatable::Int(
                    Felt252::from_str_radix(&no_prefix_hex, 16).map_err(de::Error::custom)?,
                ));
            } else {
                return Err(String::from("hex prefix error")).map_err(de::Error::custom);
            };
        }
        Ok(data)
    }
}

struct ReferenceIdsVisitor;

impl<'de> de::Visitor<'de> for ReferenceIdsVisitor {
    type Value = HashMap<String, usize>;

    fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
        formatter.write_str("a map with string keys and integer values")
    }

    fn visit_map<A>(self, mut map: A) -> Result<Self::Value, A::Error>
    where
        A: MapAccess<'de>,
    {
        let mut data: HashMap<String, usize> = HashMap::new();

        while let Some((key, value)) = map.next_entry::<String, usize>()? {
            data.insert(key, value);
        }

        Ok(data)
    }
}

struct ValueAddressVisitor;

impl<'de> de::Visitor<'de> for ValueAddressVisitor {
    type Value = ValueAddress;

    fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
        formatter.write_str("a string representing the address in memory of a variable")
    }

    fn visit_str<E>(self, value: &str) -> Result<Self::Value, E>
    where
        E: de::Error,
    {
        let parse_res = deserialize_utils::parse_value(value);

        if let Ok((_, res)) = parse_res {
            return Ok(res);
        }

        Ok(ValueAddress::no_hint_reference_default())
    }
}

pub fn deserialize_felt_hex<'de, D: Deserializer<'de>>(d: D) -> Result<Felt252, D::Error> {
    d.deserialize_str(Felt252Visitor)
}

pub fn deserialize_array_of_bigint_hex<'de, D: Deserializer<'de>>(
    d: D,
) -> Result<Vec<MaybeRelocatable>, D::Error> {
    d.deserialize_seq(MaybeRelocatableVisitor)
}

pub fn deserialize_map_to_string_and_usize_hashmap<'de, D: Deserializer<'de>>(
    d: D,
) -> Result<HashMap<String, usize>, D::Error> {
    d.deserialize_map(ReferenceIdsVisitor)
}

pub fn deserialize_value_address<'de, D: Deserializer<'de>>(
    d: D,
) -> Result<ValueAddress, D::Error> {
    d.deserialize_str(ValueAddressVisitor)
}

pub fn deserialize_program_json(reader: &[u8]) -> Result<ProgramJson, ProgramError> {
    let program_json = serde_json::from_slice(reader)?;
    Ok(program_json)
}
pub fn deserialize_and_parse_program(
    reader: &[u8],
    entrypoint: Option<&str>,
) -> Result<Program, ProgramError> {
    let program_json: ProgramJson = deserialize_program_json(reader)?;
    parse_program_json(program_json, entrypoint)
}

pub fn parse_program_json(
    program_json: ProgramJson,
    entrypoint: Option<&str>,
) -> Result<Program, ProgramError> {
    if PRIME_STR != program_json.prime {
        return Err(ProgramError::PrimeDiffers(program_json.prime));
    }

    let entrypoint_pc = match entrypoint {
        Some(entrypoint) => match program_json
            .identifiers
            .get(&format!("__main__.{entrypoint}"))
        {
            Some(entrypoint_identifier) => entrypoint_identifier.pc,
            None => return Err(ProgramError::EntrypointNotFound(entrypoint.to_string())),
        },
        None => None,
    };

    let start = match program_json.identifiers.get("__main__.__start__") {
        Some(identifier) => identifier.pc,
        None => None,
    };
    let end = match program_json.identifiers.get("__main__.__end__") {
        Some(identifier) => identifier.pc,
        None => None,
    };

    let mut constants = HashMap::new();
    for (key, value) in program_json.identifiers.iter() {
        if value.type_.as_deref() == Some("const") {
            let value = value
                .value
                .clone()
                .ok_or_else(|| ProgramError::ConstWithoutValue(key.clone()))?;
            constants.insert(key.clone(), value);
        }
    }

    let hints_collection = HintsCollection::new(&program_json.hints, program_json.data.len())?;

    let shared_program_data = SharedProgramData {
        data: program_json.data,
        hints_collection,
        main: entrypoint_pc,
        start,
        end,
        error_message_attributes: program_json
            .attributes
            .into_iter()
            .filter(|attr| attr.name == "error_message")
            .collect(),
        instruction_locations: program_json
            .debug_info
            .map(|debug_info| debug_info.instruction_locations),
        identifiers: program_json.identifiers,
        constants,
        reference_manager: Program::get_reference_list(&program_json.reference_manager),
    };
    Ok(Program {
        shared_program_data: Arc::new(shared_program_data),
        builtins: program_json.builtins,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use assert_matches::assert_matches;
    use core::num::NonZeroUsize;
    use felt::felt_str;
    use num_traits::One;
    use num_traits::Zero;

    #[cfg(target_arch = "wasm32")]
    use wasm_bindgen_test::*;

    #[test]
    #[cfg_attr(target_arch = "wasm32", wasm_bindgen_test)]
    fn deserialize_bigint_from_string_json_gives_error() {
        let invalid_even_length_hex_json = r#"
            {
                "prime": "0bx000A"
            }"#;

        // ProgramJson result instance for the json with an even length encoded hex.
        let even_result: Result<ProgramJson, _> =
            serde_json::from_str(invalid_even_length_hex_json);

        assert!(even_result.is_err());

        let invalid_odd_length_hex_json = r#"
            {
                "prime": "0bx00A"
            }"#;

        // ProgramJson result instance for the json with an odd length encoded hex.
        let odd_result: Result<ProgramJson, _> = serde_json::from_str(invalid_odd_length_hex_json);

        assert!(odd_result.is_err());
    }

    #[test]
    #[cfg_attr(target_arch = "wasm32", wasm_bindgen_test)]
    fn deserialize_bigint_invalid_char_error() {
        let invalid_char = r#"
            {
                "prime": "0xlambda"
            }"#;

        let invalid_char_error: Result<ProgramJson, _> = serde_json::from_str(invalid_char);

        assert!(invalid_char_error.is_err());
    }

    #[test]
    #[cfg_attr(target_arch = "wasm32", wasm_bindgen_test)]
    fn deserialize_bigint_no_prefix_error() {
        let no_prefix = r#"
            {
                "prime": "00A"
            }"#;

        // ProgramJson result instance for the json with an odd length encoded hex.
        let no_prefix_error: Result<ProgramJson, _> = serde_json::from_str(no_prefix);

        assert!(no_prefix_error.is_err());
    }

    #[test]
    #[cfg_attr(target_arch = "wasm32", wasm_bindgen_test)]
    fn deserialize_from_string_json() {
        let valid_json = r#"
            {
                "prime": "0x800000000000011000000000000000000000000000000000000000000000001",
                "attributes": [],
                "debug_info": {
                    "instruction_locations": {}
                }, 
                "builtins": [],
                "data": [
                    "0x480680017fff8000",
                    "0x3e8",
                    "0x480680017fff8000",
                    "0x7d0",
                    "0x48307fff7ffe8000",
                    "0x208b7fff7fff7ffe"
                ],
                "identifiers": {
                    "__main__.main": {
                        "decorators": [],
                        "pc": 0,
                        "type": "function"
                    },
                    "__main__.main.Args": {
                        "full_name": "__main__.main.Args",
                        "members": {},
                        "size": 0,
                        "type": "struct"
                    },
                    "__main__.main.ImplicitArgs": {
                        "full_name": "__main__.main.ImplicitArgs",
                        "members": {},
                        "size": 0,
                        "type": "struct"
                    }
                },
                "hints": {
                    "0": [
                        {
                            "accessible_scopes": [
                                "starkware.cairo.common.alloc",
                                "starkware.cairo.common.alloc.alloc"
                            ],
                            "code": "memory[ap] = segments.add()",
                            "flow_tracking_data": {
                                "ap_tracking": {
                                    "group": 0,
                                    "offset": 0
                                },
                                "reference_ids": {
                                    "starkware.cairo.common.math.split_felt.high": 0,
                                    "starkware.cairo.common.math.split_felt.low": 14,
                                    "starkware.cairo.common.math.split_felt.range_check_ptr": 16,
                                    "starkware.cairo.common.math.split_felt.value": 12
                                }
                            }
                        }
                    ]
                },
                "reference_manager": {
                    "references": [
                        {
                            "ap_tracking_data": {
                                "group": 0,
                                "offset": 0
                            },
                            "pc": 0,
                            "value": "[cast(fp + (-4), felt*)]"
                        },
                        {
                            "ap_tracking_data": {
                                "group": 0,
                                "offset": 0
                            },
                            "pc": 0,
                            "value": "[cast(fp + (-3), felt*)]"
                        },
                        {
                            "ap_tracking_data": {
                                "group": 0,
                                "offset": 0
                            },
                            "pc": 0,
                            "value": "cast([fp + (-3)] + 2, felt)"
                        },
                        {
                            "ap_tracking_data": {
                                "group": 0,
                                "offset": 0
                            },
                            "pc": 0,
                            "value": "[cast(fp, felt**)]"
                        }
                    ]
                }
            }"#;

        // ProgramJson instance for the json with an even length encoded hex.
        let program_json: ProgramJson = serde_json::from_str(valid_json).unwrap();

        let data: Vec<MaybeRelocatable> = vec![
            MaybeRelocatable::Int(Felt252::new(5189976364521848832_i64)),
            MaybeRelocatable::Int(Felt252::new(1000_i64)),
            MaybeRelocatable::Int(Felt252::new(5189976364521848832_i64)),
            MaybeRelocatable::Int(Felt252::new(2000_i64)),
            MaybeRelocatable::Int(Felt252::new(5201798304953696256_i64)),
            MaybeRelocatable::Int(Felt252::new(2345108766317314046_i64)),
        ];

        let mut hints = BTreeMap::new();
        hints.insert(
            0,
            vec![HintParams {
                code: "memory[ap] = segments.add()".to_string(),
                accessible_scopes: vec![
                    String::from("starkware.cairo.common.alloc"),
                    String::from("starkware.cairo.common.alloc.alloc"),
                ],
                flow_tracking_data: FlowTrackingData {
                    ap_tracking: ApTracking {
                        group: 0,
                        offset: 0,
                    },
                    reference_ids: HashMap::from([
                        (
                            String::from("starkware.cairo.common.math.split_felt.high"),
                            0,
                        ),
                        (
                            String::from("starkware.cairo.common.math.split_felt.low"),
                            14,
                        ),
                        (
                            String::from("starkware.cairo.common.math.split_felt.range_check_ptr"),
                            16,
                        ),
                        (
                            String::from("starkware.cairo.common.math.split_felt.value"),
                            12,
                        ),
                    ]),
                },
            }],
        );

        let reference_manager = ReferenceManager {
            references: vec![
                Reference {
                    ap_tracking_data: ApTracking {
                        group: 0,
                        offset: 0,
                    },
                    pc: Some(0),
                    value_address: ValueAddress {
                        offset1: OffsetValue::Reference(Register::FP, -4, false),
                        offset2: OffsetValue::Value(0),
                        dereference: true,
                        value_type: "felt".to_string(),
                    },
                },
                Reference {
                    ap_tracking_data: ApTracking {
                        group: 0,
                        offset: 0,
                    },
                    pc: Some(0),
                    value_address: ValueAddress {
                        offset1: OffsetValue::Reference(Register::FP, -3, false),
                        offset2: OffsetValue::Value(0),
                        dereference: true,
                        value_type: "felt".to_string(),
                    },
                },
                Reference {
                    ap_tracking_data: ApTracking {
                        group: 0,
                        offset: 0,
                    },
                    pc: Some(0),
                    value_address: ValueAddress {
                        offset1: OffsetValue::Reference(Register::FP, -3, true),
                        offset2: OffsetValue::Immediate(Felt252::new(2)),
                        dereference: false,
                        value_type: "felt".to_string(),
                    },
                },
                Reference {
                    ap_tracking_data: ApTracking {
                        group: 0,
                        offset: 0,
                    },
                    pc: Some(0),
                    value_address: ValueAddress {
                        offset1: OffsetValue::Reference(Register::FP, 0, false),
                        offset2: OffsetValue::Value(0),
                        dereference: true,
                        value_type: "felt*".to_string(),
                    },
                },
            ],
        };

        assert_eq!(
            program_json.prime,
            "0x800000000000011000000000000000000000000000000000000000000000001"
        );
        assert!(program_json.builtins.is_empty());
        assert_eq!(program_json.data, data);
        assert_eq!(program_json.identifiers["__main__.main"].pc, Some(0));
        assert_eq!(program_json.hints, hints);
        assert_eq!(program_json.reference_manager, reference_manager);
    }

    #[test]
    #[cfg_attr(target_arch = "wasm32", wasm_bindgen_test)]
    fn deserialize_program_json_from_json_file_a() {
        // Open json file with (valid) even length encoded hex
        let reader =
            include_bytes!("../../../cairo_programs/manually_compiled/valid_program_a.json");

        let program_json: ProgramJson = serde_json::from_slice(reader).unwrap();

        assert_eq!(
            program_json.prime,
            "0x800000000000011000000000000000000000000000000000000000000000001"
        );
        assert!(program_json.builtins.is_empty());
        assert_eq!(program_json.data.len(), 6);
        assert_eq!(program_json.identifiers["__main__.main"].pc, Some(0));
    }

    #[test]
    #[cfg_attr(target_arch = "wasm32", wasm_bindgen_test)]
    fn deserialize_program_json_from_json_file_b() {
        // Open json file with (valid) odd length encoded hex
        let reader =
            include_bytes!("../../../cairo_programs/manually_compiled/valid_program_b.json");

        let program_json: ProgramJson = serde_json::from_slice(reader).unwrap();
        let builtins: Vec<BuiltinName> = vec![BuiltinName::output, BuiltinName::range_check];

        assert_eq!(
            program_json.prime,
            "0x800000000000011000000000000000000000000000000000000000000000001"
        );
        assert_eq!(program_json.builtins, builtins);
        assert_eq!(program_json.data.len(), 24);
        assert_eq!(program_json.identifiers["__main__.main"].pc, Some(13));
    }

    #[test]
    #[cfg_attr(target_arch = "wasm32", wasm_bindgen_test)]
    fn deserialize_program_json_from_json_file_gives_error() {
        // Open json file with (invalid) even length encoded hex
        let reader = include_bytes!(
            "../../../cairo_programs/manually_compiled/invalid_even_length_hex.json"
        );

        let even_result: Result<ProgramJson, _> = serde_json::from_slice(reader);

        assert!(even_result.is_err());

        // Open json file with (invalid) odd length encoded hex
        let reader =
            include_bytes!("../../../cairo_programs/manually_compiled/invalid_odd_length_hex.json");

        let odd_result: Result<ProgramJson, _> = serde_json::from_slice(reader);

        assert!(odd_result.is_err());
    }

    #[test]
    #[cfg_attr(target_arch = "wasm32", wasm_bindgen_test)]
    fn deserialize_missing_entrypoint_gives_error() {
        let reader =
            include_bytes!("../../../cairo_programs/manually_compiled/valid_program_a.json");

        let deserialization_result =
            deserialize_and_parse_program(reader, Some("missing_function"));
        assert!(deserialization_result.is_err());
        assert_matches!(
            deserialization_result,
            Err(ProgramError::EntrypointNotFound(_))
        );
    }

    fn get_hints_as_map(program: &Program) -> HashMap<usize, Vec<HintParams>> {
        let hints_collection = &program.shared_program_data.hints_collection;
        let hints_map: HashMap<usize, Vec<HintParams>> = hints_collection
            .iter()
            .map(|(pc, hints)| (pc, hints.to_vec()))
            .collect();

        hints_map
    }

    #[test]
    #[cfg_attr(target_arch = "wasm32", wasm_bindgen_test)]
    fn deserialize_program_test() {
        let reader =
            include_bytes!("../../../cairo_programs/manually_compiled/valid_program_a.json");

        let program: Program = deserialize_and_parse_program(reader, Some("main"))
            .expect("Failed to deserialize program");

        let builtins: Vec<BuiltinName> = Vec::new();
        let data: Vec<MaybeRelocatable> = vec![
            MaybeRelocatable::Int(Felt252::new(5189976364521848832_i64)),
            MaybeRelocatable::Int(Felt252::new(1000)),
            MaybeRelocatable::Int(Felt252::new(5189976364521848832_i64)),
            MaybeRelocatable::Int(Felt252::new(2000)),
            MaybeRelocatable::Int(Felt252::new(5201798304953696256_i64)),
            MaybeRelocatable::Int(Felt252::new(2345108766317314046_i64)),
        ];

        let hints: HashMap<_, _> = [
            (
                0,
                vec![HintParams {
                    code: "memory[ap] = segments.add()".to_string(),
                    accessible_scopes: vec![
                        String::from("starkware.cairo.common.alloc"),
                        String::from("starkware.cairo.common.alloc.alloc"),
                    ],
                    flow_tracking_data: FlowTrackingData {
                        ap_tracking: ApTracking {
                            group: 0,
                            offset: 0,
                        },
                        reference_ids: HashMap::new(),
                    },
                }],
            ),
            (
                4,
                vec![HintParams {
                    code: "import math".to_string(),
                    accessible_scopes: vec![
                        String::from("__main__"),
                        String::from("__main__.main"),
                    ],
                    flow_tracking_data: FlowTrackingData {
                        ap_tracking: ApTracking {
                            group: 5,
                            offset: 0,
                        },
                        reference_ids: HashMap::new(),
                    },
                }],
            ),
        ]
        .into();
        let mut hints_ranges = vec![None; 47];
        hints_ranges[0] = Some((0, NonZeroUsize::new(1).unwrap()));
        hints_ranges[46] = Some((1, NonZeroUsize::new(1).unwrap()));

        assert_eq!(program.builtins, builtins);
        assert_eq!(program.shared_program_data.data, data);
        assert_eq!(program.shared_program_data.main, Some(0));

        let program_hints = get_hints_as_map(&program);
        assert_eq!(program_hints, hints);
    }

    /// Deserialize a program without an entrypoint.
    #[test]
    #[cfg_attr(target_arch = "wasm32", wasm_bindgen_test)]
    fn deserialize_program_without_entrypoint() {
        let reader =
            include_bytes!("../../../cairo_programs/manually_compiled/valid_program_a.json");

        let program: Program =
            deserialize_and_parse_program(reader, None).expect("Failed to deserialize program");

        let builtins: Vec<BuiltinName> = Vec::new();
        let data: Vec<MaybeRelocatable> = vec![
            MaybeRelocatable::Int(Felt252::new(5189976364521848832_i64)),
            MaybeRelocatable::Int(Felt252::new(1000)),
            MaybeRelocatable::Int(Felt252::new(5189976364521848832_i64)),
            MaybeRelocatable::Int(Felt252::new(2000)),
            MaybeRelocatable::Int(Felt252::new(5201798304953696256_i64)),
            MaybeRelocatable::Int(Felt252::new(2345108766317314046_i64)),
        ];

        let hints: HashMap<_, _> = [
            (
                0,
                vec![HintParams {
                    code: "memory[ap] = segments.add()".to_string(),
                    accessible_scopes: vec![
                        String::from("starkware.cairo.common.alloc"),
                        String::from("starkware.cairo.common.alloc.alloc"),
                    ],
                    flow_tracking_data: FlowTrackingData {
                        ap_tracking: ApTracking {
                            group: 0,
                            offset: 0,
                        },
                        reference_ids: HashMap::new(),
                    },
                }],
            ),
            (
                4,
                vec![HintParams {
                    code: "import math".to_string(),
                    accessible_scopes: vec![
                        String::from("__main__"),
                        String::from("__main__.main"),
                    ],
                    flow_tracking_data: FlowTrackingData {
                        ap_tracking: ApTracking {
                            group: 5,
                            offset: 0,
                        },
                        reference_ids: HashMap::new(),
                    },
                }],
            ),
        ]
        .into();

        assert_eq!(program.builtins, builtins);
        assert_eq!(program.shared_program_data.data, data);
        assert_eq!(program.shared_program_data.main, None);

        let program_hints = get_hints_as_map(&program);
        assert_eq!(program_hints, hints);
    }

    #[test]
    #[cfg_attr(target_arch = "wasm32", wasm_bindgen_test)]
    fn deserialize_constant() {
        let reader = include_bytes!(
            "../../../cairo_programs/manually_compiled/deserialize_constant_test.json"
        );

        let program_json: ProgramJson = serde_json::from_slice(reader).unwrap();
        let mut identifiers: HashMap<String, Identifier> = HashMap::new();

        identifiers.insert(
            String::from("__main__.main"),
            Identifier {
                pc: Some(0),
                type_: Some(String::from("function")),
                value: None,
                full_name: None,
                members: None,
                cairo_type: None,
            },
        );
        identifiers.insert(
            String::from("__main__.compare_abs_arrays.SIZEOF_LOCALS"),
            Identifier {
                pc: None,
                type_: Some(String::from("const")),
                value: Some(felt_str!(
                    "-3618502788666131213697322783095070105623107215331596699973092056135872020481"
                )),
                full_name: None,
                members: None,
                cairo_type: None,
            },
        );
        identifiers.insert(
            String::from("starkware.cairo.common.cairo_keccak.keccak.unsigned_div_rem"),
            Identifier {
                pc: None,
                type_: Some(String::from("alias")),
                value: None,
                full_name: None,
                members: None,
                cairo_type: None,
            },
        );
        identifiers.insert(
            String::from("starkware.cairo.common.cairo_keccak.packed_keccak.ALL_ONES"),
            Identifier {
                pc: None,
                type_: Some(String::from("const")),
                value: Some(felt_str!(
                    "-106710729501573572985208420194530329073740042555888586719234"
                )),
                full_name: None,
                members: None,
                cairo_type: None,
            },
        );
        identifiers.insert(
            String::from("starkware.cairo.common.cairo_keccak.packed_keccak.BLOCK_SIZE"),
            Identifier {
                pc: None,
                type_: Some(String::from("const")),
                value: Some(Felt252::new(3)),
                full_name: None,
                members: None,
                cairo_type: None,
            },
        );
        identifiers.insert(
            String::from("starkware.cairo.common.alloc.alloc.SIZEOF_LOCALS"),
            Identifier {
                pc: None,
                type_: Some(String::from("const")),
                value: Some(Felt252::zero()),
                full_name: None,
                members: None,
                cairo_type: None,
            },
        );
        identifiers.insert(
            String::from("starkware.cairo.common.uint256.SHIFT"),
            Identifier {
                pc: None,
                type_: Some(String::from("const")),
                value: Some(felt_str!("340282366920938463463374607431768211456")),
                full_name: None,
                members: None,
                cairo_type: None,
            },
        );

        assert_eq!(program_json.identifiers, identifiers);
    }

    #[test]
    #[cfg_attr(target_arch = "wasm32", wasm_bindgen_test)]
    fn value_address_no_hint_reference_default_test() {
        let valid_json = r#"
            {
                "prime": "0x800000000000011000000000000000000000000000000000000000000000001",
                "attributes": [],
                "debug_info": {
                    "instruction_locations": {}
                },  
                "builtins": [],
                "data": [
                ],
                "identifiers": {
                },
                "hints": {
                },
                "reference_manager": {
                    "references": [
                        {
                            "ap_tracking_data": {
                                "group": 0,
                                "offset": 0
                            },
                            "pc": 0,
                            "value": ""
                        }
                    ]
                }
            }"#;

        let program_json: ProgramJson = serde_json::from_str(valid_json).unwrap();

        let reference_manager = ReferenceManager {
            references: vec![Reference {
                ap_tracking_data: ApTracking {
                    group: 0,
                    offset: 0,
                },
                pc: Some(0),
                value_address: ValueAddress::no_hint_reference_default(),
            }],
        };

        assert_eq!(program_json.reference_manager, reference_manager);
    }

    #[test]
    #[cfg_attr(target_arch = "wasm32", wasm_bindgen_test)]
    fn deserialize_attributes_test() {
        let valid_json = r#"
            {
                "prime": "0x800000000000011000000000000000000000000000000000000000000000001",
                "attributes": [
                    {
                        "accessible_scopes": [
                            "openzeppelin.security.safemath.library",
                            "openzeppelin.security.safemath.library.SafeUint256",
                            "openzeppelin.security.safemath.library.SafeUint256.add"
                        ],
                        "end_pc": 381,
                        "flow_tracking_data": {
                            "ap_tracking": {
                                "group": 14,
                                "offset": 35
                            },
                            "reference_ids": {}
                        },
                        "name": "error_message",
                        "start_pc": 379,
                        "value": "SafeUint256: addition overflow"
                    },
                    {
                        "accessible_scopes": [
                            "openzeppelin.security.safemath.library",
                            "openzeppelin.security.safemath.library.SafeUint256",
                            "openzeppelin.security.safemath.library.SafeUint256.sub_le"
                        ],
                        "end_pc": 404,
                        "flow_tracking_data": {
                            "ap_tracking": {
                                "group": 15,
                                "offset": 60
                            },
                            "reference_ids": {}
                        },
                        "name": "error_message",
                        "start_pc": 402,
                        "value": "SafeUint256: subtraction overflow"
                    }
                ], 
                "debug_info": {
                    "instruction_locations": {}
                },           
                "builtins": [],
                "data": [
                ],
                "identifiers": {
                },
                "hints": {
                },
                "reference_manager": {
                    "references": [
                    ]
                }
            }"#;

        let program_json: ProgramJson = serde_json::from_str(valid_json).unwrap();

        let attributes: Vec<Attribute> = vec![
            Attribute {
                name: String::from("error_message"),
                start_pc: 379,
                end_pc: 381,
                value: String::from("SafeUint256: addition overflow"),
                flow_tracking_data: Some(FlowTrackingData {
                    ap_tracking: ApTracking {
                        group: 14,
                        offset: 35,
                    },
                    reference_ids: HashMap::new(),
                }),
            },
            Attribute {
                name: String::from("error_message"),
                start_pc: 402,
                end_pc: 404,
                value: String::from("SafeUint256: subtraction overflow"),
                flow_tracking_data: Some(FlowTrackingData {
                    ap_tracking: ApTracking {
                        group: 15,
                        offset: 60,
                    },
                    reference_ids: HashMap::new(),
                }),
            },
        ];

        assert_eq!(program_json.attributes, attributes);
    }

    #[test]
    #[cfg_attr(target_arch = "wasm32", wasm_bindgen_test)]
    fn deserialize_instruction_locations_test_no_parent() {
        let valid_json = r#"
            {
                "prime": "0x800000000000011000000000000000000000000000000000000000000000001",
                "attributes": [], 
                "debug_info": {
                    "file_contents": {},
                    "instruction_locations": {
                        "0": {
                            "accessible_scopes": [
                                "starkware.cairo.lang.compiler.lib.registers",
                                "starkware.cairo.lang.compiler.lib.registers.get_fp_and_pc"
                            ],
                            "flow_tracking_data": {
                                "ap_tracking": {
                                    "group": 0,
                                    "offset": 0
                                },
                                "reference_ids": {}
                            },
                            "hints": [],
                            "inst": {
                                "end_col": 73,
                                "end_line": 7,
                                "input_file": {
                                    "filename": "/Users/user/test/env/lib/python3.9/site-packages/starkware/cairo/lang/compiler/lib/registers.cairo"
                                },
                                "start_col": 5,
                                "start_line": 7
                            }
                        },
                        "3": {
                            "accessible_scopes": [
                                "starkware.cairo.common.alloc",
                                "starkware.cairo.common.alloc.alloc"
                            ],
                            "flow_tracking_data": {
                                "ap_tracking": {
                                    "group": 1,
                                    "offset": 1
                                },
                                "reference_ids": {}
                            },
                            "hints": [],
                            "inst": {
                                "end_col": 40,
                                "end_line": 5,
                                "input_file": {
                                    "filename": "/Users/user/test/env/lib/python3.9/site-packages/starkware/cairo/common/alloc.cairo"
                                },
                                "start_col": 5,
                                "start_line": 5
                            }
                        }
                    }
                },          
                "builtins": [],
                "data": [
                ],
                "identifiers": {
                },
                "hints": {
                },
                "reference_manager": {
                    "references": [
                    ]
                }
            }"#;

        let program_json: ProgramJson = serde_json::from_str(valid_json).unwrap();

        let debug_info: DebugInfo = DebugInfo {
            instruction_locations: HashMap::from([
                (
                    0,
                    InstructionLocation {
                        inst: Location {
                            end_line: 7,
                            end_col: 73,
                            input_file: InputFile { filename: String::from("/Users/user/test/env/lib/python3.9/site-packages/starkware/cairo/lang/compiler/lib/registers.cairo") },
                            parent_location: None,
                            start_line: 7,
                            start_col: 5,
                        },
                        hints: vec![],
                    },
                ),
                (
                    3,
                    InstructionLocation {
                        inst: Location {
                            end_line: 5,
                            end_col: 40,
                            input_file: InputFile { filename: String::from("/Users/user/test/env/lib/python3.9/site-packages/starkware/cairo/common/alloc.cairo") },
                            parent_location: None,
                            start_line: 5,
                            start_col: 5,
                        },
                        hints: vec![],
                    },
                ),
            ]),
        };

        assert_eq!(program_json.debug_info, Some(debug_info));
    }

    #[test]
    #[cfg_attr(target_arch = "wasm32", wasm_bindgen_test)]
    fn deserialize_instruction_locations_test_with_parent() {
        let valid_json = r#"
            {
                "prime": "0x800000000000011000000000000000000000000000000000000000000000001",
                "attributes": [], 
                "debug_info": {
                    "file_contents": {},
                    "instruction_locations": {
                        "4": {
                            "accessible_scopes": [
                                "__main__",
                                "__main__",
                                "__main__.constructor"
                            ],
                            "flow_tracking_data": null,
                            "hints": [],
                            "inst": {
                                "end_col": 36,
                                "end_line": 9,
                                "input_file": {
                                    "filename": "test/contracts/cairo/always_fail.cairo"
                                },
                                "parent_location": [
                                    {
                                        "end_col": 36,
                                        "end_line": 9,
                                        "input_file": {
                                            "filename": "test/contracts/cairo/always_fail.cairo"
                                        },
                                        "parent_location": [
                                            {
                                                "end_col": 15,
                                                "end_line": 11,
                                                "input_file": {
                                                    "filename": "test/contracts/cairo/always_fail.cairo"
                                                },
                                                "start_col": 5,
                                                "start_line": 11
                                            },
                                            "While trying to retrieve the implicit argument 'syscall_ptr' in:"
                                        ],
                                        "start_col": 18,
                                        "start_line": 9
                                    },
                                    "While expanding the reference 'syscall_ptr' in:"
                                ],
                                "start_col": 18,
                                "start_line": 9
                            }
                        }
                    }
                },          
                "builtins": [],
                "data": [
                ],
                "identifiers": {
                },
                "hints": {
                },
                "reference_manager": {
                    "references": [
                    ]
                }
            }"#;

        let program_json: ProgramJson = serde_json::from_str(valid_json).unwrap();

        let debug_info: DebugInfo = DebugInfo { instruction_locations: HashMap::from(
            [
                (4, InstructionLocation {
                    inst: Location { end_line: 9, end_col: 36,input_file: InputFile { filename: String::from("test/contracts/cairo/always_fail.cairo") }, parent_location: Some(
                        (Box::new(Location {
                            end_line: 9,
                            end_col: 36,
                            input_file: InputFile { filename: String::from("test/contracts/cairo/always_fail.cairo") },
                            parent_location: Some(
                                (   Box::new(Location {
                                    end_line: 11,
                                    end_col: 15,
                                    input_file: InputFile { filename: String::from("test/contracts/cairo/always_fail.cairo") },
                                    parent_location: None,
                                    start_line: 11,
                                    start_col: 5,
                                })
                                    , String::from("While trying to retrieve the implicit argument 'syscall_ptr' in:")
                                )
                            ),
                            start_line: 9,
                            start_col: 18,
                        }), String::from( "While expanding the reference 'syscall_ptr' in:"))
                    ), start_line: 9, start_col: 18 },
                    hints: vec![],
                }),
            ]
        ) };

        assert_eq!(program_json.debug_info, Some(debug_info));
    }

    #[test]
    #[cfg_attr(target_arch = "wasm32", wasm_bindgen_test)]
    fn deserialize_program_with_type_definition() {
        let reader = include_bytes!("../../../cairo_programs/uint256_integration_tests.json");

        let program_json: ProgramJson = serde_json::from_slice(reader).unwrap();

        assert_eq!(
            program_json.identifiers["starkware.cairo.common.alloc.alloc.Return"]
                .cairo_type
                .as_ref()
                .expect("key not found"),
            "(ptr: felt*)"
        );
        assert_eq!(
            program_json.identifiers["starkware.cairo.common.uint256.uint256_add.Return"]
                .cairo_type
                .as_ref()
                .expect("key not found"),
            "(res: starkware.cairo.common.uint256.Uint256, carry: felt)"
        );
        assert_eq!(
            program_json.identifiers["__main__.test_unsigned_div_rem.Return"]
                .cairo_type
                .as_ref()
                .expect("key not found"),
            "()"
        );
    }

    #[test]
    #[cfg_attr(target_arch = "wasm32", wasm_bindgen_test)]
    fn deserialize_nonbase10_number_errors() {
        let valid_json = r#"
        {
            "value" : 0x123
        }"#;

        let iden: Result<Identifier, serde_json::Error> = serde_json::from_str(valid_json);
        assert!(iden.err().is_some());
    }

    #[test]
    #[cfg_attr(target_arch = "wasm32", wasm_bindgen_test)]
    fn test_felt_from_number_with_scientific_notation() {
        let n = Number::deserialize(serde_json::Value::from(1000000000000000000000000000_u128))
            .unwrap();
        assert_eq!(n.to_string(), "1e27".to_owned());

        assert_matches!(
            felt_from_number(n),
            Ok(x) if x == Some(Felt252::one() * Felt252::from(10).pow(27))
        );
    }

    #[test]
    #[cfg_attr(target_arch = "wasm32", wasm_bindgen_test)]
    fn test_felt_from_number_with_scientific_notation_with_fractional_part() {
        let n = serde_json::Value::Number(Number::from_f64(64e+74).unwrap());

        assert_matches!(
            felt_from_number(n),
            Ok(x) if x == Some(Felt252::from_str_radix("64", 10).unwrap() * Felt252::from(10).pow(74))
        );
    }

    #[test]
    #[cfg_attr(target_arch = "wasm32", wasm_bindgen_test)]
    fn test_felt_from_number_with_scientific_notation_with_fractional_part_f64_max() {
        let n = serde_json::Value::Number(Number::from_f64(f64::MAX).unwrap());
        assert_eq!(
            felt_from_number(n).unwrap(),
            Some(
                Felt252::from_str_radix(
                    "2082797363194934431336897723140298717588791783575467744530053896730196177808",
                    10
                )
                .unwrap()
            )
        );
    }

    #[test]
    #[cfg_attr(target_arch = "wasm32", wasm_bindgen_test)]
    fn test_felt_from_number_with_scientific_notation_big_exponent() {
        #[derive(Deserialize, Debug, PartialEq)]
        struct Test {
            #[serde(deserialize_with = "felt_from_number")]
            f: Option<Felt252>,
        }
        let malicious_input = &format!(
            "{{ \"f\": {}e{} }}",
            String::from_utf8(vec![b'9'; 1000]).unwrap(),
            u32::MAX
        );
        let f = serde_json::from_str::<Test>(malicious_input)
            .unwrap()
            .f
            .unwrap();
        assert_eq!(
            f,
            Felt252::from_str_radix(
                "2471602022505793130446032259107029522557827898253184929958153020344968292412",
                10
            )
            .unwrap()
        );
    }

    #[test]
    #[cfg_attr(target_arch = "wasm32", wasm_bindgen_test)]
    fn deserialize_program_with_invalid_hint_pc() {
        let reader = br#"{
            "attributes": [],
            "builtins": [],
            "compiler_version": "0.11.0",
            "data": [
                "0x41241"
            ],
            "debug_info": {
                "instruction_locations": {}
            },
            "hints": {
                "1": [
                    {
                        "accessible_scopes": [],
                        "code": "",
                        "flow_tracking_data": {
                            "ap_tracking": {
                                "group": 0,
                                "offset": 0
                            },
                            "reference_ids": {}
                        }
                    }
                ]
            },
            "identifiers": {
                "__main__.main": {}
            },
            "main_scope": "",
            "prime": "0x800000000000011000000000000000000000000000000000000000000000001",
            "reference_manager": {
                "references": []
            }
        }"#;

        let deserialization_result = deserialize_and_parse_program(reader, Some("main"));

        assert!(deserialization_result.is_err());
        assert_matches!(
            deserialization_result.unwrap_err(),
            ProgramError::InvalidHintPc(1, 1)
        );
    }
}
