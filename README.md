# kostwatch

[![CI](https://github.com/masterazul/kostwatch/actions/workflows/ci.yml/badge.svg)](https://github.com/masterazul/kostwatch/actions/workflows/ci.yml)
[![Security](https://github.com/masterazul/kostwatch/actions/workflows/security.yml/badge.svg)](https://github.com/masterazul/kostwatch/actions/workflows/security.yml)
[![License: MIT](https://img.shields.io/badge/license-MIT-blue.svg)](LICENSE)
![Rust](https://img.shields.io/badge/rust-2021-orange.svg)

Kubernetes cost optimizer. It compares what your workloads **request** against what they
actually **use**, prices the gap, and tells you where the money is going and how much you
could save by rightsizing.

No cluster required to try it — feed it a snapshot:

```
cargo build --release
./target/release/kostwatch sample > snapshot.json
./target/release/kostwatch analyze --snapshot snapshot.json
```

```
monthly cost: 448.95 USD    potential savings: 196.56 USD (44%)
namespace                workload               cost      saves      cpu->  flags
payments                 api                  197.10     164.40  1.00->0.15  cpu-overprovisioned,mem-overprovisioned
legacy                   cron-runner           32.85      32.15  0.50->0.01  cpu-overprovisioned,mem-overprovisioned,idle
search                   indexer              219.00       0.00  2.00->2.00
```

Findings are sorted by savings, so the biggest wins are on top. `search/indexer` is right
sized — usage tracks its request — so kostwatch leaves it alone.

## How rightsizing works

For each workload the recommended request is `usage × 1.25` (a 25% headroom), clamped to a
small floor so nothing is sized to zero, and never raised above the current request — this
is a *savings* tool, not an autoscaler. A workload is flagged:

- `cpu-overprovisioned` / `mem-overprovisioned` when the recommendation is below 80% of the
  request,
- `idle` when it is burning almost no CPU and holding almost no memory.

Savings are the priced difference between the current and recommended requests over a
730-hour month.

## Live, from Prometheus

```
kostwatch analyze --prometheus http://prometheus.monitoring:9090
```

It runs four instant queries (cpu/mem request and usage) and groups by `namespace` plus a
label (default `pod`). Pass `--label workload` and the grouping in the queries follows —
useful with the kube-prometheus recording rules that expose a `workload` label. The queries
are sensible defaults; tune them to your cluster.

The client speaks plain HTTP to stay dependency-free (no TLS crate), so point it at an
in-cluster endpoint or a `kubectl port-forward`; an `https://` URL is rejected up front
rather than failing with an opaque transport error.

## Pricing

Defaults are USD `0.04/vCPU-hour` and `0.005/GiB-hour` over a 730-hour month. Override with
`--pricing pricing.json`:

```json
{ "currency": "USD", "cpu_hour": 0.031, "mem_gb_hour": 0.004 }
```

## Output

Human table by default, or `--json` for the full report (per-workload current cost,
recommended cost, savings and flags) to pipe into a dashboard.

## License

MIT
