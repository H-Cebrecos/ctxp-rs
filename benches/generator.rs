use ctxp::{Event, EventKind, Source};
use rand::Rng;

pub fn random_event(rng: &mut impl Rng, num_sources: u8) -> Event {
    Event {
        source_id: rng.gen_range(0..num_sources),
        kind: random_kind(rng),
        value1: if rng.gen_bool(0.7) {
            Some(rng.r#gen())
        } else {
            None
        },
        value2: if rng.gen_bool(0.5) {
            Some(rng.r#gen())
        } else {
            None
        },
        cycle: if rng.gen_bool(0.9) {
            Some(rng.r#gen())
        } else {
            None
        },
    }
}

fn random_kind(rng: &mut impl Rng) -> EventKind {
    match rng.gen_range(0..16u8) {
        0 => EventKind::Sync,
        1 => EventKind::Interrupt,
        2 => EventKind::Rfi,
        3 => EventKind::BranchTaken,
        4 => EventKind::BranchNotTaken,
        5 => EventKind::Call,
        6 => EventKind::Return,
        7 => EventKind::MemRead(rng.gen_range(1..=4)),
        8 => EventKind::MemWrite(rng.gen_range(1..=4)),
        9 => EventKind::Overflow,
        10 => EventKind::Context,
        11 => EventKind::WallClock,
        12 => EventKind::Info(rng.gen_range(0..=7)),
        13 => EventKind::Data,
        14 => EventKind::Counter,
        _ => EventKind::LastPC,
    }
}

pub fn random_sources(n: usize) -> Vec<Source> {
    (0..n)
        .map(|i| Source {
            id: i as u8,
            name: format!("CPU{}", i),
        })
        .collect()
}
