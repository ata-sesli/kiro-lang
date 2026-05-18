use std::collections::HashMap;

use kiro_runtime::{HostResult, KIRO_RUNTIME_ABI_VERSION, KiroError, RuntimeVal};

#[test]
fn host_error_can_carry_message_while_preserving_name() {
    let err = KiroError::message("IoError", "failed to read config.txt");

    assert_eq!(err.name, "IoError");
    assert_eq!(err.message.as_deref(), Some("failed to read config.txt"));
    assert_eq!(err.to_string(), "IoError: failed to read config.txt");
}

#[test]
fn runtime_helpers_validate_arity_and_arguments() {
    let args = vec![RuntimeVal::from("model.onnx"), RuntimeVal::from(3.0)];

    RuntimeVal::expect_arity(&args, 2, "load").expect("arity should match");
    assert_eq!(
        RuntimeVal::expect_arg(&args, 0, "load")
            .expect("arg should exist")
            .as_str()
            .expect("arg should be string"),
        "model.onnx"
    );
    assert_eq!(
        RuntimeVal::expect_arg(&args, 1, "load")
            .expect("arg should exist")
            .as_num()
            .expect("arg should be num"),
        3.0
    );

    let arity_err = RuntimeVal::expect_arity(&args, 1, "load").expect_err("arity should fail");
    assert_eq!(arity_err.name, "ArgumentError");
    assert!(
        arity_err.to_string().contains("expected 1 argument"),
        "unexpected error: {}",
        arity_err
    );

    let missing_err = RuntimeVal::expect_arg(&args, 3, "load").expect_err("arg should be missing");
    assert_eq!(missing_err.name, "ArgumentError");
}

#[test]
fn runtime_helpers_expose_list_map_and_void_shapes() {
    let list = RuntimeVal::List(vec![RuntimeVal::from("a")]);
    assert_eq!(list.as_list().expect("list expected").len(), 1);

    let mut map = HashMap::new();
    map.insert("answer".to_string(), RuntimeVal::from(42.0));
    let map = RuntimeVal::Map(map);
    assert!(map.as_map().expect("map expected").contains_key("answer"));

    RuntimeVal::Void.as_void().expect("void expected");
}

#[test]
fn host_result_alias_and_abi_version_are_public() {
    fn ok_host(_args: Vec<RuntimeVal>) -> HostResult {
        Ok(RuntimeVal::Void)
    }

    assert_eq!(KIRO_RUNTIME_ABI_VERSION, 1);
    assert!(ok_host(vec![]).is_ok());
}
