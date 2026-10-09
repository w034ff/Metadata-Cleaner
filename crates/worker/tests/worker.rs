//! Starts the standalone worker binary and talks to it as the app does
//! (design §5, §11.3).

use std::io::Read;
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

use mcleaner_worker::client::{WorkerError, WorkerProcess};
use mcleaner_worker::protocol::{Request, Response};

const ANSWER_TIMEOUT: Duration = Duration::from_secs(30);

fn start() -> WorkerProcess {
    WorkerProcess::spawn(
        env!("CARGO_BIN_EXE_mcleaner-worker").as_ref(),
        std::iter::empty::<&str>(),
    )
    .expect("worker should start")
}

fn pong() -> Response {
    Response::Pong {
        version: mcleaner_worker::VERSION.into(),
    }
}

#[test]
fn answers_ping_repeatedly() {
    let mut worker = start();
    for _ in 0..3 {
        let (response, body) = worker.request(&Request::Ping, ANSWER_TIMEOUT).unwrap();
        assert_eq!(response, pong());
        assert!(body.is_empty());
    }
}

#[test]
fn exits_when_the_main_process_closes_stdin() {
    let mut child = Command::new(env!("CARGO_BIN_EXE_mcleaner-worker"))
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()
        .unwrap();
    drop(child.stdin.take());
    let deadline = Instant::now() + ANSWER_TIMEOUT;
    loop {
        if let Some(status) = child.try_wait().unwrap() {
            assert!(status.success());
            break;
        }
        assert!(
            Instant::now() < deadline,
            "worker did not exit after stdin closed"
        );
        std::thread::sleep(Duration::from_millis(50));
    }
    let mut rest = Vec::new();
    child.stdout.take().unwrap().read_to_end(&mut rest).unwrap();
    assert!(rest.is_empty());
}

#[cfg(feature = "test-hooks")]
mod hooks {
    use super::*;

    #[test]
    fn a_crash_is_reported_and_the_worker_is_not_used_again() {
        let mut worker = start();
        assert!(matches!(
            worker.request(&Request::CrashForTest, ANSWER_TIMEOUT),
            Err(WorkerError::Crashed)
        ));
        assert!(matches!(
            worker.request(&Request::Ping, ANSWER_TIMEOUT),
            Err(WorkerError::Crashed)
        ));
        assert!(worker.is_dead());
        // A fresh worker works after the crash.
        assert!(start().request(&Request::Ping, ANSWER_TIMEOUT).is_ok());
    }

    #[test]
    fn a_hang_times_out_and_kills_the_worker() {
        let mut worker = start();
        let started = Instant::now();
        assert!(matches!(
            worker.request(&Request::HangForTest, Duration::from_secs(1)),
            Err(WorkerError::Timeout)
        ));
        assert!(started.elapsed() < Duration::from_secs(10));
        assert!(matches!(
            worker.request(&Request::Ping, ANSWER_TIMEOUT),
            Err(WorkerError::Crashed)
        ));
    }

    #[test]
    fn allocations_within_the_limit_succeed() {
        let mut worker = start();
        let (response, _) = worker
            .request(&Request::AllocateForTest { mebibytes: 256 }, ANSWER_TIMEOUT)
            .unwrap();
        assert_eq!(response, Response::Allocated);
        // The worker is still usable after a large allocation it was allowed.
        assert_eq!(
            worker.request(&Request::Ping, ANSWER_TIMEOUT).unwrap().0,
            pong()
        );
    }

    #[test]
    fn exceeding_the_memory_limit_ends_only_the_worker() {
        let mut worker = start();
        // 3 GiB is above WORKER_MEMORY_LIMIT (2 GiB) on every runner, and
        // the test process itself keeps running to see the result.
        assert!(matches!(
            worker.request(
                &Request::AllocateForTest { mebibytes: 3072 },
                ANSWER_TIMEOUT
            ),
            Err(WorkerError::Crashed)
        ));
        assert!(start().request(&Request::Ping, ANSWER_TIMEOUT).is_ok());
    }
}
