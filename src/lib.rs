//! # ctxp
//!
//! A Rust library for encoding and decoding processor traces in the CTXP format.
//! CTXP attempts to be a unified, architecture-agnostic trace exchange format,
//! defining two wire representations of the same trace data: a UTF-8 text format
//! intended for human inspection, and a compact binary format intended for
//! efficient storage and real-time transcoding.
//!
//! For format details see the [CTXP specification](https://github.com/accemic/C-Trace-eXPort-format).
//!
//! ## Library structure
//!
//! - [`codec`]: Encoders and decoders for both CTXP representations
//!   ([`codec::text`] and [`codec::binary`]). Both expose the same
//!   [`codec::Encode`] trait so encoding targets are interchangeable.
//!
//! - [`transcoders`]: Ready-made transcoders that convert from other trace
//!   formats into CTXP. Each transcoder pairs an upstream decoder (e.g. for
//!   ARM ETMv4) with a CTXP encoder, presenting the pipeline as a single
//!   composable unit.
//!
//! - [`error`]: The library's unified error type [`error::Error`] and
//!   [`error::Result`] alias used throughout.
//!
//! ## Design notes
//!
//! Encoders are generic over [`std::io::Write`] and buffer internally, so they
//! can target files, sockets, in-memory buffers, or any other sink without
//! additional wrapping. Decoders are generic over [`std::io::Read`] and
//! implement [`Iterator`], making them natural sources in a processing pipeline.
//!
//! ## Status
//!
//! This library is under active development. Decoding and transcoding support
//! are not yet implemented.

pub mod codec;
pub use codec::*;

pub mod error;
pub use error::{Error, Result};
