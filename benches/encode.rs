// benches/encode.rs
use criterion::{BenchmarkId, Criterion, Throughput, black_box, criterion_group, criterion_main};
use ctxp::{
    Format::{Binary, Text},
    *,
};
use rand::{Rng, SeedableRng, rngs::StdRng};

// --- r#generators ---

fn random_sources(n: usize) -> Vec<Source> {
    (0..n)
        .map(|i| Source {
            id: i as u8,
            name: format!("CPU{}", i),
        })
        .collect()
}

fn random_kind(rng: &mut impl Rng) -> EventKind {
    match rng.r#gen_range(0..19u8) {
        0 => EventKind::Sync {
            target: rng.r#gen(),
        },
        1 => EventKind::Interrupt {
            origin: rng.r#gen(),
            target: rng.r#gen(),
        },
        2 => EventKind::Rfi {
            origin: rng.r#gen(),
            target: rng.r#gen(),
        },
        3 => EventKind::BranchTaken {
            origin: rng.r#gen(),
            target: rng.r#gen(),
        },
        4 => EventKind::BranchNotTaken {
            origin: rng.r#gen(),
            target: rng.r#gen(),
        },
        5 => EventKind::Call {
            origin: rng.r#gen(),
            target: rng.r#gen(),
        },
        6 => EventKind::Return {
            origin: rng.r#gen(),
            target: rng.r#gen(),
        },
        7 => EventKind::MemReadUnknownData { addr: rng.r#gen() },
        8 => EventKind::MemRead {
            width: random_width(rng),
            addr: random_opt(rng),
            value: rng.r#gen(),
        },
        9 => EventKind::MemWriteUnknownData { addr: rng.r#gen() },
        10 => EventKind::MemWrite {
            width: random_width(rng),
            addr: random_opt(rng),
            value: rng.r#gen(),
        },
        11 => EventKind::Overflow,
        12 => EventKind::Context { value: rng.r#gen() },
        13 => EventKind::WallClock { value: rng.r#gen() },
        14 => EventKind::Info {
            kind: random_info(rng),
            value1: rng.r#gen(),
            value2: rng.r#gen(),
        },
        15 => EventKind::Data { tag: rng.r#gen() },
        16 => EventKind::Counter {
            count: rng.r#gen(),
            kind: random_counter_kind(rng),
            region: rng.r#gen_range(0..8),
            tag: rng.r#gen(),
        },
        17 => EventKind::LastPC {
            prev_pc: rng.r#gen(),
        },
        _ => EventKind::Overflow,
    }
}

fn random_width(rng: &mut impl Rng) -> AccessWidth {
    match rng.r#gen_range(0..4u8) {
        0 => AccessWidth::W1,
        1 => AccessWidth::W2,
        2 => AccessWidth::W4,
        _ => AccessWidth::W8,
    }
}

fn random_info(rng: &mut impl Rng) -> InfoKind {
    match rng.r#gen_range(0..3u8) {
        0 => InfoKind::I1,
        1 => InfoKind::I2,
        _ => InfoKind::I3,
    }
}

fn random_counter_kind(rng: &mut impl Rng) -> CounterKind {
    match rng.r#gen_range(0..4u8) {
        0 => CounterKind::InstructionFetchThreshold,
        1 => CounterKind::DataReadThreshold,
        2 => CounterKind::DataWrite,
        _ => CounterKind::DataRead,
    }
}

fn random_opt(rng: &mut impl Rng) -> Option<u64> {
    if rng.r#gen_bool(0.8) {
        Some(rng.r#gen())
    } else {
        None
    }
}

fn random_event(rng: &mut impl Rng, num_sources: u8) -> Event {
    Event {
        source_id: rng.r#gen_range(0..num_sources),
        kind: random_kind(rng),
        cycle: if rng.r#gen_bool(0.9) {
            Some(rng.r#gen())
        } else {
            None
        },
    }
}

fn make_events(n: usize, num_sources: u8) -> Vec<Event> {
    let mut rng = StdRng::seed_from_u64(42);
    (0..n)
        .map(|_| random_event(&mut rng, num_sources))
        .collect()
}

// --- benchmarks ---

fn bench_text_encoder(c: &mut Criterion) {
    let sources = random_sources(4);
    let mut group = c.benchmark_group("text_encoder");

    for size in [1_000u64, 10_000, 100_000] {
        let events = make_events(size as usize, 4);
        group.throughput(Throughput::Elements(size));
        group.bench_with_input(BenchmarkId::from_parameter(size), &events, |b, events| {
            b.iter(|| {
                let enc = Encoder::new(std::io::sink(), &sources, Text).unwrap();
                for event in events {
                    enc.write_event(black_box(event)).unwrap();
                }
                enc.flush().unwrap();
            });
        });
    }
    group.finish();
}

fn bench_binary_encoder(c: &mut Criterion) {
    let sources = random_sources(4);
    let mut group = c.benchmark_group("binary_encoder");

    for size in [1_000u64, 10_000, 100_000] {
        let events = make_events(size as usize, 4);
        group.throughput(Throughput::Elements(size));
        group.bench_with_input(BenchmarkId::from_parameter(size), &events, |b, events| {
            b.iter(|| {
                let enc = Encoder::new(std::io::sink(), &sources, Binary).unwrap();
                for event in events {
                    enc.write_event(black_box(event)).unwrap();
                }
                enc.flush().unwrap();
            });
        });
    }
    group.finish();
}

fn bench_text_decoder(c: &mut Criterion) {
    let sources = random_sources(4);
    let mut group = c.benchmark_group("text_decoder");

    for size in [1_000u64, 10_000, 100_000] {
        let events = make_events(size as usize, 4);

        let buf = {
            let mut buf = Vec::new();
            {
                let enc = Encoder::new(&mut buf, &sources, Text).unwrap();
                for event in &events {
                    enc.write_event(event).unwrap();
                }
                enc.flush().unwrap();
            }
            buf
        };

        group.throughput(Throughput::Elements(size));
        group.bench_with_input(BenchmarkId::from_parameter(size), &buf, |b, buf| {
            b.iter(|| {
                let dec = Decoder::new(black_box(buf.as_slice()), Text).unwrap();
                for event in dec {
                    black_box(event.unwrap());
                }
            });
        });
    }
    group.finish();
}

fn bench_binary_decoder(c: &mut Criterion) {
    let sources = random_sources(4);
    let mut group = c.benchmark_group("binary_decoder");

    for size in [1_000u64, 10_000, 100_000] {
        let events = make_events(size as usize, 4);

        let buf = {
            let mut buf = Vec::new();
            {
                let enc = Encoder::new(&mut buf, &sources, Binary).unwrap();
                for event in &events {
                    enc.write_event(event).unwrap();
                }
                enc.flush().unwrap();
            }
            buf
        };

        group.throughput(Throughput::Elements(size));
        group.bench_with_input(BenchmarkId::from_parameter(size), &buf, |b, buf| {
            b.iter(|| {
                let dec = Decoder::new(black_box(buf.as_slice()), Binary).unwrap();
                for event in dec {
                    black_box(event.unwrap());
                }
            });
        });
    }
    group.finish();
}

fn bench_transcode_text_to_binary(c: &mut Criterion) {
    let sources = random_sources(4);
    let mut group = c.benchmark_group("transcode_txt_to_bin");

    for size in [1_000u64, 10_000, 100_000] {
        let events = make_events(size as usize, 4);

        let txt_buf = {
            let mut buf = Vec::new();
            {
                let enc = Encoder::new(&mut buf, &sources, Text).unwrap();
                for event in &events {
                    enc.write_event(event).unwrap();
                }
                enc.flush().unwrap();
            }
            buf
        };

        group.throughput(Throughput::Elements(size));
        group.bench_with_input(BenchmarkId::from_parameter(size), &txt_buf, |b, txt_buf| {
            b.iter(|| {
                let dec = Decoder::new(black_box(txt_buf.as_slice()), Text).unwrap();
                let enc = Encoder::new(std::io::sink(), dec.sources(), Binary).unwrap();
                for event in dec {
                    enc.write_event(&black_box(event.unwrap())).unwrap();
                }
                enc.flush().unwrap();
            });
        });
    }
    group.finish();
}

criterion_group!(
    benches,
    bench_text_encoder,
    bench_binary_encoder,
    bench_text_decoder,
    bench_binary_decoder,
    bench_transcode_text_to_binary,
);
criterion_main!(benches);
