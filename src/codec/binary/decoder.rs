use crate::unpack_counter;
use crate::{AccessWidth, Event, EventKind, InfoKind, Source};
use crate::{Decode, error};
use std::io::{BufReader, Read};
use std::iter::FusedIterator;

/// A streaming decoder for the CTXP binary format (`.ctxp`).
///
/// Wraps any [`Read`] source and parses tightly packed 26-byte event
/// records into [`Event`]s. The header and metadata sections are consumed
/// on construction and available via [`Decode::sources`]; after that the
/// decoder acts as an [`Iterator`] over [`Result<Event>`], yielding one
/// item per record.
///
/// The iterator is fused — on EOF or equivalent break in the underlying
/// stream all subsequent calls will return [`None`]. Errors are yielded
/// as [`Err`] and do not fuse the iterator. Note that EOF mid-record is
/// itself an error, as it indicates a truncated or malformed file.
#[derive(Debug)]
pub struct BinaryDecoder<R: Read> {
    reader: BufReader<R>,
    sources: Vec<Source>,
}

impl<R: Read> BinaryDecoder<R> {
    pub fn new(reader: R) -> error::Result<Self> {
        let mut dec = Self {
            reader: BufReader::new(reader),
            sources: Vec::new(),
        };
        dec.read_header()?;
        dec.read_metadata()?;
        Ok(dec)
    }

    fn read_header(&mut self) -> error::Result<()> {
        let mut magic = [0u8; 4];
        self.reader.read_exact(&mut magic)?;
        if &magic != b"CTXP" {
            return Err(error::Error::Parse(format!("invalid magic: {:?}", magic)));
        }
        let header_size = self.read_u16()?;
        let version = self.read_u16()?;
        if version != 1 {
            return Err(error::Error::Parse(format!(
                "unsupported version: {}",
                version
            )));
        }
        if header_size != 8 {
            return Err(error::Error::Parse(format!(
                "unexpected header size: {}",
                header_size
            )));
        }
        Ok(())
    }

    fn read_metadata(&mut self) -> error::Result<()> {
        let section_type = self.read_u16()?;
        let section_length = self.read_u16()?;

        if section_type != 0x0001 {
            return Err(error::Error::Parse(format!(
                "expected metadata section (0x0001), got {:#06x}",
                section_type
            )));
        }

        let payload_len = (section_length as usize)
            .checked_sub(4)
            .ok_or_else(|| error::Error::Parse("metadata section length too small".into()))?;

        let mut payload = vec![0u8; payload_len];
        self.reader.read_exact(&mut payload)?;

        let mut i = 0;
        while i < payload.len() {
            let source_id = payload[i];
            i += 1;
            let name_len = payload[i] as usize;
            i += 1;

            if i + name_len > payload.len() {
                return Err(error::Error::Parse("metadata name out of bounds".into()));
            }

            let name = std::str::from_utf8(&payload[i..i + name_len])
                .map_err(|_| error::Error::Parse("source name is not valid UTF-8".into()))?
                .to_string();
            i += name_len;

            self.sources.push(Source {
                id: source_id,
                name,
            });
        }

        Ok(())
    }

    fn read_event_after_source_id(&mut self, source_id: u8) -> error::Result<Event> {
        let type_byte = self.read_u8()?;
        let value1 = self.read_u64()?;
        let value2 = self.read_u64()?;
        let cycle_raw = self.read_u64()?;

        let has_cycle = (type_byte & 0x80) != 0;
        let base_type = type_byte & 0x7F;
        let cycle = if has_cycle { Some(cycle_raw) } else { None };
        let kind = decode_type_byte(base_type, value1, value2)?;

        Ok(Event {
            source_id,
            kind,
            cycle,
        })
    }

    fn read_u8(&mut self) -> error::Result<u8> {
        let mut buf = [0u8; 1];
        self.reader.read_exact(&mut buf)?;
        Ok(buf[0])
    }

    fn read_u16(&mut self) -> error::Result<u16> {
        let mut buf = [0u8; 2];
        self.reader.read_exact(&mut buf)?;
        Ok(u16::from_le_bytes(buf))
    }

    fn read_u64(&mut self) -> error::Result<u64> {
        let mut buf = [0u8; 8];
        self.reader.read_exact(&mut buf)?;
        Ok(u64::from_le_bytes(buf))
    }
}

impl<R: Read> Iterator for BinaryDecoder<R> {
    type Item = error::Result<Event>;

    fn next(&mut self) -> Option<Self::Item> {
        match self.read_u8() {
            Err(e) if e.is_eof() => None,
            Err(e) => Some(Err(e)),
            Ok(source_id) => Some(self.read_event_after_source_id(source_id)),
        }
    }
}

impl<R: Read> FusedIterator for BinaryDecoder<R> {}

impl<R: Read> Decode for BinaryDecoder<R> {
    fn sources(&self) -> &[Source] {
        &self.sources
    }
}

fn decode_type_byte(base: u8, value1: u64, value2: u64) -> error::Result<EventKind> {
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

        _ => Err(error::Error::Parse(format!(
            "unknown event type byte: {:#04x}",
            base
        ))),
    }
}
