use super::{CancellationToken, RuntimeError, RuntimeResult};
use orchestrix_process::{OwnedProcess, ProcessEvent};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use std::collections::VecDeque;
use std::time::{Duration, Instant};

/// Closed local routes/reasons: neither names nor content supplied by the
/// provider can become diagnostic text. This stays outside normalized events.
pub(super) enum DiagnosticStage {
    Framing,
    RpcResponse,
    Configuration,
    Admission,
    Notification,
    Observer,
    Attempt,
}
pub(super) enum DiagnosticReason {
    InvalidJson,
    InvalidEnvelope,
    FrameBudget,
    PartialFrame,
    ResponseIdMismatch,
    MissingResult,
    MissingMethod,
    UnexpectedResponse,
    UnsupportedGlobal,
    ThreadScope,
    TurnScope,
    ThreadStartedScope,
    GlobalScope,
    WarningScope,
    McpMetadataScope,
    AuthenticationChanged,
    DeltaType,
    OutputBudget,
    TerminalStatus,
    InvalidTurnId,
    ConfigurationRejected,
    ObservationRejected,
    OperationFailed,
}
pub(super) fn diagnosed(
    error: RuntimeError,
    stage: DiagnosticStage,
    reason: DiagnosticReason,
    message: Option<&Value>,
) -> RuntimeError {
    error.with_adapter_diagnostic(diagnostic(stage, reason, message))
}
pub(super) fn diagnostic(
    stage: DiagnosticStage,
    reason: DiagnosticReason,
    message: Option<&Value>,
) -> Value {
    use DiagnosticReason as R;
    use DiagnosticStage as S;
    let stage = match stage {
        S::Framing => "framing",
        S::RpcResponse => "rpc-response",
        S::Configuration => "configuration",
        S::Admission => "admission",
        S::Notification => "notification",
        S::Observer => "observer",
        S::Attempt => "attempt",
    };
    let reason = match reason {
        R::InvalidJson => "invalid-json",
        R::InvalidEnvelope => "invalid-envelope",
        R::FrameBudget => "frame-budget",
        R::PartialFrame => "partial-frame",
        R::ResponseIdMismatch => "response-id-mismatch",
        R::MissingResult => "missing-result",
        R::MissingMethod => "missing-method",
        R::UnexpectedResponse => "unexpected-response",
        R::UnsupportedGlobal => "unsupported-global",
        R::ThreadScope => "thread-scope",
        R::TurnScope => "turn-scope",
        R::ThreadStartedScope => "thread-started-scope",
        R::GlobalScope => "global-scope",
        R::WarningScope => "warning-scope",
        R::McpMetadataScope => "mcp-metadata-scope",
        R::AuthenticationChanged => "authentication-changed",
        R::DeltaType => "delta-type",
        R::OutputBudget => "output-budget",
        R::TerminalStatus => "terminal-status",
        R::InvalidTurnId => "invalid-turn-id",
        R::ConfigurationRejected => "configuration-rejected",
        R::ObservationRejected => "observation-rejected",
        R::OperationFailed => "operation-failed",
    };
    let params = message.and_then(|message| message.get("params"));
    let nested = |key: &str| params.and_then(|params| params.get(key));
    let method_sha256 = message
        .and_then(|message| message.get("method"))
        .and_then(Value::as_str)
        .map(|method| format!("{:x}", Sha256::digest(method.as_bytes())));
    json!({
        "version":1,"stage":stage,"reason":reason,"methodSha256":method_sha256,
        "idShape":shape(message.and_then(|message| message.get("id"))),
        "paramsShape":shape(params),
        "identityShapes":{
            "threadId":shape(nested("threadId")),
            "thread.id":shape(nested("thread").and_then(|thread| thread.get("id"))),
            "turnId":shape(nested("turnId")),
            "turn.id":shape(nested("turn").and_then(|turn| turn.get("id")))
        }
    })
}
fn shape(value: Option<&Value>) -> &'static str {
    match value {
        None => "absent",
        Some(Value::Null) => "null",
        Some(Value::Bool(_)) => "bool",
        Some(Value::Number(_)) => "number",
        Some(Value::String(_)) => "string",
        Some(Value::Array(_)) => "array",
        Some(Value::Object(_)) => "object",
    }
}

pub(super) struct Rpc {
    pub process: OwnedProcess,
    pending_bytes: Vec<u8>,
    messages: VecDeque<Value>,
    pub notifications: VecDeque<Value>,
    next_id: u64,
    frames: usize,
    pub stderr_bytes: usize,
    pub turn_submitted: bool,
    pub turn_dispatch: &'static str,
}
impl Rpc {
    pub fn new(process: OwnedProcess) -> Self {
        Self {
            process,
            pending_bytes: Vec::new(),
            messages: VecDeque::new(),
            notifications: VecDeque::new(),
            next_id: 0,
            frames: 0,
            stderr_bytes: 0,
            turn_submitted: false,
            turn_dispatch: "not-sent",
        }
    }
    pub fn notify(&self, method: &str, params: Value) -> RuntimeResult<()> {
        self.send(&json!({"method":method,"params":params}))
    }
    pub fn send(&self, value: &Value) -> RuntimeResult<()> {
        let mut bytes =
            serde_json::to_vec(value).map_err(|_| RuntimeError::new("protocol", "not-sent"))?;
        if bytes.len() > 65_535 {
            return Err(RuntimeError::new("budget", "not-sent"));
        }
        bytes.push(b'\n');
        self.process
            .send(&bytes, Duration::from_secs(5))
            .map_err(Into::into)
    }
    pub fn request(
        &mut self,
        method: &str,
        params: Value,
        timeout: Duration,
        cancel: &CancellationToken,
    ) -> RuntimeResult<Value> {
        self.next_id += 1;
        let id = self.next_id;
        let begin = Instant::now();
        if cancel.is_cancelled() {
            return Err(RuntimeError::new("policy", "not-sent"));
        }
        if method == "turn/start" {
            // A bounded write can time out after some/all bytes reached the child.
            // Mark uncertainty before handing bytes to the writer; never retry it.
            self.turn_submitted = true;
            self.turn_dispatch = "unknown";
        }
        self.send(&json!({"id":id,"method":method,"params":params}))?;
        if method == "turn/start" {
            self.turn_dispatch = "sent";
        }
        loop {
            if cancel.is_cancelled() {
                return Err(RuntimeError::new("timeout", "unknown"));
            }
            if begin.elapsed() >= timeout {
                return Err(RuntimeError::new("timeout", "unknown"));
            }
            let Some(message) = self.next(Duration::from_millis(5))? else {
                continue;
            };
            if message.get("method").is_some() {
                if message.get("id").is_some() {
                    self.deny_request(&message)?;
                } else {
                    if self.notifications.len() >= 128 {
                        return Err(RuntimeError::new("budget", "unknown"));
                    }
                    self.notifications.push_back(message);
                }
                continue;
            }
            if message["id"] != id {
                return Err(diagnosed(
                    RuntimeError::new("protocol", "unknown"),
                    DiagnosticStage::RpcResponse,
                    DiagnosticReason::ResponseIdMismatch,
                    Some(&message),
                ));
            }
            if message.get("error").is_some() {
                let tag = message["error"]["data"]["type"].as_str();
                return Err(RuntimeError::new(
                    match tag {
                        Some("authentication") => "authentication",
                        Some("rate_limit") => "rate-limit",
                        _ => "unavailable",
                    },
                    "sent",
                ));
            }
            return message.get("result").cloned().ok_or_else(|| {
                diagnosed(
                    RuntimeError::new("protocol", "unknown"),
                    DiagnosticStage::RpcResponse,
                    DiagnosticReason::MissingResult,
                    Some(&message),
                )
            });
        }
    }
    pub fn deny_request(&self, message: &Value) -> RuntimeResult<()> {
        let id = message
            .get("id")
            .ok_or_else(|| RuntimeError::new("protocol", "unknown"))?;
        let response = match message["method"].as_str() {
            Some("item/commandExecution/requestApproval" | "item/fileChange/requestApproval") => {
                json!({"id":id,"result":{"decision":"decline"}})
            }
            Some("item/permissions/requestApproval") => {
                json!({"id":id,"result":{"permissions":{},"scope":"turn"}})
            }
            _ => json!({"id":id,"error":{"code":-32601,"message":"Request unavailable"}}),
        };
        self.send(&response)
    }
    pub fn next(&mut self, timeout: Duration) -> RuntimeResult<Option<Value>> {
        if let Some(message) = self.messages.pop_front() {
            return Ok(Some(message));
        }
        // Drain ready chunks before interpreting EOF/root exit. A stderr chunk
        // or a split JSON frame is not proof that the stdout queue is empty.
        // After the first bounded wait, subsequent reads never wait at all.
        let mut queue_empty = false;
        for index in 0..128 {
            let Some(event) =
                self.process
                    .receive(if index == 0 { timeout } else { Duration::ZERO })?
            else {
                queue_empty = true;
                break;
            };
            match event {
                ProcessEvent::Stderr(bytes) => {
                    self.stderr_bytes = self.stderr_bytes.saturating_add(bytes.len())
                }
                ProcessEvent::Stdout(bytes) => {
                    self.pending_bytes.extend(bytes);
                    while let Some(end) = self.pending_bytes.iter().position(|byte| *byte == b'\n')
                    {
                        if end > 65_536 || self.messages.len() >= 128 || self.frames >= 4096 {
                            return Err(diagnosed(
                                RuntimeError::new("budget", "unknown"),
                                DiagnosticStage::Framing,
                                DiagnosticReason::FrameBudget,
                                None,
                            ));
                        }
                        let frame: Vec<_> = self.pending_bytes.drain(..=end).collect();
                        let message: Value = serde_json::from_slice(&frame).map_err(|_| {
                            diagnosed(
                                RuntimeError::new("protocol", "unknown"),
                                DiagnosticStage::Framing,
                                DiagnosticReason::InvalidJson,
                                None,
                            )
                        })?;
                        if !message.is_object() || message.get("jsonrpc").is_some() {
                            return Err(diagnosed(
                                RuntimeError::new("protocol", "unknown"),
                                DiagnosticStage::Framing,
                                DiagnosticReason::InvalidEnvelope,
                                Some(&message),
                            ));
                        }
                        self.frames += 1;
                        self.messages.push_back(message);
                    }
                    if self.pending_bytes.len() > 65_536 {
                        return Err(diagnosed(
                            RuntimeError::new("budget", "unknown"),
                            DiagnosticStage::Framing,
                            DiagnosticReason::FrameBudget,
                            None,
                        ));
                    }
                }
            }
            if let Some(message) = self.messages.pop_front() {
                return Ok(Some(message));
            }
        }
        if !queue_empty {
            return Ok(None);
        }
        let streams = self.process.streams();
        if streams.stdout_closed && !self.pending_bytes.is_empty() {
            return Err(diagnosed(
                RuntimeError::new("protocol", "unknown"),
                DiagnosticStage::Framing,
                DiagnosticReason::PartialFrame,
                None,
            ));
        }
        if streams.stdout_closed && self.process.tree()?.root_exit_code.is_some() {
            return Err(RuntimeError::new(
                "crash",
                if self.turn_submitted {
                    "unknown"
                } else {
                    "not-sent"
                },
            ));
        }
        Ok(None)
    }
}
