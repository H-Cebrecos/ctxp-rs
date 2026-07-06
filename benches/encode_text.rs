use criterion::{BenchmarkId, Criterion, Throughput, black_box, criterion_group, criterion_main};
use ctxp::{Encode, TextEncoder};
use rand::{SeedableRng, rngs::StdRng};

mod generator;

fn bench_text_encoder(c: &mut Criterion) {
    let mut rng = <StdRng as SeedableRng>::seed_from_u64(42); // deterministic
    let sources = generator::random_sources(4);

    // pre-generate events so random generation isn't part of the measurement
    let events: Vec<_> = (0..100_000)
        .map(|_| generator::random_event(&mut rng, 4))
        .collect();

    let mut group = c.benchmark_group("text_encoder");

    for size in [1_000u64, 10_000, 100_000] {
        group.throughput(Throughput::Elements(size));
        group.bench_with_input(BenchmarkId::from_parameter(size), &size, |b, &size| {
            b.iter(|| {
                let mut enc = TextEncoder::new(std::io::sink(), &sources).unwrap();

                enc.write_events(events.iter().take(size as usize).map(black_box))
                    .unwrap();
                enc.flush().unwrap();
            });
        });
    }

    group.finish();
}

criterion_group!(benches, bench_text_encoder);
criterion_main!(benches);
