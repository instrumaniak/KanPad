#![allow(missing_docs, unreachable_pub)]
//! Gungraun (callgrind) instruction-count gate for the pure sort-key hot
//! paths (Tier A). Deterministic, never wall-clock. Baseline recorded under
//! `backend-rs/benches/baselines/` (see docs/perf-runbook.md).

use backend_rs::common::order_key::{key_between, keys_between, needs_rebalance};
use gungraun::{
    Callgrind, EntryPoint, LibraryBenchmarkConfig, library_benchmark, library_benchmark_group, main,
};

// `EntryPoint::None`: gungraun by default passes
// `--toggle-collect='*::__gungraun_wrapper_mod::*'` (DEFAULT_TOGGLE in
// gungraun-runner 0.20) — that toggle never fires on this toolchain (callgrind
// records 0 Ir even when toggling the exact demangled wrapper name), so the
// gate would measure nothing and be vacuous. With no toggle-collect, callgrind
// collects from program start (`--collect-at-start` defaults to yes): the Ir of
// the bench functions plus their wrapper shims. (T046 B1; the matching
// `[profile.bench] strip = false` is in backend-rs/Cargo.toml.)
fn callgrind_no_toggle() -> LibraryBenchmarkConfig {
    let mut cfg = LibraryBenchmarkConfig::default();
    cfg.tool(Callgrind::default().entry_point(EntryPoint::None));
    cfg
}

#[library_benchmark(config = callgrind_no_toggle())]
fn bench_key_between_middle() -> String {
    key_between(Some("a0"), Some("a1")).expect("valid")
}

#[library_benchmark(config = callgrind_no_toggle())]
fn bench_keys_between_ten() -> Vec<String> {
    keys_between(Some("a0"), Some("a1"), 10).expect("valid")
}

#[library_benchmark(config = callgrind_no_toggle())]
fn bench_needs_rebalance() -> bool {
    needs_rebalance("a0V")
}

library_benchmark_group!(
    name = order_key,
    benchmarks = [
        bench_key_between_middle,
        bench_keys_between_ten,
        bench_needs_rebalance
    ]
);

main!(library_benchmark_groups = order_key);
