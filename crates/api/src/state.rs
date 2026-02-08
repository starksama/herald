use apalis::postgres::PostgresStorage;
use core::config::Settings;
use core::types::DeliveryJob;
use core::tunnel::AgentRegistry;
use once_cell::sync::Lazy;
use sqlx::PgPool;
use std::collections::HashMap;
use std::sync::Arc;
use std::sync::Mutex;
use std::sync::MutexGuard;
use tracing::warn;

#[derive(Clone)]
pub struct AppState {
    pub db: PgPool,
    pub redis: redis::Client,
    pub storage: PostgresStorage<DeliveryJob>,
    pub settings: Settings,
    pub tunnel_registry: Arc<AgentRegistry>,
}

#[derive(Debug, Clone)]
pub struct RequestId(pub String);

#[derive(Default)]
struct MetricsStore {
    http_requests: HashMap<(String, String, u16), u64>,
    signals: HashMap<(String, String), u64>,
    deliveries: HashMap<String, u64>,
    latency: HashMap<String, (u64, f64)>,
    queue_depth: HashMap<String, i64>,
}

pub struct Metrics {
    store: Mutex<MetricsStore>,
}

impl Metrics {
    pub fn new() -> Self {
        Self {
            store: Mutex::new(MetricsStore::default()),
        }
    }

    fn lock_store(&self) -> MutexGuard<'_, MetricsStore> {
        match self.store.lock() {
            Ok(guard) => guard,
            Err(poisoned) => {
                warn!("metrics store lock poisoned; continuing with inner state");
                poisoned.into_inner()
            }
        }
    }

    pub fn record_http_request(&self, method: &str, path: &str, status: u16) {
        let mut store = self.lock_store();
        *store
            .http_requests
            .entry((method.to_string(), path.to_string(), status))
            .or_insert(0) += 1;
    }

    pub fn record_signal(&self, channel: &str, urgency: &str) {
        let mut store = self.lock_store();
        *store
            .signals
            .entry((channel.to_string(), urgency.to_string()))
            .or_insert(0) += 1;
    }

    #[allow(dead_code)]
    pub fn record_delivery(&self, status: &str) {
        let mut store = self.lock_store();
        *store.deliveries.entry(status.to_string()).or_insert(0) += 1;
    }

    #[allow(dead_code)]
    pub fn record_delivery_latency(&self, channel: &str, seconds: f64) {
        let mut store = self.lock_store();
        let entry = store.latency.entry(channel.to_string()).or_insert((0, 0.0));
        entry.0 += 1;
        entry.1 += seconds;
    }

    #[allow(dead_code)]
    pub fn set_queue_depth(&self, queue: &str, depth: i64) {
        let mut store = self.lock_store();
        store.queue_depth.insert(queue.to_string(), depth);
    }

    pub fn gather(&self) -> String {
        let store = self.lock_store();
        let mut out = String::new();

        out.push_str("# TYPE herald_http_requests_total counter\n");
        for ((method, path, status), value) in &store.http_requests {
            out.push_str(&format!(
                "herald_http_requests_total{{method=\"{}\",path=\"{}\",status=\"{}\"}} {}\n",
                method, path, status, value
            ));
        }

        out.push_str("# TYPE herald_signals_total counter\n");
        for ((channel, urgency), value) in &store.signals {
            out.push_str(&format!(
                "herald_signals_total{{channel=\"{}\",urgency=\"{}\"}} {}\n",
                channel, urgency, value
            ));
        }

        out.push_str("# TYPE herald_deliveries_total counter\n");
        for (status, value) in &store.deliveries {
            out.push_str(&format!(
                "herald_deliveries_total{{status=\"{}\"}} {}\n",
                status, value
            ));
        }

        out.push_str("# TYPE herald_delivery_latency_seconds summary\n");
        for (channel, (count, sum)) in &store.latency {
            out.push_str(&format!(
                "herald_delivery_latency_seconds_count{{channel=\"{}\"}} {}\n",
                channel, count
            ));
            out.push_str(&format!(
                "herald_delivery_latency_seconds_sum{{channel=\"{}\"}} {}\n",
                channel, sum
            ));
        }

        out.push_str("# TYPE herald_queue_depth gauge\n");
        for (queue, depth) in &store.queue_depth {
            out.push_str(&format!(
                "herald_queue_depth{{queue=\"{}\"}} {}\n",
                queue, depth
            ));
        }

        out
    }
}

pub static METRICS: Lazy<Metrics> = Lazy::new(Metrics::new);
