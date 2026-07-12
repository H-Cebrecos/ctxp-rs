use std::{
    cell::RefCell,
    io::{BufWriter, Write},
    rc::Rc,
};

use crate::{AccessWidth, Encode, Event, EventKind, InfoKind, Source, error, pack_counter};

struct Inner<W: Write> {
    writer: BufWriter<W>,
    sources: Vec<Source>,
}

impl<W: Write> Inner<W> {
    fn write_event(
        &mut self,
        source_id: u8,
        kind: EventKind,
        cycle: Option<u64>,
    ) -> error::Result<()> {
        if !self.sources.iter().any(|s| s.id == source_id) {
            return Err(error::Error::UnknownSource(source_id));
        }

        let (type_byte, v1, v2) = encode_event_payload(&kind, cycle.is_some());
        let cycle_bytes: u64 = cycle.unwrap_or(0);

        self.writer.write_all(&[source_id])?;
        self.writer.write_all(&[type_byte])?;
        self.writer.write_all(&v1.to_le_bytes())?;
        self.writer.write_all(&v2.to_le_bytes())?;
        self.writer.write_all(&cycle_bytes.to_le_bytes())?;
        // total: 1 + 1 + 8 + 8 + 8 = 26 bytes

        Ok(())
    }

    fn flush(&mut self) -> error::Result<()> {
        self.writer.flush()?;
        Ok(())
    }

    fn write_header(&mut self) -> error::Result<()> {
        const HEADER_SIZE: u16 = 8;
        const VERSION: u16 = 1;
        self.writer.write_all(b"CTXP")?;
        self.writer.write_all(&HEADER_SIZE.to_le_bytes())?;
        self.writer.write_all(&VERSION.to_le_bytes())?;
        Ok(())
    }

    fn write_metadata(&mut self) -> error::Result<()> {
        let payload_size: usize = self.sources.iter().map(|s| 1 + 1 + s.name.len()).sum();
        let section_length = 2 + 2 + payload_size;

        self.writer.write_all(&0x0001u16.to_le_bytes())?; // SectionType
        self.writer
            .write_all(&(section_length as u16).to_le_bytes())?; // SectionLength

        for source in &self.sources {
            let name = source.name.as_bytes();
            self.writer.write_all(&[source.id])?;
            self.writer.write_all(&[name.len() as u8])?;
            self.writer.write_all(name)?;
        }

        Ok(())
    }
}

/// A streaming encoder for the CTXP binary format (`.ctxp`).
///
/// Wraps any [`Write`] sink and emits tightly packed 26-byte event records.
/// Sources are declared at construction and fixed for the encoder's
/// lifetime — the encoder is ready to accept events immediately.
///
/// Cheaply cloneable: cloning shares the same underlying writer and source
/// list, so multiple [`SourceHandle`]s (or clones of the encoder itself)
/// can coexist and write concurrently in a single-threaded context.
///
/// Buffers output internally — call [`Encode::flush`] when done to ensure
/// all data reaches the underlying writer.
#[derive(Clone)]
pub struct BinaryEncoder<W: Write> {
    inner: Rc<RefCell<Inner<W>>>,
}

impl<W: Write> BinaryEncoder<W> {
    /// Declares `sources` and writes the header and metadata immediately.
    /// The encoder is ready to accept events as soon as this returns.
    pub fn new(writer: W, sources: &[Source]) -> error::Result<Self> {
        let mut inner = Inner {
            writer: BufWriter::new(writer),
            sources: sources.to_vec(),
        };
        inner.write_header()?;
        inner.write_metadata()?;
        Ok(Self {
            inner: Rc::new(RefCell::new(inner)),
        })
    }

    /// Returns a handle scoped to `source_id`, which stamps every event
    /// written through it automatically.
    ///
    /// Returns [`Error::UnknownSource`] if `source_id` was not declared
    /// at construction.
    pub fn source(&self, source_id: u8) -> error::Result<SourceHandle<W>> {
        if !self
            .inner
            .borrow()
            .sources
            .iter()
            .any(|s| s.id == source_id)
        {
            return Err(error::Error::UnknownSource(source_id));
        }
        Ok(SourceHandle {
            inner: Rc::clone(&self.inner),
            source_id,
        })
    }
}

impl<W: Write> Encode for BinaryEncoder<W> {
    fn write_event(&self, event: &Event) -> error::Result<()> {
        self.inner
            .borrow_mut()
            .write_event(event.source_id, event.kind.clone(), event.cycle)
    }

    fn flush(&self) -> error::Result<()> {
        self.inner.borrow_mut().flush()
    }
}

/// A handle scoped to one source, obtained via [`BinaryEncoder::source`].
/// Stamps every event with its source id automatically — the caller
/// never needs to specify it.
#[derive(Clone)]
pub struct SourceHandle<W: Write> {
    inner: Rc<RefCell<Inner<W>>>,
    source_id: u8,
}

impl<W: Write> SourceHandle<W> {
    pub fn write_event(&self, kind: EventKind, cycle: Option<u64>) -> error::Result<()> {
        self.inner
            .borrow_mut()
            .write_event(self.source_id, kind, cycle)
    }

    /// Encodes all events produced by `events`. Stops on the first error.
    pub fn write_events(
        &self,
        events: impl IntoIterator<Item = (EventKind, Option<u64>)>,
    ) -> error::Result<()> {
        let mut inner = self.inner.borrow_mut();
        for (kind, cycle) in events {
            inner.write_event(self.source_id, kind, cycle)?;
        }
        Ok(())
    }
}

fn encode_event_payload(kind: &EventKind, with_timestamp: bool) -> (u8, u64, u64) {
    let (base, v1, v2) = match kind {
        EventKind::Sync { target } => (0b000_0000u8, 0u64, *target),
        EventKind::Interrupt { origin, target } => (0b001_0011, *origin, *target),
        EventKind::Rfi { origin, target } => (0b001_0101, *origin, *target),
        EventKind::BranchTaken { origin, target } => (0b001_0001, *origin, *target),
        EventKind::BranchNotTaken { origin, target } => (0b001_0010, *origin, *target),
        EventKind::Call { origin, target } => (0b001_0110, *origin, *target),
        EventKind::Return { origin, target } => (0b001_0111, *origin, *target),

        EventKind::MemReadUnknownData { addr } => (0b011_0000, *addr, 0),
        EventKind::MemRead { width, addr, value } => (
            match width {
                AccessWidth::W1 => 0b011_0001,
                AccessWidth::W2 => 0b011_0010,
                AccessWidth::W4 => 0b011_0100,
                AccessWidth::W8 => 0b011_1000,
            },
            addr.unwrap_or(u64::MAX),
            *value,
        ),

        EventKind::MemWriteUnknownData { addr } => (0b010_0000, *addr, 0),
        EventKind::MemWrite { width, addr, value } => (
            match width {
                AccessWidth::W1 => 0b010_0001,
                AccessWidth::W2 => 0b010_0010,
                AccessWidth::W4 => 0b010_0100,
                AccessWidth::W8 => 0b010_1000,
            },
            addr.unwrap_or(u64::MAX),
            *value,
        ),

        EventKind::Overflow => (0b101_1111, 0, 0),
        EventKind::Context { value } => (0b100_0000, 0, *value),
        EventKind::WallClock { value } => (0b100_0001, 0, *value),

        EventKind::Info {
            kind,
            value1,
            value2,
        } => (
            match kind {
                InfoKind::I1 => 0b111_0000,
                InfoKind::I2 => 0b111_0001,
                InfoKind::I3 => 0b111_0010,
            },
            *value1,
            *value2,
        ),

        EventKind::Data { tag } => (0b110_0000, 0, *tag),
        EventKind::Counter {
            count,
            kind,
            region,
            tag,
        } => (0b110_0001, *count, pack_counter(kind, *region, *tag)),
        EventKind::LastPC { prev_pc } => (0b110_0010, 0, *prev_pc),
    };

    let event_type = if with_timestamp { base | 0x80 } else { base };
    (event_type, v1, v2)
}
