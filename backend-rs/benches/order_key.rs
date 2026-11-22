#![allow(missing_docs, unreachable_pub)]
//! Criterion wall-clock benches for the pure sort-key hot paths (local/manual,
//! Tier C). Never a CI gate.

use backend_rs::common::order_key::{clamp_position, key_between, keys_between, needs_rebalance};
use criterion::{Criterion, criterion_group, criterion_main};

fn bench_key_between(c: &mut Criterion) {
    c.bench_function("key_between", |b| {
        b.iter(|| key_between(Some("a0"), Some("a1")).unwrap())
    });
}

fn bench_keys_between(c: &mut Criterion) {
    c.bench_function("keys_between_10", |b| {
        b.iter(|| keys_between(Some("a0"), Some("a1"), 10).unwrap())
    });
}

fn bench_needs_rebalance(c: &mut Criterion) {
    c.bench_function("needs_rebalance", |b| b.iter(|| needs_rebalance("a0V")));
}

fn bench_clamp_position(c: &mut Criterion) {
    c.bench_function("clamp_position", |b| b.iter(|| clamp_position(5, 10)));
}

criterion_group!(
    benches,
    bench_key_between,
    bench_keys_between,
    bench_needs_rebalance,
    bench_clamp_position
);
criterion_main!(benches);
