//! Supervise the pinned CNP1/CNP2 Simulator process on private pipes.

mod compiler;
mod physical;
mod process;
mod protocol;

use cirnet_contracts_native::validation::Registry;
use compiler::{Catalog, Plan};
use process::{Pool, Worker};
use protocol::Inputs;
use serde_json::{Value, json};
use std::{
    fmt,
    path::{Path, PathBuf},
    sync::{
        Arc,
        atomic::{AtomicBool, AtomicU64, Ordering},
    },
    time::Duration,
};

const RUN_TIMEOUT: Duration = Duration::from_secs(10);
const PREVIEW_TIMEOUT: Duration = Duration::from_secs(2);
const HELD_CLOCK: &str =
    "Static DC preview holds clocks at their initial level; use virtual capture for waveforms.";
static NEXT_ID: AtomicU64 = AtomicU64::new(1);

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SimError {
    AnalogUnavailable,
    BackendIdentityMismatch,
    BackendUnavailable,
    Cancelled,
    CaptureLimit,
    DriverError,
    EventLimit,
    GraphLimit,
    MixedSignalUnavailable,
    NotImplemented,
    ProtocolMismatch,
    ResourceConflict,
    SafeStateFailed,
    Timeout,
    Unexecutable,
}

impl SimError {
    pub fn code(self) -> &'static str {
        match self {
            Self::AnalogUnavailable => "ANALOG_UNAVAILABLE",
            Self::BackendIdentityMismatch => "BACKEND_IDENTITY_MISMATCH",
            Self::BackendUnavailable => "BACKEND_UNAVAILABLE",
            Self::Cancelled => "CANCELLED",
            Self::CaptureLimit => "CAPTURE_LIMIT",
            Self::DriverError => "DRIVER_ERROR",
            Self::EventLimit => "EVENT_LIMIT",
            Self::GraphLimit => "GRAPH_LIMIT",
            Self::MixedSignalUnavailable => "MIXED_SIGNAL_UNAVAILABLE",
            Self::NotImplemented => "NOT_IMPLEMENTED",
            Self::ProtocolMismatch => "PROTOCOL_MISMATCH",
            Self::ResourceConflict => "RESOURCE_CONFLICT",
            Self::SafeStateFailed => "SAFE_STATE_FAILED",
            Self::Timeout => "TIMEOUT",
            Self::Unexecutable => "UNEXECUTABLE",
        }
    }
}

impl fmt::Display for SimError {
    fn fmt(&self, output: &mut fmt::Formatter<'_>) -> fmt::Result {
        output.write_str(self.code())
    }
}

impl std::error::Error for SimError {}

#[derive(Clone)]
pub struct CompiledPlan {
    owner: u64,
    generation: u64,
    plan: Arc<Plan>,
}

impl CompiledPlan {
    pub fn has_endpoint(&self, name: &str) -> bool {
        self.plan.ids.contains_key(name)
    }
}

pub struct Simulator {
    identity: u64,
    worker: Worker,
    catalog: Catalog,
    registry: Arc<Registry>,
    run: Pool,
    preview: Pool,
    closed: AtomicBool,
}

impl Simulator {
    pub fn new(
        worker: PathBuf,
        sha256: &str,
        slots: usize,
        contracts_root: &Path,
        registry: Arc<Registry>,
    ) -> Result<Self, SimError> {
        if !(1..=128).contains(&slots) {
            return Err(SimError::ResourceConflict);
        }
        Ok(Self {
            identity: NEXT_ID.fetch_add(1, Ordering::Relaxed),
            worker: Worker::new(worker, sha256)?,
            catalog: Catalog::load(contracts_root)?,
            registry,
            run: Pool::new(slots),
            preview: Pool::new(1),
            closed: AtomicBool::new(false),
        })
    }

    fn compile_for(&self, graph: &Value, preview: bool) -> Result<CompiledPlan, SimError> {
        if self.closed.load(Ordering::Acquire) {
            return Err(SimError::BackendUnavailable);
        }
        Ok(CompiledPlan {
            owner: self.identity,
            generation: NEXT_ID.fetch_add(1, Ordering::Relaxed),
            plan: Arc::new(compiler::compile(
                graph,
                &self.registry,
                &self.catalog,
                preview,
            )?),
        })
    }

    pub fn compile(&self, graph: &Value) -> Result<CompiledPlan, SimError> {
        self.compile_for(graph, false)
    }

    pub fn execute(
        &self,
        plan: &CompiledPlan,
        config: &Value,
        vectors: &Value,
        clock: &Value,
        inputs: &Value,
        cancel: &AtomicBool,
    ) -> Result<Value, SimError> {
        self.execute_for(
            plan,
            Inputs {
                config,
                vectors,
                clock,
                levels: inputs,
                preview: false,
            },
            cancel,
        )
    }

    fn execute_for(
        &self,
        compiled: &CompiledPlan,
        input: Inputs<'_>,
        cancel: &AtomicBool,
    ) -> Result<Value, SimError> {
        if compiled.owner != self.identity {
            return Err(SimError::BackendIdentityMismatch);
        }
        if self.closed.load(Ordering::Acquire) {
            return Err(SimError::BackendUnavailable);
        }
        if cancel.load(Ordering::Acquire) {
            return Err(SimError::Cancelled);
        }
        let (pool, timeout) = if input.preview {
            (&self.preview, PREVIEW_TIMEOUT)
        } else {
            (&self.run, RUN_TIMEOUT)
        };
        pool.with_lane(|lane| {
            let request_id = NEXT_ID.fetch_add(1, Ordering::Relaxed);
            let request = protocol::request(
                &compiled.plan,
                &self.registry,
                &input,
                compiled.generation,
                request_id,
                cancel,
            )?;
            let response =
                lane.exchange(&self.worker, request.frame, timeout, cancel, &self.closed)?;
            let result = protocol::reply(
                &response,
                &compiled.plan,
                compiled.generation,
                request_id,
                request.duration,
                &request.probes,
            );
            if cancel.load(Ordering::Acquire) {
                lane.discard()?;
                return Err(SimError::Cancelled);
            }
            if self.closed.load(Ordering::Acquire) {
                lane.discard()?;
                return Err(SimError::BackendUnavailable);
            }
            if result == Err(SimError::ProtocolMismatch) {
                lane.discard()?;
            }
            result
        })
    }

    pub fn preview(&self, graph: &Value) -> Result<Value, SimError> {
        let plan = self.compile_for(graph, true)?;
        let config = json!({"signals":[], "duration_ns":0, "max_events":10_000});
        let input = Inputs {
            config: &config,
            vectors: &Value::Null,
            clock: &Value::Null,
            levels: &Value::Null,
            preview: true,
        };
        let capture = self.execute_for(&plan, input, &AtomicBool::new(false))?;
        let mut diagnostics = capture["diagnostics"]
            .as_array()
            .cloned()
            .unwrap_or_default();
        if !plan.plan.clocks.is_empty() {
            diagnostics.push(json!(HELD_CLOCK));
        }
        Ok(json!({
            "device_states":capture.get("device_states").cloned().unwrap_or_else(|| json!({})),
            "pin_values":capture.get("pin_values").cloned().unwrap_or_else(|| json!({})),
            "diagnostics":diagnostics,
        }))
    }

    pub fn shutdown(&self) -> Result<(), SimError> {
        self.closed.store(true, Ordering::Release);
        let run = self.run.close();
        let preview = self.preview.close();
        run.and(preview)
    }
}
