use cirnet_contracts_native::{capture, validation::Registry};
use cirnet_hardware_native::simulator::{SimError, Simulator};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use std::{
    path::PathBuf,
    sync::{Arc, atomic::AtomicBool},
};

fn simulator() -> Simulator {
    let root =
        PathBuf::from(std::env::var_os("CIRNET_CONTRACTS_ROOT").expect("pinned Contracts root"));
    let worker =
        PathBuf::from(std::env::var_os("CIRNET_SIM_CPP_WORKER").expect("pinned Simulator worker"));
    let sha256 = std::env::var("CIRNET_SIM_CPP_WORKER_SHA256").expect("pinned Simulator digest");
    let registry = Arc::new(Registry::load(&root).unwrap());
    Simulator::new(worker, &sha256, 4, &root, registry).unwrap()
}

#[test]
#[ignore = "requires the pinned Contracts resources and worker"]
fn worker_preserves_digital_samples_and_exact_ncap() {
    let simulator = simulator();
    let graph = json!({
        "schema_version": "1.0", "circuit_id": "stage3-and",
        "modules": [
            {"id":"A", "type":"DIGITAL_SWITCH", "properties":{"state":false}},
            {"id":"B", "type":"DIGITAL_SWITCH", "properties":{"state":true}},
            {"id":"G", "type":"74HC08"},
        ],
        "connections": [
            {"source":"A.OUT", "destination":"G.A1"},
            {"source":"B.OUT", "destination":"G.B1"},
        ],
    });
    let plan = simulator.compile(&graph).unwrap();
    assert!(plan.has_endpoint("G.Y1"));
    let output = simulator
        .execute(
            &plan,
            &json!({"signals":["G.Y1"], "duration_ns":40}),
            &json!([{"time_ns":20,"inputs":{"A.OUT":1}}]),
            &Value::Null,
            &Value::Null,
            &AtomicBool::new(false),
        )
        .unwrap();
    let expected = json!([
        {"signal":"G.Y1","time_ns":0,"value":"X"},
        {"signal":"G.Y1","time_ns":8,"value":0},
        {"signal":"G.Y1","time_ns":28,"value":1},
    ]);
    assert_eq!(output["samples"], expected);
    let ncap = capture::encode_digital(&output["samples"], &output["duration_ns"]).unwrap();
    // The Stage 1 legacy reference fixture uses the same signal, times and levels.
    assert_eq!(
        format!("{:x}", Sha256::digest(ncap)),
        "a7d9fbe031939f9cdbf60f0f626b325beac173d9e720eb968fc62d9b83314010"
    );
    simulator.shutdown().unwrap();
}

#[test]
#[ignore = "requires the pinned Contracts resources and worker"]
fn preview_projects_powered_dc_and_reports_unsupported_parts() {
    let simulator = simulator();
    let graph = json!({
        "schema_version":"1.1", "circuit_id":"stage3-power",
        "modules":[
            {"id":"PS","type":"DC_SUPPLY","properties":{"voltage":5}},
            {"id":"U","type":"SN74HC08N"},
            {"id":"A","type":"DIGITAL_SWITCH","properties":{"state":true}},
            {"id":"R","type":"RESISTOR_AXIAL","properties":{"resistance_ohm":330}},
            {"id":"L","type":"LED_LTL307EE"},
        ],
        "connections":[
            {"source":"PS.PLUS","destination":"U.14"},
            {"source":"PS.MINUS","destination":"U.7"},
            {"source":"PS.PLUS","destination":"U.1"},
            {"source":"A.OUT","destination":"U.2"},
            {"source":"U.3","destination":"R.A"},
            {"source":"R.B","destination":"L.A"},
            {"source":"L.K","destination":"PS.MINUS"},
        ],
    });
    let output = simulator.preview(&graph).unwrap();
    assert_eq!(output["device_states"]["U"]["state"], "POWERED");
    assert_eq!(output["device_states"]["L"]["state"], "ON");
    assert_eq!(output["device_states"]["R"]["resistanceOhms"], 330);
    assert_eq!(output["pin_values"]["U.3"], 1);
    assert_eq!(output["diagnostics"], json!([]));
    let plan = simulator.compile(&graph).unwrap();
    let config = json!({"signals":["U.3", "U.1"], "duration_ns":30});
    let cancelled = AtomicBool::new(false);
    let capture = simulator
        .execute(
            &plan,
            &config,
            &Value::Null,
            &Value::Null,
            &Value::Null,
            &cancelled,
        )
        .unwrap();
    assert_eq!(
        capture["samples"],
        json!([
            {"signal":"U.3","time_ns":0,"value":"X"},
            {"signal":"U.1","time_ns":0,"value":1},
            {"signal":"U.3","time_ns":8,"value":1},
        ])
    );
    assert_eq!(capture["device_states"], output["device_states"]);
    assert_eq!(
        simulator.execute(
            &plan,
            &config,
            &Value::Null,
            &Value::Null,
            &json!({"PS.PLUS":0}),
            &cancelled
        ),
        Err(SimError::Unexecutable),
    );

    let unsupported = json!({
        "schema_version":"1.1", "circuit_id":"stage3-unsupported",
        "modules":[{"id":"Q","type":"TRANSISTOR_2N3904"}], "connections":[],
    });
    assert!(simulator.compile(&unsupported).is_err());
    assert_eq!(
        simulator.preview(&unsupported).unwrap()["device_states"]["Q"]["state"],
        "UNSUPPORTED"
    );
    simulator.shutdown().unwrap();
}

#[test]
#[ignore = "requires the pinned Contracts resources and worker"]
fn clock_and_vectors_preserve_order_and_exact_event_budget() {
    let simulator = simulator();
    let graph = json!({
        "schema_version":"1.0", "circuit_id":"clock",
        "modules":[{"id":"C","type":"CLOCK","properties":{"frequency_hz":100_000_000}}],
        "connections":[],
    });
    let plan = simulator.compile(&graph).unwrap();
    let clock = json!({"signal":"C.OUT","period_ns":5.0,"cycles":3.0});
    let config = json!({"signals":["C.OUT"],"duration_ns":12.0,"max_events":6.0});
    let cancelled = AtomicBool::new(false);
    let run = |config: &Value, vectors: &Value, cancelled: &AtomicBool| {
        simulator.execute(&plan, config, vectors, &clock, &Value::Null, cancelled)
    };
    let capture = run(&config, &Value::Null, &cancelled).unwrap();
    let timeline: Vec<_> = capture["samples"]
        .as_array()
        .unwrap()
        .iter()
        .map(|sample| sample["time_ns"].as_u64().unwrap())
        .collect();
    assert_eq!(timeline, [0, 2, 5, 7, 10, 12]);
    let mut too_few = config.clone();
    too_few["max_events"] = json!(5);
    assert_eq!(
        run(&too_few, &Value::Null, &cancelled),
        Err(SimError::EventLimit)
    );
    let vectors = json!([
        {"time_ns":2,"inputs":{"C.OUT":0}},
        {"time_ns":5,"inputs":{"C.OUT":1}},
    ]);
    let config = json!({"signals":["C.OUT"],"duration_ns":5});
    assert_eq!(
        run(&config, &vectors, &cancelled).unwrap()["samples"],
        json!([
            {"signal":"C.OUT","time_ns":0,"value":0},
            {"signal":"C.OUT","time_ns":5,"value":1},
        ]),
    );
    assert_eq!(
        run(&config, &vectors, &AtomicBool::new(true)),
        Err(SimError::Cancelled),
    );
    assert_eq!(
        simulator.preview(&graph).unwrap()["diagnostics"],
        json!([
            "Static DC preview holds clocks at their initial level; use virtual capture for waveforms."
        ]),
    );
    simulator.shutdown().unwrap();
    assert_eq!(
        run(&config, &vectors, &cancelled),
        Err(SimError::BackendUnavailable)
    );
}

#[test]
#[ignore = "requires the pinned Contracts resources and worker"]
fn mounted_breadboard_resolves_only_validated_bonds() {
    let root = PathBuf::from(std::env::var_os("CIRNET_CONTRACTS_ROOT").unwrap());
    let fixture =
        std::fs::read(root.join("contracts/design-models/fixtures/physical-mounting-parity.json"))
            .unwrap();
    let mut fixture = cirnet_contracts_native::wire_json::parse(&fixture).unwrap();
    let graph = &mut fixture["graph"];
    graph["modules"].as_array_mut().unwrap().push(json!({
        "id":"PS","type":"DC_SUPPLY","position":{"x":-5,"y":0,"z":5},
    }));
    graph["connections"] = json!([
        {"source":"PS.PLUS","destination":"BB.contact:rail:top-positive:01"},
        {"source":"BB.contact:rail:top-positive:50","destination":"BB.contact:terminal:A:25"},
        {"source":"PS.MINUS","destination":"BB.contact:terminal:J:31"},
        {"source":"PS.PLUS","destination":"BB.contact:terminal:J:25"},
        {"source":"PS.PLUS","destination":"BB.contact:terminal:J:26"},
    ]);
    let simulator = simulator();
    let preview = simulator.preview(graph).unwrap();
    assert_eq!(preview["device_states"]["U"]["state"], "POWERED");
    assert_eq!(preview["pin_values"]["U.3"], 1);
    // Existing physical mounting semantics reject a pin bonded to the wrong strip.
    graph["modules"][1]["mount"]["contacts"][0]["contactId"] = json!("terminal:F:26");
    assert!(matches!(
        simulator.compile(graph),
        Err(SimError::Unexecutable)
    ));
    assert_eq!(simulator.preview(graph), Err(SimError::Unexecutable));
    simulator.shutdown().unwrap();
}
