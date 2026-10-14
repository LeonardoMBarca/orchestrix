//! Bounded, deterministic sanitization at the persistence boundary.
//! This is a supported-pattern policy, not a universal secret detector.
use serde_json::{Map, Value};
use std::fmt;

const MAX_TEXT_BYTES: usize = 1_048_576;
const MAX_NODES: usize = 4096;
const MAX_DEPTH: usize = 32;
pub const REDACTED: &str = "[REDACTED]";
pub const OMITTED: &str = "[OMITTED]";

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum PrivacyError {
    Limit,
}
impl fmt::Display for PrivacyError {
    fn fmt(&self, out: &mut fmt::Formatter<'_>) -> fmt::Result {
        out.write_str("Persistence privacy policy rejected input: limit")
    }
}
impl std::error::Error for PrivacyError {}

fn normalized_key(key: &str) -> String {
    key.chars()
        .filter(char::is_ascii_alphanumeric)
        .flat_map(char::to_lowercase)
        .collect()
}
fn secret_key(key: &str) -> bool {
    matches!(
        normalized_key(key).as_str(),
        "apikey"
            | "key"
            | "authorization"
            | "accesstoken"
            | "refreshtoken"
            | "idtoken"
            | "token"
            | "secret"
            | "clientsecret"
            | "password"
            | "credential"
            | "credentials"
            | "cookie"
            | "setcookie"
            | "privatekey"
            | "connectionstring"
            | "awssecretaccesskey"
            | "awsaccesskeyid"
            | "sessiontoken"
            | "awssessiontoken"
    )
}
fn transcript_key(key: &str) -> bool {
    matches!(
        normalized_key(key).as_str(),
        "transcript"
            | "transcripts"
            | "rawtranscript"
            | "providerhistory"
            | "providertranscript"
            | "runtimehistory"
            | "rawmessages"
            | "conversationhistory"
    )
}
fn token_end(bytes: &[u8], start: usize) -> usize {
    let mut end = start;
    while end < bytes.len()
        && (bytes[end].is_ascii_alphanumeric() || b"-_.+/=".contains(&bytes[end]))
    {
        end += 1;
    }
    end
}
fn known_token(bytes: &[u8], start: usize) -> Option<usize> {
    let tail = &bytes[start..];
    let prefixed = [
        b"sk-".as_slice(),
        b"ghp_",
        b"gho_",
        b"github_pat_",
        b"AIza",
        b"AKIA",
        b"ASIA",
    ]
    .iter()
    .any(|prefix| tail.starts_with(prefix));
    if !prefixed && !tail.starts_with(b"eyJ") {
        return None;
    }
    let end = token_end(bytes, start);
    let length = end - start;
    let jwt = tail.starts_with(b"eyJ")
        && bytes[start..end]
            .iter()
            .filter(|byte| **byte == b'.')
            .count()
            == 2;
    if (prefixed && length >= 12) || (jwt && length >= 24) {
        Some(end)
    } else {
        None
    }
}

/// Redacts known token forms, Bearer credentials and credential assignments.
/// Private-key material causes the entire text to be omitted conservatively.
pub fn sanitize_text(text: &str) -> Result<String, PrivacyError> {
    if text.len() > MAX_TEXT_BYTES {
        return Err(PrivacyError::Limit);
    }
    if text.contains("-----BEGIN") && text.contains("PRIVATE KEY-----") {
        return Ok(REDACTED.to_owned());
    }
    let bytes = text.as_bytes();
    let mut output = String::with_capacity(text.len());
    let mut index = 0;
    while index < bytes.len() {
        if let Some(end) = known_token(bytes, index) {
            output.push_str(REDACTED);
            index = end;
            continue;
        }
        if bytes[index..].len() >= 7 && bytes[index..index + 7].eq_ignore_ascii_case(b"bearer ") {
            output.push_str("Bearer ");
            index += 7;
            while index < bytes.len() && bytes[index].is_ascii_whitespace() {
                index += 1;
            }
            let end = token_end(bytes, index);
            if end > index {
                output.push_str(REDACTED);
                index = end;
            }
            continue;
        }
        if bytes[index].is_ascii_alphabetic() {
            let start = index;
            while index < bytes.len()
                && (bytes[index].is_ascii_alphanumeric() || b"_-".contains(&bytes[index]))
            {
                index += 1;
            }
            let name = &text[start..index];
            let mut separator = index;
            while separator < bytes.len() && bytes[separator].is_ascii_whitespace() {
                separator += 1;
            }
            if secret_key(name) && separator < bytes.len() && b"=:".contains(&bytes[separator]) {
                output.push_str(name);
                output.push_str(&text[index..=separator]);
                index = separator + 1;
                while index < bytes.len() && bytes[index].is_ascii_whitespace() {
                    output.push(bytes[index] as char);
                    index += 1;
                }
                if index < bytes.len() && b"\"'".contains(&bytes[index]) {
                    let quote = bytes[index];
                    index += 1;
                    while index < bytes.len() && bytes[index] != quote {
                        if bytes[index] == b'\\' && index + 1 < bytes.len() {
                            index += 1;
                        }
                        index += 1;
                    }
                    if index < bytes.len() {
                        index += 1;
                    }
                } else if normalized_key(name) == "authorization" {
                    while index < bytes.len() && !b"\r\n,;}".contains(&bytes[index]) {
                        index += 1;
                    }
                } else {
                    while index < bytes.len()
                        && !bytes[index].is_ascii_whitespace()
                        && !b",;}".contains(&bytes[index])
                    {
                        index += 1;
                    }
                }
                output.push_str(REDACTED);
            } else {
                output.push_str(name);
            }
            continue;
        }
        let character = text[index..].chars().next().expect("character boundary");
        output.push(character);
        index += character.len_utf8();
    }
    Ok(output)
}

/// Recursively sanitizes text and secret fields. Provider transcripts are omitted.
pub fn sanitize_json(value: &Value) -> Result<Value, PrivacyError> {
    sanitize_with_limits(value, MAX_DEPTH, MAX_NODES, MAX_TEXT_BYTES)
}

/// Sanitizes a validated aggregate checkpoint, whose observer journals and
/// immutable history can legitimately exceed a single command/document budget.
/// The input must come from the host's validated schema, never raw provider data.
/// Individual texts still have the same 1 MiB limit; no history is truncated.
pub fn sanitize_checkpoint(value: &Value) -> Result<Value, PrivacyError> {
    sanitize_with_limits(value, 128, 1_000_000, 64 * MAX_TEXT_BYTES)
}

fn sanitize_with_limits(
    value: &Value,
    max_depth: usize,
    max_nodes: usize,
    max_bytes: usize,
) -> Result<Value, PrivacyError> {
    struct Limits {
        depth: usize,
        nodes: usize,
        bytes: usize,
    }
    fn visit(
        value: &Value,
        depth: usize,
        nodes: &mut usize,
        bytes: &mut usize,
        limits: &Limits,
    ) -> Result<Value, PrivacyError> {
        *nodes += 1;
        if depth > limits.depth || *nodes > limits.nodes {
            return Err(PrivacyError::Limit);
        }
        match value {
            Value::String(text) => {
                *bytes = bytes.checked_add(text.len()).ok_or(PrivacyError::Limit)?;
                if *bytes > limits.bytes {
                    return Err(PrivacyError::Limit);
                }
                Ok(Value::String(sanitize_text(text)?))
            }
            Value::Array(values) => values
                .iter()
                .map(|value| visit(value, depth + 1, nodes, bytes, limits))
                .collect(),
            Value::Object(values) => {
                let mut result = Map::new();
                for (key, value) in values {
                    *bytes = bytes.checked_add(key.len()).ok_or(PrivacyError::Limit)?;
                    if *bytes > limits.bytes {
                        return Err(PrivacyError::Limit);
                    }
                    let cleaned_key = sanitize_text(key)?;
                    if result.contains_key(&cleaned_key) {
                        return Err(PrivacyError::Limit);
                    }
                    let cleaned = if transcript_key(key) {
                        Value::String(OMITTED.to_owned())
                    } else if secret_key(key) {
                        Value::String(REDACTED.to_owned())
                    } else {
                        visit(value, depth + 1, nodes, bytes, limits)?
                    };
                    result.insert(cleaned_key, cleaned);
                }
                Ok(Value::Object(result))
            }
            other => Ok(other.clone()),
        }
    }
    let sanitized = visit(
        value,
        0,
        &mut 0,
        &mut 0,
        &Limits {
            depth: max_depth,
            nodes: max_nodes,
            bytes: max_bytes,
        },
    )?;
    // Include JSON escaping, numbers and structural bytes in the total budget.
    if serde_json::to_vec(&sanitized)
        .map_err(|_| PrivacyError::Limit)?
        .len()
        > max_bytes
    {
        return Err(PrivacyError::Limit);
    }
    Ok(sanitized)
}
