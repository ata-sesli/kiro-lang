use std::fmt;
use std::ops::{BitOr, BitOrAssign};

#[derive(Clone, Copy, PartialEq, Eq, Hash, Default)]
pub struct Effects(u8);

impl Effects {
    pub const NONE: Self = Self(0);
    pub const PURE: Self = Self(1 << 0);
    pub const MAY_FAIL: Self = Self(1 << 1);
    pub const MAY_BLOCK: Self = Self(1 << 2);
    pub const MAY_SPAWN: Self = Self(1 << 3);
    pub const HOST_CALL: Self = Self(1 << 4);
    pub const INDIRECT_CALL: Self = Self(1 << 5);

    const ALL: [(Self, &'static str); 6] = [
        (Self::PURE, "PURE"),
        (Self::MAY_FAIL, "MAY_FAIL"),
        (Self::MAY_BLOCK, "MAY_BLOCK"),
        (Self::MAY_SPAWN, "MAY_SPAWN"),
        (Self::HOST_CALL, "HOST_CALL"),
        (Self::INDIRECT_CALL, "INDIRECT_CALL"),
    ];

    pub const fn contains(self, other: Self) -> bool {
        self.0 & other.0 == other.0
    }

    pub const fn is_empty(self) -> bool {
        self.0 == 0
    }
}

impl BitOr for Effects {
    type Output = Self;

    fn bitor(self, rhs: Self) -> Self::Output {
        Self(self.0 | rhs.0)
    }
}

impl BitOrAssign for Effects {
    fn bitor_assign(&mut self, rhs: Self) {
        self.0 |= rhs.0;
    }
}

impl fmt::Debug for Effects {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("Effects(")?;
        if self.is_empty() {
            formatter.write_str("NONE")?;
        } else {
            let mut first = true;
            for (effect, name) in Self::ALL {
                if self.contains(effect) {
                    if !first {
                        formatter.write_str(" | ")?;
                    }
                    formatter.write_str(name)?;
                    first = false;
                }
            }
        }
        formatter.write_str(")")
    }
}
