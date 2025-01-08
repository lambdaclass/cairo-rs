use serde::Serialize;

use super::LowRatio;
pub(crate) const CELLS_PER_RANGE_CHECK: u32 = 1;

#[derive(Serialize, Debug, PartialEq)]
pub(crate) struct RangeCheckInstanceDef {
    pub(crate) ratio: Option<LowRatio>,
}

impl Default for RangeCheckInstanceDef {
    fn default() -> Self {
        RangeCheckInstanceDef {
            ratio: Some(LowRatio::new(8, 1)),
        }
    }
}

impl RangeCheckInstanceDef {
    pub(crate) fn new(ratio: Option<u32>) -> Self {
        RangeCheckInstanceDef {
            ratio: ratio.map(LowRatio::new_int),
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
        let builtin_instance = RangeCheckInstanceDef {
            ratio: Some(LowRatio::new_int(10)),
        };
        assert_eq!(RangeCheckInstanceDef::new(Some(10)), builtin_instance);
    }

    #[test]
    #[cfg_attr(target_arch = "wasm32", wasm_bindgen_test)]
    fn test_default() {
        let builtin_instance = RangeCheckInstanceDef {
            ratio: Some(LowRatio::new_int(8)),
        };
        assert_eq!(RangeCheckInstanceDef::default(), builtin_instance);
    }
}
