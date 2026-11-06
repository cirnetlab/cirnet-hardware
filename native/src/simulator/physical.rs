use super::{
    SimError,
    compiler::{Module, array, split_endpoint, text},
    protocol::Cursor,
};
use serde_json::{Map, Value, json};
use std::collections::{BTreeSet, HashMap, HashSet};

const IDEAL_ESTIMATE: &str = "Ideal constant-drop series DC estimate; excludes output impedance, transistor and transient behavior.";
const LED_STATES: &[(&str, &str)] = &[
    ("OFF", "Open circuit or no forward bias."),
    (
        "UNSUPPORTED",
        "Only one LED with a series resistor and one ideal DC reference is supported; branching/other analog needs a solver.",
    ),
    (
        "UNKNOWN",
        "The powered digital output driving this DC branch is unknown.",
    ),
    (
        "UNSUPPORTED",
        "Independent floating DC references cannot be compared.",
    ),
    ("UNSUPPORTED", "DC LED subset is limited to 6 V."),
    (
        "UNSUPPORTED",
        "The supported LED subset requires a series resistor.",
    ),
    ("ON", IDEAL_ESTIMATE),
    ("FAULT", IDEAL_ESTIMATE),
];

struct Rails {
    reference: String,
    low: f64,
    voltage: f64,
}

pub(super) struct Physical {
    nodes: HashMap<String, u32>,
    potentials: Vec<Vec<(String, f64)>>,
    rails: HashMap<String, Rails>,
    pub(super) counts: [u32; 6],
    pub(super) wire: Vec<u8>,
    pub(super) fixed_sources: HashSet<String>,
    states: Map<String, Value>,
    leds: Vec<(String, f64)>,
}

impl Physical {
    pub(super) fn compile(
        modules: &[Module<'_>],
        groups: &[Vec<String>],
        signals: &HashMap<String, u32>,
    ) -> Result<Option<Self>, SimError> {
        if !modules.iter().any(|module| module.part.is_some()) {
            return Ok(None);
        }
        let nodes: HashMap<_, _> = groups
            .iter()
            .enumerate()
            .flat_map(|(node, group)| {
                group
                    .iter()
                    .map(move |endpoint| (endpoint.clone(), node as u32))
            })
            .collect();
        let mut physical = Self {
            nodes,
            potentials: vec![Vec::new(); groups.len()],
            rails: HashMap::new(),
            counts: [groups.len() as u32, 0, 0, 0, 0, 0],
            wire: Vec::new(),
            fixed_sources: HashSet::new(),
            states: Map::new(),
            leds: Vec::new(),
        };
        let mut potential_order = Vec::new();
        for module in modules
            .iter()
            .filter(|module| module.model() == "dc-source")
        {
            let (plus, minus) = (
                physical.node(&module.pin("PLUS"))?,
                physical.node(&module.pin("MINUS"))?,
            );
            let voltage = module.number("voltage", "defaultVoltage", 5.0)?;
            let enabled = module
                .property("power_on")
                .map_or(Some(true), Value::as_bool)
                .ok_or(SimError::Unexecutable)?;
            let state = if !enabled {
                "UNPOWERED"
            } else if plus == minus {
                "FAULT"
            } else {
                "POWERED"
            };
            let reason = if state == "FAULT" {
                "Source terminals are shorted."
            } else {
                "Explicit floating DC source pair."
            };
            physical.states.insert(module.id.to_owned(), json!({
                "state":state, "model":"ideal-dc-source", "voltage":setting(module,"voltage","defaultVoltage",json!(5)), "reason":reason,
            }));
            if enabled && plus != minus {
                for (node, level) in [(plus, voltage), (minus, 0.0)] {
                    let at = &mut physical.potentials[node as usize];
                    if at.is_empty() {
                        potential_order.push(node);
                    }
                    at.push((module.id.to_owned(), level));
                }
            }
        }
        for module in modules
            .iter()
            .filter(|module| module.model() == "hc08-powered")
        {
            let power = &module.simulation()["power"];
            let plus = physical.node(&module.pin(text(&power["positivePinId"])?))?;
            let minus = physical.node(&module.pin(text(&power["negativePinId"])?))?;
            let min = power["minVoltage"].as_f64().ok_or(SimError::Unexecutable)?;
            let max = power["maxVoltage"].as_f64().ok_or(SimError::Unexecutable)?;
            let matches = {
                let mut values = physical.differentials(plus, minus);
                values.next().map(|(reference, voltage, low)| {
                    (reference.to_owned(), voltage, low, values.next().is_some())
                })
            };
            let (state, reason) = if plus == minus {
                ("FAULT", "VCC and GND are shorted.")
            } else if let Some((reference, voltage, low, multiple)) = matches {
                if multiple {
                    (
                        "FAULT",
                        "Multiple DC source constraints are outside this model.",
                    )
                } else if voltage == 0.0 {
                    ("UNPOWERED", "No supply differential.")
                } else if !(min..=max).contains(&voltage) {
                    (
                        "FAULT",
                        "Reversed supply or voltage outside the declared HC08 envelope.",
                    )
                } else {
                    physical.rails.insert(
                        module.id.to_owned(),
                        Rails {
                            reference,
                            low,
                            voltage,
                        },
                    );
                    (
                        "POWERED",
                        "Ideal digital HC08 with explicit VCC/GND; no output-current or timing accuracy claim.",
                    )
                }
            } else {
                (
                    "UNPOWERED",
                    "VCC and GND require both terminals of one connected DC source.",
                )
            };
            physical.states.insert(
                module.id.to_owned(),
                json!({"state":state,"model":module.model(),"reason":reason}),
            );
        }
        physical.pack(modules, &potential_order, signals)?;
        Ok(Some(physical))
    }

    fn node(&self, signal: &str) -> Result<u32, SimError> {
        self.nodes
            .get(signal)
            .copied()
            .ok_or(SimError::Unexecutable)
    }

    fn differentials(&self, a: u32, b: u32) -> impl Iterator<Item = (&str, f64, f64)> {
        self.potentials[a as usize]
            .iter()
            .flat_map(move |(reference, high)| {
                self.potentials[b as usize]
                    .iter()
                    .filter(move |(other, _)| other == reference)
                    .map(move |(_, low)| (reference.as_str(), high - low, *low))
            })
    }

    fn differential(&self, a: u32, b: u32) -> Option<f64> {
        if a == b {
            return Some(0.0);
        }
        let mut values = self.differentials(a, b);
        let (_, first, _) = values.next()?;
        values
            .all(|(_, voltage, _)| voltage == first)
            .then_some(first)
    }

    pub(super) fn powered(&self, module: &str) -> bool {
        self.rails.contains_key(module)
    }

    pub(super) fn levels(
        &self,
        modules: &HashMap<&str, &Module<'_>>,
        source: &str,
        target: &str,
    ) -> Result<[u8; 2], SimError> {
        let src = modules[split_endpoint(source)?.0];
        let dst = modules[split_endpoint(target)?.0];
        if src.part.is_none() || dst.model() != "hc08-powered" {
            return Ok([0, 1]);
        }
        let Some(target_rails) = self.rails.get(dst.id) else {
            return Ok([2, 2]);
        };
        let ground = self.node(&dst.pin(text(&dst.simulation()["power"]["negativePinId"])?))?;
        let level = |voltage: Option<f64>| match voltage {
            Some(voltage) if voltage.abs() < 1e-9 => 0,
            Some(voltage) if (voltage - target_rails.voltage).abs() < 1e-9 => 1,
            _ => 2,
        };
        match src.model() {
            "dc-source" => {
                let fixed = level(self.differential(self.node(source)?, ground));
                Ok([fixed, fixed])
            }
            "hc08-powered" => {
                let Some(source_rails) = self.rails.get(src.id) else {
                    return Ok([2, 2]);
                };
                let source_ground =
                    self.node(&src.pin(text(&src.simulation()["power"]["negativePinId"])?))?;
                let low = self.differential(source_ground, ground);
                Ok([
                    level(low),
                    level(low.map(|voltage| voltage + source_rails.voltage)),
                ])
            }
            _ => Ok([0, 1]),
        }
    }

    fn pack(
        &mut self,
        modules: &[Module<'_>],
        potential_order: &[u32],
        signals: &HashMap<String, u32>,
    ) -> Result<(), SimError> {
        let mut references = HashMap::new();
        let mut reference_id = |reference: &str| {
            let next = references.len() as u32;
            *references.entry(reference.to_owned()).or_insert(next)
        };
        for &node in potential_order {
            for (reference, voltage) in &self.potentials[node as usize] {
                put_u32(&mut self.wire, node);
                put_u32(&mut self.wire, reference_id(reference));
                put_f64(&mut self.wire, *voltage);
                self.counts[1] += 1;
            }
        }

        let mut resistors = Vec::new();
        let mut blockers = BTreeSet::new();
        let mut drivers = Vec::new();
        let mut leds = Vec::new();
        for module in modules.iter().filter(|module| module.part.is_some()) {
            let part = module.part.expect("physical module");
            let model = module.model();
            if model == "hc08-powered" {
                if let Some(rails) = self.rails.get(module.id) {
                    let reference = reference_id(&rails.reference);
                    for gate in array(&module.simulation()["gates"]) {
                        let signal = module.pin(text(&gate["output"])?);
                        put_u32(
                            &mut drivers,
                            *signals.get(&signal).ok_or(SimError::Unexecutable)?,
                        );
                        put_u32(&mut drivers, self.node(&signal)?);
                        put_u32(&mut drivers, reference);
                        put_f64(&mut drivers, rails.low);
                        put_f64(&mut drivers, rails.voltage);
                        self.counts[4] += 1;
                    }
                }
            } else if model == "resistor-dc" {
                let resistance = module.number("resistanceOhms", "defaultOhms", 330.0)?;
                let [a, b] = array(&part["package"]["pins"]) else {
                    return Err(SimError::Unexecutable);
                };
                if resistance <= 0.0 {
                    return Err(SimError::Unexecutable);
                }
                for pin in [a, b] {
                    put_u32(&mut resistors, self.node(&module.pin(text(&pin["id"])?))?);
                }
                put_f64(&mut resistors, resistance);
                self.counts[2] += 1;
                self.states.insert(
                    module.id.to_owned(),
                    json!({
                        "state":"PASSIVE","model":"bounded-dc-series",
                        "resistanceOhms":setting(module,"resistanceOhms","defaultOhms",json!(330)),
                    }),
                );
            } else if module.simulation().is_null()
                || module.simulation()["status"] == "unsupported"
            {
                let reason = module.simulation()["reason"].as_str().unwrap_or(if module.simulation().is_null() {
                    "No execution profile is available for this component; placement and wiring remain available."
                } else {
                    "Placement and wiring only; simulation unavailable."
                });
                self.states.insert(
                    module.id.to_owned(),
                    json!({"state":"UNSUPPORTED","model":model,"reason":reason}),
                );
                for pin in array(&part["package"]["pins"]) {
                    blockers.insert(self.node(&module.pin(text(&pin["id"])?))?);
                }
            } else if model == "dc-source" {
                for pin in array(&part["package"]["pins"]) {
                    self.fixed_sources.insert(module.pin(text(&pin["id"])?));
                }
            } else if model == "led-dc" {
                let (forward, maximum) = if module.kind == "GENERIC_LED" {
                    (
                        module.number("forwardVoltageVolts", "forwardVoltage", 2.0)?,
                        module.number("maxCurrentAmps", "maxCurrentAmps", 0.02)?,
                    )
                } else {
                    (
                        module.simulation()["forwardVoltage"]
                            .as_f64()
                            .unwrap_or(2.0),
                        module.simulation()["maxCurrentAmps"]
                            .as_f64()
                            .unwrap_or(0.02),
                    )
                };
                put_u32(&mut leds, self.node(&module.pin("A"))?);
                put_u32(&mut leds, self.node(&module.pin("K"))?);
                put_f64(&mut leds, forward);
                put_f64(&mut leds, maximum);
                self.leds.push((module.id.to_owned(), maximum));
            } else {
                self.states.insert(module.id.to_owned(), json!({
                    "state":"PASSIVE","model":model,"reason":"Only declared conductive contact pairs are modeled."
                }));
            }
        }
        self.counts[3] = blockers.len() as u32;
        self.counts[5] = self.leds.len() as u32;
        self.wire.extend(resistors);
        for blocker in blockers {
            put_u32(&mut self.wire, blocker);
        }
        self.wire.extend(drivers);
        self.wire.extend(leds);
        Ok(())
    }

    pub(super) fn append_result(
        &self,
        cursor: &mut Cursor<'_>,
        endpoints: &[String],
        capture: &mut Value,
    ) -> Result<(), SimError> {
        if cursor.u32()? as usize != endpoints.len() {
            return Err(SimError::ProtocolMismatch);
        }
        let pins = endpoints
            .iter()
            .map(|name| Ok((name.clone(), level_value(cursor.u8()?)?)))
            .collect::<Result<Map<_, _>, SimError>>()?;
        if cursor.u32()? as usize != self.leds.len() {
            return Err(SimError::ProtocolMismatch);
        }
        let mut states = self.states.clone();
        for (name, maximum) in &self.leds {
            let status = cursor.u8()? as usize;
            let current = cursor.f64()?;
            let &(state, reason) = LED_STATES.get(status).ok_or(SimError::ProtocolMismatch)?;
            if !current.is_finite()
                || (status < 6 && current != 0.0)
                || (status == 6 && !(0.0..=*maximum).contains(&current))
                || (status == 7 && current <= *maximum)
            {
                return Err(SimError::ProtocolMismatch);
            }
            let mut value = json!({"state":state,"model":"bounded-dc-series","reason":reason});
            if status >= 6 {
                value["currentAmps"] = json!(current);
            }
            states.insert(name.clone(), value);
        }
        let diagnostics = states
            .values()
            .filter_map(|state| match state["state"].as_str() {
                Some("UNSUPPORTED" | "FAULT" | "UNKNOWN" | "UNPOWERED") => state["reason"].as_str(),
                _ => None,
            })
            .collect::<BTreeSet<_>>();
        capture["diagnostics"] = json!(diagnostics);
        capture["device_states"] = Value::Object(states);
        capture["pin_values"] = Value::Object(pins);
        Ok(())
    }
}

fn setting(module: &Module<'_>, property: &str, key: &str, default: Value) -> Value {
    module
        .property(property)
        .or_else(|| module.simulation().get(key))
        .cloned()
        .unwrap_or(default)
}

fn put_u32(target: &mut Vec<u8>, value: u32) {
    target.extend_from_slice(&value.to_le_bytes());
}
fn put_f64(target: &mut Vec<u8>, value: f64) {
    target.extend_from_slice(&value.to_le_bytes());
}

pub(super) fn level_value(level: u8) -> Result<Value, SimError> {
    match level {
        0 | 1 => Ok(json!(level)),
        2 => Ok(json!("X")),
        _ => Err(SimError::ProtocolMismatch),
    }
}
