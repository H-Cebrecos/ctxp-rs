//! # ctxp
//!
//! A Rust library for encoding and decoding processor traces in the CTXP format,
//! a unified, architecture-agnostic trace exchange format. CTXP has two wire
//! representations of the same trace data: a text format for human inspection,
//! and a compact binary format for efficient storage and real-time processing.
//!
//! For format details see the [CTXP specification](https://github.com/accemic/C-Trace-eXPort-format).
//!
//! This library provides an encoder and decoder for each representation.
//! All four types share the [`Encode`] and [`Decode`] traits, so code that
//! works with one representation works with the other unchanged.
//!
//! ## Event model
//!
//! CTXP traces are streams of [`Event`]s. Each event belongs to a **source** —
//! a numbered trace producer such as a CPU core or DAQ channel — and carries
//! an [`EventKind`] describing what happened. Events from different sources
//! are interleaved in the stream in the order they were recorded.
//!
//! Sources are declared once, up front, as a name-and-id pair. All events for
//! a source must reference a source id that was declared.
//!
//! [`EventKind`] covers several categories:
//!
//! - **Control flow** — branches, calls, returns, interrupts, and syncs,
//!   each carrying origin and target addresses.
//! - **Memory** — reads and writes, carrying address, value, and access width.
//! - **DAQ/Instrumentation** — counter readouts, data captures, and last-PC
//!   records for performance analysis and instrumentation.
//! - **Trace control** — overflow markers and context switches that describe
//!   the state of the trace itself rather than the traced program.
//! - **User-defined** — `Info` events for application-specific annotations.
//!
//! Every event carries an optional cycle count timestamp.
//!
//! ## Usage
//!
//! ### Direct encoding
//!
//! Construct an encoder with the full set of trace sources. The header and
//! metadata sections are written immediately, and the encoder is ready to
//! accept events as soon as construction returns:
//!
//! ```
//! use ctxp::{Encode, Event, EventKind, Source, TextEncoder};
//!
//! let sources = vec![
//!     Source { id: 0, name: "CPU0".into() },
//!     Source { id: 1, name: "CPU1".into() },
//! ];
//!
//! let enc = TextEncoder::new(std::io::stdout(), &sources)?;
//! enc.write_event(&Event {
//!     source_id: 0,
//!     kind: EventKind::Sync { target: 0x8000_0000 },
//!     cycle: Some(0),
//! })?;
//!
//! enc.flush()?;
//! # Ok::<(), ctxp::Error>(())
//! ```
//!
//! ### Source handles
//!
//! For code that produces events per source — a common pattern when tracing
//! multiple cores — call [`TextEncoder::source`] (or the binary equivalent)
//! to get a handle scoped to one source id. The handle stamps every event
//! with that id automatically, and multiple handles can be used interleaved
//! without any special setup:
//!
//! ```no_run
//! use ctxp::{EventKind, Source, TextEncoder, Encode};
//!
//! let sources = vec![
//!     Source { id: 0, name: "CPU0".into() },
//!     Source { id: 1, name: "CPU1".into() },
//! ];
//!
//! let enc = TextEncoder::new(std::fs::File::create("trace.ctxp-txt")?, &sources)?;
//! let cpu0 = enc.source(0)?;
//! let cpu1 = enc.source(1)?;
//!
//! cpu0.write_event(EventKind::Sync { target: 0x8000_0000 }, Some(0))?;
//! cpu1.write_event(EventKind::BranchTaken { origin: 0x8000_3d00, target: 0x8000_298c }, Some(24))?;
//! cpu0.write_event(EventKind::WallClock { value: 0x12000 }, Some(10))?;
//!
//! enc.flush()?;
//! # Ok::<(), ctxp::Error>(())
//! ```
//!
//! ### Transcoding
//!
//! Decoders implement [`Iterator`] and encoders implement [`Encode`], so
//! converting between formats is a single loop. The decoded source list
//! is passed straight to the new encoder:
//!
//! ```no_run
//! use ctxp::{BinaryEncoder, Decode, Encode, TextDecoder};
//!
//! let dec = TextDecoder::new(std::fs::File::open("trace.ctxp-txt")?)?;
//! let enc = BinaryEncoder::new(std::fs::File::create("trace.ctxp")?, dec.sources())?;
//!
//! for event in dec {
//!     enc.write_event(&event?)?;
//! }
//! enc.flush()?;
//! # Ok::<(), ctxp::Error>(())
//! ```
//!
//! ### Demultiplexing
//!
//! Since events from multiple sources are interleaved in the stream,
//! [`Decode::demux`] provides a push-based dispatcher that routes each event
//! to a per-source handler in a single pass, with no internal buffering.
//! This is useful for splitting one trace into per-source outputs:
//!
//! ```no_run
//! use ctxp::{BinaryDecoder, Encode, Decode, Source, TextEncoder};
//!
//! let sources = vec![
//!     Source { id: 0, name: "CPU0".into() },
//!     Source { id: 1, name: "CPU1".into() },
//! ];
//!
//! let enc = TextEncoder::new(std::fs::File::create("trace.ctxp-txt")?, &sources)?;
//! let cpu0 = enc.source(0)?;
//! let cpu1 = enc.source(1)?;
//!
//! BinaryDecoder::new(std::fs::File::open("trace.ctxp")?)?.demux()
//!     .on_source(0, |event| cpu0.write_event(event.kind.clone(), event.cycle))
//!     .on_source(1, |event| cpu1.write_event(event.kind.clone(), event.cycle))
//!     .run()?;
//!
//! enc.flush()?;
//! # Ok::<(), ctxp::Error>(())
//! ```
//!
//! ## Design notes
//!
//! Encoders are generic over [`std::io::Write`] and buffer internally, so
//! they can target files, sockets, in-memory buffers, or any other sink
//! without additional wrapping. Decoders are generic over [`std::io::Read`]
//! and implement [`Iterator`], making them natural sources in a processing
//! pipeline — including live, real-time streams, since decoding is pull-based
//! and encoding is push-based with no batching required.
//!
//! Encoders are cheaply cloneable and their methods take `&self`: cloning an
//! encoder or obtaining multiple source handles gives you multiple references
//! to the same underlying writer, not independent copies.
//!
//! ## Status
//!
//! This library is under active development.

mod codec;
mod error;
mod event;

pub use codec::*;
pub use error::{Error, Result};
pub use event::*;
