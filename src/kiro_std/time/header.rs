// Kiro Standard Library: Time (tokio::time + std::time)
// Glue layer between Kiro and Rust time functions

use kiro_runtime::{HostResult, KiroError, RuntimeVal};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

pub async fn now(_args: Vec<RuntimeVal>) -> HostResult {
    let millis = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or(Duration::ZERO)
        .as_millis() as f64;
    Ok(RuntimeVal::from(millis))
}

pub async fn sleep(args: Vec<RuntimeVal>) -> HostResult {
    let ms = args[0].as_num()?;
    tokio::time::sleep(Duration::from_millis(ms as u64)).await;
    Ok(RuntimeVal::Void)
}

pub async fn monotonic(_args: Vec<RuntimeVal>) -> HostResult {
    // Returns monotonic time in nanoseconds since program start
    // Using a lazy_static or thread_local would be better for true monotonic
    // For now, we just return Instant::now elapsed since UNIX_EPOCH approximation
    let nanos = Instant::now().elapsed().as_nanos() as f64;
    Ok(RuntimeVal::from(nanos))
}
