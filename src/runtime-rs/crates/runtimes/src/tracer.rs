// Copyright (c) 2019-2022 Alibaba Cloud
// Copyright (c) 2019-2022 Ant Group
//
// SPDX-License-Identifier: Apache-2.0
//

use std::cmp::min;
use std::sync::Arc;
use std::sync::Mutex;

use anyhow::Result;
use lazy_static::lazy_static;
use opentelemetry::global;
use tracing::span;
use tracing::subscriber::NoSubscriber;
use tracing::Span;
use tracing::Subscriber;
use tracing_subscriber::prelude::*;
use tracing_subscriber::Registry;

lazy_static! {
    /// The static KATATRACER is the variable to provide public functions to outside modules
    static ref KATATRACER: Arc<Mutex<KataTracer>> = Arc::new(Mutex::new(KataTracer::new()));

    /// The ROOTSPAN is a phantom span that is running by calling [`trace_enter_root`] at the background
    /// once the configuration is read and config.runtime.enable_tracing is enabled
    /// The ROOTSPAN exits by calling [`trace_exit_root`] on shutdown request sent from containerd
    pub static ref ROOTSPAN: Span = span!(tracing::Level::TRACE, "root-span");
}

/// The tracer wrapper for kata-containers, this contains the global static variable
/// the tracing utilities might need
/// The fields and member methods should ALWAYS be PRIVATE and be exposed in a safe
/// way to other modules
unsafe impl Send for KataTracer {}
unsafe impl Sync for KataTracer {}
struct KataTracer {
    subscriber: Arc<dyn Subscriber + Send + Sync>,
    enabled: bool,
}

impl KataTracer {
    /// Constructor of KataTracer, this is a dummy implementation for static initialization
    fn new() -> Self {
        Self {
            subscriber: Arc::new(NoSubscriber::default()),
            enabled: false,
        }
    }

    /// Set the tracing enabled flag
    fn enable(&mut self) {
        self.enabled = true;
    }

    /// Return whether the tracing is enabled, enabled by [`trace_setup`]
    fn enabled(&self) -> bool {
        self.enabled
    }
}

/// Call when the tracing is enabled (set in toml configuration file)
/// This setup the subscriber, which maintains the span's information, to global and
/// inside KATATRACER.
/// 
/// Note that the span will be noop(not collected) if a valid subscriber is set
pub fn trace_setup(sid: &str) -> Result<()> {
    let mut kt = KATATRACER.lock().unwrap();

    // enable tracing
    kt.enable();

    // derive a subscriber to collect span info
    let tracer = opentelemetry_jaeger::new_agent_pipeline()
        .with_service_name(format!("test-{}", &sid[0..min(8, sid.len())]))
        .install_simple()?;
    let layer = tracing_opentelemetry::layer().with_tracer(tracer);
    let sub = Registry::default().with(layer);

    // we use Arc to let global subscriber and katatracer to SHARE the SAME subscriber
    // this is for record the global subscriber into a global variable KATATRACER for more usages
    let subscriber = Arc::new(sub);
    tracing::subscriber::set_global_default(subscriber.clone())?;
    kt.subscriber = subscriber;

    Ok(())
}

/// Global function to shutdown the tracer and emit the span info to jaeger agent
/// The tracing information is only partially update to jaeger agent before this function is called
pub fn trace_end() {
    if KATATRACER.lock().unwrap().enabled() {
        global::shutdown_tracer_provider();
    }
}

pub fn trace_enter_root() {
    enter(&ROOTSPAN);
}

pub fn trace_exit_root() {
    exit(&ROOTSPAN);
}

/// let the subscriber enter the span, this has to be called in pair with exit(span)
/// This function allows **cross function span** to run without span guard
fn enter(span: &Span) {
    let kt = KATATRACER.lock().unwrap();
    let id: Option<span::Id> = span.into();
    kt.subscriber.enter(&id.unwrap());
}

/// let the subscriber exit the span, this has to be called in pair to enter(span)
fn exit(span: &Span) {
    let kt = KATATRACER.lock().unwrap();
    let id: Option<span::Id> = span.into();
    kt.subscriber.exit(&id.unwrap());
}
