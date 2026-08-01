# kostwatch

[![CI](https://github.com/masterazul/kostwatch/actions/workflows/ci.yml/badge.svg)](https://github.com/masterazul/kostwatch/actions/workflows/ci.yml)
[![Security](https://github.com/masterazul/kostwatch/actions/workflows/security.yml/badge.svg)](https://github.com/masterazul/kostwatch/actions/workflows/security.yml)
[![License: MIT](https://img.shields.io/badge/license-MIT-blue.svg)](LICENSE)
![Rust](https://img.shields.io/badge/rust-2021-orange.svg)

A team asks for 2 vCPUs and burns 0.1. Multiply that by the replica count, then by a
month, and the gap stops being a rounding error and becomes a line on the invoice.

kostwatch puts a number on that gap. It reads what Kubernetes was asked to reserve, reads
what the workloads actually use, and reports the difference in money — biggest waste first,
so the list is already sorted by what is worth your afternoon.

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

## Hardening

The pipeline is scoped, not just green.

- Actions run from a commit digest. A tag can be moved; a digest cannot.
- Workflows declare `permissions: contents: read`, so `GITHUB_TOKEN` has nothing to write with.
- `cargo audit --deny warnings` runs on every push and again weekly. Dependabot watches the
  crates and the pinned digests alike.
- `#![forbid(unsafe_code)]` on both crate roots — enforced by the compiler, not by habit.
- Release builds keep overflow checks. Paired with `panic = "abort"`, an overflow stops the
  process instead of wrapping into a wrong answer.
- `gitleaks` reads the full history on every push.

## License

MIT
