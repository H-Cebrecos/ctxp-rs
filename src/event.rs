use std::{fmt, str::FromStr};

use crate::error;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Source {
    pub id: u8,
    pub name: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Event {
    pub source_id: u8,
    pub kind: EventKind,
    pub cycle: Option<u64>,
}

#[non_exhaustive]
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EventKind {
    // Control Flow
    Sync {
        target: u64,
    },
    Interrupt {
        origin: u64,
        target: u64,
    },
    Rfi {
        origin: u64,
        target: u64,
    },
    BranchTaken {
        origin: u64,
        target: u64,
    },
    BranchNotTaken {
        origin: u64,
        target: u64,
    },
    Call {
        origin: u64,
        target: u64,
    },
    Return {
        origin: u64,
        target: u64,
    },

    // Data
    MemReadUnknownData {
        addr: u64,
    },
    MemRead {
        width: AccessWidth,
        addr: Option<u64>,
        value: u64,
    },
    MemWriteUnknownData {
        addr: u64,
    },
    MemWrite {
        width: AccessWidth,
        addr: Option<u64>,
        value: u64,
    },

    // Miscellaneous
    Overflow,
    Context {
        value: u64,
    },
    WallClock {
        value: u64,
    },
    Info {
        kind: InfoKind,
        value1: u64,
        value2: u64,
    },

    // DAQ/Instrumentation
    Data {
        tag: u64,
    },
    Counter {
        count: u64,
        kind: CounterKind,
        region: u8,
        tag: u16,
    },
    LastPC {
        prev_pc: u64,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AccessWidth {
    W1,
    W2,
    W4,
    W8,
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum InfoKind {
    I1,
    I2,
    I3,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CounterKind {
    InstructionFetchThreshold = 0,
    DataReadThreshold = 1,
    DataWrite = 2,
    DataRead = 3,
}

pub fn pack_counter(kind: &CounterKind, region: u8, tag: u16) -> u64 {
    let kind_bits = (*kind as u64) & 0b11; // [20:19] — 2 bits
    let region_bits = (region as u64) & 0b111; // [18:16] — 3 bits
    let tag_bits = tag as u64; // [15:0]  — 16 bits

    (kind_bits << 19) | (region_bits << 16) | tag_bits
}

pub fn unpack_counter(v: u64) -> error::Result<(CounterKind, u8, u16)> {
    let kind_bits = (v >> 19) & 0b11;
    let region_bits = (v >> 16) & 0b111;
    let tag_bits = (v & 0xFFFF) as u16;

    let kind = match kind_bits {
        0 => CounterKind::InstructionFetchThreshold,
        1 => CounterKind::DataReadThreshold,
        2 => CounterKind::DataWrite,
        3 => CounterKind::DataRead,
        _ => unreachable!(), // 2 bits can't exceed 3
    };

    Ok((kind, region_bits as u8, tag_bits))
}

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
