//! An implementation of the Cairo virtual machine
//!
//! # Feature Flags
//! - `skip_next_instruction_hint`: Enable the `skip_next_instruction()` hint. Not enabled by default.
//! - `hooks`: Enable [Hooks](vm::hooks) support for the [VirtualMachine](vm::vm_core::VirtualMachine). Not enabled by default.
//! - `with_mimalloc`: Use [MiMalloc](https://crates.io/crates/mimalloc) as the program global allocator.

#![cfg_attr(docsrs, feature(doc_cfg))]
#![deny(warnings)]
#![forbid(unsafe_code)]
#![cfg_attr(any(target_arch = "wasm32", not(feature = "std")), no_std)]

#[cfg(feature = "std")]
include!("./with_std.rs");
#[cfg(not(feature = "std"))]
include!("./without_std.rs");

mod stdlib {
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

pub mod write {
    pub use bincode::enc::write::SliceWriter;
    pub use bincode::enc::write::Writer;
    pub use bincode::error::EncodeError;

    pub struct VecWriter {
        buf: Vec<u8>,
    }

    impl VecWriter {
        pub fn new() -> Self {
            Self { buf: Vec::new() }
        }

        pub fn with_capacity(cap: usize) -> Self {
            Self {
                buf: Vec::with_capacity(cap),
            }
        }

        pub fn as_slice(&self) -> &[u8] {
            &self.buf
        }
    }

    impl Writer for VecWriter {
        fn write(&mut self, bytes: &[u8]) -> Result<(), bincode::error::EncodeError> {
            self.buf.extend_from_slice(bytes);
            Ok(())
        }
    }

    impl Into<Vec<u8>> for VecWriter {
        fn into(self) -> Vec<u8> {
            self.buf
        }
    }

    #[cfg(feature = "std")]
    use std::io::Write;

    #[cfg(feature = "std")]
    pub struct BufWriter<T: Write> {
        buf_writer: std::io::BufWriter<T>,
        bytes_written: usize,
    }

    #[cfg(feature = "std")]
    impl<T: Write> Writer for BufWriter<T> {
        fn write(&mut self, bytes: &[u8]) -> Result<(), EncodeError> {
            self.buf_writer
                .write_all(bytes)
                .map_err(|e| EncodeError::Io {
                    inner: e,
                    index: self.bytes_written,
                })?;

            self.bytes_written += bytes.len();

            Ok(())
        }
    }

    #[cfg(feature = "std")]
    impl<T: Write> BufWriter<T> {
        pub fn new(writer: T) -> Self {
            Self {
                buf_writer: std::io::BufWriter::new(writer),
                bytes_written: 0,
            }
        }

        pub fn with_capacity(cap: usize, writer: T) -> Self {
            Self {
                buf_writer: std::io::BufWriter::with_capacity(cap, writer),
                bytes_written: 0,
            }
        }

        pub fn flush(&mut self) -> std::io::Result<()> {
            self.buf_writer.flush()
        }
    }
}

pub extern crate felt;
pub mod cairo_run;
pub mod hint_processor;
pub mod math_utils;
pub mod serde;
pub mod types;
pub mod utils;
pub mod vm;

#[cfg(test)]
mod tests;
