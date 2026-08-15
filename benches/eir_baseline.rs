use std::alloc::{GlobalAlloc, Layout, System};
use std::fs;
use std::hint::black_box;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::time::{Duration, Instant};

use kiro_lang::analysis::{SourceOverlays, analyze_path_with_info};
use kiro_lang::eir::{EirProgram, lower_program};
use kiro_lang::hir::FunctionId;
use kiro_lang::interpreter::eir_runtime::EirRuntime;
use kiro_lang::interpreter::values::RuntimeVal;

struct CountingAllocator;

static ALLOCATIONS: AtomicU64 = AtomicU64::new(0);
static ALLOCATED_BYTES: AtomicU64 = AtomicU64::new(0);
static COUNT_ALLOCATIONS: AtomicBool = AtomicBool::new(false);

unsafe impl GlobalAlloc for CountingAllocator {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        record_allocation(layout.size());
        // SAFETY: The request is forwarded unchanged to the system allocator.
        unsafe { System.alloc(layout) }
    }

    unsafe fn alloc_zeroed(&self, layout: Layout) -> *mut u8 {
        record_allocation(layout.size());
        // SAFETY: The request is forwarded unchanged to the system allocator.
        unsafe { System.alloc_zeroed(layout) }
    }

    unsafe fn dealloc(&self, pointer: *mut u8, layout: Layout) {
        // SAFETY: The pointer and layout come from the corresponding system allocation.
        unsafe { System.dealloc(pointer, layout) }
    }

    unsafe fn realloc(&self, pointer: *mut u8, layout: Layout, new_size: usize) -> *mut u8 {
        record_allocation(new_size);
        // SAFETY: The pointer and layout come from the system allocator, and the new size is
        // forwarded unchanged.
        unsafe { System.realloc(pointer, layout, new_size) }
    }
}

#[global_allocator]
static GLOBAL_ALLOCATOR: CountingAllocator = CountingAllocator;

#[derive(Clone, Copy)]
enum Entry {
    Function(FunctionId),
    Initializers,
}

struct Case {
    name: &'static str,
    program: EirProgram,
    entry: Entry,
}

#[derive(Clone, Copy)]
struct AllocationSnapshot {
    count: u64,
    bytes: u64,
}

fn main() {
    let cases = build_cases();
    let args = std::env::args().skip(1).collect::<Vec<_>>();
    if let Some(index) = args.iter().position(|arg| arg == "--profile") {
        let name = args
            .get(index + 1)
            .unwrap_or_else(|| panic!("--profile requires a workload name"));
        let seconds = argument_value(&args, "--seconds").unwrap_or(8);
        let case = cases
            .iter()
            .find(|case| case.name == name)
            .unwrap_or_else(|| panic!("unknown workload '{name}'"));
        run_profile(case, Duration::from_secs(seconds));
        return;
    }

    let samples = environment_value("KIRO_BENCH_SAMPLES").unwrap_or(9);
    let iterations = environment_value("KIRO_BENCH_ITERATIONS").unwrap_or(10);
    let warmup_iterations = environment_value("KIRO_BENCH_WARMUP").unwrap_or(3);
    println!(
        "EIR baseline: release profile, {samples} samples, {iterations} iterations/sample, \
         {warmup_iterations} warm-up iterations"
    );
    println!(
        "workload             median/op       p95/op       range/op      allocs/op       bytes/op   peak frames   peak slots"
    );
    for case in &cases {
        run_benchmark(case, samples, iterations, warmup_iterations);
    }
}

fn run_benchmark(case: &Case, samples: u64, iterations: u64, warmup_iterations: u64) {
    assert!(samples > 0, "sample count must be greater than zero");
    assert!(iterations > 0, "iteration count must be greater than zero");
    assert!(
        warmup_iterations > 0,
        "warm-up iteration count must be greater than zero"
    );

    let mut warmup = EirRuntime::new(&case.program).expect("benchmark EIR should verify");
    let expected = run_once(&mut warmup, case.entry);
    for _ in 1..warmup_iterations {
        assert_eq!(run_once(&mut warmup, case.entry), expected);
    }

    let mut runtime = EirRuntime::new(&case.program).expect("benchmark EIR should verify");
    let mut durations = Vec::with_capacity(samples as usize);
    let mut allocation_count = 0_u64;
    let mut allocated_bytes = 0_u64;
    for _ in 0..samples {
        let started = Instant::now();
        for _ in 0..iterations {
            black_box(run_once(&mut runtime, case.entry));
        }
        let elapsed = started.elapsed();
        reset_allocations();
        COUNT_ALLOCATIONS.store(true, Ordering::Relaxed);
        for _ in 0..iterations {
            black_box(run_once(&mut runtime, case.entry));
        }
        COUNT_ALLOCATIONS.store(false, Ordering::Relaxed);
        let allocations = allocation_snapshot();
        durations.push(elapsed);
        allocation_count += allocations.count;
        allocated_bytes += allocations.bytes;
    }
    durations.sort_unstable();

    let operations = samples * iterations;
    let median = per_operation(durations[durations.len() / 2], iterations);
    let p95_index = ((durations.len() - 1) * 95).div_ceil(100);
    let p95 = per_operation(durations[p95_index], iterations);
    let minimum = per_operation(durations[0], iterations);
    let maximum = per_operation(durations[durations.len() - 1], iterations);
    let stats = runtime.stats();
    println!(
        "{:<18} {:>12} {:>12} {:>6}..{:<6} {:>14.2} {:>14.0} {:>13} {:>12}",
        case.name,
        format_duration(median),
        format_duration(p95),
        format_duration(minimum),
        format_duration(maximum),
        allocation_count as f64 / operations as f64,
        allocated_bytes as f64 / operations as f64,
        stats.peak_frame_depth,
        stats.peak_live_slots,
    );
}

fn run_profile(case: &Case, duration: Duration) {
    let mut runtime = EirRuntime::new(&case.program).expect("benchmark EIR should verify");
    let deadline = Instant::now() + duration;
    let mut iterations = 0_u64;
    while Instant::now() < deadline {
        black_box(run_once(&mut runtime, case.entry));
        iterations += 1;
    }
    let stats = runtime.stats();
    println!(
        "profile workload={} duration={:.3}s iterations={} steps={} peak_frames={} peak_slots={}",
        case.name,
        duration.as_secs_f64(),
        iterations,
        stats.steps_executed,
        stats.peak_frame_depth,
        stats.peak_live_slots,
    );
}

fn run_once(runtime: &mut EirRuntime<'_>, entry: Entry) -> RuntimeVal {
    match entry {
        Entry::Function(function) => runtime
            .call_function(function, Vec::new())
            .expect("benchmark function should execute"),
        Entry::Initializers => {
            runtime
                .run_initializers()
                .expect("benchmark initializers should execute");
            RuntimeVal::Void
        }
    }
}

fn build_cases() -> Vec<Case> {
    vec![
        compile_case(
            "dispatch_loop",
            r#"
fn benchmark() -> num {
    var current = 0
    var total = 0
    loop on (current < 2000) {
        on (current == 1000) {
            total = total + 2
        } off {
            total = total + 1
        }
        current = current + 1
    }
    return total
}
"#,
            Some("benchmark"),
        ),
        compile_case(
            "direct_calls",
            r#"
pure fn increment(value: num) -> num {
    return value + 1
}

fn benchmark() -> num {
    var current = 0
    loop on (current < 1000) {
        current = increment(current)
    }
    return current
}
"#,
            Some("benchmark"),
        ),
        compile_case(
            "collections",
            r#"
fn benchmark() -> num {
    var values = list num {}
    var current = 0
    loop on (current < 256) {
        values push current
        current = current + 1
    }
    return len values
}
"#,
            Some("benchmark"),
        ),
        compile_case(
            "module_init",
            r#"
fn seed() -> num {
    var current = 0
    loop on (current < 1000) {
        current = current + 1
    }
    return current
}

var cached = seed()
"#,
            None,
        ),
    ]
}

fn compile_case(name: &'static str, source: &str, entry: Option<&str>) -> Case {
    let directory = benchmark_directory().join(name);
    fs::create_dir_all(&directory).expect("benchmark directory should be created");
    let path = directory.join("main.kiro");
    fs::write(&path, source).expect("benchmark source should be written");
    let analysis = analyze_path_with_info(&path, &SourceOverlays::new())
        .unwrap_or_else(|error| panic!("benchmark '{name}' should analyze: {error:?}"));
    let function = entry.map(|entry| {
        analysis
            .hir
            .module("main")
            .and_then(|module| module.function(entry))
            .unwrap_or_else(|| panic!("benchmark '{name}' should define '{entry}'"))
            .id
    });
    let program = lower_program(&analysis.hir)
        .unwrap_or_else(|error| panic!("benchmark '{name}' should lower: {error}"));
    Case {
        name,
        program,
        entry: function.map_or(Entry::Initializers, Entry::Function),
    }
}

fn benchmark_directory() -> PathBuf {
    std::env::temp_dir().join(format!("kiro_eir_baseline_{}", std::process::id()))
}

fn reset_allocations() {
    ALLOCATIONS.store(0, Ordering::Relaxed);
    ALLOCATED_BYTES.store(0, Ordering::Relaxed);
}

fn record_allocation(bytes: usize) {
    if COUNT_ALLOCATIONS.load(Ordering::Relaxed) {
        ALLOCATIONS.fetch_add(1, Ordering::Relaxed);
        ALLOCATED_BYTES.fetch_add(bytes as u64, Ordering::Relaxed);
    }
}

fn allocation_snapshot() -> AllocationSnapshot {
    AllocationSnapshot {
        count: ALLOCATIONS.load(Ordering::Relaxed),
        bytes: ALLOCATED_BYTES.load(Ordering::Relaxed),
    }
}

fn per_operation(duration: Duration, iterations: u64) -> Duration {
    Duration::from_nanos((duration.as_nanos() / u128::from(iterations)) as u64)
}

fn format_duration(duration: Duration) -> String {
    let nanos = duration.as_nanos();
    if nanos >= 1_000_000 {
        format!("{:.2}ms", nanos as f64 / 1_000_000.0)
    } else if nanos >= 1_000 {
        format!("{:.2}us", nanos as f64 / 1_000.0)
    } else {
        format!("{nanos}ns")
    }
}

fn environment_value(name: &str) -> Option<u64> {
    std::env::var(name).ok().map(|value| {
        value
            .parse()
            .unwrap_or_else(|_| panic!("{name} must be an unsigned integer"))
    })
}

fn argument_value(args: &[String], name: &str) -> Option<u64> {
    args.iter().position(|arg| arg == name).map(|index| {
        args.get(index + 1)
            .unwrap_or_else(|| panic!("{name} requires a value"))
            .parse()
            .unwrap_or_else(|_| panic!("{name} must be an unsigned integer"))
    })
}
