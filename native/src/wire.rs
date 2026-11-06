use crate::pb;
use serde_json::{Value, json};

pub fn clock(value: &pb::ClockConfiguration) -> Value {
    json!({"signal": value.signal, "period_ns": value.period_ns, "cycles": value.cycles})
}

pub fn trigger(value: &pb::TriggerConfiguration) -> Value {
    json!({"signal": value.signal, "edge": value.edge})
}

fn protobuf_number(value: f64) -> Value {
    if value.is_nan() {
        json!("NaN")
    } else if value == f64::INFINITY {
        json!("Infinity")
    } else if value == f64::NEG_INFINITY {
        json!("-Infinity")
    } else {
        json!(value)
    }
}

pub fn generator(value: &pb::GeneratorConfiguration) -> Value {
    let mut configuration = json!({
        "channel": value.channel,
        "waveform": value.waveform,
        "frequency_hz": protobuf_number(value.frequency_hz),
        "amplitude_vpp": protobuf_number(value.amplitude_vpp),
        "offset_v": protobuf_number(value.offset_v),
        "enabled": value.enabled,
    });
    if let Some(phase) = value.phase_degrees {
        configuration["phase_degrees"] = protobuf_number(phase);
    }
    configuration
}

pub fn capture(value: &pb::CaptureConfiguration) -> Value {
    let mut configuration = json!({"signals": value.signals, "duration_ns": value.duration_ns});
    if let Some(max_events) = value.max_events {
        configuration["max_events"] = json!(max_events);
    }
    if let Some(value) = &value.trigger {
        configuration["trigger"] = trigger(value);
    }
    configuration
}

pub fn decision(value: &pb::ExecutionDecision) -> Value {
    json!({"schema_version": value.schema_version, "decision": value.decision, "reasons": value.reasons})
}

pub fn plan(value: &pb::InstrumentationExecutionPlan) -> Value {
    let endpoint_bindings: Vec<_> = value.endpoint_bindings.iter().map(|binding| {
        json!({"endpoint_id": binding.endpoint_id, "node_id": binding.node_id, "role": binding.role})
    }).collect();
    let routing_allocations: Vec<_> = value.routing_allocations.iter().map(|allocation| {
        json!({"endpoint_id": allocation.endpoint_id, "resource_id": allocation.resource_id})
    }).collect();
    let instrument_allocations: Vec<_> = value.instrument_allocations.iter().map(|allocation| {
        json!({"endpoint_id": allocation.endpoint_id, "instrument": allocation.instrument, "channel": allocation.channel})
    }).collect();
    let test_vectors: Vec<_> = value
        .test_vectors
        .iter()
        .map(|vector| json!({"time_ns": vector.time_ns, "inputs": vector.inputs}))
        .collect();
    let clock = value
        .clock_config
        .as_ref()
        .filter(|config| !config.signal.is_empty() || config.period_ns != 0 || config.cycles != 0)
        .map_or_else(|| json!({}), clock);
    let mut plan = json!({
        "schema_version": value.schema_version,
        "station_id": value.station_id,
        "experiment_id": value.experiment_id,
        "endpoint_bindings": endpoint_bindings,
        "routing_allocations": routing_allocations,
        "instrument_allocations": instrument_allocations,
        "clock_config": clock,
        "generator_config": value.generator_config.iter().map(generator).collect::<Vec<_>>(),
        "test_vectors": test_vectors,
    });
    if let Some(policy) = &value.power_policy {
        plan["power_policy"] = json!({"profile_id": policy.profile_id, "guarded": policy.guarded});
    }
    if let Some(config) = &value.capture_config {
        plan["capture_config"] = capture(config);
    }
    if let Some(policy) = &value.runtime_safety_policy {
        plan["runtime_safety_policy"] = json!({
            "profile_id": policy.profile_id, "watchdog_required": policy.watchdog_required
        });
    }
    plan
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn retains_optional_presence_and_proto_defaults() {
        assert_eq!(
            capture(&pb::CaptureConfiguration::default()),
            json!({
                "signals": [], "duration_ns": 0
            })
        );
        assert_eq!(
            capture(&pb::CaptureConfiguration {
                max_events: Some(0),
                trigger: Some(pb::TriggerConfiguration::default()),
                ..Default::default()
            }),
            json!({"signals":[],"duration_ns":0,"max_events":0,"trigger":{"signal":"","edge":""}})
        );
        assert_eq!(
            plan(&pb::InstrumentationExecutionPlan::default())["clock_config"],
            json!({})
        );
        assert_eq!(
            generator(&pb::GeneratorConfiguration {
                frequency_hz: f64::INFINITY,
                ..Default::default()
            })["frequency_hz"],
            json!("Infinity")
        );
    }
}
