use crate::{
    stdlib::{
        collections::{BTreeMap, HashMap},
        prelude::{String, Vec},
    },
    types::builtin_name::BuiltinName,
};
use serde::{Deserialize, Serialize};

use crate::Felt252;

// Serializable format, matches the file output of the python implementation
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
pub struct AirPrivateInputSerializable {
    trace_path: String,
    memory_path: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pedersen: Option<Vec<PrivateInput>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    range_check: Option<Vec<PrivateInput>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    range_check96: Option<Vec<PrivateInput>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    ecdsa: Option<Vec<PrivateInput>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    bitwise: Option<Vec<PrivateInput>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    ec_op: Option<Vec<PrivateInput>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    keccak: Option<Vec<PrivateInput>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    poseidon: Option<Vec<PrivateInput>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    add_mod: Option<PrivateInput>,
    #[serde(skip_serializing_if = "Option::is_none")]
    mul_mod: Option<PrivateInput>,
}

// Contains only builtin public inputs, useful for library users
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AirPrivateInput(pub HashMap<BuiltinName, Vec<PrivateInput>>);

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
#[serde(untagged)]
pub enum PrivateInput {
    Value(PrivateInputValue),
    Pair(PrivateInputPair),
    EcOp(PrivateInputEcOp),
    PoseidonState(PrivateInputPoseidonState),
    KeccakState(PrivateInputKeccakState),
    Signature(PrivateInputSignature),
    Mod(ModInput),
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
pub struct PrivateInputValue {
    pub index: usize,
    pub value: Felt252,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
pub struct PrivateInputPair {
    pub index: usize,
    pub x: Felt252,
    pub y: Felt252,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
pub struct PrivateInputEcOp {
    pub index: usize,
    pub p_x: Felt252,
    pub p_y: Felt252,
    pub m: Felt252,
    pub q_x: Felt252,
    pub q_y: Felt252,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
pub struct PrivateInputPoseidonState {
    pub index: usize,
    pub input_s0: Felt252,
    pub input_s1: Felt252,
    pub input_s2: Felt252,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
pub struct PrivateInputKeccakState {
    pub index: usize,
    pub input_s0: Felt252,
    pub input_s1: Felt252,
    pub input_s2: Felt252,
    pub input_s3: Felt252,
    pub input_s4: Felt252,
    pub input_s5: Felt252,
    pub input_s6: Felt252,
    pub input_s7: Felt252,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
pub struct PrivateInputSignature {
    pub index: usize,
    pub pubkey: Felt252,
    pub msg: Felt252,
    pub signature_input: SignatureInput,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
pub struct SignatureInput {
    pub r: Felt252,
    pub w: Felt252,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
pub struct ModInput {
    pub instances: Vec<ModInputInstance>,
    pub zero_value_address: usize,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
pub struct ModInputInstance {
    pub index: usize,
    pub p0: Felt252,
    pub p1: Felt252,
    pub p2: Felt252,
    pub p3: Felt252,
    pub values_ptr: usize,
    pub offsets_ptr: usize,
    pub n: usize,
    #[serde(deserialize_with = "mod_input_instance_batch_serde::deserialize")]
    #[serde(serialize_with = "mod_input_instance_batch_serde::serialize")]
    pub batch: BTreeMap<usize, ModInputMemoryVars>,
}

#[derive(Deserialize, Serialize, Clone, Debug, PartialEq, Eq)]
pub struct ModInputMemoryVars {
    pub a_offset: usize,
    pub a0: Felt252,
    pub a1: Felt252,
    pub a2: Felt252,
    pub a3: Felt252,
    pub b_offset: usize,
    pub b0: Felt252,
    pub b1: Felt252,
    pub b2: Felt252,
    pub b3: Felt252,
    pub c_offset: usize,
    pub c0: Felt252,
    pub c1: Felt252,
    pub c2: Felt252,
    pub c3: Felt252,
}

impl AirPrivateInput {
    pub fn to_serializable(
        &self,
        trace_path: String,
        memory_path: String,
    ) -> AirPrivateInputSerializable {
        AirPrivateInputSerializable {
            trace_path,
            memory_path,
            pedersen: self.0.get(&BuiltinName::pedersen).cloned(),
            range_check: self.0.get(&BuiltinName::range_check).cloned(),
            range_check96: self.0.get(&BuiltinName::range_check96).cloned(),
            ecdsa: self.0.get(&BuiltinName::ecdsa).cloned(),
            bitwise: self.0.get(&BuiltinName::bitwise).cloned(),
            ec_op: self.0.get(&BuiltinName::ec_op).cloned(),
            keccak: self.0.get(&BuiltinName::keccak).cloned(),
            poseidon: self.0.get(&BuiltinName::poseidon).cloned(),
            add_mod: self
                .0
                .get(&BuiltinName::add_mod)
                .and_then(|pi| pi.first())
                .cloned(),
            mul_mod: self
                .0
                .get(&BuiltinName::mul_mod)
                .and_then(|pi| pi.first())
                .cloned(),
        }
    }
}

impl From<AirPrivateInputSerializable> for AirPrivateInput {
    fn from(private_input: AirPrivateInputSerializable) -> Self {
        let mut inputs = HashMap::new();
        let mut insert_input = |input_name, input| {
            if let Some(input) = input {
                inputs.insert(input_name, input);
            }
        };
        insert_input(BuiltinName::pedersen, private_input.pedersen);
        insert_input(BuiltinName::range_check, private_input.range_check);
        insert_input(BuiltinName::ecdsa, private_input.ecdsa);
        insert_input(BuiltinName::bitwise, private_input.bitwise);
        insert_input(BuiltinName::ec_op, private_input.ec_op);
        insert_input(BuiltinName::keccak, private_input.keccak);
        insert_input(BuiltinName::poseidon, private_input.poseidon);

        Self(inputs)
    }
}

impl AirPrivateInputSerializable {
    pub fn serialize_json(&self) -> Result<String, serde_json::Error> {
        serde_json::to_string_pretty(&self)
    }
}

mod mod_input_instance_batch_serde {
    use super::*;

    use serde::{Deserializer, Serializer};

    pub(crate) fn serialize<S: Serializer>(
        value: &BTreeMap<usize, ModInputMemoryVars>,
        s: S,
    ) -> Result<S::Ok, S::Error> {
        let value = value.iter().map(|v| v.1).collect::<Vec<_>>();

        value.serialize(s)
    }

    pub(crate) fn deserialize<'de, D: Deserializer<'de>>(
        d: D,
    ) -> Result<BTreeMap<usize, ModInputMemoryVars>, D::Error> {
        let value = Vec::<ModInputMemoryVars>::deserialize(d)?;

        Ok(value.into_iter().enumerate().collect())
    }

    #[cfg(feature = "std")]
    #[test]
    fn test_serde() {
        let input_value = vec![
            (
                0,
                ModInputMemoryVars {
                    a_offset: 5,
                    b_offset: 5,
                    c_offset: 5,
                    a0: Felt252::from(5u32),
                    a1: Felt252::from(5u32),
                    a2: Felt252::from(5u32),
                    a3: Felt252::from(5u32),
                    b0: Felt252::from(5u32),
                    b1: Felt252::from(5u32),
                    b2: Felt252::from(5u32),
                    b3: Felt252::from(5u32),
                    c0: Felt252::from(5u32),
                    c1: Felt252::from(5u32),
                    c2: Felt252::from(5u32),
                    c3: Felt252::from(5u32),
                },
            ),
            (
                1,
                ModInputMemoryVars {
                    a_offset: 7,
                    b_offset: 7,
                    c_offset: 7,
                    a0: Felt252::from(7u32),
                    a1: Felt252::from(7u32),
                    a2: Felt252::from(7u32),
                    a3: Felt252::from(7u32),
                    b0: Felt252::from(7u32),
                    b1: Felt252::from(7u32),
                    b2: Felt252::from(7u32),
                    b3: Felt252::from(7u32),
                    c0: Felt252::from(7u32),
                    c1: Felt252::from(7u32),
                    c2: Felt252::from(7u32),
                    c3: Felt252::from(7u32),
                },
            ),
        ]
        .into_iter()
        .collect::<BTreeMap<usize, _>>();

        let bytes = Vec::new();
        let mut serializer = serde_json::Serializer::new(bytes);
        serialize(&input_value, &mut serializer).unwrap();
        let bytes = serializer.into_inner();

        let mut deserializer = serde_json::Deserializer::from_slice(&bytes);
        let output_value = deserialize(&mut deserializer).unwrap();

        assert_eq!(input_value, output_value);
    }
}

#[cfg(test)]
mod tests {
    use crate::types::layout_name::LayoutName;
    #[cfg(feature = "std")]
    use {
        super::*,
        crate::air_private_input::{AirPrivateInput, AirPrivateInputSerializable},
        assert_matches::assert_matches,
    };

    #[cfg(any(target_arch = "wasm32", not(feature = "std")))]
    use crate::alloc::string::ToString;

    #[cfg(feature = "std")]
    #[test]
    fn test_from_serializable() {
        let serializable_private_input = AirPrivateInputSerializable {
            trace_path: "trace.bin".to_string(),
            memory_path: "memory.bin".to_string(),
            pedersen: Some(vec![PrivateInput::Pair(PrivateInputPair {
                index: 0,
                x: Felt252::from(100),
                y: Felt252::from(200),
            })]),
            range_check: Some(vec![PrivateInput::Value(PrivateInputValue {
                index: 10000,
                value: Felt252::from(8000),
            })]),
            range_check96: Some(vec![PrivateInput::Value(PrivateInputValue {
                index: 10000,
                value: Felt252::from(8000),
            })]),
            ecdsa: Some(vec![PrivateInput::Signature(PrivateInputSignature {
                index: 0,
                pubkey: Felt252::from(123),
                msg: Felt252::from(456),
                signature_input: SignatureInput {
                    r: Felt252::from(654),
                    w: Felt252::from(321),
                },
            })]),
            bitwise: Some(vec![PrivateInput::Pair(PrivateInputPair {
                index: 4,
                x: Felt252::from(7),
                y: Felt252::from(8),
            })]),
            ec_op: Some(vec![PrivateInput::EcOp(PrivateInputEcOp {
                index: 1,
                p_x: Felt252::from(10),
                p_y: Felt252::from(10),
                m: Felt252::from(100),
                q_x: Felt252::from(11),
                q_y: Felt252::from(14),
            })]),
            keccak: Some(vec![PrivateInput::KeccakState(PrivateInputKeccakState {
                index: 0,
                input_s0: Felt252::from(0),
                input_s1: Felt252::from(1),
                input_s2: Felt252::from(2),
                input_s3: Felt252::from(3),
                input_s4: Felt252::from(4),
                input_s5: Felt252::from(5),
                input_s6: Felt252::from(6),
                input_s7: Felt252::from(7),
            })]),
            poseidon: Some(vec![PrivateInput::PoseidonState(
                PrivateInputPoseidonState {
                    index: 42,
                    input_s0: Felt252::from(1),
                    input_s1: Felt252::from(2),
                    input_s2: Felt252::from(3),
                },
            )]),
            add_mod: None,
            mul_mod: None,
        };

        let private_input = AirPrivateInput::from(serializable_private_input.clone());

        assert_matches!(private_input.0.get(&BuiltinName::pedersen), data if data == serializable_private_input.pedersen.as_ref());
        assert_matches!(private_input.0.get(&BuiltinName::range_check), data if data == serializable_private_input.range_check.as_ref());
        assert_matches!(private_input.0.get(&BuiltinName::ecdsa), data if data == serializable_private_input.ecdsa.as_ref());
        assert_matches!(private_input.0.get(&BuiltinName::bitwise), data if data == serializable_private_input.bitwise.as_ref());
        assert_matches!(private_input.0.get(&BuiltinName::ec_op), data if data == serializable_private_input.ec_op.as_ref());
        assert_matches!(private_input.0.get(&BuiltinName::keccak), data if data == serializable_private_input.keccak.as_ref());
        assert_matches!(private_input.0.get(&BuiltinName::poseidon), data if data == serializable_private_input.poseidon.as_ref());
    }

    #[test]
    fn serialize_air_private_input_small_layout_only_builtins() {
        let config = crate::cairo_run::CairoRunConfig {
            proof_mode: true,
            relocate_mem: true,
            trace_enabled: true,
            layout: LayoutName::small,
            ..Default::default()
        };
        let runner = crate::cairo_run::cairo_run(include_bytes!("../../cairo_programs/proof_programs/fibonacci.json"), &config, &mut crate::hint_processor::builtin_hint_processor::builtin_hint_processor_definition::BuiltinHintProcessor::new_empty()).unwrap();
        let public_input = runner.get_air_private_input();
        let serialized_public_input =
            public_input.to_serializable("/dev/null".to_string(), "/dev/null".to_string());
        assert!(serialized_public_input.pedersen.is_some());
        assert!(serialized_public_input.range_check.is_some());
        assert!(serialized_public_input.ecdsa.is_some());
        assert!(serialized_public_input.bitwise.is_none());
        assert!(serialized_public_input.ec_op.is_none());
        assert!(serialized_public_input.keccak.is_none());
        assert!(serialized_public_input.poseidon.is_none());
    }
}
