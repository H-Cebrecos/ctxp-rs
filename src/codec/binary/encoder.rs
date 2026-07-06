use std::io::{BufWriter, Result, Write};

use crate::{Encode, Event, EventKind, Source};

pub struct BinaryEncoder<W: Write> {
    writer: BufWriter<W>,
}

impl<W: Write> BinaryEncoder<W> {
    pub fn new(writer: W, sources: &[Source]) -> Result<Self> {
        let mut enc = Self {
            writer: BufWriter::new(writer),
        };
        enc.write_header()?;
        enc.write_metadata(sources)?;
        Ok(enc)
    }

    fn write_header(&mut self) -> Result<()> {
        const HEADER_SIZE: u16 = 8;
        const VERSION: u16 = 8;
        self.writer.write_all(b"CTXP")?;
        self.writer.write_all(&HEADER_SIZE.to_le_bytes())?;
        self.writer.write_all(&VERSION.to_le_bytes())?;
        Ok(())
    }

    //TODO: check
    fn write_metadata(&mut self, sources: &[Source]) -> Result<()> {
        // pre-compute total section length
        // 2 (SectionType) + 2 (SectionLength) + sum of (1 + 1 + name.len()) per source
        let payload_size: usize = sources.iter().map(|s| 1 + 1 + s.name.len()).sum();
        let section_length = 2 + 2 + payload_size;

        // header
        self.writer.write_all(&0x0001u16.to_le_bytes())?; // SectionType
        self.writer
            .write_all(&(section_length as u16).to_le_bytes())?; // SectionLength

        // entries
        for source in sources {
            let name = source.name.as_bytes(); // already UTF-8
            self.writer.write_all(&[source.id as u8])?; // SourceId
            self.writer.write_all(&[name.len() as u8])?; // NameLen
            self.writer.write_all(name)?; // Name
        }

        Ok(())
    }
}

impl<W: Write> Encode for BinaryEncoder<W> {
    fn write_event(&mut self, event: &Event) -> Result<()> {
        let type_byte = encode_type_byte(&event.kind, event.cycle.is_some())?;
        let v1 = event.value1.unwrap_or(0);
        let v2 = event.value2.unwrap_or(0);
        let cycle = event.cycle.unwrap_or(0);

        self.writer.write_all(&[event.source_id])?;
        self.writer.write_all(&[type_byte])?;
        self.writer.write_all(&v1.to_le_bytes())?; // 8 bytes
        self.writer.write_all(&v2.to_le_bytes())?; // 8 bytes
        self.writer.write_all(&cycle.to_le_bytes())?; // 8 bytes
        // total: 1 + 1 + 8 + 8 + 8 = 26 bytes ✓
        //TODO: proper assert here.
        Ok(())
    }

    fn flush(&mut self) -> Result<()> {
        self.writer.flush()?;
        Ok(())
    }
}

fn encode_type_byte(kind: &EventKind, with_timestamp: bool) -> Result<u8> {
    let base: u8 = match kind {
        EventKind::Sync => 0b0_000_0000,
        EventKind::BranchTaken => 0b0_001_0001,
        EventKind::BranchNotTaken => 0b0_001_0010,
        EventKind::Interrupt => 0b0_001_0011,
        EventKind::Rfi => 0b0_001_0101,
        EventKind::Call => 0b0_001_0110,
        EventKind::Return => 0b0_001_0111,
        EventKind::MemWrite(n) => match n {
            0 => 0b0_010_0000,
            1 => 0b0_010_0001,
            2 => 0b0_010_0010,
            4 => 0b0_010_0100,
            8 => 0b0_010_1000,
            _ => todo!("handle weird sizes"),
        },
        EventKind::MemRead(n) => match n {
            0 => 0b0_011_0000,
            1 => 0b0_011_0001,
            2 => 0b0_011_0010,
            4 => 0b0_011_0100,
            8 => 0b0_011_1000,
            _ => todo!("handle weird sizes"),
        },
        EventKind::Overflow => 0b0_101_1111,
        EventKind::Context => 0b0_100_0000,
        EventKind::WallClock => 0b0_100_0001,
        EventKind::Data => 0b0_110_0000,
        EventKind::Counter => 0b0_110_0001,
        EventKind::LastPC => 0b0_110_0010,
        EventKind::Info(n) => match n {
            1 => 0b0_111_0000,
            2 => 0b0_111_0001,
            3 => 0b0_111_0010,
            _ => todo!("handle weird sizes"),
        },
    };

    // MSB set if cycle is present
    Ok(if with_timestamp { base | 0x80 } else { base })
}
