use lazy_static::lazy_static;
use prometheus::{Encoder, Gauge, GaugeVec, IntCounter, TextEncoder, HistogramVec, register_histogram_vec};

const NAMESPACE_KATA_shim: &str = "kata_shim";

lazy_static! {
    static ref AGENT_SCRAPE_COUNT: IntCounter = prometheus::register_int_counter!(
        format!("{}_{}", NAMESPACE_KATA_shim, "scrape_count"),
        "Metrics scrape count"
    )
    .unwrap();
    static ref RPC_DURATION_HISTOGRAM: HistogramVec = register_histogram_vec!(
        format!("{}_{}", NAMESPACE_KATA_shim, "rpc_duration_histogram"),
        "RPC latency distributions",
        &["handler"]
    )
    .unwrap();
}
