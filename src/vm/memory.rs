use crate::vm::relocatable::MaybeRelocatable;
use std::collections::HashMap;
use std::convert::From;

pub struct Memory {
    data: HashMap<MaybeRelocatable, MaybeRelocatable>,
}

impl Memory {
    pub fn new() -> Memory {
        Memory {
            data: HashMap::new(),
        }
    }
    pub fn insert(&mut self, key: &MaybeRelocatable, val: &MaybeRelocatable) {
        self.data.insert(key.clone(), val.clone());
    }
    pub fn get(&self, addr: &MaybeRelocatable) -> Option<&MaybeRelocatable> {
        self.data.get(addr)
    }
}

impl<const N: usize> From<[(MaybeRelocatable, MaybeRelocatable); N]> for Memory {
    fn from(key_val_list: [(MaybeRelocatable, MaybeRelocatable); N]) -> Self {
        Memory {
            data: HashMap::from(key_val_list),
        }
    }
}

#[cfg(test)]
mod memory_tests {
    use super::*;
    use num_bigint::BigInt;
    use num_traits::FromPrimitive;

    #[test]
    fn get_test() {
        let key = MaybeRelocatable::Int(BigInt::from_i32(2).unwrap());
        let val = MaybeRelocatable::Int(BigInt::from_i32(5).unwrap());
        let _val_clone = val.clone();
        let mut mem = Memory::new();
        mem.insert(&key, &val);
        assert_eq!(matches!(mem.get(&key), _val_clone), true);
    }

    #[test]
    fn from_array_test() {
        let mem = Memory::from([(
            MaybeRelocatable::Int(BigInt::from_i32(2).unwrap()),
            MaybeRelocatable::Int(BigInt::from_i32(5).unwrap()),
        )]);
        assert_eq!(
            matches!(
                mem.get(&MaybeRelocatable::Int(BigInt::from_i32(2).unwrap())),
                _val_clone
            ),
            true
        );
    }
}
