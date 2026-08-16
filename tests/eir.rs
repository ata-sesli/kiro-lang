use std::fs;
use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

use kiro_lang::analysis::{SourceOverlays, analyze_path_with_info};
use kiro_lang::eir::{
    BasicBlock, BlockId, EirFunction, EirProgram, EirStruct, EirStructField, Instruction,
    InstructionKind, SlotId, Terminator, TerminatorKind, VerifyErrorKind, lower_program,
    print_program, verify_program,
};

fn analyze_main(source: &str) -> kiro_lang::analysis::AnalysisResult {
    let stamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("system time should be after epoch")
        .as_nanos();
    let dir = std::env::temp_dir().join(format!("kiro_eir_{}_{}", std::process::id(), stamp));
    fs::create_dir_all(&dir).expect("temporary project should be created");
    let path: PathBuf = dir.join("main.kiro");
    fs::write(&path, source).expect("test source should be written");
    analyze_path_with_info(path, &SourceOverlays::new()).expect("source should analyze")
}
use kiro_lang::hir::{
    Effects, FieldId, FunctionId, SemType, Signature, SourceAnchor, SourceId, StructId, TypeId,
    TypeTable,
};

fn anchor(start: usize, end: usize) -> SourceAnchor {
    SourceAnchor::try_from_offsets(SourceId::new(0), start, end).expect("valid test anchor")
}

fn function(
    id: u32,
    name: &str,
    signature: Signature,
    slots: Vec<TypeId>,
    parameter_count: u32,
    blocks: Vec<BasicBlock>,
) -> EirFunction {
    EirFunction {
        id: FunctionId::new(id),
        name: name.to_string(),
        signature,
        slots,
        parameter_count,
        blocks,
    }
}

#[test]
fn verifier_accepts_typed_straight_line_function_and_printer_is_deterministic() {
    let program = EirProgram {
        types: TypeTable::new(),
        errors: Vec::new(),
        structs: Vec::new(),
        globals: Vec::new(),
        host_functions: Vec::new(),
        constants: Vec::new(),
        functions: vec![function(
            0,
            "add",
            Signature::new([TypeId::NUM, TypeId::NUM], TypeId::NUM, Effects::PURE),
            vec![TypeId::NUM, TypeId::NUM, TypeId::NUM],
            2,
            vec![BasicBlock {
                instructions: vec![Instruction {
                    kind: InstructionKind::AddNum {
                        dst: SlotId::new(2),
                        lhs: SlotId::new(0),
                        rhs: SlotId::new(1),
                    },
                    anchor: anchor(10, 15),
                }],
                terminator: Terminator {
                    kind: TerminatorKind::Return(Some(SlotId::new(2))),
                    anchor: anchor(16, 24),
                },
            }],
        )],
        module_initializers: Vec::new(),
    };

    verify_program(&program).expect("well-typed EIR should verify");
    assert_eq!(
        print_program(&program),
        r#"types:
  t0 = unknown
  t1 = void
  t2 = bool
  t3 = num
  t4 = str
  t5 = bytes
constants:
functions:
fn f0 add(t3, t3) -> t3 effects=PURE {
  slots: s0:t3, s1:t3, s2:t3
  b0:
    s2 = add_num s0, s1 @0:10..15
    return s2 @0:16..24
}
module_initializers:
"#
    );
}

#[test]
fn verifier_rejects_struct_types_without_metadata() {
    let mut types = TypeTable::new();
    types.intern(SemType::Struct(StructId::new(0)));
    let program = EirProgram {
        types,
        errors: Vec::new(),
        structs: Vec::new(),
        globals: Vec::new(),
        host_functions: Vec::new(),
        constants: Vec::new(),
        functions: Vec::new(),
        module_initializers: Vec::new(),
    };

    let errors = verify_program(&program).expect_err("missing struct metadata must be rejected");
    assert!(matches!(
        errors[0].kind,
        VerifyErrorKind::InvalidStruct(id) if id == StructId::new(0)
    ));
}

#[test]
fn verifier_rejects_misordered_struct_and_field_metadata() {
    let mut types = TypeTable::new();
    types.intern(SemType::Struct(StructId::new(0)));
    let program = EirProgram {
        types,
        errors: Vec::new(),
        structs: vec![EirStruct {
            id: StructId::new(1),
            name: "Broken".to_string(),
            fields: vec![EirStructField {
                id: FieldId::new(1),
                name: "value".to_string(),
                ty: TypeId::new(99),
            }],
        }],
        globals: Vec::new(),
        host_functions: Vec::new(),
        constants: Vec::new(),
        functions: Vec::new(),
        module_initializers: Vec::new(),
    };

    let errors = verify_program(&program).expect_err("malformed struct metadata must be rejected");
    assert!(errors.iter().any(|error| matches!(
        error.kind,
        VerifyErrorKind::StructOrder { expected, actual }
            if expected == StructId::new(0) && actual == StructId::new(1)
    )));
    assert!(errors.iter().any(|error| matches!(
        error.kind,
        VerifyErrorKind::FieldOrder { expected, actual }
            if expected == FieldId::new(0) && actual == FieldId::new(1)
    )));
    assert!(errors.iter().any(|error| matches!(
        error.kind,
        VerifyErrorKind::InvalidFieldType { field, ty }
            if field == FieldId::new(1) && ty == TypeId::new(99)
    )));
}

#[test]
fn verifier_reports_typed_instruction_location_and_anchor() {
    let program = EirProgram {
        types: TypeTable::new(),
        errors: Vec::new(),
        structs: Vec::new(),
        globals: Vec::new(),
        host_functions: Vec::new(),
        constants: Vec::new(),
        functions: vec![function(
            0,
            "bad_add",
            Signature::new([TypeId::NUM, TypeId::BOOL], TypeId::NUM, Effects::NONE),
            vec![TypeId::NUM, TypeId::BOOL, TypeId::NUM],
            2,
            vec![BasicBlock {
                instructions: vec![Instruction {
                    kind: InstructionKind::AddNum {
                        dst: SlotId::new(2),
                        lhs: SlotId::new(0),
                        rhs: SlotId::new(1),
                    },
                    anchor: anchor(30, 35),
                }],
                terminator: Terminator {
                    kind: TerminatorKind::Return(Some(SlotId::new(2))),
                    anchor: anchor(36, 44),
                },
            }],
        )],
        module_initializers: Vec::new(),
    };

    let errors = verify_program(&program).expect_err("bool operand must be rejected");
    assert!(matches!(
        errors[0].kind,
        VerifyErrorKind::SlotType {
            slot,
            expected: TypeId::NUM,
            actual: TypeId::BOOL,
        } if slot == SlotId::new(1)
    ));
    assert_eq!(errors[0].function, FunctionId::new(0));
    assert_eq!(errors[0].block, BlockId::new(0));
    assert_eq!(errors[0].instruction, Some(0));
    assert_eq!(errors[0].anchor, anchor(30, 35));
}

#[test]
fn verifier_rejects_uninitialized_reads_and_invalid_branch_targets() {
    let program = EirProgram {
        types: TypeTable::new(),
        errors: Vec::new(),
        structs: Vec::new(),
        globals: Vec::new(),
        host_functions: Vec::new(),
        constants: Vec::new(),
        functions: vec![function(
            0,
            "broken_flow",
            Signature::new([], TypeId::VOID, Effects::NONE),
            vec![TypeId::BOOL, TypeId::NUM],
            0,
            vec![BasicBlock {
                instructions: vec![Instruction {
                    kind: InstructionKind::Copy {
                        dst: SlotId::new(1),
                        src: SlotId::new(1),
                    },
                    anchor: anchor(1, 2),
                }],
                terminator: Terminator {
                    kind: TerminatorKind::Branch {
                        condition: SlotId::new(0),
                        then_block: BlockId::new(1),
                        else_block: BlockId::new(7),
                    },
                    anchor: anchor(3, 4),
                },
            }],
        )],
        module_initializers: Vec::new(),
    };

    let errors = verify_program(&program).expect_err("invalid flow must be rejected");
    assert!(
        errors
            .iter()
            .any(|error| { error.kind == VerifyErrorKind::UninitializedRead(SlotId::new(1)) })
    );
    assert!(
        errors
            .iter()
            .any(|error| error.kind == VerifyErrorKind::InvalidBlock(BlockId::new(7)))
    );
}

#[test]
fn verifier_rejects_impure_direct_call_from_pure_function() {
    let callee = function(
        0,
        "impure",
        Signature::new([], TypeId::VOID, Effects::NONE),
        Vec::new(),
        0,
        vec![BasicBlock {
            instructions: Vec::new(),
            terminator: Terminator {
                kind: TerminatorKind::Return(None),
                anchor: anchor(0, 1),
            },
        }],
    );
    let caller = function(
        1,
        "pure_caller",
        Signature::new([], TypeId::VOID, Effects::PURE),
        Vec::new(),
        0,
        vec![BasicBlock {
            instructions: vec![Instruction {
                kind: InstructionKind::CallDirect {
                    dst: None,
                    function: FunctionId::new(0),
                    args: Box::new([]),
                },
                anchor: anchor(5, 14),
            }],
            terminator: Terminator {
                kind: TerminatorKind::Return(None),
                anchor: anchor(15, 21),
            },
        }],
    );
    let program = EirProgram {
        types: TypeTable::new(),
        errors: Vec::new(),
        structs: Vec::new(),
        globals: Vec::new(),
        host_functions: Vec::new(),
        constants: Vec::new(),
        functions: vec![callee, caller],
        module_initializers: Vec::new(),
    };

    let errors = verify_program(&program).expect_err("pure call boundary must be verified");
    assert!(errors.iter().any(|error| matches!(
        error.kind,
        VerifyErrorKind::EffectViolation { callee } if callee == FunctionId::new(0)
    )));
}

#[test]
fn verifier_rejects_reading_a_slot_after_it_is_moved() {
    let program = EirProgram {
        types: TypeTable::new(),
        errors: Vec::new(),
        structs: Vec::new(),
        globals: Vec::new(),
        host_functions: Vec::new(),
        constants: Vec::new(),
        functions: vec![function(
            0,
            "use_after_move",
            Signature::new([TypeId::NUM], TypeId::NUM, Effects::NONE),
            vec![TypeId::NUM, TypeId::NUM],
            1,
            vec![BasicBlock {
                instructions: vec![Instruction {
                    kind: InstructionKind::Move {
                        dst: SlotId::new(1),
                        src: SlotId::new(0),
                    },
                    anchor: anchor(1, 7),
                }],
                terminator: Terminator {
                    kind: TerminatorKind::Return(Some(SlotId::new(0))),
                    anchor: anchor(8, 16),
                },
            }],
        )],
        module_initializers: Vec::new(),
    };

    let errors = verify_program(&program).expect_err("a moved slot must become uninitialized");
    assert!(errors.iter().any(|error| {
        error.kind == VerifyErrorKind::UninitializedRead(SlotId::new(0))
            && error.instruction.is_none()
    }));
}

#[test]
fn verifier_rejects_direct_calls_with_unreported_effects() {
    let callee = function(
        0,
        "fallible",
        Signature::new([], TypeId::VOID, Effects::MAY_FAIL),
        Vec::new(),
        0,
        vec![BasicBlock {
            instructions: Vec::new(),
            terminator: Terminator {
                kind: TerminatorKind::Return(None),
                anchor: anchor(0, 1),
            },
        }],
    );
    let caller = function(
        1,
        "missing_effect",
        Signature::new([], TypeId::VOID, Effects::NONE),
        Vec::new(),
        0,
        vec![BasicBlock {
            instructions: vec![Instruction {
                kind: InstructionKind::CallDirect {
                    dst: None,
                    function: FunctionId::new(0),
                    args: Box::new([]),
                },
                anchor: anchor(2, 10),
            }],
            terminator: Terminator {
                kind: TerminatorKind::Return(None),
                anchor: anchor(11, 17),
            },
        }],
    );
    let program = EirProgram {
        types: TypeTable::new(),
        errors: Vec::new(),
        structs: Vec::new(),
        globals: Vec::new(),
        host_functions: Vec::new(),
        constants: Vec::new(),
        functions: vec![callee, caller],
        module_initializers: Vec::new(),
    };

    let errors = verify_program(&program).expect_err("callee effects must be declared by caller");
    assert!(errors.iter().any(|error| matches!(
        error.kind,
        VerifyErrorKind::EffectViolation { callee } if callee == FunctionId::new(0)
    )));
}

#[test]
fn verifier_rejects_invalid_signature_return_type() {
    let program = EirProgram {
        types: TypeTable::new(),
        errors: Vec::new(),
        structs: Vec::new(),
        globals: Vec::new(),
        host_functions: Vec::new(),
        constants: Vec::new(),
        functions: vec![function(
            0,
            "invalid_return_type",
            Signature::new([], TypeId::new(99), Effects::NONE),
            Vec::new(),
            0,
            vec![BasicBlock {
                instructions: Vec::new(),
                terminator: Terminator {
                    kind: TerminatorKind::Unreachable,
                    anchor: anchor(0, 1),
                },
            }],
        )],
        module_initializers: Vec::new(),
    };

    let errors = verify_program(&program).expect_err("signature types must exist in the table");
    assert!(
        errors
            .iter()
            .any(|error| error.kind == VerifyErrorKind::InvalidType(TypeId::new(99)))
    );
}

#[test]
fn lowering_emits_direct_calls_and_a_synthetic_module_initializer() {
    let analysis = analyze_main(
        r#"
var seed = 2

pure fn double(value: num) -> num {
    return value + value
}

fn main() -> num {
    return double(3)
}
"#,
    );

    let program = lower_program(&analysis.hir).expect("supported HIR should lower");
    verify_program(&program).expect("lowered EIR should verify");

    assert_eq!(program.functions.len(), 3);
    assert_eq!(program.module_initializers, vec![FunctionId::new(2)]);
    assert_eq!(program.functions[2].name, "main::$init");
    assert!(
        program.functions[1].blocks[0]
            .instructions
            .iter()
            .any(|instruction| matches!(
                instruction.kind,
                InstructionKind::CallDirect {
                    function,
                    ..
                } if function == FunctionId::new(0)
            ))
    );
    let initializer_effects = program.functions[2].signature.effects();
    for effect in [
        kiro_lang::hir::Effects::MAY_FAIL,
        kiro_lang::hir::Effects::MAY_BLOCK,
        kiro_lang::hir::Effects::MAY_SPAWN,
        kiro_lang::hir::Effects::HOST_CALL,
        kiro_lang::hir::Effects::INDIRECT_CALL,
    ] {
        assert!(initializer_effects.contains(effect));
    }
}

#[test]
fn lowering_builds_explicit_control_flow_for_conditions_and_while_loops() {
    let analysis = analyze_main(
        r#"
fn count(limit: num) -> num {
    var current = 0
    loop on (current < limit) {
        current = current + 1
        on (current == 2) {
            continue
        }
        on (current == 4) {
            break
        }
    }
    return current
}
"#,
    );

    let program = lower_program(&analysis.hir).expect("supported control flow should lower");
    verify_program(&program).expect("control-flow EIR should verify");
    let function = &program.functions[0];

    assert!(function.blocks.len() >= 8);
    assert!(
        function
            .blocks
            .iter()
            .any(|block| matches!(block.terminator.kind, TerminatorKind::Branch { .. }))
    );
    assert!(function.blocks.iter().any(|block| matches!(
        block.terminator.kind,
        TerminatorKind::Jump(target) if target == BlockId::new(1)
    )));
}

#[test]
fn lowering_accepts_non_void_function_when_all_conditional_branches_return() {
    let analysis = analyze_main(
        r#"
fn choose(flag: bool) -> num {
    on (flag) {
        return 1
    } off {
        return 2
    }
}
"#,
    );

    let program = lower_program(&analysis.hir).expect("fully returning branches should lower");
    verify_program(&program).expect("lowered EIR should verify");
    assert_eq!(
        program.functions[0]
            .blocks
            .iter()
            .filter(|block| matches!(block.terminator.kind, TerminatorKind::Return(Some(_))))
            .count(),
        2
    );
}

#[test]
fn lowering_emits_explicit_aggregate_instructions() {
    let analysis = analyze_main(
        r#"
fn make() -> list num {
    return list num { 1, 2 }
}
"#,
    );

    let program = lower_program(&analysis.hir).expect("list construction should lower explicitly");
    verify_program(&program).expect("aggregate EIR should verify");
    assert!(
        program.functions[0].blocks[0]
            .instructions
            .iter()
            .any(|instruction| matches!(instruction.kind, InstructionKind::MakeList { .. }))
    );
}
