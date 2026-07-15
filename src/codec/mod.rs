mod binary;
mod text;

pub use binary::*;
pub use text::*;

use std::{
    cell::RefCell, collections::HashMap, io::Write, iter::FusedIterator, marker::PhantomData,
    rc::Rc,
};

use crate::{Event, EventKind, Source};

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

pub struct BinaryEncoderTag;
pub struct TextEncoderTag;

struct Inner<W: Write, M> {
    writer: std::io::BufWriter<W>,
    sources: Vec<Source>,
    _marker: PhantomData<M>,
}

/// A handle scoped to one source, obtained via [`BinaryEncoder::source`].
/// Stamps every event with its source id automatically — the caller
/// never needs to specify it.
#[derive(Clone)]
pub struct SourceHandle<W: Write, M> {
    inner: Rc<RefCell<Inner<W, M>>>,
    source_id: u8,
}

impl<W: Write> SourceHandle<W, BinaryEncoderTag> {
    pub fn write_event(&self, kind: EventKind, cycle: Option<u64>) -> crate::Result<()> {
        self.inner
            .borrow_mut()
            .write_event(self.source_id, kind, cycle)
    }

    /// Encodes all events produced by `events`. Stops on the first error.
    pub fn write_events(
        &self,
        events: impl IntoIterator<Item = (EventKind, Option<u64>)>,
    ) -> crate::Result<()> {
        let mut inner = self.inner.borrow_mut();
        for (kind, cycle) in events {
            inner.write_event(self.source_id, kind, cycle)?;
        }
        Ok(())
    }
}
impl<W: Write> SourceHandle<W, TextEncoderTag> {
    pub fn write_event(&self, kind: EventKind, cycle: Option<u64>) -> crate::Result<()> {
        self.inner
            .borrow_mut()
            .write_event(self.source_id, kind, cycle)
    }

    /// Encodes all events produced by `events`. Stops on the first error.
    pub fn write_events(
        &self,
        events: impl IntoIterator<Item = (EventKind, Option<u64>)>,
    ) -> crate::Result<()> {
        let mut inner = self.inner.borrow_mut();
        for (kind, cycle) in events {
            inner.write_event(self.source_id, kind, cycle)?;
        }
        Ok(())
    }
}
