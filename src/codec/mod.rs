mod binary;
mod text;

pub use binary::*;
pub use text::*;

use std::{collections::HashMap, iter::FusedIterator};

use crate::Event;

/// Encodes [`Event`]s to an output stream.
///
/// Implemented by [`TextEncoder`] and [`BinaryEncoder`]. Useful for writing
/// code that is generic over the wire format, such as transcoding between
/// formats.
pub trait Encode {
    /// Encodes a single event.
    fn write_event(&self, event: &Event) -> crate::Result<()>;

    /// Encodes all events produced by `events`, flushing after the last one.
    fn write_events(&self, events: &mut dyn Iterator<Item = &Event>) -> crate::Result<()> {
        for event in events {
            self.write_event(event)?;
        }
        self.flush()?;
        Ok(())
    }

    /// Flushes any buffered output to the underlying writer.
    fn flush(&self) -> crate::Result<()>;
}

/// Decodes [`Event`]s from an input stream.
///
/// Implementations are [`FusedIterator`]s over `Result<Event>`. Once a decoder
/// reaches end-of-stream and returns `None`, it is guaranteed to never produce
/// another event.
///
/// The decoder also exposes the list of event sources parsed from the stream
/// metadata, available immediately after construction.
pub trait Decode: FusedIterator<Item = crate::Result<Event>> {
    /// Returns the event sources described by the stream metadata.
    fn sources(&self) -> &[crate::Source];

    fn demux<'a>(self) -> Demux<'a, Self>
    where
        Self: Sized,
    {
        Demux {
            decoder: self,
            handlers: HashMap::new(),
            unknown: None,
        }
    }
}

/// A push-based demultiplexer for decoded CTXP event streams.
///
/// Dispatches events to per-source handlers as they arrive from the
/// underlying decoder, processing the stream in a single pass with no
/// internal buffering.
///
/// # Why push and not pull
///
/// A pull-based API (independent iterator per source) would require
/// buffering an arbitrary number of events for sources that are not
/// being consumed, since the underlying stream is interleaved. For
/// long traces with many sources this could exhaust memory. The push
/// model avoids this entirely — each event is dispatched and dropped
/// immediately, keeping memory usage constant regardless of trace
/// length or source count.
pub struct Demux<'a, D: Decode> {
    decoder: D,
    handlers: HashMap<u8, Box<dyn FnMut(&Event) -> crate::Result<()> + 'a>>,
    unknown: Option<Box<dyn FnMut(&Event) -> crate::Result<()> + 'a>>,
}

impl<'a, D: Decode> Demux<'a, D> {
    pub fn on_source(
        mut self,
        source_id: u8,
        handler: impl FnMut(&Event) -> crate::Result<()> + 'a,
    ) -> Self {
        self.handlers.insert(source_id, Box::new(handler));
        self
    }

    /// Handler for events whose source id has no registered handler.
    /// If not set, unregistered events are silently skipped.
    pub fn on_unknown(mut self, handler: impl FnMut(&Event) -> crate::Result<()> + 'a) -> Self {
        self.unknown = Some(Box::new(handler));
        self
    }

    /// Consumes the decoder, dispatching each event to the appropriate
    /// handler. Stops and returns on the first error, whether from the
    /// decoder or a handler.
    pub fn run(mut self) -> crate::Result<()> {
        for event in self.decoder {
            let event = event?;
            match self.handlers.get_mut(&event.source_id) {
                Some(handler) => handler(&event)?,
                None => {
                    if let Some(ref mut h) = self.unknown {
                        h(&event)?;
                    }
                }
            }
        }
        Ok(())
    }
}
