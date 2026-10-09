//! Errors returned by metadata operations in `crates/core` (design §6.6).

/// Errors originating from core image operations.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CoreError {
    UnsupportedFormat,
    DecodeFailed,
    TooLarge { limit_bytes: u64 },
}

impl CoreError {
    /// Returns the error code string specified in design §6.6.
    pub fn code(&self) -> &'static str {
        match self {
            CoreError::UnsupportedFormat => "UnsupportedFormat",
            CoreError::DecodeFailed => "DecodeFailed",
            CoreError::TooLarge { .. } => "TooLarge",
        }
    }

    /// Returns the detail string for the error, formatted as "256 MB" for `TooLarge`.
    pub fn detail(&self) -> Option<String> {
        match self {
            CoreError::TooLarge { limit_bytes } => {
                let mib = limit_bytes / (1024 * 1024);
                Some(format!("{mib} MB"))
            }
            _ => None,
        }
    }
}

impl std::fmt::Display for CoreError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            CoreError::UnsupportedFormat => write!(f, "unsupported format"),
            CoreError::DecodeFailed => write!(f, "decode failed"),
            CoreError::TooLarge { limit_bytes } => {
                let mib = limit_bytes / (1024 * 1024);
                write!(f, "file too large (limit: {mib} MB)")
            }
        }
    }
}

impl std::error::Error for CoreError {}
