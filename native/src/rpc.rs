use crate::{
    pb,
    simulator::Simulator,
    station::{Operation, StationHub},
    wire,
};
use cirnet_contracts_native::wire_json;
use serde_json::{Value, json};
use std::sync::Arc;
use tokio::sync::Semaphore;
use tonic::metadata::{Ascii, MetadataValue};
use tonic::{Request, Response, Status};

mod start_wave;

pub mod canonical {
    tonic::include_proto!("cirnet.hardware.v2");
}
pub mod legacy {
    tonic::include_proto!("netcircuit.hardware.v2");
}
pub mod internal {
    tonic::include_proto!("cirnet.hardware.internal.v1");
}

const STOP_INGRESS: &str = "x-cirnet-stop-ingress";
const STOP_INGRESS_CLOCK: &str = "qpc-ns-v1";

#[cfg(windows)]
fn qpc_ns() -> Option<u64> {
    use std::sync::OnceLock;

    #[link(name = "kernel32")]
    unsafe extern "system" {
        fn QueryPerformanceCounter(value: *mut i64) -> i32;
        fn QueryPerformanceFrequency(value: *mut i64) -> i32;
    }

    static FREQUENCY: OnceLock<Option<u64>> = OnceLock::new();
    let frequency = (*FREQUENCY.get_or_init(|| {
        let mut frequency = 0_i64;
        // SAFETY: LARGE_INTEGER has the size and alignment of its i64 member.
        (unsafe { QueryPerformanceFrequency(&mut frequency) } != 0 && frequency > 0)
            .then_some(frequency as u64)
    }))?;
    let mut ticks = 0_i64;
    // SAFETY: ticks is writable storage for the LARGE_INTEGER output.
    if unsafe { QueryPerformanceCounter(&mut ticks) } == 0 || ticks < 0 {
        return None;
    }
    // Match CPython's Windows perf_counter_ns epoch and integer conversion.
    ((ticks as u128) * 1_000_000_000 / u128::from(frequency))
        .try_into()
        .ok()
}

#[cfg(not(windows))]
fn qpc_ns() -> Option<u64> {
    None
}

fn capture_stop_ingress(request: &Request<pb::Target>) -> Option<MetadataValue<Ascii>> {
    let values = request.metadata().get_all(STOP_INGRESS);
    let mut values = values.iter();
    if !values
        .next()
        .is_some_and(|value| value.as_bytes() == STOP_INGRESS_CLOCK.as_bytes())
        || values.next().is_some()
    {
        return None;
    }
    let entered = qpc_ns()?;
    let target = &request.get_ref().experiment_id;
    if target.len() != 32
        || !target
            .bytes()
            .all(|byte| byte.is_ascii_digit() || matches!(byte, b'a'..=b'f'))
    {
        return None;
    }
    format!("{STOP_INGRESS_CLOCK}:{target}:{entered}")
        .parse()
        .ok()
}

#[derive(Clone)]
pub struct HardwareRpc {
    hub: Arc<StationHub>,
    start_waves: Option<Arc<start_wave::StartWaves>>,
}

impl HardwareRpc {
    pub fn new(hub: Arc<StationHub>) -> Self {
        Self {
            hub,
            start_waves: (std::env::var("CIRNET_HARDWARE_START_RENDEZVOUS").as_deref() == Ok("1"))
                .then(|| Arc::new(start_wave::StartWaves::default())),
        }
    }

    async fn execute(
        &self,
        target: pb::Target,
        op: Operation,
    ) -> Result<Response<pb::OperationReply>, Status> {
        let reply = match self.hub.execute(target, op).await {
            Ok(outcome) => pb::OperationReply {
                accepted: outcome.accepted,
                code: outcome.code as i32,
                state: outcome.state.into(),
                result_json: outcome.result.to_string().into_bytes(),
                capture_payload: outcome.capture_payload,
            },
            Err(code) => pb::OperationReply {
                accepted: false,
                code: code as i32,
                result_json: json!({"accepted": false, "code": code.as_str_name()})
                    .to_string()
                    .into_bytes(),
                ..Default::default()
            },
        };
        Ok(Response::new(reply))
    }
}

macro_rules! hardware_service {
    ($service:path) => {
        #[tonic::async_trait]
        impl $service for HardwareRpc {
            async fn get_capabilities(
                &self,
                request: Request<pb::Target>,
            ) -> Result<Response<pb::StationCapabilities>, Status> {
                self.hub
                    .capabilities(&request.into_inner().station_id)
                    .map(Response::new)
                    .map_err(|code| Status::not_found(code.as_str_name()))
            }

            async fn enter_safe_state(
                &self,
                request: Request<pb::Target>,
            ) -> Result<Response<pb::OperationReply>, Status> {
                self.execute(request.into_inner(), Operation::EnterSafeState)
                    .await
            }
            async fn scan_topology(
                &self,
                request: Request<pb::Target>,
            ) -> Result<Response<pb::OperationReply>, Status> {
                self.execute(request.into_inner(), Operation::ScanTopology)
                    .await
            }
            async fn read_runtime_safety(
                &self,
                request: Request<pb::Target>,
            ) -> Result<Response<pb::OperationReply>, Status> {
                self.execute(request.into_inner(), Operation::ReadRuntimeSafety)
                    .await
            }
            async fn prepare_execution(
                &self,
                request: Request<pb::PrepareExecutionRequest>,
            ) -> Result<Response<pb::OperationReply>, Status> {
                let request = request.into_inner();
                let plan = wire::plan(&request.plan.unwrap_or_default());
                let decision = wire::decision(&request.decision.unwrap_or_default());
                self.execute(
                    request.target.unwrap_or_default(),
                    Operation::Prepare { plan, decision },
                )
                .await
            }
            async fn apply_routing(
                &self,
                request: Request<pb::Target>,
            ) -> Result<Response<pb::OperationReply>, Status> {
                self.execute(request.into_inner(), Operation::ApplyRouting)
                    .await
            }
            async fn guarded_power_up(
                &self,
                request: Request<pb::Target>,
            ) -> Result<Response<pb::OperationReply>, Status> {
                self.execute(request.into_inner(), Operation::GuardedPowerUp)
                    .await
            }
            async fn configure_instrument(
                &self,
                request: Request<pb::ConfigureInstrumentRequest>,
            ) -> Result<Response<pb::OperationReply>, Status> {
                let request = request.into_inner();
                let configuration = match request.instrument.as_str() {
                    "generator" => wire::generator(&request.generator.unwrap_or_default()),
                    "clock" => wire::clock(&request.clock.unwrap_or_default()),
                    "trigger" => wire::trigger(&request.trigger.unwrap_or_default()),
                    _ => Value::Null,
                };
                self.execute(
                    request.target.unwrap_or_default(),
                    Operation::ConfigureInstrument {
                        instrument: request.instrument,
                        configuration,
                    },
                )
                .await
            }
            async fn configure_virtual_circuit(
                &self,
                request: Request<pb::ConfigureVirtualRequest>,
            ) -> Result<Response<pb::OperationReply>, Status> {
                let request = request.into_inner();
                let graph = wire_json::parse(&request.design_graph_json)
                    .map_err(|_| pb::ErrorCode::InvalidConfiguration);
                self.execute(
                    request.target.unwrap_or_default(),
                    Operation::ConfigureCircuit(graph),
                )
                .await
            }
            async fn arm_capture(
                &self,
                request: Request<pb::ArmCaptureRequest>,
            ) -> Result<Response<pb::OperationReply>, Status> {
                let request = request.into_inner();
                self.execute(
                    request.target.unwrap_or_default(),
                    Operation::ArmCapture(wire::capture(
                        &request.configuration.unwrap_or_default(),
                    )),
                )
                .await
            }
            async fn start_execution(
                &self,
                request: Request<pb::Target>,
            ) -> Result<Response<pb::OperationReply>, Status> {
                let stamp = match &self.start_waves {
                    Some(waves) => waves.enter(&request).await?,
                    None => None,
                };
                let mut reply = self.execute(request.into_inner(), Operation::Start).await?;
                if let Some(stamp) = stamp {
                    reply.metadata_mut().insert(start_wave::HEADER, stamp);
                }
                Ok(reply)
            }
            async fn stop_execution(
                &self,
                request: Request<pb::Target>,
            ) -> Result<Response<pb::OperationReply>, Status> {
                let ingress = capture_stop_ingress(&request);
                let mut reply = self.execute(request.into_inner(), Operation::Stop).await?;
                if let Some(ingress) = ingress {
                    reply.metadata_mut().insert(STOP_INGRESS, ingress);
                }
                Ok(reply)
            }
            async fn read_capture(
                &self,
                request: Request<pb::Target>,
            ) -> Result<Response<pb::OperationReply>, Status> {
                self.execute(request.into_inner(), Operation::ReadCapture)
                    .await
            }
            async fn reset(
                &self,
                request: Request<pb::Target>,
            ) -> Result<Response<pb::OperationReply>, Status> {
                self.execute(request.into_inner(), Operation::Reset).await
            }
        }
    };
}

hardware_service!(canonical::hardware_service_server::HardwareService);
hardware_service!(legacy::hardware_service_server::HardwareService);

pub struct PreviewRpc {
    simulator: Arc<Simulator>,
    admission: Arc<Semaphore>,
}

impl PreviewRpc {
    pub fn new(simulator: Arc<Simulator>) -> Self {
        Self {
            simulator,
            admission: Arc::new(Semaphore::new(2)),
        }
    }
}

#[tonic::async_trait]
impl internal::simulator_preview_server::SimulatorPreview for PreviewRpc {
    async fn describe(
        &self,
        _: Request<internal::DescribeRequest>,
    ) -> Result<Response<internal::DescribeReply>, Status> {
        Ok(Response::new(internal::DescribeReply { version: 1 }))
    }

    async fn preview(
        &self,
        request: Request<internal::PreviewRequest>,
    ) -> Result<Response<internal::PreviewReply>, Status> {
        let respond = |payload_json| Ok(Response::new(internal::PreviewReply { payload_json }));
        let graph = match wire_json::parse(&request.into_inner().graph_json) {
            Ok(graph) => graph,
            Err(_) => return respond(preview_error("INVALID_CONFIGURATION")),
        };
        let permit = match self.admission.clone().try_acquire_owned() {
            Ok(permit) => permit,
            Err(_) => return respond(preview_error("RESOURCE_CONFLICT")),
        };
        let simulator = self.simulator.clone();
        let reply = tokio::task::spawn_blocking(move || {
            let _permit = permit;
            match simulator.preview(&graph) {
                Ok(value) => json!({"accepted": true, "preview": value})
                    .to_string()
                    .into_bytes(),
                Err(error) => preview_error(error.code()),
            }
        })
        .await
        .map_err(|_| Status::internal("DRIVER_ERROR"))?;
        respond(reply)
    }
}

fn preview_error(code: &str) -> Vec<u8> {
    json!({"accepted": false, "code": code})
        .to_string()
        .into_bytes()
}
