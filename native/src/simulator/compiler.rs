use super::{SimError, physical::Physical};
use cirnet_contracts_native::{validation::Registry, wire_json};
use serde_json::{Value, json};
use std::{
    collections::{BTreeMap, HashMap, HashSet},
    fs,
    path::Path,
};

const VISUAL: &[&str] = &[
    "BREADBOARD",
    "BREADBOARD_830",
    "BREADBOARD_630",
    "BREADBOARD_100",
    "BOARD",
    "POWER_SUPPLY",
    "GENERATOR",
    "OSCILLOSCOPE",
    "SIGNAL_MONITOR",
    "ANNOTATION",
    "LABEL",
];

pub(super) struct Catalog {
    parts: HashMap<String, Value>,
    signals: HashMap<String, Value>,
    contacts: HashMap<String, HashMap<String, String>>,
}

impl Catalog {
    pub(super) fn load(root: &Path) -> Result<Self, SimError> {
        let root = root
            .canonicalize()
            .map_err(|_| SimError::BackendUnavailable)?;
        let read = |file: &str| {
            let path = root
                .join("contracts/design-models")
                .join(file)
                .canonicalize()
                .map_err(|_| SimError::BackendUnavailable)?;
            if !path.starts_with(&root) {
                return Err(SimError::BackendUnavailable);
            }
            let bytes = fs::read(path).map_err(|_| SimError::BackendUnavailable)?;
            wire_json::parse(&bytes).map_err(|_| SimError::BackendUnavailable)
        };
        let by_type = |catalog: &Value, entries: &str, field: &str| {
            catalog[entries]
                .as_array()
                .ok_or(SimError::BackendUnavailable)?
                .iter()
                .map(|entry| {
                    let name = entry[field].as_str().ok_or(SimError::BackendUnavailable)?;
                    Ok((name.to_owned(), entry.clone()))
                })
                .collect::<Result<HashMap<_, _>, SimError>>()
        };
        let parts = by_type(&read("physical-parts.v1.json")?, "parts", "editorType")?;
        let signals = by_type(&read("signal-port-catalog.json")?, "components", "type")?;
        let boards = read("breadboards.json")?;
        let mut contacts = HashMap::new();
        for board in boards["profiles"]
            .as_array()
            .ok_or(SimError::BackendUnavailable)?
        {
            let mapping: HashMap<_, _> = board["contacts"]
                .as_array()
                .ok_or(SimError::BackendUnavailable)?
                .iter()
                .map(|entry| {
                    let contact = entry["contactId"]
                        .as_str()
                        .ok_or(SimError::BackendUnavailable)?;
                    let node = entry["nodeId"]
                        .as_str()
                        .ok_or(SimError::BackendUnavailable)?;
                    Ok((contact.to_owned(), node.to_owned()))
                })
                .collect::<Result<_, SimError>>()?;
            for kind in board["moduleTypes"]
                .as_array()
                .ok_or(SimError::BackendUnavailable)?
            {
                contacts.insert(
                    kind.as_str()
                        .ok_or(SimError::BackendUnavailable)?
                        .to_owned(),
                    mapping.clone(),
                );
            }
        }
        Ok(Self {
            parts,
            signals,
            contacts,
        })
    }

    fn classify(&self, modules: &[Module<'_>]) -> Result<(), SimError> {
        let (mut unknown, mut unsupported, mut analog, mut mixed, mut digital) =
            (false, false, false, false, false);
        for module in modules {
            if module.raw.get("signal_domains").is_some()
                && module.raw["signal_domains"]["version"] != "1.0"
            {
                return Err(SimError::Unexecutable);
            }
            let Some(descriptor) = self.signals.get(module.kind) else {
                unknown = true;
                continue;
            };
            match descriptor["execution"].as_str() {
                Some("VISUAL" | "DC_DIGITAL") => {}
                Some("DIGITAL") => digital = true,
                _ => {
                    unsupported = true;
                    for port in array(&descriptor["ports"]) {
                        match port["domain"].as_str() {
                            Some("ANALOG") => analog = true,
                            Some("MIXED") => mixed = true,
                            Some("DIGITAL" | "CLOCK") => digital = true,
                            _ => {}
                        }
                    }
                }
            }
        }
        if unknown {
            return Err(SimError::Unexecutable);
        }
        if mixed || analog && digital {
            return Err(SimError::MixedSignalUnavailable);
        }
        if analog {
            return Err(SimError::AnalogUnavailable);
        }
        if unsupported {
            return Err(SimError::Unexecutable);
        }
        Ok(())
    }

    fn domain<'a>(&'a self, module: &Module<'a>, pin: &str) -> Option<&'a str> {
        if let Some(descriptor) = self.signals.get(module.kind) {
            return array(&descriptor["ports"])
                .iter()
                .find(|port| port["id"] == pin)
                .and_then(|port| port["domain"].as_str());
        }
        module.raw["signal_domains"]["ports"][pin].as_str()
    }

    fn check_domains(
        &self,
        a: &Module<'_>,
        a_pin: &str,
        b: &Module<'_>,
        b_pin: &str,
    ) -> Result<(), SimError> {
        if a_pin.starts_with("contact:") || b_pin.starts_with("contact:") {
            return Ok(());
        }
        let (Some(a_domain), Some(b_domain)) = (self.domain(a, a_pin), self.domain(b, b_pin))
        else {
            return Err(SimError::Unexecutable);
        };
        if a_domain == b_domain
            || matches!(
                (a_domain, b_domain),
                ("CLOCK", "DIGITAL") | ("DIGITAL", "CLOCK")
            )
        {
            return Ok(());
        }
        let dc = |module: &Module<'_>| {
            self.signals
                .get(module.kind)
                .is_some_and(|entry| entry["execution"] == "DC_DIGITAL")
        };
        if a_domain != "MIXED" && b_domain != "MIXED" && (dc(a) || dc(b)) {
            return Ok(());
        }
        Err(SimError::Unexecutable)
    }
}

pub(super) struct Module<'a> {
    pub(super) id: &'a str,
    pub(super) kind: &'a str,
    pub(super) raw: &'a Value,
    pub(super) part: Option<&'a Value>,
}

impl Module<'_> {
    pub(super) fn properties(&self) -> &Value {
        &self.raw["properties"]
    }

    pub(super) fn property(&self, name: &str) -> Option<&Value> {
        self.properties().get(name).or_else(|| {
            (self.kind == "RESISTOR_AXIAL" && name == "resistanceOhms")
                .then(|| self.properties().get("resistance_ohm"))
                .flatten()
        })
    }

    pub(super) fn simulation(&self) -> &Value {
        self.part.map_or(&Value::Null, |part| &part["simulation"])
    }

    pub(super) fn model(&self) -> &str {
        self.simulation()["model"].as_str().unwrap_or("unavailable")
    }

    pub(super) fn number(&self, name: &str, setting: &str, default: f64) -> Result<f64, SimError> {
        let number = self
            .property(name)
            .or_else(|| self.simulation().get(setting))
            .map_or(Some(default), Value::as_f64);
        number
            .filter(|value| value.is_finite())
            .ok_or(SimError::Unexecutable)
    }

    pub(super) fn pin(&self, name: &str) -> String {
        format!("{}.{name}", self.id)
    }
}

pub(super) fn array(value: &Value) -> &[Value] {
    value.as_array().map(Vec::as_slice).unwrap_or_default()
}

pub(super) fn text(value: &Value) -> Result<&str, SimError> {
    value.as_str().ok_or(SimError::Unexecutable)
}

#[derive(Clone, Copy, Eq, PartialEq)]
enum Direction {
    Input,
    Output,
    Passive,
}

pub(super) struct Edge {
    pub(super) source: u32,
    pub(super) target: u32,
    pub(super) levels: [u8; 2],
}

pub(super) struct Gate {
    pub(super) operation: u8,
    pub(super) left: u32,
    pub(super) right: u32,
    pub(super) output: u32,
    pub(super) delay: u64,
    pub(super) enabled: bool,
}

pub(super) struct Clock {
    pub(super) name: String,
    pub(super) period: u64,
    pub(super) signal: u32,
}

pub(super) struct Plan {
    pub(super) endpoints: Vec<String>,
    pub(super) ids: HashMap<String, u32>,
    pub(super) edges: Vec<Edge>,
    pub(super) gates: Vec<Gate>,
    pub(super) sources: BTreeMap<String, u8>,
    pub(super) clocks: Vec<Clock>,
    pub(super) probes: Vec<String>,
    pub(super) physical: Option<Physical>,
}

struct Endpoint {
    name: String,
    direction: Direction,
}

#[derive(Default)]
struct Ports {
    endpoints: Vec<Endpoint>,
    ids: HashMap<String, u32>,
}

impl Ports {
    fn add(&mut self, name: String, direction: Direction) -> u32 {
        if let Some(&id) = self.ids.get(&name) {
            self.endpoints[id as usize].direction = direction;
            return id;
        }
        let id = self.endpoints.len() as u32;
        self.ids.insert(name.clone(), id);
        self.endpoints.push(Endpoint { name, direction });
        id
    }

    fn direction(&self, name: &str) -> Option<Direction> {
        self.ids
            .get(name)
            .map(|id| self.endpoints[*id as usize].direction)
    }

    fn get(&self, name: &str) -> Result<u32, SimError> {
        self.ids.get(name).copied().ok_or(SimError::Unexecutable)
    }
}

struct NamedGate {
    name: String,
    owner: String,
    gate: Gate,
}

#[derive(Default)]
struct Builder {
    ports: Ports,
    gates: Vec<NamedGate>,
    sources: BTreeMap<String, u8>,
    clocks: Vec<Clock>,
    probes: Vec<String>,
}

impl Builder {
    fn gate(
        &mut self,
        module: &Module<'_>,
        name: String,
        operation: u8,
        inputs: &[&str],
        output: &str,
        delay: u64,
    ) {
        let left = self.ports.add(module.pin(inputs[0]), Direction::Input);
        let right = if inputs.len() == 2 {
            self.ports.add(module.pin(inputs[1]), Direction::Input)
        } else {
            0
        };
        let output = self.ports.add(module.pin(output), Direction::Output);
        self.gates.push(NamedGate {
            name,
            owner: module.id.to_owned(),
            gate: Gate {
                operation,
                left,
                right,
                output,
                delay,
                enabled: true,
            },
        });
    }

    fn module(&mut self, module: &Module<'_>, preview: bool) -> Result<(), SimError> {
        let delay = match module.property("delay_ns") {
            Some(value) => value
                .as_u64()
                .filter(|delay| *delay <= 1_000_000_000)
                .ok_or(SimError::Unexecutable)?,
            None => 8,
        };
        let delay = if preview { 0 } else { delay };
        if let Some(part) = module.part {
            for pin in array(&part["package"]["pins"]) {
                self.ports
                    .add(module.pin(text(&pin["id"])?), Direction::Passive);
            }
            match module.model() {
                "dc-source" => {
                    let on = module
                        .property("power_on")
                        .map_or(Some(true), Value::as_bool)
                        .ok_or(SimError::Unexecutable)?;
                    if on {
                        let voltage = module.number("voltage", "defaultVoltage", 5.0)?;
                        for (name, value) in [("PLUS", u8::from(voltage > 0.0)), ("MINUS", 0)] {
                            let name = module.pin(name);
                            self.ports.add(name.clone(), Direction::Output);
                            self.sources.insert(name, value);
                        }
                    }
                }
                "hc08-powered" => {
                    for (index, gate) in array(&module.simulation()["gates"]).iter().enumerate() {
                        let inputs = gate["inputs"].as_array().ok_or(SimError::Unexecutable)?;
                        let [left, right] = inputs.as_slice() else {
                            return Err(SimError::Unexecutable);
                        };
                        self.gate(
                            module,
                            format!("{}:{}", module.id, index + 1),
                            0,
                            &[text(left)?, text(right)?],
                            text(&gate["output"])?,
                            delay,
                        );
                    }
                }
                _ => {}
            }
            return Ok(());
        }
        if VISUAL.contains(&module.kind) {
            return Ok(());
        }
        match module.kind {
            "DIGITAL_SWITCH" | "DIGITAL_INPUT" | "INPUT" => {
                let level = match module.property("value") {
                    Some(value) => value
                        .as_u64()
                        .filter(|value| *value <= 1)
                        .ok_or(SimError::Unexecutable)? as u8,
                    None => u8::from(
                        module
                            .property("state")
                            .map_or(Some(false), Value::as_bool)
                            .ok_or(SimError::Unexecutable)?,
                    ),
                };
                let name = module.pin("OUT");
                self.ports.add(name.clone(), Direction::Output);
                self.sources.insert(name, level);
            }
            "CLOCK" => {
                let frequency = module.number("frequency_hz", "unused", 1000.0)?;
                let period = (1_000_000_000.0 / frequency).round_ties_even();
                if frequency <= 0.0 || !period.is_finite() || period < 2.0 {
                    return Err(SimError::Unexecutable);
                }
                let name = module.pin("OUT");
                let signal = self.ports.add(name.clone(), Direction::Output);
                self.sources.insert(name.clone(), 0);
                self.clocks.push(Clock {
                    name,
                    period: period as u64,
                    signal,
                });
            }
            "LED" | "PROBE" | "OUTPUT" => {
                let name = module.pin("IN");
                self.ports.add(name.clone(), Direction::Input);
                self.probes.push(name);
            }
            "74HC08" => {
                for index in 1..=4 {
                    self.gate(
                        module,
                        format!("{}:{index}", module.id),
                        0,
                        &[&format!("A{index}"), &format!("B{index}")],
                        &format!("Y{index}"),
                        delay,
                    );
                }
            }
            "AND" => self.gate(module, module.id.into(), 0, &["A", "B"], "Y", delay),
            "OR" => self.gate(module, module.id.into(), 1, &["A", "B"], "Y", delay),
            "XOR" => self.gate(module, module.id.into(), 2, &["A", "B"], "Y", delay),
            "NOT" => self.gate(module, module.id.into(), 3, &["A"], "Y", delay),
            _ => return Err(SimError::Unexecutable),
        }
        Ok(())
    }
}

#[derive(Default)]
struct Nets {
    keys: HashMap<(String, bool, String), usize>,
    endpoints: HashMap<String, usize>,
    parent: Vec<usize>,
}

impl Nets {
    fn register(
        &mut self,
        name: &str,
        modules: &HashMap<&str, &Module<'_>>,
        catalog: &Catalog,
        ports: &Ports,
    ) -> Result<usize, SimError> {
        if let Some(&key) = self.endpoints.get(name) {
            return Ok(key);
        }
        let (id, terminal) = split_endpoint(name)?;
        let module = modules.get(id).ok_or(SimError::Unexecutable)?;
        let key = if let Some(contact) = terminal.strip_prefix("contact:") {
            let node = catalog
                .contacts
                .get(module.kind)
                .and_then(|map| map.get(contact))
                .ok_or(SimError::Unexecutable)?;
            (id.to_owned(), true, node.to_owned())
        } else if ports.ids.contains_key(name) {
            (id.to_owned(), false, terminal.to_owned())
        } else {
            return Err(SimError::Unexecutable);
        };
        let key = *self.keys.entry(key).or_insert_with(|| {
            let index = self.parent.len();
            self.parent.push(index);
            index
        });
        self.endpoints.insert(name.to_owned(), key);
        Ok(key)
    }

    fn root(&mut self, mut key: usize) -> usize {
        let mut root = key;
        while self.parent[root] != root {
            root = self.parent[root];
        }
        while self.parent[key] != key {
            let next = self.parent[key];
            self.parent[key] = root;
            key = next;
        }
        root
    }

    fn bond(
        &mut self,
        a: &str,
        b: &str,
        modules: &HashMap<&str, &Module<'_>>,
        catalog: &Catalog,
        ports: &Ports,
    ) -> Result<(), SimError> {
        let a = self.register(a, modules, catalog, ports)?;
        let b = self.register(b, modules, catalog, ports)?;
        let (a, b) = (self.root(a), self.root(b));
        if a != b {
            self.parent[a.max(b)] = a.min(b);
        }
        Ok(())
    }

    fn groups(mut self) -> Vec<Vec<String>> {
        let mut groups: HashMap<_, Vec<_>> = HashMap::new();
        for (endpoint, key) in std::mem::take(&mut self.endpoints) {
            groups.entry(self.root(key)).or_default().push(endpoint);
        }
        let mut groups: Vec<_> = groups.into_values().collect();
        for group in &mut groups {
            group.sort();
        }
        groups.sort();
        groups
    }
}

pub(super) fn split_endpoint(value: &str) -> Result<(&str, &str), SimError> {
    value
        .split_once('.')
        .filter(|(_, pin)| !pin.contains('.'))
        .ok_or(SimError::Unexecutable)
}

fn resolve(
    graph: &Value,
    modules: &[Module<'_>],
    indexed: &HashMap<&str, &Module<'_>>,
    catalog: &Catalog,
    ports: &Ports,
    preview: bool,
) -> Result<Vec<Vec<String>>, SimError> {
    let mut nets = Nets::default();
    for name in ports.ids.keys() {
        nets.register(name, indexed, catalog, ports)?;
    }
    for module in modules {
        if let Some(mount) = module.raw.get("mount") {
            let board = text(&mount["boardId"])?;
            for binding in array(&mount["contacts"]) {
                nets.bond(
                    &module.pin(text(&binding["pinId"])?),
                    &format!("{board}.contact:{}", text(&binding["contactId"])?),
                    indexed,
                    catalog,
                    ports,
                )?;
            }
        }
        if let Some(part) = module.part {
            for internal in array(&part["internalConnections"]) {
                if let Some(when) = internal.get("when") {
                    let key = text(&when["property"])?;
                    let actual = module
                        .property(key)
                        .unwrap_or(&part["editor"]["defaults"][key]);
                    let expected = &when["equals"];
                    if actual != expected
                        && !matches!((actual.as_f64(), expected.as_f64()), (Some(a), Some(b)) if a == b)
                    {
                        continue;
                    }
                }
                let Some((first, rest)) = array(&internal["pins"]).split_first() else {
                    return Err(SimError::Unexecutable);
                };
                for pin in rest {
                    nets.bond(
                        &module.pin(text(first)?),
                        &module.pin(text(pin)?),
                        indexed,
                        catalog,
                        ports,
                    )?;
                }
            }
        }
    }
    let mut edges = HashSet::new();
    for connection in array(&graph["connections"]) {
        let (source, destination) = (
            text(&connection["source"])?,
            text(&connection["destination"])?,
        );
        if source == destination
            || !edges.insert((source.min(destination), source.max(destination)))
        {
            return Err(SimError::Unexecutable);
        }
        let (source_id, source_pin) = split_endpoint(source)?;
        let (target_id, target_pin) = split_endpoint(destination)?;
        let a = indexed.get(source_id).ok_or(SimError::Unexecutable)?;
        let b = indexed.get(target_id).ok_or(SimError::Unexecutable)?;
        if !preview {
            catalog.check_domains(a, source_pin, b, target_pin)?;
        }
        if a.part.is_none()
            && b.part.is_none()
            && !source_pin.starts_with("contact:")
            && !target_pin.starts_with("contact:")
            && (ports.direction(source) != Some(Direction::Output)
                || ports.direction(destination) != Some(Direction::Input))
        {
            return Err(SimError::Unexecutable);
        }
        nets.bond(source, destination, indexed, catalog, ports)?;
    }
    Ok(nets.groups())
}

pub(super) fn compile(
    graph: &Value,
    registry: &Registry,
    catalog: &Catalog,
    preview: bool,
) -> Result<Plan, SimError> {
    let modules = graph["modules"].as_array().ok_or(SimError::Unexecutable)?;
    let connections = graph["connections"]
        .as_array()
        .ok_or(SimError::Unexecutable)?;
    if modules.len() > 2000 || connections.len() > 10_000 {
        return Err(SimError::GraphLimit);
    }
    let mut ids = HashSet::new();
    let modules: Vec<_> = modules
        .iter()
        .map(|raw| {
            let id = text(&raw["id"])?;
            let kind = text(&raw["type"])?;
            if id.contains('.') || !ids.insert(id) {
                return Err(SimError::Unexecutable);
            }
            Ok(Module {
                id,
                kind,
                raw,
                part: catalog.parts.get(kind),
            })
        })
        .collect::<Result<_, _>>()?;
    if modules.iter().any(|module| {
        module.part.is_some()
            || module.raw.get("mount").is_some()
            || module.raw.get("physicalDescriptor").is_some()
    }) {
        // Assignment reference validation is the pinned crate's public boundary
        // for the shared physical-design, mounting and collision checks.
        let reference = json!({
            "schema_version": "2.0", "assignment_id": "simulator",
            "title": "Simulator circuit", "lab_endpoints": [],
            "assessment_policy": "FUNCTIONAL_ONLY", "reference_design_graph": graph,
        });
        registry
            .validate("AssignmentDefinitionV2", "structural", &reference)
            .map_err(|_| SimError::Unexecutable)?;
    } else {
        registry
            .validate("AnySupportedCircuitGraph", "structural", graph)
            .map_err(|_| SimError::Unexecutable)?;
    }
    if !preview {
        catalog.classify(&modules)?;
    }

    let indexed: HashMap<_, _> = modules.iter().map(|module| (module.id, module)).collect();
    let mut builder = Builder::default();
    for module in &modules {
        builder.module(module, preview)?;
    }
    let groups = resolve(graph, &modules, &indexed, catalog, &builder.ports, preview)?;
    let physical = Physical::compile(&modules, &groups, &builder.ports.ids)?;
    if let Some(physical) = &physical {
        for gate in &mut builder.gates {
            if indexed[gate.owner.as_str()].model() == "hc08-powered" {
                gate.gate.enabled = physical.powered(&gate.owner);
            }
        }
    }
    let mut nets = Vec::new();
    for group in &groups {
        let drivers: Vec<_> = group
            .iter()
            .filter(|name| builder.ports.direction(name) == Some(Direction::Output))
            .collect();
        if drivers.len() > 1
            && !drivers.iter().all(|name| {
                split_endpoint(name).is_ok_and(|(module, pin)| {
                    pin == "MINUS" && indexed[module].model() == "dc-source"
                })
            })
        {
            return Err(SimError::Unexecutable);
        }
        if let Some(source) = drivers.first() {
            for target in group
                .iter()
                .filter(|name| builder.ports.direction(name) == Some(Direction::Input))
            {
                let levels = match &physical {
                    Some(physical) => physical.levels(&indexed, source, target)?,
                    None => [0, 1],
                };
                nets.push((
                    source.as_str(),
                    target.as_str(),
                    Edge {
                        source: builder.ports.get(source)?,
                        target: builder.ports.get(target)?,
                        levels,
                    },
                ));
            }
        }
    }
    nets.sort_by_key(|(source, target, _)| (*source, *target));
    builder.gates.sort_by(|a, b| a.name.cmp(&b.name));
    builder.probes.sort();
    Ok(Plan {
        endpoints: builder
            .ports
            .endpoints
            .into_iter()
            .map(|endpoint| endpoint.name)
            .collect(),
        ids: builder.ports.ids,
        edges: nets.into_iter().map(|(_, _, edge)| edge).collect(),
        gates: builder.gates.into_iter().map(|named| named.gate).collect(),
        sources: builder.sources,
        clocks: builder.clocks,
        probes: builder.probes,
        physical,
    })
}
