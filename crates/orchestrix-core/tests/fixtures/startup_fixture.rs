//! Explicit startup-only QA helper. Never launches a provider/runtime task.
use orchestrix_core::{
    daemon,
    daemon_startup::{NONCE_ENV, Phase},
};
use serde::Deserialize;
use serde_json::json;
use std::{fs, io::Write, path::PathBuf, sync::mpsc, time::Duration};

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Control {
    gate_worker_recovery: bool,
}
fn main() {
    if let Err(code) = run() {
        eprintln!("Startup fixture: {code}");
        std::process::exit(2);
    }
}
fn run() -> Result<(), String> {
    let args = std::env::args().skip(1).collect::<Vec<_>>();
    if args.len() != 3 || args[0] != "daemon" || args[1] != "--database" {
        return Err("arguments".into());
    }
    let database = PathBuf::from(&args[2]);
    let directory = database.parent().ok_or("directory")?;
    let nonce = std::env::var(NONCE_ENV).map_err(|_| "nonce")?;
    if nonce.len() != 64
        || !nonce
            .bytes()
            .all(|b| b.is_ascii_hexdigit() && !b.is_ascii_uppercase())
    {
        return Err("nonce".into());
    }
    fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(directory.join("startup-launches.jsonl"))
        .and_then(|mut file| writeln!(file, "{}", json!({"pid":std::process::id(),"nonce":nonce})))
        .map_err(|_| "launch-ledger")?;
    let control = match fs::read(directory.join("startup-fixture.json")) {
        Ok(bytes) if bytes.len() <= 1024 => {
            serde_json::from_slice::<Control>(&bytes).map_err(|_| "control")?
        }
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Control {
            gate_worker_recovery: false,
        },
        _ => return Err("control".into()),
    };
    let (done, wait) = mpsc::channel();
    let watchdog = std::thread::spawn(move || {
        if matches!(
            wait.recv_timeout(Duration::from_secs(60)),
            Err(mpsc::RecvTimeoutError::Timeout)
        ) {
            std::process::exit(86);
        }
    });
    let mut startup_done = Some(done);
    let result = daemon::serve_with_startup_observer(&database, |phase| {
        if phase == Phase::WorkerRecovery && control.gate_worker_recovery {
            let marker = directory.join(format!("startup-entered-{nonce}.json"));
            fs::OpenOptions::new()
                .create_new(true)
                .write(true)
                .open(marker)
                .and_then(|mut file| {
                    write!(
                        file,
                        "{}",
                        json!({"pid":std::process::id(),"nonce":nonce,"phase":phase})
                    )
                })
                .expect("write own startup marker");
            loop {
                if fs::read(directory.join("startup.release.json"))
                    .ok()
                    .filter(|bytes| bytes.len() <= 128)
                    .and_then(|bytes| serde_json::from_slice::<serde_json::Value>(&bytes).ok())
                    .is_some_and(|value| value["nonce"] == nonce)
                {
                    break;
                }
                std::thread::sleep(Duration::from_millis(20));
            }
        }
        if phase == Phase::IpcAvailable
            && let Some(done) = startup_done.take()
        {
            let _ = done.send(());
        }
    });
    drop(startup_done);
    let _ = watchdog.join();
    result
}
