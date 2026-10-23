//! Prometheus metrics registry, owned per `AppState`.
//!
//! Why not the `metrics` crate: a process-global recorder cannot be isolated
//! per test (the integration suites build many apps in one process), and the
//! `/metrics` scrape must never observe itself. A small atomic registry gives
//! both for free and keeps the dependency tree flat.

use std::{
    collections::BTreeMap,
    sync::{
        Mutex, PoisonError,
        atomic::{AtomicI64, AtomicU64, Ordering},
    },
    time::Duration,
};

/// Fixed histogram buckets (seconds) — shared by the HTTP-latency and
/// pool-acquire series so the two are directly comparable.
pub const BUCKETS: [f64; 12] = [
    0.001, 0.005, 0.01, 0.025, 0.05, 0.1, 0.25, 0.5, 1.0, 2.5, 5.0, 10.0,
];

/// Escape a label value per the Prometheus text format (0.0.4):
/// `\` → `\\`, `"` → `\"`, newline → `\n`.
fn escape_label(value: &str) -> String {
    let mut escaped = String::with_capacity(value.len());
    for character in value.chars() {
        match character {
            '\\' => escaped.push_str("\\\\"),
            '"' => escaped.push_str("\\\""),
            '\n' => escaped.push_str("\\n"),
            other => escaped.push(other),
        }
    }
    escaped
}

/// Append the `# HELP` / `# TYPE` header pair for one series.
fn header(out: &mut String, name: &str, kind: &str, help: &str) {
    out.push_str(&format!("# HELP {name} {help}\n"));
    out.push_str(&format!("# TYPE {name} {kind}\n"));
}

/// Fixed-bucket latency histogram. Storage is per-bucket (non-cumulative);
/// the exposition renders the buckets CUMULATIVELY, as the text format
/// requires.
#[derive(Debug, Default)]
struct Histogram {
    counts: [u64; BUCKETS.len()],
    sum: f64,
    count: u64,
}

impl Histogram {
    /// Record one observation: the first bucket whose upper bound is
    /// `>= elapsed` gets the (non-cumulative) count; `_sum`/`_count` track
    /// the raw values.
    fn observe(&mut self, elapsed: Duration) {
        let secs = elapsed.as_secs_f64();
        self.sum += secs;
        self.count += 1;
        for (index, upper) in BUCKETS.iter().enumerate() {
            if secs <= *upper {
                self.counts[index] += 1;
                break;
            }
        }
    }

    /// Append the exposition block to `out`: `HELP`, `TYPE`, the cumulative
    /// `_bucket` lines (incl. `+Inf` == total count), then `_sum` and
    /// `_count`.
    fn render(&self, name: &str, help: &str, out: &mut String) {
        header(out, name, "histogram", help);
        let mut cumulative = 0u64;
        for (index, upper) in BUCKETS.iter().enumerate() {
            cumulative += self.counts[index];
            out.push_str(&format!("{name}_bucket{{le=\"{upper}\"}} {cumulative}\n"));
        }
        let count = self.count;
        out.push_str(&format!("{name}_bucket{{le=\"+Inf\"}} {count}\n"));
        out.push_str(&format!("{name}_sum {}\n", self.sum));
        out.push_str(&format!("{name}_count {count}\n"));
    }
}

/// One request-counter series key: the MATCHED route pattern + method +
/// status (never the raw path — that would cardinality-explode the series).
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
struct RequestKey {
    method: String,
    route: String,
    status: u16,
}

/// Counters/gauges/histograms for one app instance.
#[derive(Debug, Default)]
pub struct Metrics {
    requests: Mutex<BTreeMap<RequestKey, u64>>,
    in_flight: AtomicI64,
    latency: Mutex<Histogram>,
    acquire: Mutex<Histogram>,
    throttle_rejections: AtomicU64,
}

impl Metrics {
    /// A fresh, empty registry.
    pub fn new() -> Self {
        Self::default()
    }

    /// Record one finished request. `route` is the MATCHED PATTERN
    /// (`/api/v1/cards/{id}`), never the raw path.
    pub fn observe_request(&self, method: &str, route: &str, status: u16, elapsed: Duration) {
        {
            let mut requests = self.requests.lock().unwrap_or_else(PoisonError::into_inner);
            let key = RequestKey {
                method: method.to_string(),
                route: route.to_string(),
                status,
            };
            *requests.entry(key).or_insert(0) += 1;
        }
        self.latency
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .observe(elapsed);
    }

    /// A request entered the router.
    pub fn in_flight_inc(&self) {
        self.in_flight.fetch_add(1, Ordering::Relaxed);
    }

    /// A request left the router.
    pub fn in_flight_dec(&self) {
        self.in_flight.fetch_sub(1, Ordering::Relaxed);
    }

    /// Record one successful pool acquire.
    pub fn record_acquire(&self, elapsed: Duration) {
        self.acquire
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .observe(elapsed);
    }

    /// One request rejected by the rate limiter (HTTP 429).
    pub fn inc_throttle_rejection(&self) {
        self.throttle_rejections.fetch_add(1, Ordering::Relaxed);
    }

    /// Prometheus text exposition (version 0.0.4). `pool_size`/`pool_idle`
    /// are read by the caller from `Pool::size()`/`Pool::num_idle()`.
    pub fn render(&self, pool_size: u32, pool_idle: usize) -> String {
        let mut out = String::with_capacity(1024);

        header(
            &mut out,
            "kanpad_http_requests_total",
            "counter",
            "Total HTTP requests by method, matched route pattern and status.",
        );
        {
            let requests = self.requests.lock().unwrap_or_else(PoisonError::into_inner);
            for (key, count) in requests.iter() {
                let method = escape_label(&key.method);
                let route = escape_label(&key.route);
                let status = key.status.to_string();
                out.push_str(&format!(
                    "kanpad_http_requests_total{{method=\"{method}\",route=\"{route}\",status=\"{status}\"}} {count}\n"
                ));
            }
        }

        {
            let latency = self.latency.lock().unwrap_or_else(PoisonError::into_inner);
            latency.render(
                "kanpad_http_request_duration_seconds",
                "HTTP request latency in seconds.",
                &mut out,
            );
        }

        header(
            &mut out,
            "kanpad_http_in_flight",
            "gauge",
            "HTTP requests currently being served.",
        );
        let in_flight = self.in_flight.load(Ordering::Relaxed);
        out.push_str(&format!("kanpad_http_in_flight {in_flight}\n"));

        header(
            &mut out,
            "kanpad_db_pool_size",
            "gauge",
            "Current size of the database connection pool.",
        );
        out.push_str(&format!("kanpad_db_pool_size {pool_size}\n"));

        header(
            &mut out,
            "kanpad_db_pool_idle",
            "gauge",
            "Idle connections in the database connection pool.",
        );
        out.push_str(&format!("kanpad_db_pool_idle {pool_idle}\n"));

        {
            let acquire = self.acquire.lock().unwrap_or_else(PoisonError::into_inner);
            acquire.render(
                "kanpad_db_pool_acquire_duration_seconds",
                "Database connection acquire latency in seconds.",
                &mut out,
            );
        }

        header(
            &mut out,
            "kanpad_throttle_rejections_total",
            "counter",
            "Requests rejected by the rate limiter (HTTP 429).",
        );
        let rejections = self.throttle_rejections.load(Ordering::Relaxed);
        out.push_str(&format!("kanpad_throttle_rejections_total {rejections}\n"));

        process_metrics(&mut out);

        out
    }
}

/// Best-effort `/proc` process metrics (Linux only). Every series is emitted
/// only when its source parses; any read or parse failure simply skips that
/// series — a scrape must never panic, and it must never record itself.
#[cfg(target_os = "linux")]
fn process_metrics(out: &mut String) {
    // USER_HZ is 100 on every Linux architecture this service targets.
    const USER_HZ: f64 = 100.0;

    // `/proc/self/stat`: the comm field may itself contain spaces and
    // parens, so split at the LAST `)`; the whitespace tokens after it
    // start at field 3 (state) => utime = t[11], stime = t[12],
    // starttime = t[19] (all USER_HZ clock ticks).
    if let Ok(stat) = std::fs::read_to_string("/proc/self/stat")
        && let Some((_, rest)) = stat.rsplit_once(')')
    {
        let tokens: Vec<&str> = rest.split_whitespace().collect();
        if let (Some(utime), Some(stime), Some(starttime)) =
            (tokens.get(11), tokens.get(12), tokens.get(19))
            && let (Ok(utime), Ok(stime), Ok(starttime)) = (
                utime.parse::<u64>(),
                stime.parse::<u64>(),
                starttime.parse::<u64>(),
            )
        {
            let cpu_seconds = (utime + stime) as f64 / USER_HZ;
            emit(
                out,
                "process_cpu_seconds_total",
                "counter",
                "Total user + system CPU time spent in seconds.",
                cpu_seconds,
            );
            // The absolute start time needs the boot time (`btime`); when it
            // is missing only this series is skipped.
            if let Some(boot_seconds) = boot_time_seconds() {
                let start = boot_seconds as f64 + starttime as f64 / USER_HZ;
                emit(
                    out,
                    "process_start_time_seconds",
                    "gauge",
                    "Start time of the process since the Unix epoch, in seconds.",
                    start,
                );
            }
        }
    }

    // `/proc/self/status`: `VmRSS:    <value> kB` -> bytes.
    if let Ok(status) = std::fs::read_to_string("/proc/self/status") {
        for line in status.lines() {
            if let Some(rest) = line.strip_prefix("VmRSS:")
                && let Some(kibibytes) = rest.split_whitespace().next()
                && let Ok(kibibytes) = kibibytes.parse::<u64>()
            {
                emit(
                    out,
                    "process_resident_memory_bytes",
                    "gauge",
                    "Resident memory size in bytes.",
                    kibibytes * 1024,
                );
                break;
            }
        }
    }

    // Open file descriptors (one directory entry each).
    if let Ok(entries) = std::fs::read_dir("/proc/self/fd") {
        emit(
            out,
            "process_open_fds",
            "gauge",
            "Number of open file descriptors.",
            entries.count(),
        );
    }

    // `/proc/self/limits`: `Max open files  <soft>  <hard> ...` — the hard
    // limit (second column; skipped when `unlimited`).
    if let Ok(limits) = std::fs::read_to_string("/proc/self/limits") {
        for line in limits.lines() {
            if let Some(rest) = line.strip_prefix("Max open files")
                && let Some(hard) = rest.split_whitespace().nth(1)
                && hard != "unlimited"
                && let Ok(hard) = hard.parse::<u64>()
            {
                emit(
                    out,
                    "process_max_fds",
                    "gauge",
                    "Maximum number of open file descriptors.",
                    hard,
                );
                break;
            }
        }
    }
}

/// Append one HELP/TYPE/value triple (process series only).
#[cfg(target_os = "linux")]
fn emit(out: &mut String, name: &str, kind: &str, help: &str, value: impl std::fmt::Display) {
    header(out, name, kind, help);
    out.push_str(&format!("{name} {value}\n"));
}

/// `/proc/stat`'s `btime` (seconds since the Unix epoch at boot), if it
/// parses.
#[cfg(target_os = "linux")]
fn boot_time_seconds() -> Option<u64> {
    std::fs::read_to_string("/proc/stat")
        .ok()?
        .lines()
        .find_map(|line| {
            line.strip_prefix("btime ")
                .and_then(|value| value.trim().parse::<u64>().ok())
        })
}

#[cfg(not(target_os = "linux"))]
fn process_metrics(_out: &mut String) {}

#[cfg(test)]
mod tests {
    use super::*;

    /// Two observations of the SAME `(method, route, status)` collapse into
    /// ONE counter series line with value `2`.
    #[test]
    fn same_key_observations_collapse_into_one_series() {
        let metrics = Metrics::new();
        metrics.observe_request("GET", "/api/v1/cards/{id}", 401, Duration::from_millis(3));
        metrics.observe_request("GET", "/api/v1/cards/{id}", 401, Duration::from_millis(4));
        let text = metrics.render(0, 0);
        assert_eq!(
            text.matches("kanpad_http_requests_total{").count(),
            1,
            "one series line for the same (method, route, status): {text}"
        );
        assert!(
            text.contains(
                "kanpad_http_requests_total{method=\"GET\",route=\"/api/v1/cards/{id}\",status=\"401\"} 2"
            ),
            "{text}"
        );
    }

    /// Counter lines are sorted deterministically (BTreeMap order: GET
    /// before POST, regardless of insertion order).
    #[test]
    fn request_series_are_sorted_deterministically() {
        let metrics = Metrics::new();
        metrics.observe_request("POST", "/api/v1/boards", 201, Duration::from_millis(1));
        metrics.observe_request("GET", "/api/v1/boards", 200, Duration::from_millis(1));
        let text = metrics.render(0, 0);
        let get = text
            .find("kanpad_http_requests_total{method=\"GET\"")
            .expect("GET series line");
        let post = text
            .find("kanpad_http_requests_total{method=\"POST\"")
            .expect("POST series line");
        assert!(get < post, "GET must sort before POST: {text}");
    }

    /// 2 ms then 20 ms: cumulative buckets 0.001 -> 0, 0.005 -> 1,
    /// 0.025 -> 2, `+Inf` -> 2, `_count` 2, `_sum` > 0.
    #[test]
    fn histogram_buckets_are_cumulative_and_pinned() {
        let metrics = Metrics::new();
        metrics.observe_request("GET", "/x", 200, Duration::from_millis(2));
        metrics.observe_request("GET", "/x", 200, Duration::from_millis(20));
        let text = metrics.render(0, 0);
        for expected in [
            "kanpad_http_request_duration_seconds_bucket{le=\"0.001\"} 0",
            "kanpad_http_request_duration_seconds_bucket{le=\"0.005\"} 1",
            "kanpad_http_request_duration_seconds_bucket{le=\"0.025\"} 2",
            "kanpad_http_request_duration_seconds_bucket{le=\"+Inf\"} 2",
            "kanpad_http_request_duration_seconds_count 2",
        ] {
            assert!(text.contains(expected), "missing {expected:?} in {text}");
        }
        let sum = text
            .lines()
            .find(|line| line.starts_with("kanpad_http_request_duration_seconds_sum "))
            .expect("_sum line")
            .rsplit(' ')
            .next()
            .expect("_sum value")
            .parse::<f64>()
            .expect("_sum is a float");
        assert!(sum > 0.0, "_sum must be positive, got {sum}");
    }

    /// inc / inc / dec renders `kanpad_http_in_flight 1`.
    #[test]
    fn in_flight_gauge_tracks_concurrency() {
        let metrics = Metrics::new();
        metrics.in_flight_inc();
        metrics.in_flight_inc();
        metrics.in_flight_dec();
        let text = metrics.render(0, 0);
        assert!(text.contains("kanpad_http_in_flight 1"), "{text}");
    }

    /// One throttle rejection renders `kanpad_throttle_rejections_total 1`.
    #[test]
    fn throttle_rejections_counter_renders() {
        let metrics = Metrics::new();
        metrics.inc_throttle_rejection();
        let text = metrics.render(0, 0);
        assert!(
            text.contains("kanpad_throttle_rejections_total 1"),
            "{text}"
        );
    }

    /// Pool gauges render the caller-supplied numbers exactly.
    #[test]
    fn pool_gauges_render_the_passed_numbers_exactly() {
        let text = Metrics::new().render(7, 3);
        assert!(text.contains("kanpad_db_pool_size 7"), "{text}");
        assert!(text.contains("kanpad_db_pool_idle 3"), "{text}");
    }

    /// The pool-acquire histogram renders its own series.
    #[test]
    fn pool_acquire_histogram_renders() {
        let metrics = Metrics::new();
        metrics.record_acquire(Duration::from_millis(3));
        let text = metrics.render(0, 0);
        assert!(
            text.contains("kanpad_db_pool_acquire_duration_seconds_count 1"),
            "{text}"
        );
    }

    /// Label values are escaped: `a"b\c` renders `a\"b\\c`.
    #[test]
    fn label_values_are_escaped() {
        let metrics = Metrics::new();
        metrics.observe_request("GET", "a\"b\\c", 200, Duration::from_millis(1));
        let text = metrics.render(0, 0);
        assert!(text.contains("route=\"a\\\"b\\\\c\""), "{text}");
    }

    /// On Linux the scrape carries the /proc-derived process series.
    #[cfg(target_os = "linux")]
    #[test]
    fn linux_scrape_carries_process_metrics() {
        let text = Metrics::new().render(0, 0);
        assert!(text.contains("process_resident_memory_bytes "), "{text}");
        assert!(text.contains("process_cpu_seconds_total "), "{text}");
    }
}
