// Copyright (c) 2019-2022 Alibaba Cloud
// Copyright (c) 2019-2022 Ant Group
//
// SPDX-License-Identifier: Apache-2.0
//

use std::collections::HashMap;
use std::sync::Mutex;

use anyhow::Result;
use lazy_static::lazy_static;
use opentelemetry::global;
use opentelemetry::sdk::propagation::TraceContextPropagator;
use opentelemetry::Context;
use tracing_subscriber::prelude::*;
use tracing_subscriber::Registry;

lazy_static! {
    static ref KATATRACER: Mutex<KataTracer> = Mutex::new(KataTracer::new());
}

/// The tracer wrapper for kata-containers
#[derive(Default, Debug)]
pub struct KataTracer {
    /// The root span inject its span context into this map, this is an object
    /// that satifies both injector trait and extractor trait, which are used
    /// by TextMapPropagator
    ///
    /// When any other span needs to be hung below the root span, it use this as
    /// extractor to acquire the root span's context and set it as parent
    root_ctx_obj: HashMap<String, String>,
}

impl KataTracer {
    /// Constructor
    pub fn new() -> Self {
        Self {
            root_ctx_obj: HashMap::new(),
        }
    }

    /// Inject the context into the root_ctx_obj, which can be extracted later from
    /// another thread/function.
    /// Currently, this is used to store root span's context
    pub fn inject_ctx(&mut self, ctx: &Context) {
        global::get_text_map_propagator(|prop| prop.inject_context(ctx, &mut self.root_ctx_obj));
    }

    /// Extract the context, if the extraction failed, return a copy of the parameter `ctx`
    /// If the extraction succeeded, the root_ctx_obj's corresponding context will be returned
    /// Currently, this is used to extract root span's context
    pub fn extract_with_ctx(&self, ctx: &Context) -> Context {
        global::get_text_map_propagator(|prop| prop.extract_with_context(ctx, &self.root_ctx_obj))
    }
}

/// Call once before the root span is generated (in main.rs), do
/// all the setup works
pub fn trace_setup() -> Result<()> {
    global::set_text_map_propagator(TraceContextPropagator::new());
    let tracer = opentelemetry_jaeger::new_agent_pipeline()
        .with_service_name("kata-trace")
        .install_simple()?;
    let layer = tracing_opentelemetry::layer().with_tracer(tracer);
    let subscriber = Registry::default().with(layer);
    tracing::subscriber::set_global_default(subscriber)?;

    info!(sl!(), "setup tracing");
    Ok(())
}

/// Global function to shutdown the tracer and emit the span info to jaeger agent
///
/// The tracing information is only partially update to jaeger agent if this function
/// before this function is called
pub fn trace_end() {
    global::shutdown_tracer_provider();
}

/// Wrapper of KataTracer::inject_ctx
pub fn trace_inject(ctx: &Context) {
    let mut tracer = KATATRACER.lock().unwrap();
    tracer.inject_ctx(ctx)
}

/// Wrapper of KataTracer::extract_with_ctx
pub fn trace_extract_root_ctx(ctx: &Context) -> Context {
    let tracer = KATATRACER.lock().unwrap();
    tracer.extract_with_ctx(ctx)
}
