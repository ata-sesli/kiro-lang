use std::fs;
use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

use kiro_lang::analysis::{SourceOverlays, analyze_path_with_info};
use kiro_lang::hir::{
    Effects, FunctionId, HirBinaryOp, HirCallKind, HirExprKind, HirStmtKind, SemType, Signature,
    SourceAnchor, SourceId, TypeId, TypeTable,
};

fn temp_project(name: &str, source: &str) -> PathBuf {
    let stamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("system time should be after epoch")
        .as_nanos();
    let dir =
        std::env::temp_dir().join(format!("kiro_hir_{name}_{}_{}", std::process::id(), stamp));
    fs::create_dir_all(&dir).expect("temporary project should be created");
    let path = dir.join("main.kiro");
    fs::write(&path, source).expect("test source should be written");
    path
}

fn analyze_main(source: &str) -> kiro_lang::analysis::AnalysisResult {
    let path = temp_project("analysis", source);
    analyze_path_with_info(path, &SourceOverlays::new()).expect("source should analyze")
}

#[test]
fn semantic_ids_are_dense_checked_and_deterministic() {
    let id = FunctionId::new(7);

    assert_eq!(id.raw(), 7);
    assert_eq!(usize::try_from(id).expect("u32 ID should fit usize"), 7);
    assert_eq!(format!("{id:?}"), "FunctionId(7)");
    assert!(FunctionId::try_from(usize::MAX).is_err());
}

#[test]
fn primitive_type_ids_are_fixed() {
    let types = TypeTable::new();

    assert_eq!(types.get(TypeId::UNKNOWN), Some(&SemType::Unknown));
    assert_eq!(types.get(TypeId::VOID), Some(&SemType::Void));
    assert_eq!(types.get(TypeId::BOOL), Some(&SemType::Bool));
    assert_eq!(types.get(TypeId::NUM), Some(&SemType::Num));
    assert_eq!(types.get(TypeId::STR), Some(&SemType::Str));
    assert_eq!(types.get(TypeId::BYTES), Some(&SemType::Bytes));
    assert_eq!(types.len(), 6);
}

#[test]
fn composite_types_are_interned_once() {
    let mut types = TypeTable::new();
    let list = SemType::List(TypeId::NUM);
    let first = types.intern(list.clone());
    let second = types.intern(list);
    let map = types.intern(SemType::Map(TypeId::STR, first));

    assert_eq!(first, second);
    assert_eq!(types.get(first), Some(&SemType::List(TypeId::NUM)));
    assert_eq!(types.get(map), Some(&SemType::Map(TypeId::STR, first)));
    assert_eq!(types.len(), 8);
    assert_eq!(
        format!("{types:?}"),
        "TypeTable([Unknown, Void, Bool, Num, Str, Bytes, List(TypeId(3)), Map(TypeId(4), TypeId(6))])"
    );
}

#[test]
fn signatures_use_canonical_types_and_composable_effects() {
    let effects = Effects::PURE | Effects::MAY_FAIL;
    let signature = Signature::new([TypeId::NUM, TypeId::STR], TypeId::BOOL, effects);

    assert_eq!(signature.params(), &[TypeId::NUM, TypeId::STR]);
    assert_eq!(signature.return_type(), TypeId::BOOL);
    assert!(signature.effects().contains(Effects::PURE));
    assert!(signature.effects().contains(Effects::MAY_FAIL));
    assert!(!signature.effects().contains(Effects::MAY_BLOCK));
    assert_eq!(format!("{effects:?}"), "Effects(PURE | MAY_FAIL)");
}

#[test]
fn source_anchors_preserve_source_and_offsets() {
    let anchor = SourceAnchor::try_from_offsets(SourceId::new(3), 12, 29)
        .expect("valid offsets should produce an anchor");

    assert_eq!(anchor.source(), SourceId::new(3));
    assert_eq!(anchor.start(), 12);
    assert_eq!(anchor.end(), 29);
    assert_eq!(anchor.range(), 12..29);
    assert!(SourceAnchor::try_from_offsets(SourceId::new(3), 29, 12).is_err());
    assert!(SourceAnchor::try_from_offsets(SourceId::new(3), 0, usize::MAX).is_err());
}

#[test]
fn analysis_produces_typed_hir_with_resolved_locals_and_operators() {
    let analysis = analyze_main(
        r#"
pure fn add(a: num, b: num) -> num {
    var sum = a + b
    return sum
}
"#,
    );
    let module = analysis.modules.get("main").expect("main module");
    let function = module.hir.function("add").expect("add function");

    assert_eq!(function.id, FunctionId::new(0));
    assert_eq!(function.signature.return_type(), TypeId::NUM);
    assert!(function.signature.effects().contains(Effects::PURE));

    let HirStmtKind::VarDecl { local, value } = &function.body[0].kind else {
        panic!("first statement should declare sum")
    };
    assert_eq!(value.ty, TypeId::NUM);
    assert!(matches!(
        value.kind,
        HirExprKind::Binary {
            op: HirBinaryOp::AddNum,
            ..
        }
    ));

    let HirStmtKind::Return(Some(returned)) = &function.body[1].kind else {
        panic!("second statement should return sum")
    };
    assert!(matches!(returned.kind, HirExprKind::Local(id) if id == *local));
}

#[test]
fn lexical_shadowing_assigns_distinct_local_ids() {
    let analysis = analyze_main(
        r#"
fn choose() -> num {
    var value = 1
    on (true) {
        var value = 2
        return value
    }
    return value
}
"#,
    );
    let function = analysis.modules["main"]
        .hir
        .function("choose")
        .expect("choose function");

    let HirStmtKind::VarDecl { local: outer, .. } = function.body[0].kind else {
        panic!("outer value declaration")
    };
    let HirStmtKind::On { body, .. } = &function.body[1].kind else {
        panic!("conditional body")
    };
    let HirStmtKind::VarDecl { local: inner, .. } = body[0].kind else {
        panic!("inner value declaration")
    };

    assert_ne!(outer, inner);
}

#[test]
fn analysis_classifies_direct_and_host_calls() {
    let path = temp_project(
        "calls",
        r#"
rust fn host_value() -> num

pure fn local_value() -> num {
    return 7
}

fn main() -> num {
    var left = local_value()
    return left + host_value()
}
"#,
    );
    fs::write(path.with_extension("rs"), "// test host glue\n")
        .expect("host glue marker should be written");
    let analysis =
        analyze_path_with_info(path, &SourceOverlays::new()).expect("source should analyze");
    let function = analysis.modules["main"]
        .hir
        .function("main")
        .expect("main function");

    let HirStmtKind::VarDecl { value: direct, .. } = &function.body[0].kind else {
        panic!("direct call declaration")
    };
    assert!(matches!(
        direct.kind,
        HirExprKind::Call {
            kind: HirCallKind::Direct(_),
            ..
        }
    ));

    let HirStmtKind::Return(Some(sum)) = &function.body[1].kind else {
        panic!("return expression")
    };
    let HirExprKind::Binary { rhs, .. } = &sum.kind else {
        panic!("return should contain addition")
    };
    assert!(matches!(
        rhs.kind,
        HirExprKind::Call {
            kind: HirCallKind::Host(_),
            ..
        }
    ));
}

#[test]
fn imported_calls_resolve_to_the_imported_function_id() {
    let main = temp_project(
        "imports",
        r#"
import math

fn main() -> num {
    return math.add(2, 3)
}
"#,
    );
    fs::write(
        main.parent().expect("main parent").join("math.kiro"),
        "pure fn add(a: num, b: num) -> num { return a + b }\n",
    )
    .expect("math module should be written");
    let analysis =
        analyze_path_with_info(main, &SourceOverlays::new()).expect("project should analyze");
    let imported_id = analysis.modules["math"]
        .hir
        .function("add")
        .expect("math.add")
        .id;
    let main = analysis.modules["main"]
        .hir
        .function("main")
        .expect("main function");
    let HirStmtKind::Return(Some(call)) = &main.body[0].kind else {
        panic!("main should return imported call")
    };

    assert!(matches!(
        call.kind,
        HirExprKind::Call {
            kind: HirCallKind::Direct(id),
            ..
        } if id == imported_id
    ));
}

#[test]
fn moves_and_runtime_effects_are_explicit_in_hir() {
    let analysis = analyze_main(
        r#"
fn work() -> num {
    var value = 1
    var moved = move value
    var channel = pipe num
    give channel moved
    check true
    return take channel
}
"#,
    );
    let function = analysis.modules["main"]
        .hir
        .function("work")
        .expect("work function");
    let HirStmtKind::VarDecl {
        local: source_local,
        ..
    } = function.body[0].kind
    else {
        panic!("value declaration")
    };
    let HirStmtKind::VarDecl { value: moved, .. } = &function.body[1].kind else {
        panic!("moved declaration")
    };

    assert!(matches!(moved.kind, HirExprKind::Move(id) if id == source_local));
    assert!(function.effects().contains(Effects::MAY_BLOCK));
    assert!(function.effects().contains(Effects::MAY_FAIL));
}
