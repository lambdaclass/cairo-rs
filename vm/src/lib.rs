//! # An implementation of the Cairo virtual machine
//!
//! ## Feature Flags
//! - `std`: Enables usage of the [`std`] standard library. Enabled by default.
//! - `test_utils`: Enables the following to help with tests (not enabled by default):
//!    - [`Hooks`](crate::vm::hooks::Hooks) support for the [VirtualMachine](vm::vm_core::VirtualMachine);
//!    - the `print_*` family of hints;
//!    - the `skip_next_instruction()` hints;
//!    - implementations of [`arbitrary::Arbitrary`](https://docs.rs/arbitrary/latest/arbitrary/) for some structs.
//! - `cairo-1-hints`: Enable hints that were introduced in Cairo 1. Not enabled by default.

#![cfg_attr(docsrs, feature(doc_cfg))]
#![deny(warnings)]
#![forbid(unsafe_code)]
#![cfg_attr(any(not(feature = "std")), no_std)]

#[cfg(feature = "std")]
include!("./with_std.rs");
#[cfg(not(feature = "std"))]
include!("./without_std.rs");

pub mod stdlib {
    pub mod collections {
        #[cfg(feature = "std")]
        pub use crate::with_std::collections::*;
        #[cfg(not(feature = "std"))]
        pub use crate::without_std::collections::*;
    }

    pub mod borrow {
        #[cfg(feature = "std")]
        pub use crate::with_std::borrow::*;
        #[cfg(not(feature = "std"))]
        pub use crate::without_std::borrow::*;
    }

    pub mod prelude {
        pub use crate::stdlib::{
            borrow::ToOwned,
            boxed::Box,
            clone::Clone,
            cmp::{Eq, PartialEq, Reverse},
            iter::IntoIterator,
            string::{String, ToString},
            vec::Vec,
        };
    }

    #[cfg(feature = "std")]
    pub use crate::with_std::*;
    #[cfg(not(feature = "std"))]
    pub use crate::without_std::*;
}

pub mod air_private_input;
pub mod air_public_input;
pub mod cairo_run;
pub mod hint_processor;
pub mod math_utils;
pub mod program_hash;
pub mod serde;
pub mod types;
pub mod utils;
pub mod vm;

// TODO: use `Felt` directly
pub use starknet_types_core::felt::Felt as Felt252;

#[cfg(test)]
mod tests;
