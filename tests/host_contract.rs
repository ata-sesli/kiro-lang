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
fn runtime_helpers_expose_immutable_bytes() {
    let value = RuntimeVal::bytes(vec![0, 127, 255]);

    assert_eq!(value.as_bytes().expect("bytes expected"), &[0, 127, 255]);
    assert_eq!(
        RuntimeVal::bytes(vec![1_u8, 2, 3]),
        RuntimeVal::bytes([1, 2, 3])
    );

    let cloned = value.clone();
    assert_eq!(
        cloned.as_bytes().expect("cloned bytes expected"),
        &[0, 127, 255]
    );
    assert!(RuntimeVal::from("not bytes").as_bytes().is_err());
}

#[test]
fn runtime_helpers_expose_named_struct_values() {
    let mut fields = HashMap::new();
    fields.insert("name".to_string(), RuntimeVal::from("users"));
    let value = RuntimeVal::structure("TableInfo", fields);

    assert_eq!(
        value
            .as_struct("TableInfo")
            .expect("TableInfo should decode")
            .get("name"),
        Some(&RuntimeVal::from("users"))
    );
    assert!(value.as_struct("Other").is_err());
}

#[test]
fn runtime_helpers_expose_typed_handles() {
    let handle = RuntimeVal::handle("Model", "model-state".to_string());
    let model = handle
        .as_handle("Model")
        .expect("Model handle should decode");

    assert_eq!(model.type_name(), "Model");
    assert_eq!(
        model
            .downcast_ref::<String>()
            .expect("payload should be a string"),
        "model-state"
    );

    let err = handle
        .as_handle("File")
        .expect_err("wrong handle type should fail");
    assert_eq!(err.name, "TypeError");
    assert!(
        err.to_string().contains("expected handle File"),
        "unexpected error: {}",
        err
    );
}

#[test]
fn host_result_alias_and_abi_version_are_public() {
    fn ok_host(_args: Vec<RuntimeVal>) -> HostResult {
        Ok(RuntimeVal::Void)
    }

    assert_eq!(KIRO_RUNTIME_ABI_VERSION, 4);
    assert!(ok_host(vec![]).is_ok());
}
