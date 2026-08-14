mod ids;
mod lower;
mod print;
mod verify;

use crate::hir::{Effects, FunctionId, Signature, SourceAnchor, TypeId, TypeTable};

pub use ids::{BlockId, ConstId, EirIdOverflow, SlotId};
pub use lower::{LowerError, LowerErrorKind, lower_program};
pub use print::print_program;
pub use verify::{VerifyError, VerifyErrorKind, verify_program};

#[derive(Debug, Clone)]
pub struct EirProgram {
    pub types: TypeTable,
    pub constants: Vec<Constant>,
    pub functions: Vec<EirFunction>,
    pub module_initializers: Vec<FunctionId>,
}

impl EirProgram {
    pub fn function(&self, id: FunctionId) -> Option<&EirFunction> {
        let index = usize::try_from(id).ok()?;
        self.functions
            .get(index)
            .filter(|function| function.id == id)
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum Constant {
    Bool(bool),
    Num(f64),
    Str(String),
}

impl Constant {
    pub const fn ty(&self) -> TypeId {
        match self {
            Self::Bool(_) => TypeId::BOOL,
            Self::Num(_) => TypeId::NUM,
            Self::Str(_) => TypeId::STR,
        }
    }
}

#[derive(Debug, Clone)]
pub struct EirFunction {
    pub id: FunctionId,
    pub name: String,
    pub signature: Signature,
    pub slots: Vec<TypeId>,
    pub parameter_count: u32,
    pub blocks: Vec<BasicBlock>,
}

impl EirFunction {
    pub fn effects(&self) -> Effects {
        self.signature.effects()
    }
}

#[derive(Debug, Clone)]
pub struct BasicBlock {
    pub instructions: Vec<Instruction>,
    pub terminator: Terminator,
}

#[derive(Debug, Clone)]
pub struct Instruction {
    pub kind: InstructionKind,
    pub anchor: SourceAnchor,
}

#[derive(Debug, Clone)]
pub enum InstructionKind {
    Const {
        dst: SlotId,
        constant: ConstId,
    },
    Copy {
        dst: SlotId,
        src: SlotId,
    },
    Move {
        dst: SlotId,
        src: SlotId,
    },
    AddNum {
        dst: SlotId,
        lhs: SlotId,
        rhs: SlotId,
    },
    ConcatString {
        dst: SlotId,
        lhs: SlotId,
        rhs: SlotId,
    },
    SubNum {
        dst: SlotId,
        lhs: SlotId,
        rhs: SlotId,
    },
    MulNum {
        dst: SlotId,
        lhs: SlotId,
        rhs: SlotId,
    },
    DivNum {
        dst: SlotId,
        lhs: SlotId,
        rhs: SlotId,
    },
    EqNum {
        dst: SlotId,
        lhs: SlotId,
        rhs: SlotId,
    },
    EqString {
        dst: SlotId,
        lhs: SlotId,
        rhs: SlotId,
    },
    EqBool {
        dst: SlotId,
        lhs: SlotId,
        rhs: SlotId,
    },
    NeNum {
        dst: SlotId,
        lhs: SlotId,
        rhs: SlotId,
    },
    NeString {
        dst: SlotId,
        lhs: SlotId,
        rhs: SlotId,
    },
    NeBool {
        dst: SlotId,
        lhs: SlotId,
        rhs: SlotId,
    },
    GtNum {
        dst: SlotId,
        lhs: SlotId,
        rhs: SlotId,
    },
    LtNum {
        dst: SlotId,
        lhs: SlotId,
        rhs: SlotId,
    },
    GeNum {
        dst: SlotId,
        lhs: SlotId,
        rhs: SlotId,
    },
    LeNum {
        dst: SlotId,
        lhs: SlotId,
        rhs: SlotId,
    },
    CallDirect {
        dst: Option<SlotId>,
        function: FunctionId,
        args: Box<[SlotId]>,
    },
}

impl InstructionKind {
    pub fn destination(&self) -> Option<SlotId> {
        match self {
            Self::Const { dst, .. }
            | Self::Copy { dst, .. }
            | Self::Move { dst, .. }
            | Self::AddNum { dst, .. }
            | Self::ConcatString { dst, .. }
            | Self::SubNum { dst, .. }
            | Self::MulNum { dst, .. }
            | Self::DivNum { dst, .. }
            | Self::EqNum { dst, .. }
            | Self::EqString { dst, .. }
            | Self::EqBool { dst, .. }
            | Self::NeNum { dst, .. }
            | Self::NeString { dst, .. }
            | Self::NeBool { dst, .. }
            | Self::GtNum { dst, .. }
            | Self::LtNum { dst, .. }
            | Self::GeNum { dst, .. }
            | Self::LeNum { dst, .. } => Some(*dst),
            Self::CallDirect { dst, .. } => *dst,
        }
    }

    pub fn read_slots(&self) -> Vec<SlotId> {
        match self {
            Self::Const { .. } => Vec::new(),
            Self::Copy { src, .. } | Self::Move { src, .. } => vec![*src],
            Self::AddNum { lhs, rhs, .. }
            | Self::ConcatString { lhs, rhs, .. }
            | Self::SubNum { lhs, rhs, .. }
            | Self::MulNum { lhs, rhs, .. }
            | Self::DivNum { lhs, rhs, .. }
            | Self::EqNum { lhs, rhs, .. }
            | Self::EqString { lhs, rhs, .. }
            | Self::EqBool { lhs, rhs, .. }
            | Self::NeNum { lhs, rhs, .. }
            | Self::NeString { lhs, rhs, .. }
            | Self::NeBool { lhs, rhs, .. }
            | Self::GtNum { lhs, rhs, .. }
            | Self::LtNum { lhs, rhs, .. }
            | Self::GeNum { lhs, rhs, .. }
            | Self::LeNum { lhs, rhs, .. } => vec![*lhs, *rhs],
            Self::CallDirect { args, .. } => args.to_vec(),
        }
    }
}

#[derive(Debug, Clone)]
pub struct Terminator {
    pub kind: TerminatorKind,
    pub anchor: SourceAnchor,
}

#[derive(Debug, Clone)]
pub enum TerminatorKind {
    Jump(BlockId),
    Branch {
        condition: SlotId,
        then_block: BlockId,
        else_block: BlockId,
    },
    Return(Option<SlotId>),
    Throw(SlotId),
    Unreachable,
}
