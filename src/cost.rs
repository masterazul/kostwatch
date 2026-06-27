use crate::model::{Finding, Pricing, Report, Workload};

pub const HOURS_PER_MONTH: f64 = 730.0;
const GIB: f64 = 1024.0 * 1024.0 * 1024.0;
const HEADROOM: f64 = 1.25;
const CPU_FLOOR: f64 = 0.01;
const MEM_FLOOR: f64 = 16.0 * 1024.0 * 1024.0;
const IDLE_CPU: f64 = 0.005;
const OVERPROVISION: f64 = 0.8;

pub fn monthly_cost(cpu: f64, mem: f64, replicas: u32, pricing: &Pricing) -> f64 {
    let per_replica = cpu * pricing.cpu_hour + (mem / GIB) * pricing.mem_gb_hour;
    per_replica * replicas as f64 * HOURS_PER_MONTH
}

fn rightsize(usage: f64, request: f64, floor: f64) -> f64 {
    let target = (usage * HEADROOM).max(floor);
    if target < request {
        target
    } else {
        request
    }
}

pub fn analyze(workloads: &[Workload], pricing: &Pricing) -> Report {
    let mut findings = Vec::with_capacity(workloads.len());
    let mut total_cost = 0.0;
    let mut total_savings = 0.0;

    for w in workloads {
        let current = monthly_cost(w.cpu_request, w.mem_request, w.replicas, pricing);
        let cpu_rec = rightsize(w.cpu_usage, w.cpu_request, CPU_FLOOR);
        let mem_rec = rightsize(w.mem_usage, w.mem_request, MEM_FLOOR);
        let recommended = monthly_cost(cpu_rec, mem_rec, w.replicas, pricing);
        let savings = (current - recommended).max(0.0);

        let mut flags = Vec::new();
        if cpu_rec < w.cpu_request * OVERPROVISION {
            flags.push("cpu-overprovisioned".to_string());
        }
        if mem_rec < w.mem_request * OVERPROVISION {
            flags.push("mem-overprovisioned".to_string());
        }
        if w.cpu_usage < IDLE_CPU && w.mem_usage < MEM_FLOOR {
            flags.push("idle".to_string());
        }

        total_cost += current;
        total_savings += savings;
        findings.push(Finding {
            namespace: w.namespace.clone(),
            name: w.name.clone(),
            replicas: w.replicas,
            monthly_cost: current,
            recommended_cost: recommended,
            monthly_savings: savings,
            cpu_request: w.cpu_request,
            cpu_recommended: cpu_rec,
            mem_request: w.mem_request,
            mem_recommended: mem_rec,
            flags,
        });
    }

    findings.sort_by(|a, b| {
        b.monthly_savings
            .partial_cmp(&a.monthly_savings)
            .unwrap_or(std::cmp::Ordering::Equal)
    });

    Report {
        currency: pricing.currency.clone(),
        total_monthly_cost: total_cost,
        total_monthly_savings: total_savings,
        findings,
    }
}
