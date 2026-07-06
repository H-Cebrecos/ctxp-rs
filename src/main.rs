use std::fs::File;

use ctxp::{
    BinaryEncoder, Decode, Encode, Event, EventKind, Source, TextDecoder, TextEncoder, error,
};

fn main() -> error::Result<()> {
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

    // transcode: parse the text file and re-encode as binary
    let dec = TextDecoder::new(File::open("trace.ctxp-txt")?)?;
    let mut bin_transcoded =
        BinaryEncoder::new(File::create("trace.ctxp-transcoded")?, dec.sources())?;
    for event in dec {
        bin_transcoded.write_event(&event?)?;
    }
    bin_transcoded.flush()?;

    // compare the two binary files
    let direct = std::fs::read("trace.ctxp")?;
    let transcoded = std::fs::read("trace.ctxp-transcoded")?;

    if direct == transcoded {
        println!("OK: direct and transcoded binary outputs are identical");
    } else {
        eprintln!("MISMATCH: outputs differ");
        eprintln!("  direct:     {} bytes", direct.len());
        eprintln!("  transcoded: {} bytes", transcoded.len());

        // print first differing byte for quick diagnosis
        for (i, (a, b)) in direct.iter().zip(transcoded.iter()).enumerate() {
            if a != b {
                eprintln!(
                    "  first difference at byte {:#06x}: {:#04x} vs {:#04x}",
                    i, a, b
                );
                break;
            }
        }
    }

    Ok(())
}
