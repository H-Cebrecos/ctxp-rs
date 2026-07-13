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
