use std::fs::File;

use ctxp::{BinaryEncoder, Encode, Event, EventKind, Source, TextEncoder};

fn main() {
    let sources = vec![
        Source {
            id: 0,
            name: "CPU0".into(),
        },
        Source {
            id: 1,
            name: "CPU1".into(),
        },
    ];

    let events = vec![
        Event {
            source_id: 0,
            kind: EventKind::Sync,
            value1: None,
            value2: Some(0x80000000),
            cycle: Some(0),
        },
        Event {
            source_id: 0,
            kind: EventKind::BranchNotTaken,
            value1: Some(0x80003ca2),
            value2: Some(0x80003ca6),
            cycle: Some(4),
        },
        Event {
            source_id: 1,
            kind: EventKind::MemRead(1),
            value1: Some(0x70000064),
            value2: Some(0),
            cycle: Some(1),
        },
        Event {
            source_id: 0,
            kind: EventKind::WallClock,
            value1: None,
            value2: Some(0x12000),
            cycle: Some(10),
        },
        Event {
            source_id: 1,
            kind: EventKind::WallClock,
            value1: None,
            value2: Some(0x12000),
            cycle: Some(4),
        },
        Event {
            source_id: 1,
            kind: EventKind::BranchTaken,
            value1: Some(0x80003d00),
            value2: Some(0x8000298c),
            cycle: Some(24),
        },
    ];

    let mut txt = TextEncoder::new(File::create("trace.ctxp-txt").unwrap(), &sources).unwrap();
    for event in &events {
        txt.write_event(event).unwrap();
    }
    txt.flush().unwrap();

    let mut bin = BinaryEncoder::new(File::create("trace.ctxp").unwrap(), &sources).unwrap();
    for event in &events {
        bin.write_event(event).unwrap();
    }
    bin.flush().unwrap();
}
