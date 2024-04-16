#[cfg(all(feature = "arbitrary", feature = "std"))]
use arbitrary::{self, Arbitrary, Unstructured};
#[cfg(all(feature = "clap", feature = "std"))]
use clap::{builder::PossibleValue, ValueEnum};
use core::fmt::{self, Display};
use serde::{Deserialize, Serialize};

/// Enum representing the name of a Cairo Layout
#[cfg_attr(all(feature = "arbitrary", feature = "std"), derive(Arbitrary))]
#[derive(Serialize, Deserialize, Debug, PartialEq, Copy, Clone, Eq, Hash)]
#[allow(non_camel_case_types)]
pub enum LayoutName {
    plain,
    small,
    dex,
    recursive,
    starknet,
    starknet_with_keccak,
    recursive_large_output,
    all_solidity,
    all_cairo,
    dynamic,
}

impl LayoutName {
    pub fn to_str(&self) -> &'static str {
        match self {
            LayoutName::plain => "plain",
            LayoutName::small => "small",
            LayoutName::dex => "dex",
            LayoutName::recursive => "recursive",
            LayoutName::starknet => "starknet",
            LayoutName::starknet_with_keccak => "starknet_with_keccak",
            LayoutName::recursive_large_output => "recursive_large_output",
            LayoutName::all_solidity => "all_solidity",
            LayoutName::all_cairo => "all_cairo",
            LayoutName::dynamic => "all_cairo",
        }
    }
}

impl Display for LayoutName {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.to_str().fmt(f)
    }
}

#[cfg(all(feature = "clap", feature = "std"))]
impl ValueEnum for LayoutName {
    fn value_variants<'a>() -> &'a [Self] {
        &[
            Self::plain,
            Self::small,
            Self::dex,
            Self::recursive,
            Self::starknet,
            Self::starknet_with_keccak,
            Self::recursive_large_output,
            Self::all_solidity,
            Self::all_cairo,
            Self::dynamic,
        ]
    }

    fn to_possible_value(&self) -> Option<PossibleValue> {
        Some(PossibleValue::new(self.to_str()))
    }
}

#[cfg(all(feature = "arbitrary", feature = "std"))]
pub fn arbitrary_layout(u: &mut Unstructured) -> arbitrary::Result<LayoutName> {
    let layouts = [
        LayoutName::plain,
        LayoutName::small,
        LayoutName::dex,
        LayoutName::starknet,
        LayoutName::starknet_with_keccak,
        LayoutName::recursive_large_output,
        LayoutName::all_cairo,
        LayoutName::all_solidity,
        LayoutName::dynamic,
    ];
    Ok(*u.choose(&layouts)?)
}
