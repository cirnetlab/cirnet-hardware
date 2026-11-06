use crate::{
    pb::{self, ErrorCode as Code},
    simulator::{CompiledPlan, Simulator},
};
use cirnet_contracts_native::{capture, validation::Registry};
use serde_json::{Map, Value, json};
use std::{
    collections::{HashMap, HashSet},
    sync::{
        Arc, Mutex, MutexGuard,
        atomic::{AtomicBool, Ordering},
    },
    time::Duration,
};
use tokio::{
    sync::{Mutex as AsyncMutex, Semaphore, oneshot},
    task::spawn_blocking,
    time::{Instant, timeout},
};

mod jobs;
use jobs::{Cleanup, Completion, Job};
#[cfg(test)]
mod tests;

const VIRTUAL_STATION: &str = "virtual-station-01";
const PHYSICAL_STATION: &str = "physical-station-01";
const VIRTUAL_SESSIONS: usize = 64;
const ANALYZER_CHANNELS: usize = 4;
const RUN_TIMEOUT: Duration = Duration::from_secs(10);
const STOP_TIMEOUT: Duration = Duration::from_secs(2);

pub enum Operation {
    EnterSafeState,
    ScanTopology,
    ReadRuntimeSafety,
    Prepare {
        plan: Value,
        decision: Value,
    },
    ConfigureCircuit(Result<Value, Code>),
    ConfigureInstrument {
        instrument: String,
        configuration: Value,
    },
    ApplyRouting,
    GuardedPowerUp,
    ArmCapture(Value),
    Start,
    Stop,
    ReadCapture,
    Reset,
}

#[derive(Debug)]
pub struct Outcome {
    pub accepted: bool,
    pub code: Code,
    pub state: &'static str,
    pub result: Value,
    pub capture_payload: Vec<u8>,
}

impl Outcome {
    fn success(state: State) -> Self {
        let state = state.wire();
        Self::new(
            true,
            Code::Ok,
            state,
            json!({"accepted":true,"code":"OK","state":state}),
        )
    }

    fn virtual_control(state: State) -> Self {
        let mut result = Self::success(state);
        result.result["hardware_applied"] = json!(false);
        result.result["mode"] = json!("simulation");
        result
    }

    fn new(accepted: bool, code: Code, state: &'static str, result: Value) -> Self {
        Self {
            accepted,
            code,
            state,
            result,
            capture_payload: Vec::new(),
        }
    }
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
enum State {
    #[default]
    Idle,
    Safe,
    Ready,
    Armed,
    Running,
    Capturing,
    Stopping,
    Fault,
}

impl State {
    fn wire(self) -> &'static str {
        match self {
            Self::Idle => "IDLE",
            Self::Safe => "SAFE_STATE",
            Self::Ready => "READY",
            Self::Armed => "ARMED",
            Self::Running => "RUNNING",
            Self::Capturing => "CAPTURING",
            Self::Stopping => "STOPPING",
            Self::Fault => "FAULT",
        }
    }
}

#[derive(Default)]
struct Session {
    control: AsyncMutex<()>,
    state: Mutex<SessionState>,
}

#[derive(Default)]
struct SessionState {
    status: State,
    retired: bool,
    owner: Option<String>,
    generation: u64,
    compiled: Option<CompiledPlan>,
    preparation: Option<Preparation>,
    capture: Option<Arc<Value>>,
    job: Option<Arc<Job>>,
}

impl SessionState {
    fn check_owner(&self, target: &pb::Target) -> Result<(), Code> {
        if self.retired
            || self
                .owner
                .as_ref()
                .is_some_and(|owner| owner != &target.experiment_id)
        {
            return Err(Code::ResourceConflict);
        }
        Ok(())
    }

    fn current_job(&self, generation: u64, job: &Arc<Job>) -> bool {
        !self.retired
            && self.generation == generation
            && self
                .job
                .as_ref()
                .is_some_and(|running| Arc::ptr_eq(running, job))
    }

    fn fence(&mut self) {
        self.generation = self.generation.wrapping_add(1);
    }
}

struct Preparation {
    aliases: Vec<(String, String)>,
    capture: Value,
    vectors: Value,
    clock: Value,
}

impl Preparation {
    fn remap_signal<'a>(&'a self, signal: &'a str) -> &'a str {
        self.aliases
            .iter()
            .find(|(alias, _)| alias == signal)
            .map_or(signal, |(_, port)| port)
    }

    fn run_input(&self) -> Result<(Value, Value, Value), Code> {
        let mut capture = self.capture.clone();
        for signal in capture["signals"]
            .as_array_mut()
            .ok_or(Code::Unexecutable)?
        {
            *signal = json!(self.remap_signal(signal.as_str().ok_or(Code::Unexecutable)?));
        }
        let mut vectors = self.vectors.clone();
        for vector in vectors.as_array_mut().ok_or(Code::Unexecutable)? {
            let inputs = vector["inputs"].as_object_mut().ok_or(Code::Unexecutable)?;
            *inputs = std::mem::take(inputs)
                .into_iter()
                .map(|(name, value)| (self.remap_signal(&name).to_owned(), value))
                .collect();
        }
        let mut clock = self.clock.clone();
        if let Some(signal) = clock["signal"].as_str() {
            clock["signal"] = json!(self.remap_signal(signal));
        }
        Ok((capture, vectors, clock))
    }
}

pub struct StationHub {
    simulator: Arc<Simulator>,
    registry: Arc<Registry>,
    sessions: Mutex<HashMap<String, Arc<Session>>>,
    base_session: Arc<Session>,
    run_slots: Arc<Semaphore>,
    work_slots: Arc<Semaphore>,
    closing: AtomicBool,
}

impl StationHub {
    pub fn new(
        simulator: Arc<Simulator>,
        registry: Arc<Registry>,
        workers: usize,
    ) -> Result<Self, Code> {
        if workers == 0 || workers > Semaphore::MAX_PERMITS {
            return Err(Code::InvalidConfiguration);
        }
        Ok(Self {
            simulator,
            registry,
            sessions: Mutex::default(),
            base_session: Arc::default(),
            run_slots: Arc::new(Semaphore::new(workers)),
            work_slots: Arc::new(Semaphore::new(workers)),
            closing: AtomicBool::new(false),
        })
    }

    pub fn capabilities(&self, station_id: &str) -> Result<pb::StationCapabilities, Code> {
        let virtual_station = match station_id {
            VIRTUAL_STATION => true,
            PHYSICAL_STATION => false,
            _ => return Err(Code::HardwareUnavailable),
        };
        let mut analyzer = pb::InstrumentCapability::default();
        let mut scope = pb::InstrumentCapability::default();
        let mut generator = pb::GeneratorCapability::default();
        let mut breadboards = Vec::new();
        if virtual_station {
            analyzer.channels = ANALYZER_CHANNELS as u32;
            analyzer.status = Some("digital-events".into());
            scope.status = Some("NOT_IMPLEMENTED".into());
            generator.status = Some("NOT_IMPLEMENTED".into());
            breadboards = ["BB0", "BB1"]
                .into_iter()
                .map(|id| pb::Breadboard {
                    id: id.into(),
                    model: "generic-full-size".into(),
                })
                .collect();
        }
        Ok(pb::StationCapabilities {
            schema_version: "1.0".into(),
            station_id: station_id.into(),
            mode: if virtual_station {
                "simulation"
            } else {
                "hardware"
            }
            .into(),
            available: virtual_station,
            virtual_digital: virtual_station,
            hardware_rpc_version: "2.0".into(),
            fpga_protocol_version: "UNAVAILABLE".into(),
            firmware_version: if virtual_station {
                "virtual-events-1.0"
            } else {
                "UNAVAILABLE"
            }
            .into(),
            hardware_profile_version: if virtual_station {
                "virtual-1.0"
            } else {
                "UNAVAILABLE"
            }
            .into(),
            breadboards,
            instrument_capabilities: Some(pb::InstrumentCapabilities {
                logic_analyzer: Some(analyzer),
                oscilloscope: Some(scope),
                generator: Some(generator),
            }),
            ..Default::default()
        })
    }

    pub async fn execute(&self, target: pb::Target, operation: Operation) -> Result<Outcome, Code> {
        self.ensure_open()?;
        match target.station_id.as_str() {
            PHYSICAL_STATION => return physical_operation(&target, operation),
            VIRTUAL_STATION => {}
            _ => return Err(Code::HardwareUnavailable),
        }
        let session = self.session(&target.experiment_id)?;
        let control = session.control.lock().await;
        self.ensure_open()?;
        lock(&session.state).check_owner(&target)?;
        match operation {
            Operation::Start => {
                let (waiter, receiver) = self.launch(&session, &target)?;
                drop(control);
                waiter.receive(receiver).await
            }
            Operation::ReadCapture => {
                let (state, capture) = capture_snapshot(&session);
                drop(control);
                if let Some(capture) = capture {
                    let lane = self
                        .work_slots
                        .clone()
                        .acquire_owned()
                        .await
                        .map_err(|_| Code::HardwareUnavailable)?;
                    spawn_blocking(move || {
                        let _lane = lane;
                        encode_capture(state, &capture)
                    })
                    .await
                    .map_err(|_| Code::DriverError)?
                } else {
                    Ok(Outcome::new(
                        true,
                        Code::NotImplemented,
                        state.wire(),
                        json!({
                            "available": false, "code": "NOT_IMPLEMENTED"
                        }),
                    ))
                }
            }
            Operation::EnterSafeState | Operation::Stop | Operation::Reset => {
                self.safe(&session, &target, operation).await
            }
            Operation::ConfigureCircuit(graph) => self.configure(&session, graph?).await,
            Operation::Prepare { plan, decision } => {
                self.prepare(&session, &target, &plan, &decision)
            }
            Operation::ArmCapture(configuration) => self.arm(&session, configuration),
            Operation::ScanTopology | Operation::ReadRuntimeSafety => Err(Code::NotImplemented),
            Operation::ApplyRouting | Operation::GuardedPowerUp => {
                let inner = lock(&session.state);
                inner.preparation.as_ref().ok_or(Code::InvalidState)?;
                Ok(Outcome::virtual_control(inner.status))
            }
            Operation::ConfigureInstrument { instrument, .. } => {
                if !matches!(instrument.as_str(), "clock" | "generator" | "trigger") {
                    return Err(Code::Unexecutable);
                }
                Ok(Outcome::new(
                    false,
                    Code::NotImplemented,
                    lock(&session.state).status.wire(),
                    json!({"accepted":false,"code":"NOT_IMPLEMENTED","feature":instrument}),
                ))
            }
        }
    }

    fn ensure_open(&self) -> Result<(), Code> {
        if self.closing.load(Ordering::Acquire) {
            return Err(Code::HardwareUnavailable);
        }
        Ok(())
    }

    fn session(&self, experiment: &str) -> Result<Arc<Session>, Code> {
        let mut sessions = lock(&self.sessions);
        self.ensure_open()?;
        if experiment.is_empty() {
            return Ok(self.base_session.clone());
        }
        if let Some(session) = sessions.get(experiment) {
            return Ok(session.clone());
        }
        if sessions.len() >= VIRTUAL_SESSIONS {
            return Err(Code::ResourceConflict);
        }
        let session = Arc::new(Session::default());
        sessions.insert(experiment.to_owned(), session.clone());
        Ok(session)
    }

    async fn configure(&self, session: &Session, graph: Value) -> Result<Outcome, Code> {
        let lane = {
            let mut inner = lock(&session.state);
            if !matches!(inner.status, State::Idle | State::Safe | State::Ready) {
                return Err(Code::InvalidState);
            }
            inner.compiled = None;
            if !matches!(
                graph["schema_version"].as_str(),
                Some("1.0" | "1.1" | "1.2")
            ) || !graph["modules"].is_array()
                || !graph["connections"].is_array()
            {
                return Err(Code::Unexecutable);
            }
            self.work_slots
                .clone()
                .try_acquire_owned()
                .map_err(|_| Code::ResourceConflict)?
        };
        let simulator = self.simulator.clone();
        let compiled = spawn_blocking(move || {
            let _lane = lane;
            simulator.compile(&graph).map_err(|_| Code::Unexecutable)
        })
        .await
        .map_err(|_| Code::DriverError)??;
        self.ensure_open()?;
        let mut inner = lock(&session.state);
        inner.fence();
        inner.compiled = Some(compiled);
        inner.preparation = None;
        inner.capture = None;
        inner.status = State::Ready;
        Ok(Outcome::success(inner.status))
    }

    fn prepare(
        &self,
        session: &Session,
        target: &pb::Target,
        plan: &Value,
        decision: &Value,
    ) -> Result<Outcome, Code> {
        verify_plan_target(target, plan)?;
        let mut inner = lock(&session.state);
        if inner.status != State::Ready {
            return Err(Code::InvalidState);
        }
        let compiled = inner.compiled.as_ref().ok_or(Code::InvalidState)?;
        let mut plan = self
            .registry
            .validate("InstrumentationExecutionPlan", "transport", plan)
            .map_err(|_| Code::Unexecutable)?;
        let decision = self
            .registry
            .validate("ExecutionDecision", "transport", decision)
            .map_err(|_| Code::Unexecutable)?;
        if !decision["decision"]
            .as_str()
            .is_some_and(|value| value.starts_with("RUN_"))
        {
            return Err(Code::Unexecutable);
        }
        let aliases = plan_aliases(&plan, compiled)?;
        inner.preparation = Some(Preparation {
            aliases,
            capture: plan["capture_config"].take(),
            vectors: plan["test_vectors"].take(),
            clock: plan["clock_config"].take(),
        });
        inner.owner = Some(target.experiment_id.clone());
        Ok(Outcome::success(inner.status))
    }

    fn arm(&self, session: &Session, configuration: Value) -> Result<Outcome, Code> {
        let mut inner = lock(&session.state);
        if inner.status != State::Ready {
            return Err(Code::InvalidState);
        }
        let preparation = inner.preparation.as_mut().ok_or(Code::InvalidState)?;
        if configuration.get("trigger").is_some() {
            return Err(Code::NotImplemented);
        }
        preparation.capture = self
            .registry
            .validate("CaptureConfiguration", "transport", &configuration)
            .map_err(|_| Code::InvalidConfiguration)?;
        inner.status = State::Armed;
        Ok(Outcome::success(inner.status))
    }

    fn launch(
        &self,
        session: &Arc<Session>,
        target: &pb::Target,
    ) -> Result<(StartWait, oneshot::Receiver<WorkResult>), Code> {
        let mut inner = lock(&session.state);
        self.ensure_open()?;
        if inner.owner.as_deref() != Some(target.experiment_id.as_str()) {
            return Err(Code::ResourceConflict);
        }
        if inner.status != State::Armed {
            return Err(Code::InvalidState);
        }
        let preparation = inner.preparation.as_ref().ok_or(Code::InvalidState)?;
        let (capture, vectors, clock) = preparation.run_input()?;
        let compiled = inner.compiled.clone().ok_or(Code::InvalidState)?;
        let permit = self
            .run_slots
            .clone()
            .try_acquire_owned()
            .map_err(|_| Code::ResourceConflict)?;
        let job = Arc::new(Job::new(permit));
        let waiter = StartWait::new(session.clone(), job.clone(), inner.generation);
        let simulator = self.simulator.clone();
        let (sender, receiver) = oneshot::channel();
        inner.job = Some(job.clone());
        inner.status = State::Running;
        spawn_blocking(move || {
            let completion = Completion::new(&job);
            let result = simulator
                .execute(
                    &compiled,
                    &capture,
                    &vectors,
                    &clock,
                    &json!({}),
                    &job.cancel,
                )
                .map_err(|error| error.code());
            let cleanup = if result == Err("SAFE_STATE_FAILED") {
                Cleanup::Unconfirmed
            } else {
                Cleanup::Reaped
            };
            completion.report(cleanup);
            let _ = sender.send(result);
        });
        Ok((waiter, receiver))
    }

    async fn safe(
        &self,
        session: &Arc<Session>,
        target: &pb::Target,
        operation: Operation,
    ) -> Result<Outcome, Code> {
        if matches!(operation, Operation::EnterSafeState) && target.experiment_id.is_empty() {
            return Err(Code::InvalidConfiguration);
        }
        let job = {
            let mut inner = lock(&session.state);
            inner.fence();
            inner.preparation = None;
            if matches!(operation, Operation::Reset) {
                inner.compiled = None;
                inner.capture = None;
            }
            if let Some(job) = inner.job.clone() {
                inner.capture = None;
                inner.status = State::Stopping;
                job.cancel.store(true, Ordering::Release);
                Some(job)
            } else {
                None
            }
        };
        if let Some(job) = job {
            if let Err(error) = job.wait_for_cleanup(STOP_TIMEOUT).await {
                lock(&session.state).status = State::Fault;
                return Err(error);
            }
        }
        let mut inner = lock(&session.state);
        inner.job = None;
        inner.owner = if matches!(operation, Operation::EnterSafeState) {
            Some(target.experiment_id.clone())
        } else {
            None
        };
        inner.status = State::Safe;
        if matches!(operation, Operation::Stop) && !target.experiment_id.is_empty() {
            inner.retired = true;
            let mut sessions = lock(&self.sessions);
            if sessions
                .get(&target.experiment_id)
                .is_some_and(|active| Arc::ptr_eq(active, session))
            {
                sessions.remove(&target.experiment_id);
            }
        }
        Ok(Outcome::virtual_control(inner.status))
    }

    /// Close admission before cancelling all owned work. Only verified reaping is safe.
    pub async fn shutdown(&self) -> Result<(), Code> {
        self.closing.store(true, Ordering::Release);
        let mut sessions: Vec<_> = lock(&self.sessions).values().cloned().collect();
        sessions.push(self.base_session.clone());
        let jobs: Vec<_> = sessions
            .into_iter()
            .filter_map(|session| {
                let mut inner = lock(&session.state);
                let job = inner.job.clone()?;
                inner.fence();
                inner.capture = None;
                job.cancel.store(true, Ordering::Release);
                Some(job)
            })
            .collect();
        let deadline = Instant::now() + STOP_TIMEOUT;
        let mut result = Ok(());
        for job in jobs {
            let remaining = deadline.saturating_duration_since(Instant::now());
            if let Err(error) = job.wait_for_cleanup(remaining).await {
                result = Err(error);
            }
        }
        result
    }
}

type WorkResult = Result<Value, &'static str>;

struct StartWait {
    session: Arc<Session>,
    job: Arc<Job>,
    generation: u64,
    finished: bool,
}

impl StartWait {
    fn new(session: Arc<Session>, job: Arc<Job>, generation: u64) -> Self {
        Self {
            session,
            job,
            generation,
            finished: false,
        }
    }

    async fn receive(mut self, receiver: oneshot::Receiver<WorkResult>) -> Result<Outcome, Code> {
        let result = timeout(RUN_TIMEOUT, receiver).await;
        let mut state = lock(&self.session.state);
        self.finished = true;
        if !state.current_job(self.generation, &self.job) {
            return Err(Code::InvalidState);
        }
        let mut capture = match result {
            Ok(Ok(Ok(capture))) => capture,
            Ok(Ok(Err(error))) => {
                state.status = State::Fault;
                return Err(worker_code(error));
            }
            Ok(Err(_)) => {
                state.status = State::Fault;
                return Err(Code::DriverError);
            }
            Err(_) => {
                self.job.cancel.store(true, Ordering::Release);
                state.status = State::Fault;
                return Err(Code::Timeout);
            }
        };
        if let Err(error) = restore_aliases(&state, &mut capture) {
            state.status = State::Fault;
            return Err(error);
        }
        state.capture = Some(Arc::new(capture));
        state.job = None;
        Ok(Outcome::success(state.status))
    }
}

impl Drop for StartWait {
    fn drop(&mut self) {
        if self.finished {
            return;
        }
        let mut inner = lock(&self.session.state);
        if inner.current_job(self.generation, &self.job) {
            self.job.cancel.store(true, Ordering::Release);
            inner.fence();
            inner.status = State::Fault;
            inner.capture = None;
        }
    }
}

fn restore_aliases(state: &SessionState, capture: &mut Value) -> Result<(), Code> {
    let aliases = &state.preparation.as_ref().ok_or(Code::DriverError)?.aliases;
    for sample in capture["samples"].as_array_mut().ok_or(Code::DriverError)? {
        let port = sample["signal"].as_str().ok_or(Code::DriverError)?;
        if let Some((alias, _)) = aliases.iter().rev().find(|(_, value)| value == port) {
            sample["signal"] = json!(alias);
        }
    }
    Ok(())
}

fn worker_code(error: &str) -> Code {
    match error {
        "BACKEND_UNAVAILABLE" => Code::HardwareUnavailable,
        "DRIVER_ERROR" | "SAFE_STATE_FAILED" => Code::DriverError,
        "PROTOCOL_MISMATCH" => Code::ProtocolMismatch,
        "RESOURCE_CONFLICT" => Code::ResourceConflict,
        "TIMEOUT" => Code::Timeout,
        _ => Code::Unexecutable,
    }
}

fn verify_plan_target(target: &pb::Target, plan: &Value) -> Result<(), Code> {
    if plan["station_id"].as_str() != Some(&target.station_id)
        || plan["experiment_id"].as_str() != Some(&target.experiment_id)
    {
        return Err(Code::ResourceConflict);
    }
    Ok(())
}

fn plan_aliases(plan: &Value, compiled: &CompiledPlan) -> Result<Vec<(String, String)>, Code> {
    let bindings = plan["endpoint_bindings"]
        .as_array()
        .ok_or(Code::Unexecutable)?;
    let mut aliases = Vec::with_capacity(bindings.len());
    let mut roles = HashMap::new();
    for binding in bindings {
        let alias = binding["endpoint_id"].as_str().ok_or(Code::Unexecutable)?;
        let port = binding["node_id"].as_str().ok_or(Code::Unexecutable)?;
        let role = binding["role"].as_str().ok_or(Code::Unexecutable)?;
        if roles.insert(alias, role).is_some() || !compiled.has_endpoint(port) {
            return Err(Code::Unexecutable);
        }
        aliases.push((alias.to_owned(), port.to_owned()));
    }
    let mut channels = HashSet::new();
    let mut allocated = HashSet::new();
    for allocation in plan["instrument_allocations"]
        .as_array()
        .ok_or(Code::Unexecutable)?
    {
        let alias = allocation["endpoint_id"]
            .as_str()
            .ok_or(Code::Unexecutable)?;
        let channel = allocation["channel"].as_u64().ok_or(Code::Unexecutable)?;
        if allocation["instrument"] != "LOGIC_ANALYZER"
            || roles.get(alias) != Some(&"LOGIC_ANALYZER")
            || !(1..=ANALYZER_CHANNELS as u64).contains(&channel)
            || !channels.insert(channel)
            || !allocated.insert(alias)
        {
            return Err(Code::Unexecutable);
        }
    }
    if roles.iter().any(|(alias, role)| {
        matches!(*role, "LOGIC_ANALYZER" | "SCOPE" | "GENERATOR") && !allocated.contains(alias)
    }) || !plan["generator_config"]
        .as_array()
        .is_some_and(Vec::is_empty)
        || !plan["capture_config"]["signals"]
            .as_array()
            .is_some_and(|signals| signals.len() <= ANALYZER_CHANNELS)
    {
        return Err(Code::Unexecutable);
    }
    Ok(aliases)
}

fn capture_snapshot(session: &Session) -> (State, Option<Arc<Value>>) {
    let mut inner = lock(&session.state);
    if inner.capture.is_some() && inner.status == State::Running {
        inner.status = State::Capturing;
    }
    (inner.status, inner.capture.clone())
}

fn encode_capture(state: State, capture: &Value) -> Result<Outcome, Code> {
    let available = capture["available"].as_bool().ok_or(Code::DriverError)?;
    let empty = json!([]);
    let samples = capture.get("samples").unwrap_or(&empty);
    let events = samples.as_array().ok_or(Code::DriverError)?;
    let mut metadata: Map<String, Value> = capture
        .as_object()
        .ok_or(Code::DriverError)?
        .iter()
        .filter(|(key, _)| *key != "samples")
        .map(|(k, v)| (k.clone(), v.clone()))
        .collect();
    let code = metadata.get("code").map_or(Code::Ok, |value| {
        value
            .as_str()
            .and_then(Code::from_str_name)
            .unwrap_or(Code::DriverError)
    });
    let accepted = metadata
        .get("accepted")
        .and_then(Value::as_bool)
        .unwrap_or(true);
    let mut payload = Vec::new();
    if available {
        let duration = if let Some(duration) = capture.get("duration_ns") {
            duration.clone()
        } else {
            let mut end = 0;
            for event in events {
                end = end.max(event["time_ns"].as_u64().ok_or(Code::DriverError)?);
            }
            json!(end)
        };
        payload = capture::encode_digital(samples, &duration).map_err(|_| Code::DriverError)?;
        metadata.insert("duration_ns".to_owned(), duration);
        metadata.insert("sample_count".to_owned(), json!(events.len()));
    } else if !events.is_empty() {
        return Err(Code::DriverError);
    }
    let mut result = Outcome::new(accepted, code, state.wire(), Value::Object(metadata));
    result.capture_payload = payload;
    Ok(result)
}

fn physical_operation(target: &pb::Target, operation: Operation) -> Result<Outcome, Code> {
    match operation {
        Operation::EnterSafeState if target.experiment_id.is_empty() => {
            Err(Code::InvalidConfiguration)
        }
        Operation::EnterSafeState | Operation::Stop | Operation::Reset => Ok(Outcome::new(
            false,
            Code::HardwareUnavailable,
            "OFFLINE",
            json!({"accepted":false,"code":"HARDWARE_UNAVAILABLE","state":"OFFLINE","hardware_applied":false}),
        )),
        Operation::Prepare { plan, .. } => {
            verify_plan_target(target, &plan)?;
            Err(Code::HardwareUnavailable)
        }
        Operation::ConfigureCircuit(_) => Err(Code::Unexecutable),
        Operation::ConfigureInstrument { instrument, .. } => match instrument.as_str() {
            "clock" | "trigger" => Err(Code::NotImplemented),
            "generator" => Err(Code::HardwareUnavailable),
            _ => Err(Code::Unexecutable),
        },
        _ => Err(Code::HardwareUnavailable),
    }
}

fn lock<T>(mutex: &Mutex<T>) -> MutexGuard<'_, T> {
    mutex
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
}
