//! Error codes and their serialization across IPC (design §6.6).

use mcleaner_core::error::CoreError;
use serde::Serialize;
use ts_rs::TS;

use crate::worker_pool::WorkerPoolError;

/// The error codes of design §6.6.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, TS)]
pub enum ErrorCode {
    UnsupportedFormat,
    DecodeFailed,
    PdfOpenFailed,
    PdfEncrypted,
    PdfSigned,
    TooLarge,
    WorkerCrashed,
    WorkerTimeout,
    VerifyFailed,
    SameFolderAsSource,
    ReadFailed,
    WriteFailed,
    JobRunning,
    UnknownHandle,
    InvalidParams,
}

impl ErrorCode {
    /// Every code, so that [`ErrorCode::from_name`] cannot miss one.
    pub const ALL: [Self; 15] = [
        Self::UnsupportedFormat,
        Self::DecodeFailed,
        Self::PdfOpenFailed,
        Self::PdfEncrypted,
        Self::PdfSigned,
        Self::TooLarge,
        Self::WorkerCrashed,
        Self::WorkerTimeout,
        Self::VerifyFailed,
        Self::SameFolderAsSource,
        Self::ReadFailed,
        Self::WriteFailed,
        Self::JobRunning,
        Self::UnknownHandle,
        Self::InvalidParams,
    ];

    /// The name of the code as it appears in design §6.6.
    pub fn name(self) -> &'static str {
        match self {
            Self::UnsupportedFormat => "UnsupportedFormat",
            Self::DecodeFailed => "DecodeFailed",
            Self::PdfOpenFailed => "PdfOpenFailed",
            Self::PdfEncrypted => "PdfEncrypted",
            Self::PdfSigned => "PdfSigned",
            Self::TooLarge => "TooLarge",
            Self::WorkerCrashed => "WorkerCrashed",
            Self::WorkerTimeout => "WorkerTimeout",
            Self::VerifyFailed => "VerifyFailed",
            Self::SameFolderAsSource => "SameFolderAsSource",
            Self::ReadFailed => "ReadFailed",
            Self::WriteFailed => "WriteFailed",
            Self::JobRunning => "JobRunning",
            Self::UnknownHandle => "UnknownHandle",
            Self::InvalidParams => "InvalidParams",
        }
    }

    /// The code named `name` in design §6.6, if there is one.
    pub fn from_name(name: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|code| code.name() == name)
    }
}

/// The error payload of a command or of an item in a list (design §6.6).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct IpcError {
    pub code: ErrorCode,
    pub detail: Option<String>,
}

impl IpcError {
    /// An error with a detail string.
    pub fn new(code: ErrorCode, detail: impl Into<String>) -> Self {
        Self {
            code,
            detail: Some(detail.into()),
        }
    }

    /// An error without a detail.
    pub fn from_code(code: ErrorCode) -> Self {
        Self { code, detail: None }
    }

    /// Turns the `{ code, detail }` a worker answered with into an IPC error.
    ///
    /// A code that design §6.6 does not list becomes `WorkerCrashed`, and
    /// the worker's own code goes into `detail` so it is not lost.
    pub fn from_worker(code: &str, detail: Option<&str>) -> Self {
        match ErrorCode::from_name(code) {
            Some(known) => Self {
                code: known,
                detail: detail.map(str::to_owned),
            },
            None => Self::new(
                ErrorCode::WorkerCrashed,
                match detail {
                    Some(detail) => format!("{code}: {detail}"),
                    None => code.to_owned(),
                },
            ),
        }
    }
}

impl From<CoreError> for IpcError {
    fn from(err: CoreError) -> Self {
        let code = match err {
            CoreError::UnsupportedFormat => ErrorCode::UnsupportedFormat,
            CoreError::DecodeFailed => ErrorCode::DecodeFailed,
            CoreError::TooLarge { .. } => ErrorCode::TooLarge,
        };
        Self {
            code,
            detail: err.detail(),
        }
    }
}

impl From<&WorkerPoolError> for IpcError {
    fn from(err: &WorkerPoolError) -> Self {
        match err {
            WorkerPoolError::WorkerCrashed => Self::from_code(ErrorCode::WorkerCrashed),
            WorkerPoolError::WorkerTimeout => Self::from_code(ErrorCode::WorkerTimeout),
            WorkerPoolError::Remote { code, detail } => Self::from_worker(code, detail.as_deref()),
        }
    }
}

impl From<WorkerPoolError> for IpcError {
    fn from(err: WorkerPoolError) -> Self {
        Self::from(&err)
    }
}

impl std::fmt::Display for IpcError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match &self.detail {
            Some(detail) => write!(f, "{}: {detail}", self.code.name()),
            None => f.write_str(self.code.name()),
        }
    }
}

impl std::error::Error for IpcError {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn names_match_the_serialized_codes() {
        for code in ErrorCode::ALL {
            let json = serde_json::to_string(&code).expect("serializing a code");
            assert_eq!(json, format!("\"{}\"", code.name()));
            assert_eq!(ErrorCode::from_name(code.name()), Some(code));
        }
        assert_eq!(ErrorCode::from_name("UnknownCode"), None);
    }

    #[test]
    fn serializes_as_code_and_detail() {
        let json = serde_json::to_string(&IpcError::from_code(ErrorCode::UnknownHandle))
            .expect("serializing an error");
        assert_eq!(json, r#"{"code":"UnknownHandle","detail":null}"#);
        let json = serde_json::to_string(&IpcError::new(ErrorCode::TooLarge, "256 MB"))
            .expect("serializing an error");
        assert_eq!(json, r#"{"code":"TooLarge","detail":"256 MB"}"#);
    }

    #[test]
    fn maps_core_errors() {
        assert_eq!(
            IpcError::from(CoreError::DecodeFailed),
            IpcError::from_code(ErrorCode::DecodeFailed)
        );
        assert_eq!(
            IpcError::from(CoreError::UnsupportedFormat),
            IpcError::from_code(ErrorCode::UnsupportedFormat)
        );
        assert_eq!(
            IpcError::from(CoreError::TooLarge {
                limit_bytes: 256 * 1024 * 1024
            }),
            IpcError::new(ErrorCode::TooLarge, "256 MB")
        );
    }

    #[test]
    fn maps_pool_errors() {
        assert_eq!(
            IpcError::from(WorkerPoolError::WorkerCrashed),
            IpcError::from_code(ErrorCode::WorkerCrashed)
        );
        assert_eq!(
            IpcError::from(WorkerPoolError::WorkerTimeout),
            IpcError::from_code(ErrorCode::WorkerTimeout)
        );
        assert_eq!(
            IpcError::from(WorkerPoolError::Remote {
                code: "PdfEncrypted".into(),
                detail: None,
            }),
            IpcError::from_code(ErrorCode::PdfEncrypted)
        );
        assert_eq!(
            IpcError::from(WorkerPoolError::Remote {
                code: "PdfSigned".into(),
                detail: None,
            }),
            IpcError::from_code(ErrorCode::PdfSigned)
        );
    }

    #[test]
    fn an_unknown_worker_code_is_a_crash_that_keeps_the_code() {
        assert_eq!(
            IpcError::from_worker("Surprise", None),
            IpcError::new(ErrorCode::WorkerCrashed, "Surprise")
        );
        assert_eq!(
            IpcError::from_worker("Surprise", Some("more")),
            IpcError::new(ErrorCode::WorkerCrashed, "Surprise: more")
        );
    }
}
