pub(crate) const _CELLS_PER_SIGNATURE: u32 = 2;
pub(crate) const _INPUT_CELLS_PER_SIGNATURE: u32 = 2;

#[derive(Clone, Debug, PartialEq)]
pub(crate) struct KeccakInstanceDef {
    pub(crate) _ratio: u32,
    pub(crate) _repetitions: u32,
    pub(crate) _height: u32,
    pub(crate) _n_hash_bits: u32,
}

impl KeccakInstanceDef {
    pub(crate) fn default() -> Self {
        KeccakInstanceDef {
            _ratio: 512,
            _repetitions: 1,
            _height: 256,
            _n_hash_bits: 251,
        }
    }

    pub(crate) fn new(ratio: u32) -> Self {
        KeccakInstanceDef {
            _ratio: ratio,
            _repetitions: 1,
            _height: 256,
            _n_hash_bits: 251,
        }
    }

    pub(crate) fn _cells_per_builtin(&self) -> u32 {
        _CELLS_PER_SIGNATURE
    }

    pub(crate) fn _range_check_units_per_builtin(&self) -> u32 {
        0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn get_range_check_units_per_builtin() {
        let builtin_instance = KeccakInstanceDef::default();
        assert_eq!(builtin_instance._range_check_units_per_builtin(), 0);
    }

    #[test]
    fn get_cells_per_builtin() {
        let builtin_instance = KeccakInstanceDef::default();
        assert_eq!(builtin_instance._cells_per_builtin(), 2);
    }

    #[test]
    fn test_new() {
        let builtin_instance = KeccakInstanceDef {
            _ratio: 8,
            _repetitions: 1,
            _height: 256,
            _n_hash_bits: 251,
        };
        assert_eq!(KeccakInstanceDef::new(8), builtin_instance);
    }

    #[test]
    fn test_default() {
        let builtin_instance = KeccakInstanceDef {
            _ratio: 512,
            _repetitions: 1,
            _height: 256,
            _n_hash_bits: 251,
        };
        assert_eq!(KeccakInstanceDef::default(), builtin_instance);
    }
}
