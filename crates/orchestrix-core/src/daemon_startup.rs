//! Private, bounded launch telemetry. It never grants readiness or admission.
//! All elapsed values in this channel share the reporting process's origin.
use serde::{Deserialize, Serialize};
use std::{io::Write, time::Instant};

pub const MAX_REPORT_BYTES: usize = 2048;
pub const MAX_PHASES: usize = 9;
pub const NONCE_ENV: &str = "ORCHESTRIX_STARTUP_NONCE";

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Phase {
    Ownership,
    Database,
    Restoration,
    WorkerRecovery,
    ReviewRecovery,
    IntegrationRecovery,
    PendingCommands,
    IpcBinding,
    IpcAvailable,
}
impl Phase {
    pub fn recovering(self) -> bool {
        matches!(
            self,
            Self::WorkerRecovery
                | Self::ReviewRecovery
                | Self::IntegrationRecovery
                | Self::PendingCommands
        )
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "kebab-case", deny_unknown_fields)]
pub enum Message {
    Header {
        schema_version: u32,
        nonce: String,
        scope: String,
        pid: u32,
    },
    Phase {
        phase: Phase,
        elapsed_ms: u64,
    },
    Outcome {
        phase: Phase,
        failed: bool,
        elapsed_ms: u64,
    },
}
fn valid_token(value: &str) -> bool {
    value.len() == 64
        && value
            .bytes()
            .all(|b| b.is_ascii_hexdigit() && !b.is_ascii_uppercase())
}

pub struct StartupReporter {
    writer: Option<Box<dyn Write + Send>>,
    started: Instant,
    sent: usize,
    current: Option<Phase>,
    finished: bool,
}
impl StartupReporter {
    pub fn from_environment(scope: &str) -> Result<Self, &'static str> {
        match std::env::var(NONCE_ENV) {
            Ok(nonce) => Self::with_writer(
                &nonce,
                scope,
                std::process::id(),
                Box::new(std::io::stdout()),
            ),
            Err(std::env::VarError::NotPresent) => Ok(Self::silent()),
            Err(_) => Err("startup-nonce"),
        }
    }
    pub fn silent() -> Self {
        Self {
            writer: None,
            started: Instant::now(),
            sent: 0,
            current: None,
            finished: false,
        }
    }
    fn with_writer(
        nonce: &str,
        scope: &str,
        pid: u32,
        writer: Box<dyn Write + Send>,
    ) -> Result<Self, &'static str> {
        if !valid_token(nonce) || !valid_token(scope) || pid == 0 {
            return Err("startup-header");
        }
        let mut report = Self {
            writer: Some(writer),
            ..Self::silent()
        };
        report.send(Message::Header {
            schema_version: 1,
            nonce: nonce.into(),
            scope: scope.into(),
            pid,
        });
        Ok(report)
    }
    pub fn enter(&mut self, phase: Phase) {
        if self.finished || self.current.is_some_and(|old| old >= phase) {
            return;
        }
        self.current = Some(phase);
        self.send(Message::Phase {
            phase,
            elapsed_ms: self.elapsed(),
        });
    }
    pub fn finish(&mut self) {
        if self.finished {
            return;
        }
        self.finished = true;
        self.send(Message::Outcome {
            phase: self.current.unwrap_or(Phase::Ownership),
            failed: false,
            elapsed_ms: self.elapsed(),
        });
    }
    fn elapsed(&self) -> u64 {
        self.started.elapsed().as_millis().min(u64::MAX as u128) as u64
    }
    fn send(&mut self, message: Message) {
        let Some(writer) = &mut self.writer else {
            return;
        };
        let Ok(mut bytes) = serde_json::to_vec(&message) else {
            self.writer = None;
            return;
        };
        bytes.push(b'\n');
        if self.sent + bytes.len() > MAX_REPORT_BYTES
            || writer
                .write_all(&bytes)
                .and_then(|()| writer.flush())
                .is_err()
        {
            // This optional channel cannot make startup fail or block admission.
            self.writer = None;
            return;
        }
        self.sent += bytes.len();
    }
}
impl Drop for StartupReporter {
    fn drop(&mut self) {
        if !self.finished {
            self.send(Message::Outcome {
                phase: self.current.unwrap_or(Phase::Ownership),
                failed: true,
                elapsed_ms: self.elapsed(),
            });
        }
    }
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ReportObservation {
    pub phase: Option<Phase>,
    pub phases: Vec<PhaseObservation>,
    pub daemon_elapsed_ms: u64,
    pub report_completed: bool,
    pub reported_failure: bool,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PhaseObservation {
    pub phase: Phase,
    pub elapsed_ms: u64,
}
pub struct ReportDecoder {
    nonce: String,
    scope: String,
    pid: u32,
    total: usize,
    buffered: Vec<u8>,
    header: bool,
    observation: ReportObservation,
}
impl ReportDecoder {
    pub fn new(nonce: String, scope: String, pid: u32) -> Result<Self, &'static str> {
        if !valid_token(&nonce) || !valid_token(&scope) || pid == 0 {
            return Err("startup-header");
        }
        Ok(Self {
            nonce,
            scope,
            pid,
            total: 0,
            buffered: vec![],
            header: false,
            observation: ReportObservation::default(),
        })
    }
    pub fn observation(&self) -> &ReportObservation {
        &self.observation
    }
    pub fn feed(&mut self, bytes: &[u8]) -> Result<(), &'static str> {
        self.total = self
            .total
            .checked_add(bytes.len())
            .ok_or("startup-report-budget")?;
        if self.total > MAX_REPORT_BYTES {
            return Err("startup-report-budget");
        }
        self.buffered.extend_from_slice(bytes);
        while let Some(end) = self.buffered.iter().position(|b| *b == b'\n') {
            if end > 512 {
                return Err("startup-report-budget");
            }
            let message: Message = serde_json::from_slice(&self.buffered[..end])
                .map_err(|_| "startup-report-schema")?;
            self.buffered.drain(..=end);
            match message {
                Message::Header {
                    schema_version,
                    nonce,
                    scope,
                    pid,
                } => {
                    if self.header
                        || schema_version != 1
                        || nonce != self.nonce
                        || scope != self.scope
                        || pid != self.pid
                    {
                        return Err("startup-report-binding");
                    }
                    self.header = true;
                }
                Message::Phase { phase, elapsed_ms } => {
                    if !self.header
                        || self.observation.report_completed
                        || self.observation.phase.is_some_and(|old| old >= phase)
                        || self.observation.phases.len() >= MAX_PHASES
                        || elapsed_ms < self.observation.daemon_elapsed_ms
                    {
                        return Err("startup-report-order");
                    }
                    self.observation.phase = Some(phase);
                    self.observation
                        .phases
                        .push(PhaseObservation { phase, elapsed_ms });
                    self.observation.daemon_elapsed_ms = elapsed_ms;
                }
                Message::Outcome {
                    phase,
                    failed,
                    elapsed_ms,
                } => {
                    if !self.header
                        || self.observation.report_completed
                        || self.observation.phase != Some(phase)
                        || elapsed_ms < self.observation.daemon_elapsed_ms
                        || (!failed && phase != Phase::IpcAvailable)
                    {
                        return Err("startup-report-order");
                    }
                    self.observation.daemon_elapsed_ms = elapsed_ms;
                    self.observation.report_completed = true;
                    self.observation.reported_failure = failed;
                }
            }
        }
        if self.buffered.len() > 512 {
            return Err("startup-report-budget");
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::{Arc, Mutex};
    #[derive(Clone)]
    struct Sink(Arc<Mutex<Vec<u8>>>);
    impl Write for Sink {
        fn write(&mut self, bytes: &[u8]) -> std::io::Result<usize> {
            self.0.lock().unwrap().extend_from_slice(bytes);
            Ok(bytes.len())
        }
        fn flush(&mut self) -> std::io::Result<()> {
            Ok(())
        }
    }
    #[test]
    fn report_is_bound_bounded_and_incrementally_decoded_without_readiness_authority() {
        let sink = Sink(Arc::new(Mutex::new(vec![])));
        let nonce = "a".repeat(64);
        let scope = "b".repeat(64);
        let mut reporter =
            StartupReporter::with_writer(&nonce, &scope, 42, Box::new(sink.clone())).unwrap();
        for phase in [
            Phase::Ownership,
            Phase::Database,
            Phase::Restoration,
            Phase::WorkerRecovery,
            Phase::ReviewRecovery,
            Phase::IntegrationRecovery,
            Phase::PendingCommands,
            Phase::IpcBinding,
            Phase::IpcAvailable,
        ] {
            reporter.enter(phase);
        }
        reporter.finish();
        drop(reporter);
        let bytes = sink.0.lock().unwrap();
        assert!(bytes.len() < MAX_REPORT_BYTES);
        let mut decoder = ReportDecoder::new(nonce, scope, 42).unwrap();
        for fragment in bytes.chunks(7) {
            decoder.feed(fragment).unwrap();
        }
        assert_eq!(decoder.observation().phase, Some(Phase::IpcAvailable));
        assert_eq!(decoder.observation().phases.len(), MAX_PHASES);
        assert!(
            decoder
                .observation()
                .phases
                .windows(2)
                .all(|pair| pair[0].phase < pair[1].phase
                    && pair[0].elapsed_ms <= pair[1].elapsed_ms)
        );
        assert!(decoder.observation().report_completed);
        assert!(!decoder.observation().reported_failure);
        let mut wrong = ReportDecoder::new("c".repeat(64), "b".repeat(64), 42).unwrap();
        assert_eq!(wrong.feed(&bytes), Err("startup-report-binding"));
        let mut overflow = ReportDecoder::new("a".repeat(64), "b".repeat(64), 42).unwrap();
        assert_eq!(
            overflow.feed(&vec![b'x'; MAX_REPORT_BYTES + 1]),
            Err("startup-report-budget")
        );
    }
    #[test]
    fn failed_phase_is_retained_and_backward_clock_or_phase_is_rejected() {
        let sink = Sink(Arc::new(Mutex::new(vec![])));
        {
            let mut report = StartupReporter::with_writer(
                &"a".repeat(64),
                &"b".repeat(64),
                42,
                Box::new(sink.clone()),
            )
            .unwrap();
            report.enter(Phase::Restoration);
        }
        let mut decoder = ReportDecoder::new("a".repeat(64), "b".repeat(64), 42).unwrap();
        decoder.feed(&sink.0.lock().unwrap()).unwrap();
        assert!(decoder.observation().reported_failure);
        assert_eq!(decoder.observation().phase, Some(Phase::Restoration));
        assert_eq!(
            decoder.feed(b"{\"type\":\"phase\",\"phase\":\"database\",\"elapsed_ms\":0}\n"),
            Err("startup-report-order")
        );
    }
}
