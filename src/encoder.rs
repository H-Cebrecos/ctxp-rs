use std::{
    cell::RefCell,
    io::{BufWriter, Write},
    rc::Rc,
};

use crate::{AccessWidth, Event, EventKind, Format, InfoKind, Source, pack_counter};

struct EncoderState<W: Write> {
    writer: RefCell<BufWriter<W>>,
    sources: Vec<Source>,
}

impl<W: Write> EncoderState<W> {
    fn write_header_binary(&self) -> crate::Result<()> {
        const HEADER_SIZE: u16 = 8;
        const VERSION: u16 = 1;
        let mut writer = self.writer.borrow_mut();
        writer.write_all(b"CTXP")?;
        writer.write_all(&HEADER_SIZE.to_le_bytes())?;
        writer.write_all(&VERSION.to_le_bytes())?;
        Ok(())
    }

    fn write_metadata_binary(&self) -> crate::Result<()> {
        let payload_size: usize = self.sources.iter().map(|s| 1 + 1 + s.name.len()).sum();
        let section_length = 2 + 2 + payload_size;

        let mut writer = self.writer.borrow_mut();

        writer.write_all(&0x0001u16.to_le_bytes())?; // SectionType
        writer.write_all(&(section_length as u16).to_le_bytes())?; // SectionLength

        for source in &self.sources {
            let name = source.name.as_bytes();
            writer.write_all(&[source.id])?;
            writer.write_all(&[name.len() as u8])?;
            writer.write_all(name)?;
        }

        Ok(())
    }

    fn write_header_text(&self) -> crate::Result<()> {
        writeln!(
            self.writer.borrow_mut(),
            "HDR:format=accemic//ctxp-txt,ver=1"
        )?;
        Ok(())
    }

    fn write_metadata_text(&self) -> crate::Result<()> {
        let mut writer = self.writer.borrow_mut();
        write!(writer, "META:")?;
        for (i, source) in self.sources.iter().enumerate() {
            if i > 0 {
                write!(writer, ",")?;
            }
            let escaped = source.name.replace('\\', "\\\\").replace('"', "\\\"");
            write!(writer, "#{}=\"{}\"", source.id, &escaped)?;
        }
        writeln!(writer)?;
        Ok(())
    }

    fn write_event_text(&self, event: &Event) -> crate::Result<()> {
        writeln!(self.writer.borrow_mut(), "{}", event)?;
        Ok(())
    }

    fn write_event_binary(&self, event: &Event) -> crate::Result<()> {
        if !self.sources.iter().any(|s| s.id == event.source_id) {
            return Err(crate::Error::UnknownSource(event.source_id));
        }

        let (type_byte, v1, v2) = Self::encode_event_payload(&event.kind, event.cycle.is_some());
        let cycle_bytes: u64 = event.cycle.unwrap_or(0);

        let mut writer = self.writer.borrow_mut();
        writer.write_all(&[event.source_id])?;
        writer.write_all(&[type_byte])?;
        writer.write_all(&v1.to_le_bytes())?;
        writer.write_all(&v2.to_le_bytes())?;
        writer.write_all(&cycle_bytes.to_le_bytes())?;
        // total: 1 + 1 + 8 + 8 + 8 = 26 bytes

        Ok(())
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
}

/// Encodes [`Event`]s to an output stream.
pub struct Encoder<W: Write> {
    format: Format,
    state: Rc<EncoderState<W>>,
}

impl<W: Write> Encoder<W> {
    pub fn new(writer: W, sources: &[Source], format: Format) -> crate::Result<Self> {
        let state = EncoderState {
            writer: RefCell::new(BufWriter::new(writer)),
            sources: sources.to_vec(),
        };

        match format {
            Format::Binary => {
                state.write_header_binary()?;
                state.write_metadata_binary()?;
            }
            Format::Text => {
                state.write_header_text()?;
                state.write_metadata_text()?;
            }
        }
        state.writer.borrow_mut().flush()?;

        Ok(Self {
            state: Rc::new(state),
            format,
        })
    }

    /// Returns the event sources described by the stream metadata.
    pub fn sources(&self) -> &[crate::Source] {
        &self.state.sources
    }

    /// Encodes a single event.
    pub fn write_event(&self, event: &Event) -> crate::Result<()> {
        match self.format {
            Format::Binary => self.state.write_event_binary(event),
            Format::Text => self.state.write_event_text(event),
        }
    }

    /// Encodes all events produced by `events`, flushing after the last one.
    pub fn write_events(&self, events: &mut dyn Iterator<Item = &Event>) -> crate::Result<()> {
        for event in events {
            self.write_event(event)?;
        }
        self.flush()?;
        Ok(())
    }

    /// Flushes any buffered output to the underlying writer.
    pub fn flush(&self) -> crate::Result<()> {
        self.state.writer.borrow_mut().flush()?;
        Ok(())
    }

    /// Returns a handle scoped to `source_id`, which stamps every event
    /// written through it automatically.
    ///
    /// Returns [`Error::UnknownSource`] if `source_id` was not declared
    /// at construction.
    pub fn source(&self, source_id: u8) -> crate::Result<SourceHandle<W>> {
        if !self.sources().iter().any(|s| s.id == source_id) {
            Err(crate::Error::UnknownSource(source_id))
        } else {
            Ok(SourceHandle {
                id: source_id,
                format: self.format,
                state: Rc::clone(&self.state),
            })
        }
    }
}

pub struct SourceHandle<W: Write> {
    id: u8,
    format: Format,
    state: Rc<EncoderState<W>>,
}

impl<W: Write> SourceHandle<W> {
    pub fn write_event(&self, kind: EventKind, cycle: Option<u64>) -> crate::Result<()> {
        match self.format {
            Format::Binary => self.state.write_event_binary(&Event {
                source_id: self.id,
                kind,
                cycle,
            }),
            Format::Text => self.state.write_event_text(&Event {
                source_id: self.id,
                kind,
                cycle,
            }),
        }
    }

    /// Encodes all events produced by `events`. Stops on the first error.
    pub fn write_events(
        &self,
        events: impl IntoIterator<Item = (EventKind, Option<u64>)>,
    ) -> crate::Result<()> {
        for (kind, cycle) in events {
            self.write_event(kind, cycle)?;
        }
        Ok(())
    }
}
