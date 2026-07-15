use core::fmt;
use std::str::FromStr;

pub use encoder::*;

use crate::{AccessWidth, Event, EventKind, InfoKind, error, pack_counter, unpack_counter};

impl fmt::Display for Event {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let payload = match &self.kind {
            EventKind::Sync { target } => {
                format!("SYNC::{:#x}", target)
            }
            EventKind::Interrupt { origin, target } => {
                format!("INTERRUPT:{:#x}:{:#x}", origin, target)
            }
            EventKind::Rfi { origin, target } => {
                format!("RFI:{:#x}:{:#x}", origin, target)
            }
            EventKind::BranchTaken { origin, target } => {
                format!("BRANCH_TAKEN:{:#x}:{:#x}", origin, target)
            }
            EventKind::BranchNotTaken { origin, target } => {
                format!("BRANCH_NOTTAKEN:{:#x}:{:#x}", origin, target)
            }
            EventKind::Call { origin, target } => {
                format!("CALL:{:#x}:{:#x}", origin, target)
            }
            EventKind::Return { origin, target } => {
                format!("RETURN:{:#x}:{:#x}", origin, target)
            }
            EventKind::MemReadUnknownData { addr } => {
                format!("MEMREAD_0:{:#x}:", addr)
            }
            EventKind::MemRead { width, addr, value } => match width {
                AccessWidth::W1 => format!(
                    "MEMREAD_1:{}:{:#x}",
                    addr.map(|a| format!("{:#x}", a)).unwrap_or_default(),
                    value
                ),
                AccessWidth::W2 => format!(
                    "MEMREAD_2:{}:{:#x}",
                    addr.map(|a| format!("{:#x}", a)).unwrap_or_default(),
                    value
                ),
                AccessWidth::W4 => format!(
                    "MEMREAD_4:{}:{:#x}",
                    addr.map(|a| format!("{:#x}", a)).unwrap_or_default(),
                    value
                ),
                AccessWidth::W8 => format!(
                    "MEMREAD_8:{}:{:#x}",
                    addr.map(|a| format!("{:#x}", a)).unwrap_or_default(),
                    value
                ),
            },
            EventKind::MemWriteUnknownData { addr } => {
                format!("MEMWRITE_0:{:#x}:", addr)
            }
            EventKind::MemWrite { width, addr, value } => match width {
                AccessWidth::W1 => format!(
                    "MEMWRITE_1:{}:{:#x}",
                    addr.map(|a| format!("{:#x}", a)).unwrap_or_default(),
                    value
                ),
                AccessWidth::W2 => format!(
                    "MEMWRITE_2:{}:{:#x}",
                    addr.map(|a| format!("{:#x}", a)).unwrap_or_default(),
                    value
                ),
                AccessWidth::W4 => format!(
                    "MEMWRITE_4:{}:{:#x}",
                    addr.map(|a| format!("{:#x}", a)).unwrap_or_default(),
                    value
                ),
                AccessWidth::W8 => format!(
                    "MEMWRITE_8:{}:{:#x}",
                    addr.map(|a| format!("{:#x}", a)).unwrap_or_default(),
                    value
                ),
            },
            EventKind::Overflow => "OVERFLOW::".into(),
            EventKind::Context { value } => {
                format!("CONTEXT::{:#x}", value)
            }
            EventKind::WallClock { value } => {
                format!("WALLCLOCK::{:#x}", value)
            }
            EventKind::Info {
                kind,
                value1,
                value2,
            } => match kind {
                InfoKind::I1 => format!("INFO_1:{:#x}:{:#x}", value1, value2),
                InfoKind::I2 => format!("INFO_2:{:#x}:{:#x}", value1, value2),
                InfoKind::I3 => format!("INFO_3:{:#x}:{:#x}", value1, value2),
            },
            EventKind::Data { tag } => {
                format!("DAQ_DATA::{:#x}", tag)
            }
            EventKind::Counter {
                count,
                kind,
                region,
                tag,
            } => {
                format!(
                    "DAQ_COUNTER:{:#x}:{:#x}",
                    count,
                    pack_counter(kind, *region, *tag)
                )
            }
            EventKind::LastPC { prev_pc } => {
                format!("DAQ_PC::{:#x}", prev_pc)
            }
        };

        write!(
            f,
            "{:<50} {}",
            format!("#{}:{}", self.source_id, payload),
            self.cycle.map(|v| format!("@ {}", v)).unwrap_or_default(),
        )?;
        Ok(())
    }
}

impl FromStr for Event {
    type Err = error::Error;

    fn from_str(s: &str) -> error::Result<Self> {
        // split event body and optional cycle
        let (body, cycle) = match s.split_once(" @ ") {
            Some((b, c)) => (
                b,
                Some(
                    c.trim()
                        .parse::<u64>()
                        .map_err(|_| error::Error::Parse("invalid cycle count".into()))?,
                ),
            ),
            None => (s.trim_end(), None),
        };

        // strip leading '#'
        let body = body
            .strip_prefix('#')
            .ok_or_else(|| error::Error::Parse("expected '#'".into()))?;

        // split source_id from the rest
        let (source_str, rest) = body
            .split_once(':')
            .ok_or_else(|| error::Error::Parse("missing source_id".into()))?;
        let source_id = source_str
            .parse::<u8>()
            .map_err(|_| error::Error::Parse(format!("invalid source_id: '{}'", source_str)))?;

        // remainder has the form <TYPE>:<VALUE1>?:<VALUE2>?
        let mut parts = rest.splitn(3, ':');

        let kind = parts
            .next()
            .ok_or_else(|| error::Error::Parse("missing event kind".into()))?;
        let v1 = parts.next().unwrap_or("").trim();
        let v2 = parts.next().unwrap_or("").trim();

        // helper closures
        let hex = |s: &str| -> error::Result<u64> {
            let s = s.trim().strip_prefix("0x").unwrap_or(s);
            u64::from_str_radix(s, 16)
                .map_err(|_| error::Error::Parse(format!("invalid hex: '{}'", s)))
        };
        let hex_opt = |s: &str| -> error::Result<Option<u64>> {
            if s.is_empty() {
                Ok(None)
            } else {
                Ok(Some(hex(s)?))
            }
        };

        let kind = match kind {
            "SYNC" => Ok(EventKind::Sync { target: hex(v2)? }),
            "INTERRUPT" => Ok(EventKind::Interrupt {
                origin: hex(v1)?,
                target: hex(v2)?,
            }),
            "RFI" => Ok(EventKind::Rfi {
                origin: hex(v1)?,
                target: hex(v2)?,
            }),
            "BRANCH_TAKEN" => Ok(EventKind::BranchTaken {
                origin: hex(v1)?,
                target: hex(v2)?,
            }),
            "BRANCH_NOTTAKEN" => Ok(EventKind::BranchNotTaken {
                origin: hex(v1)?,
                target: hex(v2)?,
            }),
            "CALL" => Ok(EventKind::Call {
                origin: hex(v1)?,
                target: hex(v2)?,
            }),
            "RETURN" => Ok(EventKind::Return {
                origin: hex(v1)?,
                target: hex(v2)?,
            }),
            "MEMREAD_0" => Ok(EventKind::MemReadUnknownData { addr: hex(v1)? }),
            "MEMREAD_1" => Ok(EventKind::MemRead {
                width: AccessWidth::W1,
                addr: hex_opt(v1)?,
                value: hex(v2)?,
            }),
            "MEMREAD_2" => Ok(EventKind::MemRead {
                width: AccessWidth::W2,
                addr: hex_opt(v1)?,
                value: hex(v2)?,
            }),
            "MEMREAD_4" => Ok(EventKind::MemRead {
                width: AccessWidth::W4,
                addr: hex_opt(v1)?,
                value: hex(v2)?,
            }),
            "MEMREAD_8" => Ok(EventKind::MemRead {
                width: AccessWidth::W8,
                addr: hex_opt(v1)?,
                value: hex(v2)?,
            }),
            "MEMWRITE_0" => Ok(EventKind::MemWriteUnknownData { addr: hex(v1)? }),
            "MEMWRITE_1" => Ok(EventKind::MemWrite {
                width: AccessWidth::W1,
                addr: hex_opt(v1)?,
                value: hex(v2)?,
            }),
            "MEMWRITE_2" => Ok(EventKind::MemWrite {
                width: AccessWidth::W2,
                addr: hex_opt(v1)?,
                value: hex(v2)?,
            }),
            "MEMWRITE_4" => Ok(EventKind::MemWrite {
                width: AccessWidth::W4,
                addr: hex_opt(v1)?,
                value: hex(v2)?,
            }),
            "MEMWRITE_8" => Ok(EventKind::MemWrite {
                width: AccessWidth::W8,
                addr: hex_opt(v1)?,
                value: hex(v2)?,
            }),
            "OVERFLOW" => Ok(EventKind::Overflow),
            "CONTEXT" => Ok(EventKind::Context { value: hex(v2)? }),
            "WALLCLOCK" => Ok(EventKind::WallClock { value: hex(v2)? }),
            "INFO_1" => Ok(EventKind::Info {
                kind: InfoKind::I1,
                value1: hex(v1)?,
                value2: hex(v2)?,
            }),
            "INFO_2" => Ok(EventKind::Info {
                kind: InfoKind::I2,
                value1: hex(v1)?,
                value2: hex(v2)?,
            }),
            "INFO_3" => Ok(EventKind::Info {
                kind: InfoKind::I3,
                value1: hex(v1)?,
                value2: hex(v2)?,
            }),
            "DAQ_DATA" => Ok(EventKind::Data { tag: hex(v2)? }),
            "DAQ_COUNTER" => {
                let count = hex(v1)?;
                let packed = hex(v2)?;
                let (counter_kind, region, tag) = unpack_counter(packed)?;
                Ok(EventKind::Counter {
                    count,
                    kind: counter_kind,
                    region,
                    tag,
                })
            }
            "DAQ_PC" => Ok(EventKind::LastPC { prev_pc: hex(v2)? }),
            _ => Err(error::Error::UnknownEventKind(kind.to_string())),
        }?;

        Ok(Event {
            source_id,
            kind,
            cycle,
        })
    }
}

mod encoder {
    use std::{
        cell::RefCell,
        io::{BufWriter, Write},
        marker::PhantomData,
        rc::Rc,
    };

    use crate::{
        Encode, Event, EventKind, Source, SourceHandle, TextEncoderTag, codec::Inner, error,
    };

    impl<W: Write> Inner<W, TextEncoderTag> {
        pub fn write_event(
            &mut self,
            source_id: u8,
            kind: EventKind,
            cycle: Option<u64>,
        ) -> error::Result<()> {
            if !self.sources.iter().any(|s| s.id == source_id) {
                return Err(error::Error::UnknownSource(source_id));
            }
            writeln!(
                self.writer,
                "{}",
                Event {
                    source_id,
                    kind,
                    cycle
                }
            )?;
            Ok(())
        }

        pub fn flush(&mut self) -> error::Result<()> {
            self.writer.flush()?;
            Ok(())
        }

        fn write_header(&mut self) -> error::Result<()> {
            writeln!(self.writer, "HDR:format=accemic//ctxp-txt,ver=1")?;
            Ok(())
        }

        fn write_metadata(&mut self) -> error::Result<()> {
            write!(self.writer, "META:")?;
            for (i, source) in self.sources.iter().enumerate() {
                if i > 0 {
                    write!(self.writer, ",")?;
                }
                let escaped = source.name.replace('\\', "\\\\").replace('"', "\\\"");
                write!(self.writer, "#{}=\"{}\"", source.id, &escaped)?;
            }
            writeln!(self.writer)?;
            Ok(())
        }
    }

    /// A streaming encoder for the CTXP text format (`.ctxp-txt`).
    ///
    /// Wraps any [`Write`] sink and emits UTF-8 text lines, one per event.
    /// Sources are declared at construction and fixed for the encoder's
    /// lifetime — the encoder is ready to accept events immediately.
    ///
    /// Cheaply cloneable: cloning shares the same underlying writer and
    /// source list, so multiple [`SourceHandle`]s (or clones of the encoder
    /// itself) can coexist and write concurrently in a single-threaded context.
    ///
    /// Buffers output internally — call [`Encode::flush`] when done to ensure
    /// all data reaches the underlying writer.
    #[derive(Clone)]
    pub struct TextEncoder<W: Write> {
        inner: Rc<RefCell<Inner<W, TextEncoderTag>>>,
    }

    impl<W: Write> TextEncoder<W> {
        /// Declares `sources` and writes the header and metadata immediately.
        /// The encoder is ready to accept events as soon as this returns.
        pub fn new(writer: W, sources: &[Source]) -> error::Result<Self> {
            let mut inner = Inner {
                writer: BufWriter::new(writer),
                sources: sources.to_vec(),
                _marker: PhantomData,
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
        pub fn source(&self, source_id: u8) -> error::Result<SourceHandle<W, TextEncoderTag>> {
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

    impl<W: Write> Encode for TextEncoder<W> {
        fn write_event(&self, event: &Event) -> error::Result<()> {
            self.inner
                .borrow_mut()
                .write_event(event.source_id, event.kind.clone(), event.cycle)
        }

        fn flush(&self) -> error::Result<()> {
            self.inner.borrow_mut().flush()
        }
    }
}
