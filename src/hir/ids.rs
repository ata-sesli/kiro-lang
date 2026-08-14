use std::fmt;
use std::num::TryFromIntError;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct IdOverflow {
    kind: &'static str,
    value: usize,
}

impl IdOverflow {
    pub const fn kind(self) -> &'static str {
        self.kind
    }

    pub const fn value(self) -> usize {
        self.value
    }
}

impl fmt::Display for IdOverflow {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "{} value {} exceeds the u32 semantic ID range",
            self.kind, self.value
        )
    }
}

impl std::error::Error for IdOverflow {}

macro_rules! semantic_id {
    ($name:ident) => {
        #[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
        pub struct $name(u32);

        impl $name {
            pub const fn new(raw: u32) -> Self {
                Self(raw)
            }

            pub const fn raw(self) -> u32 {
                self.0
            }
        }

        impl From<u32> for $name {
            fn from(raw: u32) -> Self {
                Self::new(raw)
            }
        }

        impl From<$name> for u32 {
            fn from(id: $name) -> Self {
                id.raw()
            }
        }

        impl TryFrom<usize> for $name {
            type Error = IdOverflow;

            fn try_from(value: usize) -> Result<Self, Self::Error> {
                u32::try_from(value).map(Self::new).map_err(|_| IdOverflow {
                    kind: stringify!($name),
                    value,
                })
            }
        }

        impl TryFrom<$name> for usize {
            type Error = TryFromIntError;

            fn try_from(id: $name) -> Result<Self, Self::Error> {
                usize::try_from(id.raw())
            }
        }
    };
}

semantic_id!(SourceId);
semantic_id!(ModuleId);
semantic_id!(FunctionId);
semantic_id!(HostFunctionId);
semantic_id!(StructId);
semantic_id!(HandleId);
semantic_id!(ErrorId);
semantic_id!(FieldId);
semantic_id!(LocalId);
semantic_id!(TypeId);
