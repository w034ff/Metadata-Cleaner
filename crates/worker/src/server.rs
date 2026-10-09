//! The worker side: answers requests on stdin/stdout until stdin closes
//! (design §5).

use std::io::{self, BufReader, BufWriter};
use std::path::Path;

use mcleaner_core::detect::MAX_PDF_FILE_BYTES;
use mcleaner_core::error::CoreError;

use crate::VERSION;
use crate::pdf;
use crate::protocol::{Request, Response, read_message, write_message};

/// How much memory one worker may use (design §5.3). Allocations beyond it
/// fail and end the worker, which the main process reports as a crash.
pub const WORKER_MEMORY_LIMIT: u64 = 2 * 1024 * 1024 * 1024;

/// Runs the worker loop.
///
/// # Errors
///
/// Returns an I/O error when the memory limit cannot be set or stdin or
/// stdout fails. A clean end of stdin (the main process closed the pipe or
/// exited) returns `Ok(())`.
pub fn run() -> io::Result<()> {
    apply_memory_limit()?;
    let mut input = BufReader::new(io::stdin().lock());
    let mut output = BufWriter::new(io::stdout().lock());
    while let Some((request, _body)) = read_message::<_, Request>(&mut input)? {
        let (response, body) = handle(request);
        write_message(&mut output, &response, &body)?;
    }
    Ok(())
}

enum ReadFileError {
    ReadFailed,
    TooLarge(String),
}

impl From<ReadFileError> for Response {
    fn from(err: ReadFileError) -> Self {
        match err {
            ReadFileError::ReadFailed => Self::Error {
                code: "ReadFailed".into(),
                detail: None,
            },
            ReadFileError::TooLarge(detail) => Self::Error {
                code: "TooLarge".into(),
                detail: Some(detail),
            },
        }
    }
}

fn read_pdf_file(path: &Path) -> Result<Vec<u8>, ReadFileError> {
    let metadata = match std::fs::metadata(path) {
        Ok(m) => m,
        Err(_) => return Err(ReadFileError::ReadFailed),
    };
    if metadata.len() > MAX_PDF_FILE_BYTES {
        let detail = CoreError::TooLarge {
            limit_bytes: MAX_PDF_FILE_BYTES,
        }
        .detail()
        .unwrap_or_default();
        return Err(ReadFileError::TooLarge(detail));
    }
    match std::fs::read(path) {
        Ok(bytes) => {
            if bytes.len() as u64 > MAX_PDF_FILE_BYTES {
                let detail = CoreError::TooLarge {
                    limit_bytes: MAX_PDF_FILE_BYTES,
                }
                .detail()
                .unwrap_or_default();
                return Err(ReadFileError::TooLarge(detail));
            }
            Ok(bytes)
        }
        Err(_) => Err(ReadFileError::ReadFailed),
    }
}

fn handle(request: Request) -> (Response, Vec<u8>) {
    match request {
        Request::Ping => (
            Response::Pong {
                version: VERSION.into(),
            },
            Vec::new(),
        ),
        Request::Inspect { path, details } => {
            let bytes = match read_pdf_file(&path) {
                Ok(b) => b,
                Err(err) => return (Response::from(err), Vec::new()),
            };
            let inspection = match pdf::inspect(&bytes) {
                Ok(insp) => insp,
                Err(e) => {
                    return (
                        Response::Error {
                            code: e.code().into(),
                            detail: None,
                        },
                        Vec::new(),
                    );
                }
            };
            let details = if details {
                match pdf::details(&bytes) {
                    Ok(d) => Some(d),
                    Err(e) => {
                        return (
                            Response::Error {
                                code: e.code().into(),
                                detail: None,
                            },
                            Vec::new(),
                        );
                    }
                }
            } else {
                None
            };
            (
                Response::Inspected {
                    inspection,
                    details,
                },
                Vec::new(),
            )
        }
        Request::Clean { path } => {
            let bytes = match read_pdf_file(&path) {
                Ok(b) => b,
                Err(err) => return (Response::from(err), Vec::new()),
            };
            let inspection = match pdf::inspect(&bytes) {
                Ok(insp) => insp,
                Err(e) => {
                    return (
                        Response::Error {
                            code: e.code().into(),
                            detail: None,
                        },
                        Vec::new(),
                    );
                }
            };
            let removed = inspection.kinds;
            let cleaned_bytes = match pdf::clean(&bytes) {
                Ok(c) => c,
                Err(e) => {
                    return (
                        Response::Error {
                            code: e.code().into(),
                            detail: None,
                        },
                        Vec::new(),
                    );
                }
            };
            match pdf::inspect(&cleaned_bytes) {
                Ok(re_insp) if re_insp.kinds.is_empty() => {}
                _ => {
                    return (
                        Response::Error {
                            code: "VerifyFailed".into(),
                            detail: None,
                        },
                        Vec::new(),
                    );
                }
            }
            (Response::Cleaned { removed }, cleaned_bytes)
        }
        #[cfg(feature = "test-hooks")]
        Request::AllocateForTest { mebibytes } => {
            const MIB: usize = 1024 * 1024;
            let size = usize::try_from(mebibytes)
                .unwrap_or(usize::MAX)
                .saturating_mul(MIB);
            // Writing every page makes the operating system commit the
            // memory, so the limit is hit here rather than lazily later.
            let mut block = vec![0u8; size];
            for byte in block.iter_mut().step_by(4096) {
                *byte = 1;
            }
            std::hint::black_box(&block);
            (Response::Allocated, Vec::new())
        }
        #[cfg(feature = "test-hooks")]
        Request::CrashForTest => std::process::abort(),
        #[cfg(feature = "test-hooks")]
        Request::HangForTest => loop {
            std::thread::park();
        },
    }
}

#[cfg(target_os = "linux")]
fn apply_memory_limit() -> io::Result<()> {
    // RLIMIT_AS caps the address space; it is set before the first request
    // so that every PDF is parsed under it.
    rlimit::setrlimit(
        rlimit::Resource::AS,
        WORKER_MEMORY_LIMIT,
        WORKER_MEMORY_LIMIT,
    )
}

#[cfg(windows)]
fn apply_memory_limit() -> io::Result<()> {
    // On Windows the main process puts the worker in a job object with the
    // limit before sending the first request (crate::windows_job).
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ping_answers_with_the_crate_version() {
        assert_eq!(
            handle(Request::Ping),
            (
                Response::Pong {
                    version: env!("CARGO_PKG_VERSION").into()
                },
                Vec::new()
            )
        );
    }
}
