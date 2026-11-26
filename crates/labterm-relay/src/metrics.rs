//! Prometheus counters, rendered by hand: a handful of numbers don't need a metrics library.

use std::collections::BTreeMap;
use std::fmt::Write as _;
use std::sync::Mutex;
use std::sync::atomic::{AtomicU64, Ordering};

#[derive(Default)]
pub struct Metrics {
    pub tickets_issued: AtomicU64,
    pub connections: AtomicU64,
    pub sessions_open: AtomicU64,
    pub bytes_up: AtomicU64,
    pub bytes_down: AtomicU64,
    tickets_denied: Mutex<BTreeMap<&'static str, u64>>,
    connect_denied: Mutex<BTreeMap<&'static str, u64>>,
}

impl Metrics {
    pub fn ticket_denied(&self, reason: &'static str) {
        *self
            .tickets_denied
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .entry(reason)
            .or_default() += 1;
    }

    pub fn connect_denied(&self, reason: &'static str) {
        *self
            .connect_denied
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .entry(reason)
            .or_default() += 1;
    }

    pub fn render(&self, allow_rules: usize) -> String {
        let mut s = String::new();
        let mut one = |name: &str, kind: &str, help: &str, v: u64| {
            let _ = write!(
                s,
                "# HELP {name} {help}\n# TYPE {name} {kind}\n{name} {v}\n"
            );
        };
        one(
            "hatch_relay_allow_rules",
            "gauge",
            "Rules in the loaded allowlist.",
            allow_rules as u64,
        );
        one(
            "hatch_relay_tickets_issued_total",
            "counter",
            "Tickets signed.",
            self.tickets_issued.load(Ordering::Relaxed),
        );
        one(
            "hatch_relay_connections_total",
            "counter",
            "TCP connections opened to hosts.",
            self.connections.load(Ordering::Relaxed),
        );
        one(
            "hatch_relay_sessions_open",
            "gauge",
            "WebSocket sessions open now.",
            self.sessions_open.load(Ordering::Relaxed),
        );
        one(
            "hatch_relay_bytes_up_total",
            "counter",
            "Bytes from browsers to hosts, counted when a session ends.",
            self.bytes_up.load(Ordering::Relaxed),
        );
        one(
            "hatch_relay_bytes_down_total",
            "counter",
            "Bytes from hosts to browsers, counted when a session ends.",
            self.bytes_down.load(Ordering::Relaxed),
        );
        for (name, help, map) in [
            (
                "hatch_relay_tickets_denied_total",
                "Ticket requests refused, by reason.",
                &self.tickets_denied,
            ),
            (
                "hatch_relay_connects_denied_total",
                "Connects refused or failed, by reason.",
                &self.connect_denied,
            ),
        ] {
            let _ = write!(s, "# HELP {name} {help}\n# TYPE {name} counter\n");
            for (reason, v) in map.lock().unwrap_or_else(|e| e.into_inner()).iter() {
                let _ = writeln!(s, "{name}{{reason=\"{reason}\"}} {v}");
            }
        }
        s
    }
}
