use std::fmt;
use std::num::TryFromIntError;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct EirIdOverflow {
    kind: &'static str,
    value: usize,
}

impl fmt::Display for EirIdOverflow {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "{} value {} exceeds the u32 EIR ID range",
            self.kind, self.value
        )
    }
}

impl std::error::Error for EirIdOverflow {}

macro_rules! eir_id {
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

        impl TryFrom<usize> for $name {
            type Error = EirIdOverflow;

            fn try_from(value: usize) -> Result<Self, Self::Error> {
                u32::try_from(value)
                    .map(Self::new)
                    .map_err(|_| EirIdOverflow {
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

eir_id!(BlockId);
eir_id!(SlotId);
eir_id!(ConstId);
eir_id!(GlobalId);
