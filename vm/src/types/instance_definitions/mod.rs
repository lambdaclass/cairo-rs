use serde::Serialize;

pub mod bitwise_instance_def;
pub mod builtins_instance_def;
pub mod diluted_pool_instance_def;
pub mod ec_op_instance_def;
pub mod ecdsa_instance_def;
pub mod keccak_instance_def;
#[allow(unused)]
pub mod mod_instance_def;
pub mod pedersen_instance_def;
pub mod poseidon_instance_def;
pub mod range_check_instance_def;

#[derive(Serialize, Debug, PartialEq, Copy, Clone)]
pub struct LowRatio {
    pub numerator: u32,
    pub denominator: u32,
}

impl LowRatio {
    pub fn new(numerator: u32, denominator: u32) -> Self {
        Self {
            numerator,
            denominator,
        }
    }

    pub fn new_int(numerator: u32) -> Self {
        Self {
            numerator,
            denominator: 1,
        }
    }
}
