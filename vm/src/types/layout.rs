use crate::{types::layout_name::LayoutName, vm::errors::runner_errors::RunnerError};

use super::{
    builtin_name::BuiltinName,
    instance_definitions::{
        builtins_instance_def::BuiltinsInstanceDef,
        diluted_pool_instance_def::DilutedPoolInstanceDef,
    },
};

pub(crate) const DEFAULT_MEMORY_UNITS_PER_STEP: u32 = 8;
pub(crate) const DEFAULT_CPU_COMPONENT_STEP: u32 = 1;

use serde::{Deserialize, Deserializer, Serialize};

#[derive(Serialize, Debug)]
pub struct CairoLayout {
    pub(crate) name: LayoutName,
    pub(crate) cpu_component_step: u32,
    pub(crate) rc_units: u32,
    pub(crate) memory_units_per_step: u32,
    pub(crate) builtins: BuiltinsInstanceDef,
    pub(crate) public_memory_fraction: u32,
    pub(crate) diluted_pool_instance_def: Option<DilutedPoolInstanceDef>,
}

impl CairoLayout {
    pub(crate) fn plain_instance() -> CairoLayout {
        CairoLayout {
            name: LayoutName::plain,
            rc_units: 16,
            cpu_component_step: DEFAULT_CPU_COMPONENT_STEP,
            memory_units_per_step: DEFAULT_MEMORY_UNITS_PER_STEP,
            builtins: BuiltinsInstanceDef::plain(),
            public_memory_fraction: 4,
            diluted_pool_instance_def: None,
        }
    }

    pub(crate) fn small_instance() -> CairoLayout {
        CairoLayout {
            name: LayoutName::small,
            rc_units: 16,
            cpu_component_step: DEFAULT_CPU_COMPONENT_STEP,
            memory_units_per_step: DEFAULT_MEMORY_UNITS_PER_STEP,
            builtins: BuiltinsInstanceDef::small(),
            public_memory_fraction: 4,
            diluted_pool_instance_def: None,
        }
    }

    pub(crate) fn dex_instance() -> CairoLayout {
        CairoLayout {
            name: LayoutName::dex,
            rc_units: 4,
            cpu_component_step: DEFAULT_CPU_COMPONENT_STEP,
            memory_units_per_step: DEFAULT_MEMORY_UNITS_PER_STEP,
            builtins: BuiltinsInstanceDef::dex(),
            public_memory_fraction: 4,
            diluted_pool_instance_def: None,
        }
    }

    pub(crate) fn recursive_instance() -> CairoLayout {
        CairoLayout {
            name: LayoutName::recursive,
            rc_units: 4,
            cpu_component_step: DEFAULT_CPU_COMPONENT_STEP,
            memory_units_per_step: DEFAULT_MEMORY_UNITS_PER_STEP,
            builtins: BuiltinsInstanceDef::recursive(),
            public_memory_fraction: 8,
            diluted_pool_instance_def: Some(DilutedPoolInstanceDef::default()),
        }
    }

    pub(crate) fn starknet_instance() -> CairoLayout {
        CairoLayout {
            name: LayoutName::starknet,
            rc_units: 4,
            cpu_component_step: DEFAULT_CPU_COMPONENT_STEP,
            memory_units_per_step: DEFAULT_MEMORY_UNITS_PER_STEP,
            builtins: BuiltinsInstanceDef::starknet(),
            public_memory_fraction: 8,
            diluted_pool_instance_def: Some(DilutedPoolInstanceDef::new(2, 4, 16)),
        }
    }

    pub(crate) fn starknet_with_keccak_instance() -> CairoLayout {
        CairoLayout {
            name: LayoutName::starknet_with_keccak,
            rc_units: 4,
            cpu_component_step: DEFAULT_CPU_COMPONENT_STEP,
            memory_units_per_step: DEFAULT_MEMORY_UNITS_PER_STEP,
            builtins: BuiltinsInstanceDef::starknet_with_keccak(),
            public_memory_fraction: 8,
            diluted_pool_instance_def: Some(DilutedPoolInstanceDef::default()),
        }
    }

    pub(crate) fn recursive_large_output_instance() -> CairoLayout {
        CairoLayout {
            name: LayoutName::recursive_large_output,
            rc_units: 4,
            cpu_component_step: DEFAULT_CPU_COMPONENT_STEP,
            memory_units_per_step: DEFAULT_MEMORY_UNITS_PER_STEP,
            builtins: BuiltinsInstanceDef::recursive_large_output(),
            public_memory_fraction: 8,
            diluted_pool_instance_def: Some(DilutedPoolInstanceDef::default()),
        }
    }
    pub(crate) fn recursive_with_poseidon() -> CairoLayout {
        CairoLayout {
            name: LayoutName::recursive_with_poseidon,
            rc_units: 4,
            cpu_component_step: DEFAULT_CPU_COMPONENT_STEP,
            memory_units_per_step: DEFAULT_MEMORY_UNITS_PER_STEP,
            builtins: BuiltinsInstanceDef::recursive_with_poseidon(),
            public_memory_fraction: 8,
            diluted_pool_instance_def: Some(DilutedPoolInstanceDef::new(8, 4, 16)),
        }
    }

    pub(crate) fn all_cairo_instance() -> CairoLayout {
        CairoLayout {
            name: LayoutName::all_cairo,
            rc_units: 4,
            cpu_component_step: DEFAULT_CPU_COMPONENT_STEP,
            memory_units_per_step: DEFAULT_MEMORY_UNITS_PER_STEP,
            builtins: BuiltinsInstanceDef::all_cairo(),
            public_memory_fraction: 8,
            diluted_pool_instance_def: Some(DilutedPoolInstanceDef::default()),
        }
    }

    pub(crate) fn all_solidity_instance() -> CairoLayout {
        CairoLayout {
            name: LayoutName::all_solidity,
            rc_units: 8,
            cpu_component_step: DEFAULT_CPU_COMPONENT_STEP,
            memory_units_per_step: DEFAULT_MEMORY_UNITS_PER_STEP,
            builtins: BuiltinsInstanceDef::all_solidity(),
            public_memory_fraction: 8,
            diluted_pool_instance_def: Some(DilutedPoolInstanceDef::default()),
        }
    }

    pub(crate) fn dynamic_instance(params: CairoLayoutParams) -> CairoLayout {
        CairoLayout {
            name: LayoutName::dynamic,
            rc_units: params.rc_units,
            cpu_component_step: params.cpu_component_step,
            memory_units_per_step: params.memory_units_per_step,
            public_memory_fraction: 8,
            diluted_pool_instance_def: Some(DilutedPoolInstanceDef::from_log_units_per_step(
                params.log_diluted_units_per_step,
            )),
            builtins: BuiltinsInstanceDef::dynamic(params),
        }
    }
}

#[cfg(feature = "test_utils")]
use arbitrary::{self, Arbitrary};

#[cfg_attr(feature = "test_utils", derive(Arbitrary))]
#[derive(Deserialize, Debug, Clone, Default)]
#[serde(try_from = "RawCairoLayoutParams")]
pub struct CairoLayoutParams {
    pub rc_units: u32,
    pub cpu_component_step: u32,
    pub memory_units_per_step: u32,
    pub log_diluted_units_per_step: i32,
    pub pedersen_ratio: u32,
    pub range_check_ratio: u32,
    pub ecdsa_ratio: u32,
    pub bitwise_ratio: u32,
    pub ec_op_ratio: u32,
    pub keccak_ratio: u32,
    pub poseidon_ratio: u32,
    pub range_check96_ratio: u32,
    pub range_check96_ratio_den: u32,
    pub add_mod_ratio: u32,
    pub add_mod_ratio_den: u32,
    pub mul_mod_ratio: u32,
    pub mul_mod_ratio_den: u32,
}

impl CairoLayoutParams {
    #[cfg(feature = "std")]
    pub fn from_file(params_path: &std::path::Path) -> std::io::Result<Self> {
        let params_file = std::fs::File::open(params_path)?;
        let params = serde_json::from_reader(params_file)?;
        Ok(params)
    }
}

// The CairoLayoutParams contains aditional constraints that can't be validated by serde alone.
// To work around this. we use an aditional structure `RawCairoLayoutParams` that gets deserialized by serde
// and then its tranformed into `CairoLayoutParams`.

#[derive(Deserialize, Debug, Default, Clone)]
pub struct RawCairoLayoutParams {
    pub rc_units: u32,
    pub cpu_component_step: u32,
    pub memory_units_per_step: u32,
    pub log_diluted_units_per_step: i32,
    #[serde(deserialize_with = "bool_from_int_or_bool")]
    pub uses_pedersen_builtin: bool,
    pub pedersen_ratio: u32,
    #[serde(deserialize_with = "bool_from_int_or_bool")]
    pub uses_range_check_builtin: bool,
    pub range_check_ratio: u32,
    #[serde(deserialize_with = "bool_from_int_or_bool")]
    pub uses_ecdsa_builtin: bool,
    pub ecdsa_ratio: u32,
    #[serde(deserialize_with = "bool_from_int_or_bool")]
    pub uses_bitwise_builtin: bool,
    pub bitwise_ratio: u32,
    #[serde(deserialize_with = "bool_from_int_or_bool")]
    pub uses_ec_op_builtin: bool,
    pub ec_op_ratio: u32,
    #[serde(deserialize_with = "bool_from_int_or_bool")]
    pub uses_keccak_builtin: bool,
    pub keccak_ratio: u32,
    #[serde(deserialize_with = "bool_from_int_or_bool")]
    pub uses_poseidon_builtin: bool,
    pub poseidon_ratio: u32,
    #[serde(deserialize_with = "bool_from_int_or_bool")]
    pub uses_range_check96_builtin: bool,
    pub range_check96_ratio: u32,
    pub range_check96_ratio_den: u32,
    #[serde(deserialize_with = "bool_from_int_or_bool")]
    pub uses_add_mod_builtin: bool,
    pub add_mod_ratio: u32,
    pub add_mod_ratio_den: u32,
    #[serde(deserialize_with = "bool_from_int_or_bool")]
    pub uses_mul_mod_builtin: bool,
    pub mul_mod_ratio: u32,
    pub mul_mod_ratio_den: u32,
}

impl TryFrom<RawCairoLayoutParams> for CairoLayoutParams {
    type Error = RunnerError;

    fn try_from(value: RawCairoLayoutParams) -> Result<Self, Self::Error> {
        if !value.uses_pedersen_builtin && value.pedersen_ratio != 0 {
            return Err(RunnerError::BadDynamicLayoutBuiltinRatio(
                BuiltinName::pedersen,
            ));
        }
        if !value.uses_range_check_builtin && value.range_check_ratio != 0 {
            return Err(RunnerError::BadDynamicLayoutBuiltinRatio(
                BuiltinName::range_check,
            ));
        }
        if !value.uses_ecdsa_builtin && value.ecdsa_ratio != 0 {
            return Err(RunnerError::BadDynamicLayoutBuiltinRatio(
                BuiltinName::ecdsa,
            ));
        }
        if !value.uses_bitwise_builtin && value.bitwise_ratio != 0 {
            return Err(RunnerError::BadDynamicLayoutBuiltinRatio(
                BuiltinName::bitwise,
            ));
        }
        if !value.uses_ec_op_builtin && value.ec_op_ratio != 0 {
            return Err(RunnerError::BadDynamicLayoutBuiltinRatio(
                BuiltinName::ec_op,
            ));
        }
        if !value.uses_keccak_builtin && value.keccak_ratio != 0 {
            return Err(RunnerError::BadDynamicLayoutBuiltinRatio(
                BuiltinName::keccak,
            ));
        }
        if !value.uses_poseidon_builtin && value.poseidon_ratio != 0 {
            return Err(RunnerError::BadDynamicLayoutBuiltinRatio(
                BuiltinName::poseidon,
            ));
        }
        if !value.uses_range_check96_builtin && value.range_check96_ratio != 0 {
            return Err(RunnerError::BadDynamicLayoutBuiltinRatio(
                BuiltinName::range_check96,
            ));
        }
        if !value.uses_add_mod_builtin && value.add_mod_ratio != 0 {
            return Err(RunnerError::BadDynamicLayoutBuiltinRatio(
                BuiltinName::add_mod,
            ));
        }
        if !value.uses_mul_mod_builtin && value.mul_mod_ratio != 0 {
            return Err(RunnerError::BadDynamicLayoutBuiltinRatio(
                BuiltinName::mul_mod,
            ));
        }

        Ok(CairoLayoutParams {
            rc_units: value.rc_units,
            log_diluted_units_per_step: value.log_diluted_units_per_step,
            cpu_component_step: value.cpu_component_step,
            memory_units_per_step: value.memory_units_per_step,
            range_check96_ratio_den: value.range_check96_ratio_den,
            mul_mod_ratio_den: value.mul_mod_ratio_den,
            add_mod_ratio_den: value.add_mod_ratio_den,
            pedersen_ratio: value.pedersen_ratio,
            range_check_ratio: value.range_check_ratio,
            ecdsa_ratio: value.ecdsa_ratio,
            bitwise_ratio: value.bitwise_ratio,
            ec_op_ratio: value.ec_op_ratio,
            keccak_ratio: value.keccak_ratio,
            poseidon_ratio: value.poseidon_ratio,
            range_check96_ratio: value.range_check96_ratio,
            add_mod_ratio: value.add_mod_ratio,
            mul_mod_ratio: value.mul_mod_ratio,
        })
    }
}

fn bool_from_int_or_bool<'de, D>(deserializer: D) -> Result<bool, D::Error>
where
    D: Deserializer<'de>,
{
    #[derive(Deserialize)]
    #[serde(untagged)]
    enum IntOrBool {
        Int(i64),
        Boolean(bool),
    }

    match IntOrBool::deserialize(deserializer)? {
        IntOrBool::Int(0) => Ok(false),
        IntOrBool::Int(_) => Ok(true),
        IntOrBool::Boolean(v) => Ok(v),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[cfg(feature = "mod_builtin")]
    use crate::types::instance_definitions::mod_instance_def::ModInstanceDef;

    use crate::types::instance_definitions::{
        bitwise_instance_def::BitwiseInstanceDef, ec_op_instance_def::EcOpInstanceDef,
        ecdsa_instance_def::EcdsaInstanceDef, keccak_instance_def::KeccakInstanceDef,
        pedersen_instance_def::PedersenInstanceDef, poseidon_instance_def::PoseidonInstanceDef,
        range_check_instance_def::RangeCheckInstanceDef, LowRatio,
    };

    #[cfg(target_arch = "wasm32")]
    use wasm_bindgen_test::*;

    #[test]
    #[cfg_attr(target_arch = "wasm32", wasm_bindgen_test)]
    fn get_plain_instance() {
        let layout = CairoLayout::plain_instance();
        let builtins = BuiltinsInstanceDef::plain();
        assert_eq!(layout.name, LayoutName::plain);
        assert_eq!(layout.rc_units, 16);
        assert_eq!(layout.builtins, builtins);
        assert_eq!(layout.public_memory_fraction, 4);
        assert_eq!(layout.diluted_pool_instance_def, None);
    }

    #[test]
    #[cfg_attr(target_arch = "wasm32", wasm_bindgen_test)]
    fn get_small_instance() {
        let layout = CairoLayout::small_instance();
        let builtins = BuiltinsInstanceDef::small();
        assert_eq!(layout.name, LayoutName::small);
        assert_eq!(layout.rc_units, 16);
        assert_eq!(layout.builtins, builtins);
        assert_eq!(layout.public_memory_fraction, 4);
        assert_eq!(layout.diluted_pool_instance_def, None);
    }

    #[test]
    #[cfg_attr(target_arch = "wasm32", wasm_bindgen_test)]
    fn get_dex_instance() {
        let layout = CairoLayout::dex_instance();
        let builtins = BuiltinsInstanceDef::dex();
        assert_eq!(layout.name, LayoutName::dex);
        assert_eq!(layout.rc_units, 4);
        assert_eq!(layout.builtins, builtins);
        assert_eq!(layout.public_memory_fraction, 4);
        assert_eq!(layout.diluted_pool_instance_def, None);
    }

    #[test]
    fn get_recursive_instance() {
        let layout = CairoLayout::recursive_instance();
        let builtins = BuiltinsInstanceDef::recursive();
        assert_eq!(layout.name, LayoutName::recursive);
        assert_eq!(layout.rc_units, 4);
        assert_eq!(layout.builtins, builtins);
        assert_eq!(layout.public_memory_fraction, 8);
        assert_eq!(
            layout.diluted_pool_instance_def,
            Some(DilutedPoolInstanceDef::default())
        );
    }

    #[test]
    fn get_starknet_instance() {
        let layout = CairoLayout::starknet_instance();
        let builtins = BuiltinsInstanceDef::starknet();
        assert_eq!(layout.name, LayoutName::starknet);
        assert_eq!(layout.rc_units, 4);
        assert_eq!(layout.builtins, builtins);
        assert_eq!(layout.public_memory_fraction, 8);
        assert_eq!(
            layout.diluted_pool_instance_def,
            Some(DilutedPoolInstanceDef::new(2, 4, 16))
        );
    }

    #[test]
    fn get_starknet_with_keccak_instance() {
        let layout = CairoLayout::starknet_with_keccak_instance();
        let builtins = BuiltinsInstanceDef::starknet_with_keccak();
        assert_eq!(layout.name, LayoutName::starknet_with_keccak);
        assert_eq!(layout.rc_units, 4);
        assert_eq!(layout.builtins, builtins);
        assert_eq!(layout.public_memory_fraction, 8);
        assert_eq!(
            layout.diluted_pool_instance_def,
            Some(DilutedPoolInstanceDef::default())
        );
    }

    #[test]
    fn get_recursive_large_output_instance() {
        let layout = CairoLayout::recursive_large_output_instance();
        let builtins = BuiltinsInstanceDef::recursive_large_output();
        assert_eq!(layout.name, LayoutName::recursive_large_output);
        assert_eq!(layout.rc_units, 4);
        assert_eq!(layout.builtins, builtins);
        assert_eq!(layout.public_memory_fraction, 8);
        assert_eq!(
            layout.diluted_pool_instance_def,
            Some(DilutedPoolInstanceDef::default())
        );
    }

    #[test]
    fn get_all_cairo_instance() {
        let layout = CairoLayout::all_cairo_instance();
        let builtins = BuiltinsInstanceDef::all_cairo();
        assert_eq!(layout.name, LayoutName::all_cairo);
        assert_eq!(layout.rc_units, 4);
        assert_eq!(layout.builtins, builtins);
        assert_eq!(layout.public_memory_fraction, 8);
        assert_eq!(
            layout.diluted_pool_instance_def,
            Some(DilutedPoolInstanceDef::default())
        );
    }

    #[test]
    fn get_all_solidity_instance() {
        let layout = CairoLayout::all_solidity_instance();
        let builtins = BuiltinsInstanceDef::all_solidity();
        assert_eq!(layout.name, LayoutName::all_solidity);
        assert_eq!(layout.rc_units, 8);
        assert_eq!(layout.builtins, builtins);
        assert_eq!(layout.public_memory_fraction, 8);
        assert_eq!(
            layout.diluted_pool_instance_def,
            Some(DilutedPoolInstanceDef::default())
        );
    }

    #[test]
    fn get_dynamic_instance() {
        // dummy cairo layout params
        let params = CairoLayoutParams {
            rc_units: 32,
            cpu_component_step: 8,
            memory_units_per_step: 16,
            log_diluted_units_per_step: 5,
            pedersen_ratio: 32,
            range_check_ratio: 32,
            ecdsa_ratio: 32,
            bitwise_ratio: 32,
            ec_op_ratio: 32,
            keccak_ratio: 32,
            poseidon_ratio: 0,
            range_check96_ratio: 8,
            range_check96_ratio_den: 16,
            add_mod_ratio: 8,
            add_mod_ratio_den: 16,
            mul_mod_ratio: 32,
            mul_mod_ratio_den: 16,
        };

        let layout = CairoLayout::dynamic_instance(params);

        assert_eq!(layout.name, LayoutName::dynamic);
        assert_eq!(layout.rc_units, 32);
        assert_eq!(layout.cpu_component_step, 8);
        assert_eq!(layout.memory_units_per_step, 16);
        assert_eq!(layout.public_memory_fraction, 8); // hardcoded
        assert_eq!(
            layout.diluted_pool_instance_def,
            Some(DilutedPoolInstanceDef {
                units_per_step: 32,
                ..DilutedPoolInstanceDef::default() // hardcoded
            })
        );

        assert!(layout.builtins.output);
        assert_eq!(
            layout.builtins.pedersen,
            Some(PedersenInstanceDef { ratio: Some(32) })
        );
        assert_eq!(
            layout.builtins.range_check,
            Some(RangeCheckInstanceDef {
                ratio: Some(LowRatio::new_int(32))
            })
        );
        assert_eq!(
            layout.builtins.ecdsa,
            Some(EcdsaInstanceDef { ratio: Some(32) })
        );
        assert_eq!(
            layout.builtins.bitwise,
            Some(BitwiseInstanceDef { ratio: Some(32) })
        );
        assert_eq!(
            layout.builtins.ec_op,
            Some(EcOpInstanceDef { ratio: Some(32) })
        );
        assert_eq!(
            layout.builtins.keccak,
            Some(KeccakInstanceDef { ratio: Some(32) })
        );
        assert_eq!(
            layout.builtins.poseidon,
            Some(PoseidonInstanceDef { ratio: Some(0) }),
        );
        assert_eq!(
            layout.builtins.range_check96,
            Some(RangeCheckInstanceDef {
                ratio: Some(LowRatio::new(8, 16))
            })
        );
        #[cfg(feature = "mod_builtin")]
        {
            assert_eq!(
                layout.builtins.mul_mod,
                Some(ModInstanceDef {
                    ratio: Some(LowRatio {
                        numerator: 32,
                        denominator: 16
                    }),
                    word_bit_len: 96, // hardcoded
                    batch_size: 1     // hardcoded
                }),
            );
            assert_eq!(
                layout.builtins.add_mod,
                Some(ModInstanceDef {
                    ratio: Some(LowRatio {
                        numerator: 8,
                        denominator: 16
                    }),
                    word_bit_len: 96, // hardcoded
                    batch_size: 1     // hardcoded
                })
            );
        }
        #[cfg(not(feature = "mod_builtin"))]
        {
            assert_eq!(layout.builtins.mul_mod, None,);
            assert_eq!(layout.builtins.add_mod, None,);
        }
    }

    #[test]
    fn parse_dynamic_instance() {
        let cairo_layout_params_json = "{\n\
            \"rc_units\": 4,\n\
            \"log_diluted_units_per_step\": 4,\n\
            \"cpu_component_step\": 8,\n\
            \"memory_units_per_step\": 8,\n\
            \"uses_pedersen_builtin\": true,\n\
            \"pedersen_ratio\": 256,\n\
            \"uses_range_check_builtin\": true,\n\
            \"range_check_ratio\": 8,\n\
            \"uses_ecdsa_builtin\": true,\n\
            \"ecdsa_ratio\": 2048,\n\
            \"uses_bitwise_builtin\": true,\n\
            \"bitwise_ratio\": 16,\n\
            \"uses_ec_op_builtin\": true,\n\
            \"ec_op_ratio\": 1024,\n\
            \"uses_keccak_builtin\": true,\n\
            \"keccak_ratio\": 2048,\n\
            \"uses_poseidon_builtin\": true,\n\
            \"poseidon_ratio\": 256,\n\
            \"uses_range_check96_builtin\": true,\n\
            \"range_check96_ratio\": 8,\n\
            \"range_check96_ratio_den\": 1,\n\
            \"uses_add_mod_builtin\": true,\n\
            \"add_mod_ratio\": 128,\n\
            \"add_mod_ratio_den\": 1,\n\
            \"uses_mul_mod_builtin\": true,\n\
            \"mul_mod_ratio\": 256,\n\
            \"mul_mod_ratio_den\": 1\n\
        }\n\
        ";

        serde_json::from_str::<CairoLayoutParams>(cairo_layout_params_json).unwrap();
    }
}
