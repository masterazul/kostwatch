use kostwatch::cost::{analyze, monthly_cost, HOURS_PER_MONTH};
use kostwatch::model::{Pricing, Workload};
use kostwatch::prom::{merge, parse_vector, Queries};
use serde_json::json;

fn approx(a: f64, b: f64) -> bool {
    (a - b).abs() < 1e-6
}

fn workload(cpu_req: f64, mem_req: f64, cpu_use: f64, mem_use: f64) -> Workload {
    Workload {
        namespace: "n".to_string(),
        name: "a".to_string(),
        replicas: 1,
        cpu_request: cpu_req,
        mem_request: mem_req,
        cpu_usage: cpu_use,
        mem_usage: mem_use,
    }
}

#[test]
fn cost_math_matches_pricing() {
    let pricing = Pricing::default();
    let gib = 1024.0_f64.powi(3);
    let c = monthly_cost(1.0, gib, 1, &pricing);
    assert!(approx(c, (0.04 + 0.005) * HOURS_PER_MONTH));
    assert!(approx(monthly_cost(1.0, gib, 3, &pricing), c * 3.0));
}

#[test]
fn recommends_downsizing_overprovisioned() {
    let gib = 1024.0_f64.powi(3);
    let report = analyze(&[workload(1.0, gib, 0.1, gib * 0.1)], &Pricing::default());
    let f = &report.findings[0];
    assert!(f.monthly_savings > 0.0);
    assert!(f.cpu_recommended < f.cpu_request);
    assert!(f.flags.contains(&"cpu-overprovisioned".to_string()));
    assert!(report.total_monthly_savings > 0.0);
}

#[test]
fn right_sized_workload_has_no_savings() {
    let gib = 1024.0_f64.powi(3);
    let report = analyze(&[workload(1.0, gib, 0.95, gib * 0.95)], &Pricing::default());
    assert!(approx(report.findings[0].monthly_savings, 0.0));
}

#[test]
fn flags_idle_workload() {
    let mib = 1024.0 * 1024.0;
    let report = analyze(&[workload(0.5, 512.0 * mib, 0.0, mib)], &Pricing::default());
    assert!(report.findings[0].flags.contains(&"idle".to_string()));
}

#[test]
fn findings_sorted_by_savings_desc() {
    let gib = 1024.0_f64.powi(3);
    let over = Workload {
        namespace: "n".to_string(),
        name: "over".to_string(),
        replicas: 4,
        cpu_request: 2.0,
        mem_request: gib,
        cpu_usage: 0.1,
        mem_usage: gib * 0.1,
    };
    let tight = Workload {
        namespace: "n".to_string(),
        name: "tight".to_string(),
        replicas: 1,
        cpu_request: 1.0,
        mem_request: gib,
        cpu_usage: 0.95,
        mem_usage: gib * 0.95,
    };
    let report = analyze(&[tight, over], &Pricing::default());
    assert!(report.findings[0].monthly_savings >= report.findings[1].monthly_savings);
    assert_eq!(report.findings[0].name, "over");
}

#[test]
fn parses_and_merges_prometheus_vectors() {
    let cpu_req = json!({"status":"success","data":{"resultType":"vector","result":[
        {"metric":{"namespace":"web","pod":"api-1"},"value":[1700000000.0,"0.5"]},
        {"metric":{"namespace":"web"},"value":[1700000000.0,"9.9"]}
    ]}});
    let cpu_use = json!({"status":"success","data":{"resultType":"vector","result":[
        {"metric":{"namespace":"web","pod":"api-1"},"value":[1700000000.0,"0.05"]}
    ]}});

    let cr = parse_vector(&cpu_req, "pod");
    let cu = parse_vector(&cpu_use, "pod");
    assert_eq!(cr.len(), 1);
    assert_eq!(cr[0], ("web".to_string(), "api-1".to_string(), 0.5));

    let empty: Vec<(String, String, f64)> = Vec::new();
    let merged = merge(&cr, &empty, &cu, &empty);
    assert_eq!(merged.len(), 1);
    assert!(approx(merged[0].cpu_request, 0.5));
    assert!(approx(merged[0].cpu_usage, 0.05));
    assert_eq!(merged[0].replicas, 1);
}

#[test]
fn workload_without_usage_series_is_dropped() {
    let req: Vec<(String, String, f64)> = vec![
        ("web".to_string(), "novo".to_string(), 4.0),
        ("web".to_string(), "medido".to_string(), 1.0),
    ];
    let uso: Vec<(String, String, f64)> = vec![("web".to_string(), "medido".to_string(), 0.5)];
    let vazio: Vec<(String, String, f64)> = Vec::new();

    let merged = merge(&req, &vazio, &uso, &vazio);
    assert_eq!(merged.len(), 1);
    assert_eq!(merged[0].name, "medido");
}

#[test]
fn non_finite_samples_are_discarded() {
    let body = json!({"status":"success","data":{"resultType":"vector","result":[
        {"metric":{"namespace":"web","pod":"a"},"value":[1700000000.0,"NaN"]},
        {"metric":{"namespace":"web","pod":"b"},"value":[1700000000.0,"+Inf"]},
        {"metric":{"namespace":"web","pod":"c"},"value":[1700000000.0,"0.7"]}
    ]}});
    let rows = parse_vector(&body, "pod");
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0].1, "c");
    assert!(rows.iter().all(|(_, _, v)| v.is_finite()));
}

#[test]
fn label_flag_changes_query_grouping() {
    let default = Queries::default();
    assert!(default.cpu_request.contains("sum by (namespace, pod)"));
    assert_eq!(default.label, "pod");

    let workload = Queries::with_label("workload");
    assert!(workload
        .cpu_request
        .contains("sum by (namespace, workload)"));
    assert!(workload.mem_usage.contains("sum by (namespace, workload)"));
    assert_eq!(workload.label, "workload");
}
