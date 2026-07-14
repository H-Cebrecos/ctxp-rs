use ctxp::*;

fn make_sources() -> Vec<Source> {
    vec![
        Source {
            id: 0,
            name: "CPU0".into(),
        },
        Source {
            id: 1,
            name: "CPU1".into(),
        },
    ]
}

fn make_events() -> Vec<Event> {
    vec![
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
    ]
}

fn assert_bytes_eq(label: &str, a: &[u8], b: &[u8]) {
    if a != b {
        for (i, (x, y)) in a.iter().zip(b.iter()).enumerate() {
            if x != y {
                panic!(
                    "[{label}] first difference at byte {:#06x}: {:#04x} vs {:#04x}",
                    i, x, y
                );
            }
        }
        panic!("[{label}] lengths differ: {} vs {}", a.len(), b.len());
    }
}

#[test]
fn text_encode_decode_roundtrip() {
    let sources = make_sources();
    let events = make_events();

    let mut buf = Vec::new();
    {
        let enc = TextEncoder::new(&mut buf, &sources).unwrap();
        for event in &events {
            enc.write_event(event).unwrap();
        }
        enc.flush().unwrap();
    }
    let dec = TextDecoder::new(buf.as_slice()).unwrap();
    let decoded: Vec<Event> = dec.map(|e| e.unwrap()).collect();

    assert_eq!(events, decoded);
}

#[test]
fn binary_encode_decode_roundtrip() {
    let sources = make_sources();
    let events = make_events();

    let mut buf = Vec::new();
    {
        let enc = BinaryEncoder::new(&mut buf, &sources).unwrap();
        for event in &events {
            enc.write_event(event).unwrap();
        }
        enc.flush().unwrap();
    }
    let dec = BinaryDecoder::new(buf.as_slice()).unwrap();
    let decoded: Vec<Event> = dec.map(|e| e.unwrap()).collect();

    assert_eq!(events, decoded);
}

#[test]
fn transcode_text_to_binary() {
    let sources = make_sources();
    let events = make_events();

    // encode directly to binary
    let mut direct = Vec::new();
    {
        let enc = BinaryEncoder::new(&mut direct, &sources).unwrap();
        for event in &events {
            enc.write_event(event).unwrap();
        }
        enc.flush().unwrap();
    }
    // encode to text then transcode to binary
    let mut txt_buf = Vec::new();
    {
        let mut txt_enc = TextEncoder::new(&mut txt_buf, &sources).unwrap();
        for event in &events {
            txt_enc.write_event(event).unwrap();
        }
        txt_enc.flush().unwrap();
    }
    let dec = TextDecoder::new(txt_buf.as_slice()).unwrap();
    let mut transcoded = Vec::new();
    {
        let bin_enc = BinaryEncoder::new(&mut transcoded, dec.sources()).unwrap();
        for event in dec {
            bin_enc.write_event(&event.unwrap()).unwrap();
        }
        bin_enc.flush().unwrap();
    }
    assert_bytes_eq("txt->bin transcode", &direct, &transcoded);
}

#[test]
fn transcode_binary_to_text() {
    let sources = make_sources();
    let events = make_events();

    // encode directly to text
    let mut direct_txt = Vec::new();
    {
        let enc = TextEncoder::new(&mut direct_txt, &sources).unwrap();
        for event in &events {
            enc.write_event(event).unwrap();
        }
        enc.flush().unwrap();
    }
    // encode to binary then transcode to text
    let mut bin_buf = Vec::new();
    {
        let bin_enc = BinaryEncoder::new(&mut bin_buf, &sources).unwrap();
        for event in &events {
            bin_enc.write_event(event).unwrap();
        }
        bin_enc.flush().unwrap();
    }
    let dec = BinaryDecoder::new(bin_buf.as_slice()).unwrap();
    let mut transcoded_txt = Vec::new();
    {
        let txt_enc = TextEncoder::new(&mut transcoded_txt, dec.sources()).unwrap();
        for event in dec {
            txt_enc.write_event(&event.unwrap()).unwrap();
        }
        txt_enc.flush().unwrap();
    }
    assert_bytes_eq("bin->txt transcode", &direct_txt, &transcoded_txt);
}

#[test]
fn shared_encoder_matches_direct() {
    let sources = make_sources();
    let events = make_events();

    // direct encoding
    let mut direct = Vec::new();
    {
        let enc = TextEncoder::new(&mut direct, &sources).unwrap();
        for event in &events {
            enc.write_event(event).unwrap();
        }
        enc.flush().unwrap();
    }
    // shared encoder
    let mut shared_buf = Vec::new();
    {
        let enc = TextEncoder::new(&mut shared_buf, &sources).unwrap();
        let cpu0 = enc.source(0).unwrap();
        let cpu1 = enc.source(1).unwrap();
        for event in &events {
            match event.source_id {
                0 => cpu0.write_event(event.kind.clone(), event.cycle).unwrap(),
                1 => cpu1.write_event(event.kind.clone(), event.cycle).unwrap(),
                _ => unreachable!(),
            }
        }
        enc.flush().unwrap();
    }
    assert_bytes_eq("shared encoder", &direct, &shared_buf);
}

#[test]
fn demux_reencodes_correctly() {
    let sources = make_sources();
    let events = make_events();

    // direct encoding
    let mut direct = Vec::new();
    {
        let enc = TextEncoder::new(&mut direct, &sources).unwrap();
        for event in &events {
            enc.write_event(event).unwrap();
        }
        enc.flush().unwrap();
    }
    // encode to text, demux and re-encode
    let mut reencoded = Vec::new();
    {
        let enc = TextEncoder::new(&mut reencoded, &sources).unwrap();
        let cpu0 = enc.source(0).unwrap();
        let cpu1 = enc.source(1).unwrap();

        let mut dmx = TextDecoder::new(direct.as_slice()).unwrap().demux();
        dmx.on_source(0, |event| cpu0.write_event(event.kind.clone(), event.cycle));
        dmx.on_source(1, |event| cpu1.write_event(event.kind.clone(), event.cycle));
        dmx.run().unwrap();

        enc.flush().unwrap();
    }
    assert_bytes_eq("demux re-encode", &direct, &reencoded);
}
