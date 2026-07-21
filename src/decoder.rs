use std::{
    collections::HashMap,
    fs::File,
    io::{BufRead, BufReader},
    iter::FusedIterator,
    path::Path,
};

use crate::{AccessWidth, Error, Event, EventKind, Format, InfoKind, Source, unpack_counter};

struct DecoderState<R: BufRead> {
    reader: R,
    sources: Vec<Source>,
}

impl<R: BufRead> DecoderState<R> {
    fn read_u8(&mut self) -> crate::Result<u8> {
        let mut buf = [0u8; 1];
        self.reader.read_exact(&mut buf)?;
        Ok(buf[0])
    }

    fn read_u16(&mut self) -> crate::Result<u16> {
        let mut buf = [0u8; 2];
        self.reader.read_exact(&mut buf)?;
        Ok(u16::from_le_bytes(buf))
    }

    fn read_u64(&mut self) -> crate::Result<u64> {
        let mut buf = [0u8; 8];
        self.reader.read_exact(&mut buf)?;
        Ok(u64::from_le_bytes(buf))
    }

    fn read_header_binary(&mut self) -> crate::Result<()> {
        let mut magic = [0u8; 4];
        self.reader.read_exact(&mut magic)?;
        if &magic != b"CTXP" {
            return Err(Error::Parse(format!("invalid magic: {:?}", magic)));
        }
        let header_size = self.read_u16()?;
        let version = self.read_u16()?;
        if version != 1 {
            return Err(Error::Parse(format!("unsupported version: {}", version)));
        }
        if header_size != 8 {
            return Err(Error::Parse(format!(
                "unexpected header size: {}",
                header_size
            )));
        }
        Ok(())
    }

    fn read_metadata_binary(&mut self) -> crate::Result<()> {
        let section_type = self.read_u16()?;
        let section_length = self.read_u16()?;

        if section_type != 0x0001 {
            return Err(Error::Parse(format!(
                "expected metadata section (0x0001), got {:#06x}",
                section_type
            )));
        }

        let payload_len = (section_length as usize)
            .checked_sub(4)
            .ok_or_else(|| Error::Parse("metadata section length too small".into()))?;

        let mut payload = vec![0u8; payload_len];
        self.reader.read_exact(&mut payload)?;

        let mut i = 0;
        while i < payload.len() {
            let source_id = payload[i];
            i += 1;
            let name_len = payload[i] as usize;
            i += 1;

            if i + name_len > payload.len() {
                return Err(Error::Parse("metadata name out of bounds".into()));
            }

            let name = std::str::from_utf8(&payload[i..i + name_len])
                .map_err(|_| Error::Parse("source name is not valid UTF-8".into()))?
                .to_string();
            i += name_len;

            self.sources.push(Source {
                id: source_id,
                name,
            });
        }

        Ok(())
    }

    fn read_header_text(&mut self) -> crate::Result<()> {
        let mut line = String::new();
        self.reader.read_line(&mut line)?;
        if line.trim_end() != "HDR:format=accemic//ctxp-txt,ver=1" {
            return Err(Error::Parse(format!(
                "invalid or unsupported header: '{}'",
                line.trim_end()
            )));
        }
        Ok(())
    }

    fn read_metadata_text(&mut self) -> crate::Result<()> {
        let mut line = String::new();
        self.reader.read_line(&mut line)?;
        let line = line.trim_end();
        let entries = line
            .strip_prefix("META:")
            .ok_or_else(|| Error::Parse("expected META section".into()))?;
        self.sources = Self::parse_meta_entries(entries)?;
        Ok(())
    }

    fn parse_meta_entries(s: &str) -> crate::Result<Vec<Source>> {
        let mut sources = Vec::new();
        let mut chars = s.chars().peekable();

        loop {
            match chars.next() {
                Some('#') => {}
                None => break,
                Some(c) => return Err(Error::Parse(format!("expected '#', got '{}'", c))),
            }

            let mut id_str = String::new();
            loop {
                match chars.next() {
                    Some('=') => break,
                    Some(c) => id_str.push(c),
                    None => return Err(Error::Parse("unexpected end in source id".into())),
                }
            }
            let id = id_str
                .parse::<u8>()
                .map_err(|_| Error::Parse(format!("invalid source id: '{}'", id_str)))?;

            match chars.next() {
                Some('"') => {}
                _ => return Err(Error::Parse("expected '\"' after '='".into())),
            }

            let mut name = String::new();
            loop {
                match chars.next() {
                    Some('\\') => match chars.next() {
                        Some('"') => name.push('"'),
                        Some('\\') => name.push('\\'),
                        Some(c) => {
                            return Err(Error::Parse(format!(
                                "invalid escape sequence: '\\{}'",
                                c
                            )));
                        }
                        None => return Err(Error::Parse("unexpected end in escape".into())),
                    },
                    Some('"') => break,
                    Some(c) => name.push(c),
                    None => return Err(Error::Parse("unterminated source name".into())),
                }
            }

            sources.push(Source { id, name });

            match chars.peek() {
                Some(',') => {
                    chars.next();
                }
                _ => break,
            }
        }

        Ok(sources)
    }

    fn read_event_after_source_id(&mut self, source_id: u8) -> crate::Result<Event> {
        let type_byte = self.read_u8()?;
        let value1 = self.read_u64()?;
        let value2 = self.read_u64()?;
        let cycle_raw = self.read_u64()?;

        let has_cycle = (type_byte & 0x80) != 0;
        let base_type = type_byte & 0x7F;
        let cycle = if has_cycle { Some(cycle_raw) } else { None };
        let kind = Self::decode_type_byte(base_type, value1, value2)?;

        Ok(Event {
            source_id,
            kind,
            cycle,
        })
    }

    fn decode_type_byte(base: u8, value1: u64, value2: u64) -> crate::Result<EventKind> {
        match base {
            0b000_0000 => Ok(EventKind::Sync { target: value2 }),
            0b001_0001 => Ok(EventKind::BranchTaken {
                origin: value1,
                target: value2,
            }),
            0b001_0010 => Ok(EventKind::BranchNotTaken {
                origin: value1,
                target: value2,
            }),
            0b001_0011 => Ok(EventKind::Interrupt {
                origin: value1,
                target: value2,
            }),
            0b001_0101 => Ok(EventKind::Rfi {
                origin: value1,
                target: value2,
            }),
            0b001_0110 => Ok(EventKind::Call {
                origin: value1,
                target: value2,
            }),
            0b001_0111 => Ok(EventKind::Return {
                origin: value1,
                target: value2,
            }),

            0b010_0000 => Ok(EventKind::MemWriteUnknownData { addr: value1 }),
            0b010_0001 => Ok(EventKind::MemWrite {
                width: AccessWidth::W1,
                addr: Some(value1),
                value: value2,
            }),
            0b010_0010 => Ok(EventKind::MemWrite {
                width: AccessWidth::W2,
                addr: Some(value1),
                value: value2,
            }),
            0b010_0100 => Ok(EventKind::MemWrite {
                width: AccessWidth::W4,
                addr: Some(value1),
                value: value2,
            }),
            0b010_1000 => Ok(EventKind::MemWrite {
                width: AccessWidth::W8,
                addr: Some(value1),
                value: value2,
            }),

            0b011_0000 => Ok(EventKind::MemReadUnknownData { addr: value1 }),
            0b011_0001 => Ok(EventKind::MemRead {
                width: AccessWidth::W1,
                addr: Some(value1),
                value: value2,
            }),
            0b011_0010 => Ok(EventKind::MemRead {
                width: AccessWidth::W2,
                addr: Some(value1),
                value: value2,
            }),
            0b011_0100 => Ok(EventKind::MemRead {
                width: AccessWidth::W4,
                addr: Some(value1),
                value: value2,
            }),
            0b011_1000 => Ok(EventKind::MemRead {
                width: AccessWidth::W8,
                addr: Some(value1),
                value: value2,
            }),

            0b101_1111 => Ok(EventKind::Overflow),
            0b100_0000 => Ok(EventKind::Context { value: value2 }),
            0b100_0001 => Ok(EventKind::WallClock { value: value2 }),

            0b110_0000 => Ok(EventKind::Data { tag: value2 }),
            0b110_0001 => {
                let (kind, region, tag) = unpack_counter(value2)?;
                Ok(EventKind::Counter {
                    count: value1,
                    kind,
                    region,
                    tag,
                })
            }
            0b110_0010 => Ok(EventKind::LastPC { prev_pc: value2 }),

            0b111_0000 => Ok(EventKind::Info {
                kind: InfoKind::I1,
                value1,
                value2,
            }),
            0b111_0001 => Ok(EventKind::Info {
                kind: InfoKind::I2,
                value1,
                value2,
            }),
            0b111_0010 => Ok(EventKind::Info {
                kind: InfoKind::I3,
                value1,
                value2,
            }),

            _ => Err(Error::Parse(format!(
                "unknown event type byte: {:#04x}",
                base
            ))),
        }
    }
}

/// Decodes [`Event`]s from an input stream.
///
/// Implementations are [`FusedIterator`]s over `Result<Event>`. Once a decoder
/// reaches end-of-stream and returns `None`, it is guaranteed to never produce
/// another event.
///
/// The decoder also exposes the list of event sources parsed from the stream
/// metadata, available immediately after construction.
pub struct Decoder<R: BufRead> {
    format: Format,
    state: DecoderState<R>,
}

impl<R: BufRead> Decoder<R> {
    pub fn new(reader: R, format: Format) -> crate::Result<Self> {
        let mut state = DecoderState {
            reader: reader,
            sources: Vec::new(),
        };

        match format {
            Format::Binary => {
                state.read_header_binary()?;
                state.read_metadata_binary()?;
            }
            Format::Text => {
                state.read_header_text()?;
                state.read_metadata_text()?;
            }
        }

        Ok(Self { format, state })
    }

    /// Returns the event sources described by the stream metadata.
    pub fn sources(&self) -> &[crate::Source] {
        &self.state.sources
    }

    pub fn demux<'a>(self) -> Demux<'a, R> {
        Demux {
            dec: self,
            handlers: HashMap::new(),
            on_unhandled: None,
        }
    }
}

impl Decoder<BufReader<File>> {
    pub fn open(path: &Path) -> crate::Result<Self> {
        let file = BufReader::new(File::open(path)?);
        let format = Self::detect_format(path)?;
        Self::new(file, format)
    }

    pub fn detect_format(path: &Path) -> crate::Result<Format> {
        let name = path
            .file_name()
            .and_then(|n| n.to_str())
            .ok_or_else(|| Error::InvalidFileName)?;
        if name.ends_with(".ctxp.txt") {
            Ok(Format::Text)
        } else if name.ends_with(".ctxp") {
            Ok(Format::Binary)
        } else {
            Err(Error::InvalidFileExtension(String::from(name)))
        }
    }
}

impl<R: BufRead> Iterator for Decoder<R> {
    type Item = crate::Result<Event>;

    fn next(&mut self) -> Option<Self::Item> {
        match &self.format {
            Format::Binary => match self.state.read_u8() {
                Err(e) if e.is_eof() => None,
                Err(e) => Some(Err(e)),
                Ok(source_id) => Some(self.state.read_event_after_source_id(source_id)),
            },
            Format::Text => {
                let mut line = String::new();
                match self.state.reader.read_line(&mut line) {
                    Ok(0) => None,
                    Ok(_) => Some(line.trim_end().parse::<Event>()),
                    Err(e) => Some(Err(e.into())),
                }
            }
        }
    }
}

impl<R: BufRead> FusedIterator for Decoder<R> {}

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
pub struct Demux<'a, R: BufRead> {
    dec: Decoder<R>,
    handlers: HashMap<u8, Box<dyn FnMut(Event) -> Result<(), Error> + 'a>>,
    on_unhandled: Option<Box<dyn FnMut(Event) -> Result<(), Error> + 'a>>,
}

impl<'a, R: BufRead> Demux<'a, R> {
    pub fn on_source<F>(&mut self, id: u8, handler: F) -> &mut Self
    where
        F: FnMut(Event) -> Result<(), Error> + 'a,
    {
        self.handlers.insert(id, Box::new(handler));
        self
    }

    pub fn on_unhandled<F>(&mut self, handler: F) -> &mut Self
    where
        F: FnMut(Event) -> Result<(), Error> + 'a,
    {
        self.on_unhandled = Some(Box::new(handler));
        self
    }

    /// Drives the decoder to completion, dispatching each event.
    /// Stops at the first decode error or handler error.
    pub fn run(mut self) -> crate::Result<()> {
        for event in self.dec {
            let event = event?;
            match self.handlers.get_mut(&event.source_id) {
                Some(h) => h(event)?,
                None => {
                    if let Some(h) = self.on_unhandled.as_mut() {
                        h(event)?;
                    }
                    // else: silently drop — or make this a hard error,
                    // see below
                }
            }
        }
        Ok(())
    }
}
