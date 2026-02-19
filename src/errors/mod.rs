pub mod code;
pub mod convert;
pub mod diagnostic;
pub mod render;

pub use code::{ErrorCode, ErrorPhase};
pub use convert::panic_payload_to_string;
pub use diagnostic::KiroError;
pub use render::emit_error;
