use std::collections::{BTreeMap, BTreeSet};
use std::time::Duration;

use serde_json::Value;

use crate::model::Workload;

pub struct Queries {
    pub cpu_request: String,
    pub mem_request: String,
    pub cpu_usage: String,
    pub mem_usage: String,
    pub label: String,
}

impl Queries {
    pub fn with_label(label: &str) -> Self {
        Queries {
            cpu_request: format!("sum by (namespace, {label}) (kube_pod_container_resource_requests{{resource=\"cpu\"}})"),
            mem_request: format!("sum by (namespace, {label}) (kube_pod_container_resource_requests{{resource=\"memory\"}})"),
            cpu_usage: format!("sum by (namespace, {label}) (rate(container_cpu_usage_seconds_total{{container!=\"\"}}[5m]))"),
            mem_usage: format!("sum by (namespace, {label}) (container_memory_working_set_bytes{{container!=\"\"}})"),
            label: label.to_string(),
        }
    }
}

impl Default for Queries {
    fn default() -> Self {
        Queries::with_label("pod")
    }
}

pub fn parse_vector(body: &Value, label: &str) -> Vec<(String, String, f64)> {
    let mut out = Vec::new();
    let results = body
        .get("data")
        .and_then(|d| d.get("result"))
        .and_then(|r| r.as_array());
    let Some(items) = results else {
        return out;
    };
    for item in items {
        let metric = item.get("metric");
        let namespace = metric
            .and_then(|m| m.get("namespace"))
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string();
        let name = metric
            .and_then(|m| m.get(label))
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string();
        let value = item
            .get("value")
            .and_then(|v| v.as_array())
            .and_then(|pair| pair.get(1))
            .and_then(|v| v.as_str())
            .and_then(|s| s.parse::<f64>().ok())
            .filter(|v| v.is_finite())
            .unwrap_or(0.0);
        if !name.is_empty() {
            out.push((namespace, name, value));
        }
    }
    out
}

type Vector = [(String, String, f64)];

fn slot<'a>(
    map: &'a mut BTreeMap<(String, String), Workload>,
    ns: &str,
    name: &str,
) -> &'a mut Workload {
    map.entry((ns.to_string(), name.to_string()))
        .or_insert_with(|| Workload {
            namespace: ns.to_string(),
            name: name.to_string(),
            replicas: 1,
            cpu_request: 0.0,
            mem_request: 0.0,
            cpu_usage: 0.0,
            mem_usage: 0.0,
        })
}

pub fn merge(
    cpu_req: &Vector,
    mem_req: &Vector,
    cpu_use: &Vector,
    mem_use: &Vector,
) -> Vec<Workload> {
    let mut map: BTreeMap<(String, String), Workload> = BTreeMap::new();
    for (ns, name, v) in cpu_req {
        slot(&mut map, ns, name).cpu_request = *v;
    }
    for (ns, name, v) in mem_req {
        slot(&mut map, ns, name).mem_request = *v;
    }
    let mut measured: BTreeSet<(String, String)> = BTreeSet::new();
    for (ns, name, v) in cpu_use {
        slot(&mut map, ns, name).cpu_usage = *v;
        measured.insert((ns.clone(), name.clone()));
    }
    for (ns, name, v) in mem_use {
        slot(&mut map, ns, name).mem_usage = *v;
        measured.insert((ns.clone(), name.clone()));
    }
    map.into_iter()
        .filter(|(key, _)| measured.contains(key))
        .map(|(_, workload)| workload)
        .collect()
}

pub struct Client {
    base: String,
    agent: ureq::Agent,
}

impl Client {
    pub fn new(base: &str) -> Self {
        Client {
            base: base.trim_end_matches('/').to_string(),
            agent: ureq::AgentBuilder::new()
                .timeout(Duration::from_secs(15))
                .build(),
        }
    }

    pub fn query(&self, promql: &str) -> Result<Value, String> {
        self.agent
            .get(&format!("{}/api/v1/query", self.base))
            .query("query", promql)
            .call()
            .map_err(|e| e.to_string())?
            .into_json::<Value>()
            .map_err(|e| e.to_string())
    }
}

pub fn fetch(client: &Client, queries: &Queries) -> Result<Vec<Workload>, String> {
    let cpu_req = parse_vector(&client.query(&queries.cpu_request)?, &queries.label);
    let mem_req = parse_vector(&client.query(&queries.mem_request)?, &queries.label);
    let cpu_use = parse_vector(&client.query(&queries.cpu_usage)?, &queries.label);
    let mem_use = parse_vector(&client.query(&queries.mem_usage)?, &queries.label);
    Ok(merge(&cpu_req, &mem_req, &cpu_use, &mem_use))
}
