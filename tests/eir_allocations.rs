use std::alloc::{GlobalAlloc, Layout, System};
use std::fs;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

use kiro_lang::analysis::{SourceOverlays, analyze_path_with_info};
use kiro_lang::eir::lower_program;
use kiro_lang::interpreter::eir_runtime::EirRuntime;
use kiro_lang::interpreter::values::RuntimeVal;

struct CountingAllocator;

static COUNT_ALLOCATIONS: AtomicBool = AtomicBool::new(false);
static ALLOCATIONS: AtomicU64 = AtomicU64::new(0);

unsafe impl GlobalAlloc for CountingAllocator {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        record_allocation();
        // SAFETY: The request is forwarded unchanged to the system allocator.
        unsafe { System.alloc(layout) }
    }

    unsafe fn alloc_zeroed(&self, layout: Layout) -> *mut u8 {
        record_allocation();
        // SAFETY: The request is forwarded unchanged to the system allocator.
        unsafe { System.alloc_zeroed(layout) }
    }

    unsafe fn dealloc(&self, pointer: *mut u8, layout: Layout) {
        // SAFETY: The pointer and layout come from the corresponding system allocation.
        unsafe { System.dealloc(pointer, layout) }
    }

    unsafe fn realloc(&self, pointer: *mut u8, layout: Layout, new_size: usize) -> *mut u8 {
        record_allocation();
        // SAFETY: The pointer and layout come from the system allocator, and the new size is
        // forwarded unchanged.
        unsafe { System.realloc(pointer, layout, new_size) }
    }
}

#[global_allocator]
static GLOBAL_ALLOCATOR: CountingAllocator = CountingAllocator;

#[test]
fn direct_calls_allocate_only_the_callee_frame_slots() {
    let path = temp_source(
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
    );
    let analysis = analyze_path_with_info(&path, &SourceOverlays::new())
        .expect("allocation fixture should analyze");
    let function = analysis
        .hir
        .module("main")
        .and_then(|module| module.function("benchmark"))
        .expect("allocation fixture should define benchmark")
        .id;
    let program = lower_program(&analysis.hir).expect("allocation fixture should lower");
    let mut runtime = EirRuntime::new(&program).expect("allocation fixture should verify");

    runtime
        .call_function(function, Vec::new())
        .expect("warm-up execution should succeed");
    ALLOCATIONS.store(0, Ordering::Relaxed);
    COUNT_ALLOCATIONS.store(true, Ordering::Relaxed);
    let value = runtime
        .call_function(function, Vec::new())
        .expect("measured execution should succeed");
    COUNT_ALLOCATIONS.store(false, Ordering::Relaxed);
    let allocations = ALLOCATIONS.load(Ordering::Relaxed);

    assert_eq!(value, RuntimeVal::Float(1000.0));
    assert!(
        allocations <= 1_002,
        "expected one slot allocation per call plus the root frame, observed {allocations}"
    );
}

fn record_allocation() {
    if COUNT_ALLOCATIONS.load(Ordering::Relaxed) {
        ALLOCATIONS.fetch_add(1, Ordering::Relaxed);
    }
}

fn temp_source(source: &str) -> PathBuf {
    let stamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("system time should be after epoch")
        .as_nanos();
    let directory = std::env::temp_dir().join(format!(
        "kiro_eir_allocations_{}_{}",
        std::process::id(),
        stamp
    ));
    fs::create_dir_all(&directory).expect("temporary fixture directory should be created");
    let path = directory.join("main.kiro");
    fs::write(&path, source).expect("allocation fixture should be written");
    path
}
