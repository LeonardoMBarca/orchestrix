//! Bounded diagnostic facts about the existing Git read deadlines.
//! These values explain a guard; they never select, renew or enforce a budget.
use serde::Serialize;
use std::time::Duration;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub(crate) enum DeadlineBoundary {
    PreflightCheckpoint,
    FilesystemCheckpoint,
    BeforeCommand,
    CommandWait,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub(crate) enum LimitingBudget {
    Command,
    ObservationRemaining,
    Equal,
}

/// Compare the original durations, before rounding them for diagnostic output.
/// The caller retains its original budget decision and authoritative guards.
pub(crate) fn limiting_budget(
    command_limit: Duration,
    observation_remaining: Option<Duration>,
) -> LimitingBudget {
    match observation_remaining {
        None => LimitingBudget::Command,
        Some(remaining) => match command_limit.cmp(&remaining) {
            std::cmp::Ordering::Less => LimitingBudget::Command,
            std::cmp::Ordering::Greater => LimitingBudget::ObservationRemaining,
            std::cmp::Ordering::Equal => LimitingBudget::Equal,
        },
    }
}

#[derive(Clone, Copy, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct CommandDeadline {
    builtin: &'static str,
    command_limit_ms: u64,
    effective_limit_ms: u64,
    observation_remaining_at_selection_ms: Option<u64>,
    limiting_budget: LimitingBudget,
    transport_elapsed_ms: Option<u64>,
}

impl CommandDeadline {
    /// `effective_limit` is the value already selected by the executor. This
    /// constructor does not calculate a replacement deadline or read a clock.
    /// Only a fixed builtin label may be recorded, never its arguments or paths.
    pub(crate) fn new(
        builtin: &'static str,
        command_limit: Duration,
        effective_limit: Duration,
        observation_remaining_at_selection: Option<Duration>,
        transport_elapsed: Option<Duration>,
    ) -> Self {
        Self {
            builtin: bounded_builtin(builtin),
            command_limit_ms: milliseconds(command_limit),
            effective_limit_ms: milliseconds(effective_limit),
            observation_remaining_at_selection_ms: observation_remaining_at_selection
                .map(milliseconds),
            limiting_budget: limiting_budget(command_limit, observation_remaining_at_selection),
            transport_elapsed_ms: transport_elapsed.map(milliseconds),
        }
    }
}

fn milliseconds(duration: Duration) -> u64 {
    duration.as_millis().try_into().unwrap_or(u64::MAX)
}

fn bounded_builtin(builtin: &'static str) -> &'static str {
    match builtin {
        "--version" | "rev-parse" | "symbolic-ref" | "config" | "ls-files" | "ls-tree"
        | "cat-file" | "status" | "diff" | "worktree" | "branch" | "update-ref"
        | "for-each-ref" | "check-ref-format" | "check-attr" | "hash-object" | "show-ref"
        | "rev-list" | "merge-base" | "read-tree" | "update-index" => builtin,
        _ => "unsupported",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn command_and_observation_limits_remain_distinct_including_exact_ties() {
        let command = Duration::from_secs(15);
        assert_eq!(limiting_budget(command, None), LimitingBudget::Command);
        assert_eq!(
            limiting_budget(command, Some(Duration::from_secs(90))),
            LimitingBudget::Command
        );
        assert_eq!(
            limiting_budget(command, Some(Duration::from_secs(3))),
            LimitingBudget::ObservationRemaining
        );
        assert_eq!(
            limiting_budget(command, Some(command)),
            LimitingBudget::Equal
        );
        assert_eq!(
            limiting_budget(command, Some(Duration::ZERO)),
            LimitingBudget::ObservationRemaining
        );
        assert_eq!(
            limiting_budget(Duration::ZERO, Some(Duration::ZERO)),
            LimitingBudget::Equal
        );
    }

    #[test]
    fn classification_precedes_submillisecond_display_rounding() {
        let lower = Duration::from_nanos(100);
        let higher = Duration::from_nanos(900);
        let clipped = CommandDeadline::new("cat-file", higher, lower, Some(lower), None);
        let command = CommandDeadline::new("cat-file", lower, lower, Some(higher), None);
        assert_eq!(clipped.command_limit_ms, clipped.effective_limit_ms);
        assert_eq!(command.command_limit_ms, command.effective_limit_ms);
        assert_eq!(
            clipped.limiting_budget,
            LimitingBudget::ObservationRemaining
        );
        assert_eq!(command.limiting_budget, LimitingBudget::Command);
        assert_eq!(limiting_budget(lower, Some(lower)), LimitingBudget::Equal);
    }

    #[test]
    fn reported_effective_value_is_not_replaced_by_a_new_budget_decision() {
        let facts = CommandDeadline::new(
            "status",
            Duration::from_secs(15),
            Duration::from_secs(2),
            Some(Duration::from_secs(3)),
            Some(Duration::from_millis(2_001)),
        );
        // Facts retain the caller's already selected value even when it differs
        // from min(command, remaining). Logging cannot manufacture authority.
        let value = serde_json::to_value(facts).unwrap();
        assert_eq!(value["commandLimitMs"], 15_000);
        assert_eq!(value["effectiveLimitMs"], 2_000);
        assert_eq!(value["observationRemainingAtSelectionMs"], 3_000);
        assert_eq!(value["limitingBudget"], "observation-remaining");
        assert_eq!(value["transportElapsedMs"], 2_001);
    }

    #[test]
    fn diagnostic_payload_is_closed_and_does_not_retain_private_text() {
        let facts = CommandDeadline::new(
            "C:\\private-canary\\project --token=must-not-appear",
            Duration::MAX,
            Duration::from_secs(15),
            None,
            None,
        );
        let value = serde_json::to_value(facts).unwrap();
        assert_eq!(value["builtin"], "unsupported");
        assert_eq!(value["commandLimitMs"], u64::MAX);
        assert!(value["observationRemainingAtSelectionMs"].is_null());
        assert!(value["transportElapsedMs"].is_null());
        let text = serde_json::to_string(&facts).unwrap();
        assert!(text.len() < 512);
        assert!(!text.contains("private-canary"));
        assert!(!text.contains("must-not-appear"));
        assert_eq!(value.as_object().unwrap().len(), 6);
    }

    #[test]
    fn boundary_labels_are_fixed_and_contain_no_location_data() {
        for (boundary, label) in [
            (
                DeadlineBoundary::PreflightCheckpoint,
                "preflight-checkpoint",
            ),
            (
                DeadlineBoundary::FilesystemCheckpoint,
                "filesystem-checkpoint",
            ),
            (DeadlineBoundary::BeforeCommand, "before-command"),
            (DeadlineBoundary::CommandWait, "command-wait"),
        ] {
            assert_eq!(serde_json::to_value(boundary).unwrap(), label);
        }
    }
}
