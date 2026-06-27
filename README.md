# kostwatch

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
monthly cost: 1234.56 USD    potential savings: 612.30 USD (50%)
namespace                workload               cost      saves      cpu->  flags
legacy                   cron-runner          ...        ...     0.50->0.01  cpu-overprovisioned,idle
...
```

## Live, from Prometheus

```
kostwatch analyze --prometheus http://prometheus.monitoring:9090
```

It runs four instant queries (cpu/mem request and usage) and groups by `namespace` plus a
label (default `pod`; pass `--label workload` if you have the kube-prometheus recording
rules). The queries are sensible defaults — tune them to your cluster.

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
