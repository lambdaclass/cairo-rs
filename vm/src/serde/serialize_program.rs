use crate::{
    stdlib::{
        collections::{BTreeMap, HashMap},
        prelude::*,
    },
    types::builtin_name::BuiltinName,
};

use serde::{Deserialize, Serialize};

use super::deserialize_program::{
    ApTracking, Attribute, DebugInfo, FlowTrackingData, HintParams, Identifier, Member,
    ProgramJson, Reference, ReferenceManager, ValueAddress,
};
use crate::types::program::Program;
use crate::types::relocatable::MaybeRelocatable;
use crate::Felt252;

// This struct is used to Serialize and Deserialize a Program struct
// Their fields are equal to the ProgramJson
// but keeping the default Serialization and Deserialization traits implementation
#[derive(Serialize, Deserialize, Debug, PartialEq)]
pub(crate) struct ProgramSerializer {
    pub prime: String,
    pub builtins: Vec<BuiltinName>,
    pub data: Vec<MaybeRelocatable>,
    pub identifiers: HashMap<String, IdentifierSerializer>,
    pub hints: BTreeMap<usize, Vec<HintParamsSerializer>>,
    pub reference_manager: ReferenceManagerSerializer,
    pub attributes: Vec<Attribute>,
    pub debug_info: Option<DebugInfo>,
}

impl From<ProgramSerializer> for ProgramJson {
    fn from(program_json: ProgramSerializer) -> ProgramJson {
        let mut identifiers = HashMap::new();
        for (key, identifier) in program_json.identifiers.clone() {
            identifiers.insert(key, identifier.into());
        }

        let mut hints: BTreeMap<usize, Vec<HintParams>> = BTreeMap::new();
        for (key, hint_params_vec) in &program_json.hints {
            let mut new_hint_params_vec = Vec::new();
            for hint_param in hint_params_vec {
                new_hint_params_vec.push(hint_param.clone().into());
            }
            hints.insert(*key, new_hint_params_vec);
        }

        let mut reference_manager: ReferenceManager = ReferenceManager {
            references: Vec::new(),
        };

        for reference in &program_json.reference_manager.references {
            reference_manager.references.push(reference.clone().into());
        }
        ProgramJson {
            prime: program_json.prime,
            builtins: program_json.builtins,
            data: program_json.data,
            identifiers,
            hints,
            reference_manager,
            attributes: program_json.attributes,
            debug_info: program_json.debug_info,
        }
    }
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
pub struct HintParamsSerializer {
    pub code: String,
    pub accessible_scopes: Vec<String>,
    pub flow_tracking_data: FlowTrackingDataSerializer,
}
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
pub struct FlowTrackingDataSerializer {
    pub ap_tracking: ApTracking,
    pub reference_ids: HashMap<String, usize>,
}

impl From<FlowTrackingDataSerializer> for FlowTrackingData {
    fn from(flow_tracking_data_serialer: FlowTrackingDataSerializer) -> FlowTrackingData {
        FlowTrackingData {
            ap_tracking: flow_tracking_data_serialer.ap_tracking,
            reference_ids: flow_tracking_data_serialer.reference_ids,
        }
    }
}

impl From<FlowTrackingData> for FlowTrackingDataSerializer {
    fn from(flow_tracking_data_serialer: FlowTrackingData) -> FlowTrackingDataSerializer {
        FlowTrackingDataSerializer {
            ap_tracking: flow_tracking_data_serialer.ap_tracking,
            reference_ids: flow_tracking_data_serialer.reference_ids,
        }
    }
}

impl From<HintParamsSerializer> for HintParams {
    fn from(hint_params_serializer: HintParamsSerializer) -> HintParams {
        HintParams {
            code: hint_params_serializer.code,
            accessible_scopes: hint_params_serializer.accessible_scopes,
            flow_tracking_data: hint_params_serializer.flow_tracking_data.into(),
        }
    }
}

impl From<HintParams> for HintParamsSerializer {
    fn from(hint_params_serializer: HintParams) -> HintParamsSerializer {
        HintParamsSerializer {
            code: hint_params_serializer.code,
            accessible_scopes: hint_params_serializer.accessible_scopes,
            flow_tracking_data: hint_params_serializer.flow_tracking_data.into(),
        }
    }
}

#[derive(Serialize, Deserialize, Debug, PartialEq, Eq, Clone)]
pub(crate) struct IdentifierSerializer {
    pub pc: Option<usize>,
    pub type_: Option<String>,
    #[serde(default)]
    pub value: Option<Felt252>,

    pub full_name: Option<String>,
    pub members: Option<HashMap<String, Member>>,
    pub cairo_type: Option<String>,
    pub size: Option<usize>,
}

impl From<IdentifierSerializer> for Identifier {
    fn from(identifier_serialer: IdentifierSerializer) -> Identifier {
        Self {
            pc: identifier_serialer.pc,
            type_: identifier_serialer.type_,
            value: identifier_serialer.value,
            full_name: identifier_serialer.full_name,
            members: identifier_serialer.members,
            cairo_type: identifier_serialer.cairo_type,
            size: identifier_serialer.size,
        }
    }
}

impl From<Identifier> for IdentifierSerializer {
    fn from(identifier_serialer: Identifier) -> IdentifierSerializer {
        IdentifierSerializer {
            pc: identifier_serialer.pc,
            type_: identifier_serialer.type_,
            value: identifier_serialer.value,
            full_name: identifier_serialer.full_name,
            members: identifier_serialer.members,
            cairo_type: identifier_serialer.cairo_type,
            size: identifier_serialer.size,
        }
    }
}

#[derive(Serialize, Deserialize, Debug, PartialEq, Eq, Clone, Default)]
pub struct ReferenceManagerSerializer {
    pub references: Vec<ReferenceSerializer>,
}

#[derive(Serialize, Deserialize, Debug, PartialEq, Eq, Clone)]
pub struct ReferenceSerializer {
    pub ap_tracking_data: ApTracking,
    pub pc: Option<usize>,
    pub value_address: ValueAddress,
}

impl From<Reference> for ReferenceSerializer {
    fn from(reference: Reference) -> ReferenceSerializer {
        ReferenceSerializer {
            ap_tracking_data: reference.ap_tracking_data,
            pc: reference.pc,
            value_address: reference.value_address,
        }
    }
}

impl From<ReferenceSerializer> for Reference {
    fn from(reference: ReferenceSerializer) -> Reference {
        Reference {
            ap_tracking_data: reference.ap_tracking_data,
            pc: reference.pc,
            value_address: reference.value_address,
        }
    }
}

impl From<&Program> for ProgramSerializer {
    fn from(program: &Program) -> Self {
        let references = program
            .shared_program_data
            .reference_manager
            .clone()
            .into_iter()
            .map(|r| ReferenceSerializer {
                value_address: ValueAddress {
                    offset1: r.offset1,
                    offset2: r.offset2,
                    outer_dereference: r.outer_dereference,
                    inner_dereference: r.inner_dereference,
                    value_type: r.cairo_type.unwrap_or_default(),
                },
                ap_tracking_data: r.ap_tracking_data.unwrap_or_default(),
                pc: None,
            })
            .collect::<Vec<_>>();

        let mut identifiers = HashMap::new();
        for (key, identifier) in program.shared_program_data.identifiers.clone() {
            identifiers.insert(key, identifier.into());
        }

        let mut hints: BTreeMap<usize, Vec<HintParamsSerializer>> = BTreeMap::new();
        for (key, hint_params_vec) in BTreeMap::from(&program.shared_program_data.hints_collection)
        {
            let mut new_hints_params = Vec::new();
            for hint_params in hint_params_vec {
                new_hints_params.push(hint_params.clone().into());
            }
            hints.insert(key, new_hints_params);
        }

        ProgramSerializer {
            prime: program.prime().into(),
            builtins: program.builtins.clone(),
            data: program.shared_program_data.data.clone(),
            identifiers,
            hints,
            attributes: program.shared_program_data.error_message_attributes.clone(),
            debug_info: program
                .shared_program_data
                .instruction_locations
                .clone()
                .map(|instruction_locations| DebugInfo {
                    instruction_locations,
                }),
            reference_manager: ReferenceManagerSerializer { references },
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::serde::deserialize_program::parse_program_json;

    use super::*;

    #[cfg(target_arch = "wasm32")]
    use wasm_bindgen_test::*;

    #[test]
    #[cfg_attr(target_arch = "wasm32", wasm_bindgen_test)]
    fn program_json_from_program_test() {
        let programs_bytes: Vec<Vec<u8>> = [
            include_bytes!("../../../cairo_programs/keccak.json").to_vec(),
            include_bytes!("../../../cairo_programs/assert_nn.json").to_vec(),
            include_bytes!("../../../cairo_programs/bitwise_recursion.json").to_vec(),
            include_bytes!("../../../cairo_programs/blake2s_felts.json").to_vec(),
            include_bytes!("../../../cairo_programs/cairo_finalize_keccak_block_size_1000.json")
                .to_vec(),
            include_bytes!("../../../cairo_programs/bitwise_recursion.json").to_vec(),
            include_bytes!("../../../cairo_programs/keccak.json").to_vec(),
            include_bytes!("../../../cairo_programs/ec_double_slope.json").to_vec(),
            include_bytes!("../../../cairo_programs/example_blake2s.json").to_vec(),
            include_bytes!("../../../cairo_programs/fibonacci.json").to_vec(),
            include_bytes!("../../../cairo_programs/integration.json").to_vec(),
            include_bytes!("../../../cairo_programs/bitwise_recursion.json").to_vec(),
            include_bytes!("../../../cairo_programs/keccak_integration_tests.json").to_vec(),
            include_bytes!("../../../cairo_programs/math_integration_tests.json").to_vec(),
            include_bytes!("../../../cairo_programs/pedersen_test.json").to_vec(),
            include_bytes!("../../../cairo_programs/poseidon_hash.json").to_vec(),
            include_bytes!("../../../cairo_programs/poseidon_multirun.json").to_vec(),
            include_bytes!("../../../cairo_programs/reduce.json").to_vec(),
            include_bytes!("../../../cairo_programs/secp_ec.json").to_vec(),
            include_bytes!("../../../cairo_programs/sha256_test.json").to_vec(),
            include_bytes!("../../../cairo_programs/uint256_integration_tests.json").to_vec(),
        ]
        .to_vec();
        for bytes in programs_bytes {
            let original_program = Program::from_bytes(&bytes, Some("main")).unwrap();

            let program_serializer = ProgramSerializer::from(&original_program);

            let program_json = ProgramJson::from(program_serializer);

            let new_program = parse_program_json(program_json, Some("main")).unwrap();

            assert_eq!(&original_program, &new_program);
        }
    }

    #[test]
    #[cfg_attr(target_arch = "wasm32", wasm_bindgen_test)]
    fn serialize_and_deserialize_programs() {
        let programs_bytes: Vec<Vec<u8>> = [
            include_bytes!("../../../cairo_programs/keccak.json").to_vec(),
            include_bytes!("../../../cairo_programs/assert_nn.json").to_vec(),
            include_bytes!("../../../cairo_programs/bitwise_recursion.json").to_vec(),
            include_bytes!("../../../cairo_programs/blake2s_felts.json").to_vec(),
            include_bytes!("../../../cairo_programs/cairo_finalize_keccak_block_size_1000.json")
                .to_vec(),
            include_bytes!("../../../cairo_programs/bitwise_recursion.json").to_vec(),
            include_bytes!("../../../cairo_programs/keccak.json").to_vec(),
            include_bytes!("../../../cairo_programs/ec_double_slope.json").to_vec(),
            include_bytes!("../../../cairo_programs/example_blake2s.json").to_vec(),
            include_bytes!("../../../cairo_programs/fibonacci.json").to_vec(),
            include_bytes!("../../../cairo_programs/integration.json").to_vec(),
            include_bytes!("../../../cairo_programs/bitwise_recursion.json").to_vec(),
            include_bytes!("../../../cairo_programs/keccak_integration_tests.json").to_vec(),
            include_bytes!("../../../cairo_programs/math_integration_tests.json").to_vec(),
            include_bytes!("../../../cairo_programs/pedersen_test.json").to_vec(),
            include_bytes!("../../../cairo_programs/poseidon_hash.json").to_vec(),
            include_bytes!("../../../cairo_programs/poseidon_multirun.json").to_vec(),
            include_bytes!("../../../cairo_programs/reduce.json").to_vec(),
            include_bytes!("../../../cairo_programs/secp_ec.json").to_vec(),
            include_bytes!("../../../cairo_programs/sha256_test.json").to_vec(),
            include_bytes!("../../../cairo_programs/uint256_integration_tests.json").to_vec(),
        ]
        .to_vec();

        for bytes in programs_bytes {
            let original_program = Program::from_bytes(&bytes, Some("main")).unwrap();
            let program_serialized = original_program.serialize().unwrap();
            let new_program = Program::deserialize(&program_serialized, Some("main")).unwrap();

            assert_eq!(original_program, new_program);
        }
    }
}
