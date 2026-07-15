use std::{fs::File, io::BufReader, path::Path};

use ctxp::{
    Format::{Binary, Text},
    *,
};

fn main() -> ctxp::Result<()> {
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
            kind: EventKind::Sync { target: 0x80000000 },
            cycle: Some(0),
        },
        Event {
            source_id: 0,
            kind: EventKind::BranchNotTaken {
                origin: 0x80003ca2,
                target: 0x80003ca6,
            },
            cycle: Some(4),
        },
        Event {
            source_id: 1,
            kind: EventKind::MemRead {
                width: AccessWidth::W1,
                addr: Some(0x70000064),
                value: 0,
            },
            cycle: Some(1),
        },
        Event {
            source_id: 0,
            kind: EventKind::WallClock { value: 0x12000 },
            cycle: Some(10),
        },
        Event {
            source_id: 1,
            kind: EventKind::WallClock { value: 0x12000 },
            cycle: Some(4),
        },
        Event {
            source_id: 1,
            kind: EventKind::BranchTaken {
                origin: 0x80003d00,
                target: 0x8000298c,
            },
            cycle: Some(24),
        },
    ];

    // --- encode directly to both formats ---
    let w = File::create("trace.ctxp.txt")?;
    let txt = TextEncoder::new(w, &sources)?;
    for event in &events {
        txt.write_event(event)?;
    }
    txt.flush()?;

    let bin = BinaryEncoder::new(File::create("trace.ctxp")?, &sources)?;
    for event in &events {
        bin.write_event(event)?;
    }
    bin.flush()?;

    // --- transcode text -> binary ---
    let dec = Decoder::new(BufReader::new(File::open("trace.ctxp.txt")?), Text)?;
    let bin_transcoded = BinaryEncoder::new(File::create("trace.ctxp-transcoded")?, dec.sources())?;
    for event in dec {
        bin_transcoded.write_event(&event?)?;
    }
    bin_transcoded.flush()?;

    compare_files(
        "direct vs transcoded (txt->bin)",
        "trace.ctxp",
        "trace.ctxp-transcoded",
    )?;

    // --- shared encoder example ---
    let enc = TextEncoder::new(File::create("trace-shared.ctxp.txt")?, &sources)?;

    let cpu0 = enc.source(0)?;
    let cpu1 = enc.source(1)?;

    cpu0.write_event(EventKind::Sync { target: 0x80000000 }, Some(0))?;
    cpu0.write_event(
        EventKind::BranchNotTaken {
            origin: 0x80003ca2,
            target: 0x80003ca6,
        },
        Some(4),
    )?;
    cpu1.write_event(
        EventKind::MemRead {
            width: AccessWidth::W1,
            addr: Some(0x70000064),
            value: 0,
        },
        Some(1),
    )?;
    cpu0.write_event(EventKind::WallClock { value: 0x12000 }, Some(10))?;
    cpu1.write_event(EventKind::WallClock { value: 0x12000 }, Some(4))?;
    cpu1.write_event(
        EventKind::BranchTaken {
            origin: 0x80003d00,
            target: 0x8000298c,
        },
        Some(24),
    )?;

    enc.flush()?;

    compare_files(
        "direct vs shared",
        "trace.ctxp.txt",
        "trace-shared.ctxp.txt",
    )?;

    // --- demux and re-encode per source ---
    let enc = TextEncoder::new(File::create("trace-reencoded.ctxp.txt")?, &sources)?;
    let cpu0 = enc.source(0)?;
    let cpu1 = enc.source(1)?;

    let mut dmx = Decoder::open(Path::new("trace.ctxp.txt"))?.demux();
    dmx.on_source(0, |event| cpu0.write_event(event.kind.clone(), event.cycle));
    dmx.on_source(1, |event| cpu1.write_event(event.kind.clone(), event.cycle));
    dmx.run()?;

    enc.flush()?;

    compare_files(
        "direct vs demux re-encoded",
        "trace.ctxp.txt",
        "trace-reencoded.ctxp.txt",
    )?;
    // --- round-trip binary: decode binary, re-encode as binary ---
    let bin_dec = Decoder::new(BufReader::new(File::open("trace.ctxp")?), Binary)?;
    let bin_roundtrip =
        BinaryEncoder::new(File::create("trace.ctxp-roundtrip")?, bin_dec.sources())?;
    for event in bin_dec {
        bin_roundtrip.write_event(&event?)?;
    }
    bin_roundtrip.flush()?;

    compare_files("binary round-trip", "trace.ctxp", "trace.ctxp-roundtrip")?;

    // --- transcode binary -> text, compare with original text ---
    let bin_dec2 = Decoder::new(BufReader::new(File::open("trace.ctxp")?), Binary)?;
    let txt_from_bin =
        TextEncoder::new(File::create("trace.ctxp.txt-from-bin")?, bin_dec2.sources())?;
    for event in bin_dec2 {
        txt_from_bin.write_event(&event?)?;
    }
    txt_from_bin.flush()?;

    compare_files(
        "binary->text vs original text",
        "trace.ctxp.txt",
        "trace.ctxp.txt-from-bin",
    )?;

    Ok(())
}

fn compare_files(label: &str, path_a: &str, path_b: &str) -> ctxp::Result<()> {
    let a = std::fs::read(path_a)?;
    let b = std::fs::read(path_b)?;

    if a == b {
        println!("OK [{label}]: outputs are identical ({} bytes)", a.len());
    } else {
        eprintln!("MISMATCH [{label}]:");
        eprintln!("  {path_a}: {} bytes", a.len());
        eprintln!("  {path_b}: {} bytes", b.len());
        for (i, (x, y)) in a.iter().zip(b.iter()).enumerate() {
            if x != y {
                eprintln!(
                    "  first difference at byte {:#06x}: {:#04x} vs {:#04x}",
                    i, x, y
                );
                break;
            }
        }
        if a.len() != b.len() {
            eprintln!("  lengths differ: {} vs {}", a.len(), b.len());
        }
    }

    Ok(())
}
