use std::fmt;
use std::io::Result;
use std::str::FromStr;

mod binary;
mod text;
pub use binary::*;
pub use text::*;

pub struct Source {
    pub id: u8,
    pub name: String,
}

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
// impl FromStr for EventKind {
//     type Err = crate::error::Error;

//     fn from_str(s: &str) -> Result<Self> {
//         // handle the parametric variants first since they need prefix matching
//         if let Some(n) = s.strip_prefix("MEMREAD_") {
//             return Ok(Self::MemRead(n.parse()?));
//         }
//         if let Some(n) = s.strip_prefix("MEMWRITE_") {
//             return Ok(Self::MemWrite(n.parse()?));
//         }
//         if let Some(n) = s.strip_prefix("INFO_") {
//             return Ok(Self::Info(n.parse()?));
//         }

//         match s {
//             "SYNC" => Ok(Self::Sync),
//             "INT" => Ok(Self::Interrupt),
//             "RFI" => Ok(Self::Rfi),
//             "BT" => Ok(Self::BranchTaken),
//             "BNT" => Ok(Self::BranchNotTaken),
//             "CALL" => Ok(Self::Call),
//             "RET" => Ok(Self::Return),
//             "OVF" => Ok(Self::Overflow),
//             "CTX" => Ok(Self::Context),
//             "WC" => Ok(Self::WallClock),
//             "DATA" => Ok(Self::Data),
//             "CNT" => Ok(Self::Counter),
//             "LPC" => Ok(Self::LastPC),
//             _ => Err(crate::error::Error::UnknownEventKind(s.to_string())),
//         }
//     }
// }

pub struct Event {
    pub source_id: u8,
    pub kind: EventKind,
    pub value1: Option<u64>,
    pub value2: Option<u64>,
    pub cycle: Option<u64>,
}

//TODO: check and alignment.
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

pub trait Encode {
    fn write_event(&mut self, event: &Event) -> Result<()>;

    fn write_events<'a>(&mut self, events: impl IntoIterator<Item = &'a Event>) -> Result<()> {
        for event in events {
            self.write_event(event)?;
        }
        Ok(())
    }

    fn flush(&mut self) -> Result<()>;
}

pub trait Decode {}
