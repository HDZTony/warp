//! Consume `warp-remote-queue.json` when Warp runs embedded in Wormhole desktop.

use std::path::Path;

use agent_core::{WarpAgentKind, WarpRemoteQueueItem, mark_task_completed, mark_task_failed,
                 mark_task_running, shell_escape_prompt, take_next_queue_item};

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

pub fn acknowledge_success(data_dir: &Path, task_id: &str, command: &str) {
    let _ = mark_task_running(data_dir, task_id);
    let _ = agent_core::append_transcript_event(data_dir, task_id, "stdout", command);
    let _ = mark_task_completed(data_dir, task_id, Some("dispatched to Warp terminal"));
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
        assert!(cmd.contains("'it's fine'"));
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
