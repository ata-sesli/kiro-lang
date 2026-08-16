#[path = "../src/kiro_std/bytes/header.rs"]
mod bytes_std;

use kiro_lang::{StdAssets, canonical_std_module_name, std_asset_path};
use kiro_runtime::RuntimeVal;
use std::future::Future;
use std::task::{Context, Poll, Waker};

#[test]
fn std_bytes_converts_slices_and_concatenates() {
    let empty = run(bytes_std::empty(Vec::new())).expect("empty bytes should succeed");
    assert!(empty.as_bytes().expect("bytes expected").is_empty());

    let hello = run(bytes_std::from_str(vec![RuntimeVal::from("hello")]))
        .expect("UTF-8 encoding should succeed");
    assert_eq!(hello.as_bytes().expect("bytes expected"), b"hello");

    let hex = run(bytes_std::to_hex(vec![hello.clone()])).expect("hex encoding should succeed");
    assert_eq!(hex.as_str().expect("string expected"), "68656c6c6f");

    let decoded = run(bytes_std::from_hex(vec![hex])).expect("hex decoding should succeed");
    let slice = run(bytes_std::slice(vec![
        decoded,
        RuntimeVal::from(1.0),
        RuntimeVal::from(4.0),
    ]))
    .expect("valid slice should succeed");
    let joined = run(bytes_std::concat(vec![
        RuntimeVal::bytes(b"h".as_slice()),
        slice,
    ]))
    .expect("concatenation should succeed");
    let text = run(bytes_std::to_str(vec![joined])).expect("valid UTF-8 should decode");
    assert_eq!(text.as_str().expect("string expected"), "hell");
}

#[test]
fn std_bytes_rejects_invalid_inputs() {
    let invalid_hex = run(bytes_std::from_hex(vec![RuntimeVal::from("abc")]))
        .expect_err("odd-length hex must fail");
    assert_eq!(invalid_hex.name, "InvalidHex");

    let invalid_utf8 = run(bytes_std::to_str(vec![RuntimeVal::bytes([0xff])]))
        .expect_err("invalid UTF-8 must fail");
    assert_eq!(invalid_utf8.name, "InvalidUtf8");

    let invalid_slice = run(bytes_std::slice(vec![
        RuntimeVal::bytes([1, 2]),
        RuntimeVal::from(0.5),
        RuntimeVal::from(1.0),
    ]))
    .expect_err("fractional indexes must fail");
    assert_eq!(invalid_slice.name, "InvalidRange");
}

fn run<T>(future: impl Future<Output = T>) -> T {
    let mut future = std::pin::pin!(future);
    let waker = Waker::noop();
    let mut context = Context::from_waker(waker);
    match future.as_mut().poll(&mut context) {
        Poll::Ready(value) => value,
        Poll::Pending => panic!("standard bytes operation unexpectedly suspended"),
    }
}

#[test]
fn std_bytes_assets_are_registered() {
    assert_eq!(canonical_std_module_name("bytes"), Some("std_bytes"));
    assert_eq!(
        std_asset_path("bytes", "std_bytes.kiro").as_deref(),
        Some("bytes/std_bytes.kiro")
    );
    assert!(StdAssets::get("bytes/header.rs").is_some());
    assert!(StdAssets::get("bytes/std_bytes.kiro").is_some());
}
