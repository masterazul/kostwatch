#![forbid(unsafe_code)]

use std::process::ExitCode;

use kostwatch::cost::analyze;
use kostwatch::model::{Pricing, Report, Workload};
use kostwatch::prom::{self, Client, Queries};

const USAGE: &str = "kostwatch - kubernetes cost optimizer

usage:
  kostwatch analyze (--snapshot <file> | --prometheus <url>) [options]
  kostwatch sample > snapshot.json

options:
  --snapshot <file>     read a workloads json snapshot
  --prometheus <url>    pull metrics from a prometheus http endpoint
  --pricing <file>      override the pricing model (json)
  --label <name>        prometheus grouping label (default: pod)
  --json                emit the full report as json

a snapshot is a json array of:
  {namespace, name, replicas, cpu_request, mem_request, cpu_usage, mem_usage}
cpu in cores, memory in bytes (per replica).
";

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let command = args.first().map(String::as_str).unwrap_or("help");

    match command {
        "help" | "-h" | "--help" => {
            print!("{USAGE}");
            ExitCode::SUCCESS
        }
        "version" | "-V" => {
            println!("kostwatch {}", env!("CARGO_PKG_VERSION"));
            ExitCode::SUCCESS
        }
        "sample" => {
            println!("{}", serde_json::to_string_pretty(&sample()).unwrap());
            ExitCode::SUCCESS
        }
        "analyze" => analyze_cmd(&args[1..]),
        other => {
            eprintln!("unknown command: {other}\n{USAGE}");
            ExitCode::FAILURE
        }
    }
}

fn flag<'a>(args: &'a [String], name: &str) -> Option<&'a str> {
    args.iter()
        .position(|a| a == name)
        .and_then(|i| args.get(i + 1))
        .map(String::as_str)
}

fn analyze_cmd(args: &[String]) -> ExitCode {
    let json = args.iter().any(|a| a == "--json");

    let pricing = match flag(args, "--pricing") {
        Some(path) => match read_json::<Pricing>(path) {
            Ok(p) => p,
            Err(e) => return fail(&format!("pricing: {e}")),
        },
        None => Pricing::default(),
    };

    let workloads = if let Some(path) = flag(args, "--snapshot") {
        match read_json::<Vec<Workload>>(path) {
            Ok(w) => w,
            Err(e) => return fail(&format!("snapshot: {e}")),
        }
    } else if let Some(url) = flag(args, "--prometheus") {
        if url
            .trim_start()
            .to_ascii_lowercase()
            .starts_with("https://")
        {
            return fail("prometheus: https endpoints are not supported (built without TLS); use an in-cluster http:// url or a local port-forward");
        }
        let queries = match flag(args, "--label") {
            Some(label) => Queries::with_label(label),
            None => Queries::default(),
        };
        match prom::fetch(&Client::new(url), &queries) {
            Ok(w) => w,
            Err(e) => return fail(&format!("prometheus: {e}")),
        }
    } else {
        return fail("provide --snapshot <file> or --prometheus <url>");
    };

    if workloads.is_empty() {
        return fail("no workloads found");
    }

    let report = analyze(&workloads, &pricing);
    if json {
        println!("{}", serde_json::to_string_pretty(&report).unwrap());
    } else {
        print_report(&report);
    }
    ExitCode::SUCCESS
}

fn read_json<T: serde::de::DeserializeOwned>(path: &str) -> Result<T, String> {
    let raw = std::fs::read(path).map_err(|e| e.to_string())?;
    serde_json::from_slice(&raw).map_err(|e| e.to_string())
}

fn print_report(report: &Report) {
    println!(
        "monthly cost: {:.2} {}    potential savings: {:.2} {} ({:.0}%)",
        report.total_monthly_cost,
        report.currency,
        report.total_monthly_savings,
        report.currency,
        percent(report.total_monthly_savings, report.total_monthly_cost),
    );
    println!(
        "{:<24} {:<16} {:>10} {:>10} {:>10}  flags",
        "namespace", "workload", "cost", "saves", "cpu->"
    );
    for f in &report.findings {
        println!(
            "{:<24} {:<16} {:>10.2} {:>10.2} {:>5.2}->{:<4.2}  {}",
            truncate(&f.namespace, 24),
            truncate(&f.name, 16),
            f.monthly_cost,
            f.monthly_savings,
            f.cpu_request,
            f.cpu_recommended,
            f.flags.join(","),
        );
    }
}

fn percent(part: f64, whole: f64) -> f64 {
    if whole <= 0.0 {
        0.0
    } else {
        part / whole * 100.0
    }
}

fn truncate(s: &str, max: usize) -> String {
    if s.chars().count() <= max {
        s.to_string()
    } else {
        let head: String = s.chars().take(max - 1).collect();
        format!("{head}…")
    }
}

fn fail(message: &str) -> ExitCode {
    eprintln!("error: {message}");
    ExitCode::FAILURE
}

fn sample() -> Vec<Workload> {
    let mib = 1024.0 * 1024.0;
    vec![
        Workload {
            namespace: "payments".into(),
            name: "api".into(),
            replicas: 6,
            cpu_request: 1.0,
            mem_request: 1024.0 * mib,
            cpu_usage: 0.12,
            mem_usage: 240.0 * mib,
        },
        Workload {
            namespace: "search".into(),
            name: "indexer".into(),
            replicas: 3,
            cpu_request: 2.0,
            mem_request: 4096.0 * mib,
            cpu_usage: 1.6,
            mem_usage: 3600.0 * mib,
        },
        Workload {
            namespace: "legacy".into(),
            name: "cron-runner".into(),
            replicas: 2,
            cpu_request: 0.5,
            mem_request: 512.0 * mib,
            cpu_usage: 0.001,
            mem_usage: 8.0 * mib,
        },
    ]
}

#[cfg(test)]
mod tests {
    use super::truncate;

    #[test]
    fn truncate_is_utf8_safe() {
        assert_eq!(truncate("short", 24), "short");
        let long = "coração-de-um-namespace-bem-longo";
        let out = truncate(long, 10);
        assert!(out.chars().count() <= 10);
        assert!(out.ends_with('…'));
    }
}
