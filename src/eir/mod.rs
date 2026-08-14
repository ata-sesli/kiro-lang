mod ids;
mod lower;
mod print;
mod verify;

use crate::hir::{
    Effects, ErrorId, FieldId, FunctionId, HostFunctionId, Signature, SourceAnchor, StructId,
    TypeId, TypeTable,
};

pub use ids::{BlockId, ConstId, EirIdOverflow, GlobalId, SlotId};
pub use lower::{LowerError, LowerErrorKind, lower_program};
pub use print::print_program;
pub use verify::{VerifyError, VerifyErrorKind, verify_program};

#[derive(Debug, Clone)]
pub struct EirProgram {
    pub types: TypeTable,
    pub errors: Vec<String>,
    pub globals: Vec<TypeId>,
    pub host_functions: Vec<EirHostFunction>,
    pub constants: Vec<Constant>,
    pub functions: Vec<EirFunction>,
    pub module_initializers: Vec<FunctionId>,
}

#[derive(Debug, Clone)]
pub struct EirHostFunction {
    pub id: HostFunctionId,
    pub module: String,
    pub name: String,
    pub signature: Signature,
    pub anchor: SourceAnchor,
}

impl EirProgram {
    pub fn function(&self, id: FunctionId) -> Option<&EirFunction> {
        let index = usize::try_from(id).ok()?;
        self.functions
            .get(index)
            .filter(|function| function.id == id)
    }

    pub fn error_id_by_name(&self, name: &str) -> Option<ErrorId> {
        self.errors.iter().enumerate().find_map(|(index, symbol)| {
            let short_name = symbol
                .rsplit_once('.')
                .map_or(symbol.as_str(), |(_, name)| name);
            if short_name == name {
                ErrorId::try_from(index).ok()
            } else {
                None
            }
        })
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
    MoveGlobal {
        dst: SlotId,
        global: GlobalId,
    },
    LoadGlobal {
        dst: SlotId,
        global: GlobalId,
    },
    StoreGlobal {
        global: GlobalId,
        src: SlotId,
    },
    MakeError {
        dst: SlotId,
        error: ErrorId,
    },
    MakeFunction {
        dst: SlotId,
        function: FunctionId,
    },
    MakeHostFunction {
        dst: SlotId,
        function: HostFunctionId,
    },
    IsError {
        dst: SlotId,
        value: SlotId,
    },
    ErrorMatches {
        dst: SlotId,
        value: SlotId,
        error: ErrorId,
    },
    IsTruthy {
        dst: SlotId,
        value: SlotId,
    },
    Check {
        condition: SlotId,
        message: Option<ConstId>,
    },
    MakeAddress {
        dst: SlotId,
    },
    MakeRef {
        dst: SlotId,
        value: SlotId,
    },
    Deref {
        dst: SlotId,
        address: SlotId,
    },
    StoreDeref {
        address: SlotId,
        src: SlotId,
    },
    MakeList {
        dst: SlotId,
        items: Box<[SlotId]>,
    },
    MakeMap {
        dst: SlotId,
        entries: Box<[(SlotId, SlotId)]>,
    },
    MakeStruct {
        dst: SlotId,
        structure: StructId,
        fields: Box<[(FieldId, SlotId)]>,
    },
    GetField {
        dst: SlotId,
        target: SlotId,
        field: FieldId,
    },
    SetField {
        target: SlotId,
        fields: Box<[FieldId]>,
        src: SlotId,
    },
    GetIndex {
        dst: SlotId,
        collection: SlotId,
        key: SlotId,
    },
    Push {
        collection: SlotId,
        value: SlotId,
    },
    Len {
        dst: SlotId,
        collection: SlotId,
    },
    MakeRange {
        dst: SlotId,
        start: SlotId,
        end: SlotId,
    },
    IterInit {
        dst: SlotId,
        iterable: SlotId,
    },
    IterHasNext {
        dst: SlotId,
        iterable: SlotId,
        index: SlotId,
    },
    IterGet {
        dst: SlotId,
        iterable: SlotId,
        index: SlotId,
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
    CallHost {
        dst: Option<SlotId>,
        function: HostFunctionId,
        args: Box<[SlotId]>,
    },
    CallIndirect {
        dst: Option<SlotId>,
        callee: SlotId,
        args: Box<[SlotId]>,
    },
    MakePipe {
        dst: SlotId,
        capacity: Option<usize>,
    },
    Give {
        channel: SlotId,
        value: SlotId,
    },
    Take {
        dst: SlotId,
        channel: SlotId,
    },
    Close {
        channel: SlotId,
    },
    Rest,
    Spawn {
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
            | Self::MoveGlobal { dst, .. }
            | Self::LoadGlobal { dst, .. }
            | Self::MakeError { dst, .. }
            | Self::MakeFunction { dst, .. }
            | Self::MakeHostFunction { dst, .. }
            | Self::IsError { dst, .. }
            | Self::ErrorMatches { dst, .. }
            | Self::IsTruthy { dst, .. }
            | Self::MakeAddress { dst }
            | Self::MakeRef { dst, .. }
            | Self::Deref { dst, .. }
            | Self::MakeList { dst, .. }
            | Self::MakeMap { dst, .. }
            | Self::MakeStruct { dst, .. }
            | Self::GetField { dst, .. }
            | Self::GetIndex { dst, .. }
            | Self::Len { dst, .. }
            | Self::MakeRange { dst, .. }
            | Self::IterInit { dst, .. }
            | Self::IterHasNext { dst, .. }
            | Self::IterGet { dst, .. }
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
            Self::CallDirect { dst, .. }
            | Self::CallHost { dst, .. }
            | Self::CallIndirect { dst, .. } => *dst,
            Self::MakePipe { dst, .. } | Self::Take { dst, .. } => Some(*dst),
            Self::StoreGlobal { .. }
            | Self::Check { .. }
            | Self::StoreDeref { .. }
            | Self::SetField { .. }
            | Self::Push { .. } => None,
            Self::Give { .. } | Self::Close { .. } | Self::Rest | Self::Spawn { .. } => None,
        }
    }

    pub fn read_slots(&self) -> Vec<SlotId> {
        match self {
            Self::Const { .. } => Vec::new(),
            Self::Copy { src, .. } | Self::Move { src, .. } | Self::StoreGlobal { src, .. } => {
                vec![*src]
            }
            Self::LoadGlobal { .. } => Vec::new(),
            Self::MoveGlobal { .. } | Self::MakeAddress { .. } => Vec::new(),
            Self::MakeError { .. }
            | Self::MakeFunction { .. }
            | Self::MakeHostFunction { .. }
            | Self::MakePipe { .. }
            | Self::Rest => Vec::new(),
            Self::IsError { value, .. }
            | Self::ErrorMatches { value, .. }
            | Self::IsTruthy { value, .. } => vec![*value],
            Self::Check { condition, .. } => vec![*condition],
            Self::MakeRef { value, .. } => vec![*value],
            Self::Deref { address, .. } => vec![*address],
            Self::StoreDeref { address, src } => vec![*address, *src],
            Self::MakeList { items, .. } => items.to_vec(),
            Self::MakeMap { entries, .. } => entries
                .iter()
                .flat_map(|(key, value)| [*key, *value])
                .collect(),
            Self::MakeStruct { fields, .. } => fields.iter().map(|(_, value)| *value).collect(),
            Self::GetField { target, .. } => vec![*target],
            Self::SetField { target, src, .. } => vec![*target, *src],
            Self::GetIndex {
                collection, key, ..
            } => vec![*collection, *key],
            Self::Push { collection, value } => vec![*collection, *value],
            Self::Len { collection, .. } => vec![*collection],
            Self::MakeRange { start, end, .. } => vec![*start, *end],
            Self::IterInit { iterable, .. } => vec![*iterable],
            Self::IterHasNext {
                iterable, index, ..
            }
            | Self::IterGet {
                iterable, index, ..
            } => vec![*iterable, *index],
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
            Self::CallDirect { args, .. }
            | Self::CallHost { args, .. }
            | Self::Spawn { args, .. } => args.to_vec(),
            Self::CallIndirect { callee, args, .. } => std::iter::once(*callee)
                .chain(args.iter().copied())
                .collect(),
            Self::Give { channel, value } => vec![*channel, *value],
            Self::Take { channel, .. } | Self::Close { channel } => vec![*channel],
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
