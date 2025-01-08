use serde::Serialize;

use super::LowRatio;

pub(crate) const N_WORDS: usize = 4;

pub(crate) const CELLS_PER_MOD: u32 = 7;

#[derive(Serialize, Debug, PartialEq, Clone)]
pub(crate) struct ModInstanceDef {
    pub(crate) ratio: Option<LowRatio>,
    pub(crate) word_bit_len: u32,
    pub(crate) batch_size: usize,
}

impl ModInstanceDef {
    pub(crate) fn new(ratio: Option<u32>, batch_size: usize, word_bit_len: u32) -> Self {
        ModInstanceDef {
            ratio: ratio.map(LowRatio::new_int),
            word_bit_len,
            batch_size,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[cfg(target_arch = "wasm32")]
    use wasm_bindgen_test::*;

    #[test]
    #[cfg_attr(target_arch = "wasm32", wasm_bindgen_test)]
    fn test_new() {
        let builtin_instance = ModInstanceDef {
            ratio: Some(LowRatio::new_int(10)),
            word_bit_len: 3,
            batch_size: 3,
        };
        assert_eq!(ModInstanceDef::new(Some(10), 3, 3), builtin_instance);
    }
}
