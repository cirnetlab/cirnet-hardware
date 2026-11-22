use super::{SimError, compiler::Plan, physical::level_value};
use cirnet_contracts_native::validation::Registry;
use serde_json::{Value, json};
use std::{
    collections::{BTreeMap, HashSet},
    sync::atomic::{AtomicBool, Ordering},
};

const MAX_FRAME: usize = 16 * 1024 * 1024;
const MAX_SAMPLES: usize = 100_000;
const MAX_PROBES: usize = 4;

pub(super) struct Cursor<'a>(&'a [u8]);

impl<'a> Cursor<'a> {
    fn bytes<const SIZE: usize>(&mut self) -> Result<[u8; SIZE], SimError> {
        let (head, tail) = self
            .0
            .split_at_checked(SIZE)
            .ok_or(SimError::ProtocolMismatch)?;
        self.0 = tail;
        head.try_into().map_err(|_| SimError::ProtocolMismatch)
    }

    pub(super) fn u8(&mut self) -> Result<u8, SimError> {
        Ok(self.bytes::<1>()?[0])
    }
    pub(super) fn u32(&mut self) -> Result<u32, SimError> {
        Ok(u32::from_le_bytes(self.bytes()?))
    }
    fn u64(&mut self) -> Result<u64, SimError> {
        Ok(u64::from_le_bytes(self.bytes()?))
    }
    pub(super) fn f64(&mut self) -> Result<f64, SimError> {
        Ok(f64::from_le_bytes(self.bytes()?))
    }
    fn is_empty(&self) -> bool {
        self.0.is_empty()
    }
}

pub(super) struct Inputs<'a> {
    pub(super) config: &'a Value,
    pub(super) vectors: &'a Value,
    pub(super) clock: &'a Value,
    pub(super) levels: &'a Value,
    pub(super) preview: bool,
}

pub(super) struct Request {
    pub(super) frame: Vec<u8>,
    pub(super) probes: Vec<u32>,
    pub(super) duration: u64,
}

struct Events<'a> {
    rows: Vec<u8>,
    limit: usize,
    cancel: &'a AtomicBool,
}

impl Events<'_> {
    fn add(&mut self, time: u64, signal: u32, level: u8) -> Result<(), SimError> {
        if self.cancel.load(Ordering::Acquire) {
            return Err(SimError::Cancelled);
        }
        if self.count() >= self.limit {
            return Err(SimError::EventLimit);
        }
        put64(&mut self.rows, time);
        put32(&mut self.rows, signal);
        self.rows.push(level);
        Ok(())
    }

    fn count(&self) -> usize {
        self.rows.len() / 13
    }
}

fn levels(plan: &Plan, value: &Value) -> Result<BTreeMap<String, u8>, SimError> {
    if value.is_null() {
        return Ok(BTreeMap::new());
    }
    value
        .as_object()
        .ok_or(SimError::Unexecutable)?
        .iter()
        .map(|(signal, level)| {
            if !plan.sources.contains_key(signal)
                || plan
                    .physical
                    .as_ref()
                    .is_some_and(|physical| physical.fixed_sources.contains(signal))
            {
                return Err(SimError::Unexecutable);
            }
            let level = level
                .as_u64()
                .filter(|level| *level <= 1)
                .ok_or(SimError::Unexecutable)?;
            Ok((signal.clone(), level as u8))
        })
        .collect()
}

fn transport_integer(value: &Value) -> Option<u64> {
    value.as_u64().or_else(|| {
        value
            .as_f64()
            .filter(|number| (0.0..=u32::MAX as f64).contains(number) && number.fract() == 0.0)
            .map(|number| number as u64)
    })
}

fn exact_timestamp(value: &Value) -> Result<u64, SimError> {
    // Oversized integer timestamps are outside every transport capture window.
    value
        .as_u64()
        .or_else(|| {
            value
                .as_number()
                .filter(|number| {
                    !number.as_str().starts_with('-')
                        && number.as_str().bytes().all(|byte| byte.is_ascii_digit())
                })
                .map(|_| u64::MAX)
        })
        .ok_or(SimError::Unexecutable)
}

fn probe_ids(plan: &Plan, signals: &Value) -> Result<Vec<u32>, SimError> {
    let signals = signals.as_array().ok_or(SimError::Unexecutable)?;
    let ids = if signals.is_empty() {
        if plan.probes.is_empty() {
            plan.sources
                .keys()
                .take(MAX_PROBES)
                .map(|signal| plan.ids[signal])
                .collect()
        } else {
            plan.probes
                .iter()
                .take(MAX_PROBES)
                .map(|signal| plan.ids[signal])
                .collect()
        }
    } else {
        if signals.len() > MAX_PROBES {
            return Err(SimError::Unexecutable);
        }
        signals
            .iter()
            .map(|signal| {
                signal
                    .as_str()
                    .and_then(|name| plan.ids.get(name))
                    .copied()
                    .ok_or(SimError::Unexecutable)
            })
            .collect::<Result<Vec<_>, _>>()?
    };
    if ids.iter().collect::<HashSet<_>>().len() != ids.len() {
        return Err(SimError::Unexecutable);
    }
    Ok(ids)
}

fn input_events<'a>(
    plan: &Plan,
    registry: &Registry,
    inputs: &Inputs<'_>,
    duration: u64,
    limit: usize,
    cancel: &'a AtomicBool,
) -> Result<Events<'a>, SimError> {
    let mut events = Events {
        rows: Vec::new(),
        limit,
        cancel,
    };
    let mut initial = plan.sources.clone();
    initial.extend(levels(plan, inputs.levels)?);
    for (signal, level) in initial {
        events.add(0, plan.ids[&signal], level)?;
    }

    let clock_config = if inputs.clock.is_null()
        || inputs
            .clock
            .as_object()
            .is_some_and(|fields| fields.is_empty())
    {
        None
    } else {
        let config = registry
            .validate("ClockConfiguration", "transport", inputs.clock)
            .map_err(|_| SimError::Unexecutable)?;
        if !plan
            .clocks
            .iter()
            .any(|clock| config["signal"] == clock.name)
        {
            return Err(SimError::Unexecutable);
        }
        Some(config)
    };
    if !inputs.preview {
        for clock in &plan.clocks {
            let config = clock_config
                .as_ref()
                .filter(|config| config["signal"] == clock.name);
            let period = match config {
                Some(config) => transport_integer(&config["period_ns"])
                    .filter(|period| *period >= 2)
                    .ok_or(SimError::Unexecutable)?,
                None => clock.period,
            };
            let mut cycles = duration / period + 1;
            if let Some(config) = config {
                cycles =
                    cycles.min(transport_integer(&config["cycles"]).ok_or(SimError::Unexecutable)?);
            }
            if cycles > 100_000 {
                return Err(SimError::EventLimit);
            }
            for cycle in 0..cycles {
                let start = cycle * period;
                let rise = start + period / 2;
                if rise <= duration {
                    events.add(rise, clock.signal, 1)?;
                }
                if let Some(fall) = start.checked_add(period).filter(|fall| *fall <= duration) {
                    events.add(fall, clock.signal, 0)?;
                }
            }
        }
    }

    if !inputs.vectors.is_null() {
        for vector in inputs.vectors.as_array().ok_or(SimError::Unexecutable)? {
            let fields = vector.as_object().ok_or(SimError::Unexecutable)?;
            if fields.len() != 2 || !fields.contains_key("time_ns") || !vector["inputs"].is_object()
            {
                return Err(SimError::Unexecutable);
            }
            let time = exact_timestamp(&vector["time_ns"])?;
            for (signal, level) in levels(plan, &vector["inputs"])? {
                if time <= duration {
                    events.add(time, plan.ids[&signal], level)?;
                }
            }
        }
    }
    Ok(events)
}

fn magic(plan: &Plan) -> [u8; 4] {
    if plan.physical.is_some() {
        *b"CNP2"
    } else {
        *b"CNP1"
    }
}

pub(super) fn request(
    plan: &Plan,
    registry: &Registry,
    inputs: &Inputs<'_>,
    generation: u64,
    request: u64,
    cancel: &AtomicBool,
) -> Result<Request, SimError> {
    let config = registry
        .validate("CaptureConfiguration", "transport", inputs.config)
        .map_err(|_| SimError::Unexecutable)?;
    if config.get("trigger").is_some() {
        return Err(SimError::NotImplemented);
    }
    let probes = probe_ids(plan, &config["signals"])?;
    let duration = transport_integer(&config["duration_ns"]).ok_or(SimError::Unexecutable)?;
    let limit = config
        .get("max_events")
        .map_or(Some(100_000), transport_integer)
        .ok_or(SimError::Unexecutable)? as usize;
    let events = input_events(plan, registry, inputs, duration, limit, cancel)?;
    let mut frame = vec![0; 4];
    frame.extend_from_slice(&magic(plan));
    for field in [request, generation, duration] {
        put64(&mut frame, field);
    }
    for count in [
        limit,
        plan.endpoints.len(),
        plan.edges.len(),
        plan.gates.len(),
        events.count(),
        probes.len(),
    ] {
        put32(&mut frame, count as u32);
    }
    if let Some(physical) = &plan.physical {
        for count in physical.counts {
            put32(&mut frame, count);
        }
    }
    for edge in &plan.edges {
        put32(&mut frame, edge.source);
        put32(&mut frame, edge.target);
        if plan.physical.is_some() {
            frame.extend(edge.levels);
        }
    }
    for gate in &plan.gates {
        frame.push(gate.operation);
        for field in [gate.left, gate.right, gate.output] {
            put32(&mut frame, field);
        }
        put64(&mut frame, gate.delay);
        if plan.physical.is_some() {
            frame.push(u8::from(gate.enabled));
        }
    }
    frame.extend(events.rows);
    for &probe in &probes {
        put32(&mut frame, probe);
    }
    if let Some(physical) = &plan.physical {
        frame.extend_from_slice(&physical.wire);
    }
    let size = frame.len() - 4;
    if size > MAX_FRAME {
        return Err(SimError::Unexecutable);
    }
    frame[..4].copy_from_slice(&(size as u32).to_le_bytes());
    Ok(Request {
        frame,
        probes,
        duration,
    })
}

pub(super) fn reply(
    bytes: &[u8],
    plan: &Plan,
    generation: u64,
    request: u64,
    duration: u64,
    probes: &[u32],
) -> Result<Value, SimError> {
    let mut cursor = Cursor(bytes);
    let size = cursor.u32()? as usize;
    if size != bytes.len() - 4
        || cursor.bytes::<4>()? != magic(plan)
        || cursor.u64()? != request
        || cursor.u64()? != generation
    {
        return Err(SimError::ProtocolMismatch);
    }
    let status = cursor.u8()?;
    let count = cursor.u32()? as usize;
    if count > MAX_SAMPLES || cursor.0.len() < count * 13 {
        return Err(SimError::ProtocolMismatch);
    }
    if status != 0 {
        if count != 0 || !cursor.is_empty() {
            return Err(SimError::ProtocolMismatch);
        }
        return Err(match status {
            1 => SimError::Unexecutable,
            2 => SimError::EventLimit,
            3 => SimError::CaptureLimit,
            _ => SimError::ProtocolMismatch,
        });
    }
    let mut samples = Vec::with_capacity(count);
    let mut last = [None; MAX_PROBES];
    let mut previous = None;
    for _ in 0..count {
        let signal = cursor.u32()?;
        let time = cursor.u64()?;
        let level = cursor.u8()?;
        let position = probes
            .iter()
            .position(|&probe| probe == signal)
            .ok_or(SimError::ProtocolMismatch)?;
        let order = (time, position);
        if time > duration
            || previous.is_some_and(|previous| previous >= order)
            || last[position] == Some(level)
            || last[position].is_none() && time != 0
        {
            return Err(SimError::ProtocolMismatch);
        }
        previous = Some(order);
        last[position] = Some(level);
        samples.push(json!({"signal":plan.endpoints[signal as usize],"time_ns":time,"value":level_value(level)?}));
    }
    if last[..probes.len()].contains(&None) {
        return Err(SimError::ProtocolMismatch);
    }
    let mut output = json!({
        "available":!samples.is_empty(), "state":"COMPLETE", "duration_ns":duration, "samples":samples,
    });
    if let Some(physical) = &plan.physical {
        physical.append_result(&mut cursor, &plan.endpoints, &mut output)?;
    }
    if !cursor.is_empty() {
        return Err(SimError::ProtocolMismatch);
    }
    Ok(output)
}

fn put32(output: &mut Vec<u8>, value: u32) {
    output.extend_from_slice(&value.to_le_bytes());
}
fn put64(output: &mut Vec<u8>, value: u64) {
    output.extend_from_slice(&value.to_le_bytes());
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn refuses_stale_generation_and_malformed_error_reply() {
        let plan = Plan {
            endpoints: Vec::new(),
            ids: Default::default(),
            edges: Vec::new(),
            gates: Vec::new(),
            sources: Default::default(),
            clocks: Vec::new(),
            probes: Vec::new(),
            physical: None,
        };
        let mut frame = Vec::new();
        put32(&mut frame, 25);
        frame.extend_from_slice(b"CNP1");
        put64(&mut frame, 99);
        put64(&mut frame, 2);
        frame.push(0);
        put32(&mut frame, 0);
        assert_eq!(
            reply(&frame, &plan, 1, 99, 0, &[]),
            Err(SimError::ProtocolMismatch)
        );
        assert!(reply(&frame, &plan, 2, 99, 0, &[]).is_ok());
        frame[24] = 4;
        assert_eq!(
            reply(&frame, &plan, 2, 99, 0, &[]),
            Err(SimError::ProtocolMismatch)
        );
    }
}
