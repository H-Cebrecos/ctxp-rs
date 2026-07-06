use std::fmt;

use std::str::FromStr;

pub mod binary;
pub mod text;

pub use binary::*;
pub use text::*;

use crate::error;

pub trait Encode {
    fn write_event(&mut self, event: &Event) -> error::Result<()>;

    fn write_events<'a>(
        &mut self,
        events: impl IntoIterator<Item = &'a Event>,
    ) -> error::Result<()> {
        for event in events {
            self.write_event(event)?;
        }
        Ok(())
    }

    fn flush(&mut self) -> error::Result<()>;
}

#[derive(Debug)]
pub struct Source {
    pub id: u8,
    pub name: String,
}

#[derive(Debug)]
pub enum EventKind {
    // Control Flow
    Sync,
    Interrupt,
    Rfi,
    BranchTaken,
    BranchNotTaken,
    Call,
    Return,

    // Data
    MemRead(u8),
    MemWrite(u8),

    // Miscellaneous
    Overflow,
    Context,
    WallClock,
    Info(u8),

    // DAQ/Instrumentation
    Data,
    Counter,
    LastPC,
}

//TODO: were do we validate that the numbers are in range?
impl fmt::Display for EventKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Sync => write!(f, "SYNC"),
            Self::BranchTaken => write!(f, "BRANCH_TAKEN"),
            Self::BranchNotTaken => write!(f, "BRANCH_NOTTAKEN"),
            Self::Call => write!(f, "CALL"),
            Self::Return => write!(f, "RETURN"),
            Self::Interrupt => write!(f, "INTERRUPT"),
            Self::Rfi => write!(f, "RFI"),
            Self::MemWrite(n) => write!(f, "MEMWRITE_{}", n),
            Self::MemRead(n) => write!(f, "MEMREAD_{}", n),
            Self::Overflow => write!(f, "OVERFLOW"),
            Self::Context => write!(f, "CONTEXT"),
            Self::WallClock => write!(f, "WALLCLOCK"),
            Self::Info(n) => write!(f, "INFO_{}", n),
            Self::Data => write!(f, "DAQ_DATA"),
            Self::Counter => write!(f, "DAQ_COUNTER"),
            Self::LastPC => write!(f, "DAQ_LAST_PC"),
        }
    }
}

//TODO
impl FromStr for EventKind {
    type Err = crate::error::Error;

    fn from_str(s: &str) -> error::Result<Self> {
        // handle the parametric variants first since they need prefix matching
        if let Some(n) = s.strip_prefix("MEMREAD_") {
            return Ok(Self::MemRead(n.parse()?));
        }
        if let Some(n) = s.strip_prefix("MEMWRITE_") {
            return Ok(Self::MemWrite(n.parse()?));
        }
        if let Some(n) = s.strip_prefix("INFO_") {
            return Ok(Self::Info(n.parse()?));
        }

        match s {
            "SYNC" => Ok(Self::Sync),
            "INTERRUPT" => Ok(Self::Interrupt),
            "RFI" => Ok(Self::Rfi),
            "BRANCH_TAKEN" => Ok(Self::BranchTaken),
            "BRANCH_NOTTAKEN" => Ok(Self::BranchNotTaken),
            "CALL" => Ok(Self::Call),
            "RETURN" => Ok(Self::Return),
            "OVERFLOW" => Ok(Self::Overflow),
            "CONTEXT" => Ok(Self::Context),
            "WALLCLOCK" => Ok(Self::WallClock),
            "DAQ_DATA" => Ok(Self::Data),
            "DAQ_COUNTER" => Ok(Self::Counter),
            "DAQ_LAST_PC" => Ok(Self::LastPC),
            _ => Err(error::Error::UnknownEventKind(s.to_string())),
        }
    }
}

#[derive(Debug)]
pub struct Event {
    pub source_id: u8,
    pub kind: EventKind,
    pub value1: Option<u64>,
    pub value2: Option<u64>,
    pub cycle: Option<u64>,
}

//TODO: check
impl fmt::Display for Event {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{:<50} {}",
            format!(
                "#{}:{}:{}:{}",
                self.source_id,
                self.kind,
                self.value1.map(|v| format!("{:#x}", v)).unwrap_or_default(),
                self.value2.map(|v| format!("{:#x}", v)).unwrap_or_default(),
            ),
            self.cycle.map(|v| format!("@ {}", v)).unwrap_or_default(),
        )?;
        Ok(())
    }
}

//TODO: check
impl FromStr for Event {
    type Err = error::Error;

    fn from_str(s: &str) -> error::Result<Self> {
        // strip leading '#'
        let s = s
            .strip_prefix('#')
            .ok_or_else(|| error::Error::Parse("expected '#'".into()))?;

        // split into the event part and optional cycle part
        let (event_part, cycle) = match s.split_once(" @ ") {
            Some((e, c)) => (
                e,
                Some(
                    c.trim()
                        .parse::<u64>()
                        .map_err(|_| error::Error::Parse("invalid cycle count".into()))?,
                ),
            ),
            None => (s, None),
        };

        // split event part into fields
        let mut parts = event_part.splitn(4, ':');
        let source_id = parts
            .next()
            .ok_or_else(|| error::Error::Parse("missing source_id".into()))?
            .parse::<u8>()
            .map_err(|_| error::Error::Parse("invalid source_id".into()))?;

        let kind = parts
            .next()
            .ok_or_else(|| error::Error::Parse("missing event kind".into()))?
            .parse::<EventKind>()?;

        let value1 = parts
            .next()
            .filter(|s| !s.is_empty())
            .map(parse_hex)
            .transpose()?;

        let value2 = parts
            .next()
            .filter(|s| !s.is_empty())
            .map(parse_hex)
            .transpose()?;

        Ok(Event {
            source_id,
            kind,
            value1,
            value2,
            cycle,
        })
    }
}

fn parse_hex(s: &str) -> error::Result<u64> {
    let s = s.trim().strip_prefix("0x").unwrap_or(s);
    u64::from_str_radix(s, 16)
        .map_err(|_| error::Error::Parse(format!("invalid hex value: '{}'", s)))
}

pub trait Decode: Iterator<Item = error::Result<Event>> {
    fn sources(&self) -> &[Source];
}
