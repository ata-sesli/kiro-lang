# EIR executor baseline

`eir_baseline.rs` measures direct EIR execution after parsing, analysis, lowering,
verification, and runtime construction. It covers dispatch-heavy loops, direct Kiro
calls, collections, and module initialization.

Run the release baseline with:

```sh
CARGO_NET_OFFLINE=true RUSTC_WRAPPER= cargo bench --bench eir_baseline
```

The 2026-08-15 pre-optimization baseline used 3 warm-up iterations, 9 samples, and
10 invocations per sample:

| Workload | Median/op | p95/op | Allocations/op | Requested bytes/op | Peak frames | Peak live slots |
| --- | ---: | ---: | ---: | ---: | ---: | ---: |
| dispatch loop | 540.00 us | 1.07 ms | 1 | 1,456 | 1 | 14 |
| direct calls | 290.98 us | 303.94 us | 2,001 | 728,520 | 2 | 8 |
| collections | 35.90 us | 82.05 us | 8 | 53,768 | 1 | 9 |
| module initialization | 114.06 us | 117.82 us | 2 | 728 | 2 | 7 |

Timing and allocation counting run in separate passes. The reported numbers are
local microbenchmark results, not end-to-end application latency. Override the run
shape with `KIRO_BENCH_WARMUP`, `KIRO_BENCH_SAMPLES`, and
`KIRO_BENCH_ITERATIONS`.

For an external profiler, keep one workload active with:

```sh
CARGO_NET_OFFLINE=true RUSTC_WRAPPER= cargo bench --bench eir_baseline -- \
  --profile direct_calls --seconds 20
```

A five-second macOS `sample` capture placed substantial time in
`EirRuntime::call_function`, `execute_instruction`, `push_frame`, slot writes,
vector construction, and allocator/free routines. Combined with the allocation
count, per-call argument/frame-slot allocation is the first hypothesis for the
future optimization phase. That first capture had allocation counting enabled, so
its CPU weights are directional rather than suitable for before/after comparison;
the final harness disables counting during timing and profile mode. No optimization
is included in this baseline.

## Optimization 1: fused direct-call frame preparation

Direct Kiro calls now copy argument values straight into the callee slot vector
instead of first collecting a temporary argument vector. The same release workload
produced:

| Metric | Before | After | Change |
| --- | ---: | ---: | ---: |
| median/op | 290.98 us | 223.94 us | 23.0% lower |
| p95/op | 303.94 us | 282.75 us | 7.0% lower |
| allocations/op | 2,001 | 1,001 | 50.0% lower |
| requested bytes/op | 728,520 | 312,520 | 57.1% lower |

No frame-slot pooling, indirect-call change, or EIR representation change is part
of this optimization.
