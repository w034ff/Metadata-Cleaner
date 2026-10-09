//! The worker side: answers requests on stdin/stdout until stdin closes
//! (design §5).

use std::io::{self, BufReader, BufWriter};

use crate::VERSION;
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

fn handle(request: Request) -> (Response, Vec<u8>) {
    match request {
        Request::Ping => (
            Response::Pong {
                version: VERSION.into(),
            },
            Vec::new(),
        ),
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
