//! Consume `warp-remote-queue.json` when Warp runs embedded in Wormhole desktop.

use std::path::Path;

use agent_core::{
    RemoteAgentTaskEvent, TaskEventLevel, WarpAgentKind, WarpRemoteQueueItem, append_task_event,
    mark_task_completed, mark_task_failed, mark_task_running, shell_escape_prompt,
    take_next_queue_item,
};

pub use agent_core::warp_remote_queue;

pub fn poll_remote_queue(data_dir: &Path) -> Option<RemoteQueueDispatch> {
    let item = take_next_queue_item(data_dir)?;
    let command = launch_command_for_item(&item);
    Some(RemoteQueueDispatch { item, command })
}

pub struct RemoteQueueDispatch {
    pub item: WarpRemoteQueueItem,
    pub command: String,
}

pub fn launch_command_for_item(item: &WarpRemoteQueueItem) -> String {
    launch_command_for(item.agent, &item.prompt)
}

pub fn launch_command_for(agent: WarpAgentKind, prompt: &str) -> String {
    let escaped = shell_escape_prompt(prompt);
    match agent {
        WarpAgentKind::Codex => {
            let profile = crate::codex_profile();
            if let Ok(bin) = std::env::var("WORMHOLE_CODEX_BIN") {
                let bin = bin.trim();
                if !bin.is_empty() {
                    return format!("\"{bin}\" --profile {profile} exec --full-auto {escaped}");
                }
            }
            format!("codex --profile {profile} exec --full-auto {escaped}")
        }
        WarpAgentKind::Cursor => format!("agent -p {escaped}"),
    }
}

pub fn acknowledge_success(data_dir: &Path, item: &WarpRemoteQueueItem, command: &str) {
    let _ = mark_task_running(data_dir, &item.id);
    let _ = agent_core::append_transcript_event(data_dir, &item.id, "stdout", command);
    record_estimated_token_usage(data_dir, item, command);
    let _ = mark_task_completed(data_dir, &item.id, Some("dispatched to Warp terminal"));
}

fn record_estimated_token_usage(data_dir: &Path, item: &WarpRemoteQueueItem, command: &str) {
    let input_tokens = estimate_tokens(&item.prompt);
    let output_tokens = estimate_tokens(command);
    let event = RemoteAgentTaskEvent {
        task_id: item.id.clone(),
        timestamp: std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(0),
        level: TaskEventLevel::Info,
        message: "token_usage".into(),
        details: serde_json::json!({
            "agent": item.agent.as_str(),
            "usage": {
                "input_tokens": input_tokens,
                "output_tokens": output_tokens,
                "total_tokens": input_tokens + output_tokens,
            },
            "estimated": true,
        }),
    };
    let _ = append_task_event(data_dir, &item.id, &event);
}

fn estimate_tokens(text: &str) -> u64 {
    let chars = text.chars().count() as u64;
    (chars + 3) / 4
}

pub fn acknowledge_failure(data_dir: &Path, task_id: &str, message: &str) {
    let _ = mark_task_failed(data_dir, task_id, message);
}

#[cfg(test)]
mod tests {
    use super::*;
    use agent_core::enqueue_prompt;
    use tempfile::tempdir;

    #[test]
    fn remote_queue_launch_command_escapes_prompt() {
        let cmd = launch_command_for(WarpAgentKind::Cursor, "it's fine");
        assert!(cmd.contains("agent -p"));
        assert!(cmd.contains("it"));
        assert!(cmd.contains("s fine"));
    }

    #[test]
    fn poll_remote_queue_returns_next_item() {
        let dir = tempdir().unwrap();
        enqueue_prompt(dir.path(), "t1", WarpAgentKind::Codex, "hello").unwrap();
        let dispatch = poll_remote_queue(dir.path()).expect("queue item");
        assert_eq!(dispatch.item.id, "t1");
        assert!(dispatch.command.contains("exec --full-auto"));
    }
}
