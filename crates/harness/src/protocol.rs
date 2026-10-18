use std::collections::HashSet;
use std::io::Write;
use std::path::PathBuf;
use std::sync::{Mutex, OnceLock};

use serde_json::Value;

static LOG: OnceLock<PathBuf> = OnceLock::new();
static SEEN: Mutex<Option<HashSet<String>>> = Mutex::new(None);

// claude 2.1.301: the Agent SDK's `StdoutMessage`, system messages by subtype
const CLAUDE: &[&str] = &[
    "active_goal",
    "assistant",
    "auth_status",
    "control_cancel_request",
    "control_request",
    "control_response",
    "conversation_reset",
    "keep_alive",
    "prompt_suggestion",
    "rate_limit_event",
    "result",
    "stream_event",
    "system/api_retry",
    "system/background_tasks_changed",
    "system/commands_changed",
    "system/compact_boundary",
    "system/control_request_progress",
    "system/elicitation_complete",
    "system/files_persisted",
    "system/hook_progress",
    "system/hook_response",
    "system/hook_started",
    "system/informational",
    "system/init",
    "system/local_command_output",
    "system/memory_recall",
    "system/mirror_error",
    "system/model_refusal_fallback",
    "system/model_refusal_no_fallback",
    "system/notification",
    "system/permission_denied",
    "system/plugin_install",
    "system/session_state_changed",
    "system/status",
    "system/task_notification",
    "system/task_progress",
    "system/task_started",
    "system/task_updated",
    "system/thinking_tokens",
    "system/worker_shutting_down",
    "tool_progress",
    "tool_use_summary",
    "user",
];

// codex 0.162.1: `codex app-server generate-json-schema`, ServerNotification and ServerRequest
const CODEX: &[&str] = &[
    "account/chatgptAuthTokens/refresh",
    "account/gatewayOAuth/changed",
    "account/login/completed",
    "account/rateLimits/updated",
    "account/updated",
    "app/list/updated",
    "applyPatchApproval",
    "attestation/generate",
    "autoApprovalReview/strictReviewRequired",
    "command/exec/outputDelta",
    "configWarning",
    "deprecationNotice",
    "error",
    "execCommandApproval",
    "externalAgentConfig/import/completed",
    "externalAgentConfig/import/progress",
    "fs/changed",
    "fuzzyFileSearch/sessionCompleted",
    "fuzzyFileSearch/sessionUpdated",
    "guardianWarning",
    "hook/completed",
    "hook/started",
    "item/agentMessage/delta",
    "item/autoApprovalReview/completed",
    "item/autoApprovalReview/started",
    "item/commandExecution/outputDelta",
    "item/commandExecution/requestApproval",
    "item/commandExecution/terminalInteraction",
    "item/completed",
    "item/fileChange/outputDelta",
    "item/fileChange/patchUpdated",
    "item/fileChange/requestApproval",
    "item/mcpToolCall/progress",
    "item/permissions/requestApproval",
    "item/plan/delta",
    "item/reasoning/summaryPartAdded",
    "item/reasoning/summaryTextDelta",
    "item/reasoning/textDelta",
    "item/started",
    "item/tool/call",
    "item/tool/requestUserInput",
    "mcpServer/elicitation/request",
    "mcpServer/event/stream/notification",
    "mcpServer/oauthLogin/completed",
    "mcpServer/startupStatus/updated",
    "model/rerouted",
    "model/safetyBuffering/updated",
    "model/verification",
    "modelProvider/authRecoveryCompleted",
    "modelProvider/authRecoveryStarted",
    "process/exited",
    "process/outputDelta",
    "project/changed",
    "remoteControl/status/changed",
    "serverRequest/resolved",
    "skills/changed",
    "thread/archived",
    "thread/attachment/updated",
    "thread/closed",
    "thread/compacted",
    "thread/deleted",
    "thread/environment/connected",
    "thread/environment/disconnected",
    "thread/goal/cleared",
    "thread/goal/updated",
    "thread/name/updated",
    "thread/prediction/updated",
    "thread/project/updated",
    "thread/queue/changed",
    "thread/realtime/closed",
    "thread/realtime/error",
    "thread/realtime/item/completed",
    "thread/realtime/item/started",
    "thread/realtime/item/transcript/delta",
    "thread/realtime/itemAdded",
    "thread/realtime/outputAudio/delta",
    "thread/realtime/sdp",
    "thread/realtime/started",
    "thread/realtime/transcript/delta",
    "thread/realtime/transcript/done",
    "thread/reverted",
    "thread/settings/updated",
    "thread/started",
    "thread/status/changed",
    "thread/tokenUsage/updated",
    "thread/unarchived",
    "turn/completed",
    "turn/diff/updated",
    "turn/moderationMetadata",
    "turn/plan/updated",
    "turn/started",
    "warning",
    "windows/worldWritableWarning",
    "windowsSandbox/setupCompleted",
];

/// Names each message a harness doesn't know in `path`, once per name while the app runs.
pub fn log_to(path: PathBuf) {
    let _ = LOG.set(path);
}

fn claude_name(v: &Value) -> String {
    let kind = v["type"].as_str().unwrap_or("(no type)");
    match v["subtype"].as_str() {
        Some(sub) if kind == "system" => format!("system/{sub}"),
        _ => kind.to_string(),
    }
}

pub(crate) fn claude(v: &Value) {
    let name = claude_name(v);
    if !CLAUDE.contains(&name.as_str()) {
        unknown("claude", &name);
    }
}

pub(crate) fn codex(method: &str) {
    if !CODEX.contains(&method) {
        unknown("codex", method);
    }
}

fn unknown(agent: &str, name: &str) {
    let Some(path) = LOG.get() else {
        return;
    };
    let line = format!("{agent} {name}");
    let mut seen = SEEN.lock().unwrap_or_else(|e| e.into_inner());
    if !seen.get_or_insert_default().insert(line.clone()) {
        return;
    }
    if std::fs::metadata(path).is_ok_and(|m| m.len() > 64_000) {
        let _ = std::fs::remove_file(path);
    }
    if let Ok(mut f) = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(path)
    {
        let _ = writeln!(f, "{line}");
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn names_system_messages_by_subtype() {
        assert_eq!(
            claude_name(&json!({"type": "system", "subtype": "init"})),
            "system/init"
        );
        assert_eq!(
            claude_name(&json!({"type": "result", "subtype": "success"})),
            "result"
        );
        assert!(CLAUDE.contains(&"system/init"));
        assert!(
            !CLAUDE.contains(&claude_name(&json!({"type": "system", "subtype": "new"})).as_str())
        );
    }
}
