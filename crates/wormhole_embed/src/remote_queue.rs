//! Consume `warp-remote-queue.json` when Warp runs embedded in Wormhole desktop.

use std::ffi::OsStr;
#[cfg(unix)]
use std::os::unix::fs::PermissionsExt;
use std::path::Path;

use agent_core::{
    append_task_event, mark_task_completed, mark_task_failed, mark_task_running,
    shell_escape_prompt, take_next_queue_item, RemoteAgentTaskEvent, TaskEventLevel,
    WarpAgentKind, WarpRemoteQueueItem,
};

pub use agent_core::warp_remote_queue;

pub fn poll_remote_queue(data_dir: &Path) -> Option<RemoteQueueDispatch> {
    poll_remote_queue_with_path(
        data_dir,
        std::env::var_os("PATH").as_deref(),
        std::env::var_os("PATHEXT").as_deref(),
    )
}

fn poll_remote_queue_with_path(
    data_dir: &Path,
    path_var: Option<&OsStr>,
    path_ext: Option<&OsStr>,
) -> Option<RemoteQueueDispatch> {
    let item = take_next_queue_item(data_dir)?;
    if let Some(message) = unavailable_agent_message_for_path(item.agent, path_var, path_ext) {
        let _ = mark_task_failed(data_dir, &item.id, &message);
        return None;
    }
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

fn unavailable_agent_message_for_path(
    agent: WarpAgentKind,
    path_var: Option<&OsStr>,
    path_ext: Option<&OsStr>,
) -> Option<String> {
    match agent {
        WarpAgentKind::Codex => None,
        WarpAgentKind::Cursor => cursor_agent_error_for_path(path_var, path_ext),
    }
}

fn cursor_agent_error_for_path(
    path_var: Option<&OsStr>,
    path_ext: Option<&OsStr>,
) -> Option<String> {
    if executable_on_path("agent", path_var, path_ext) {
        return None;
    }
    Some("Cursor agent is unavailable: `agent` was not found on PATH.".to_string())
}

fn executable_on_path(command: &str, path_var: Option<&OsStr>, path_ext: Option<&OsStr>) -> bool {
    let Some(path_var) = path_var else {
        return false;
    };
    let candidates = executable_candidates(command, path_ext);
    std::env::split_paths(path_var).any(|dir| {
        candidates.iter().any(|candidate| {
            let path = dir.join(candidate);
            is_executable_file(&path)
        })
    })
}

fn is_executable_file(path: &Path) -> bool {
    let Ok(metadata) = path.metadata() else {
        return false;
    };
    if !metadata.is_file() {
        return false;
    }
    #[cfg(unix)]
    {
        metadata.permissions().mode() & 0o111 != 0
    }
    #[cfg(not(unix))]
    {
        true
    }
}

fn executable_candidates(command: &str, path_ext: Option<&OsStr>) -> Vec<String> {
    if cfg!(windows) {
        let mut candidates = vec![command.to_string()];
        if let Some(path_ext) = path_ext.and_then(OsStr::to_str) {
            for ext in path_ext.split(';').filter(|ext| !ext.is_empty()) {
                candidates.push(format!("{command}{ext}"));
            }
        }
        candidates
    } else {
        vec![command.to_string()]
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
    use std::fs;
    use tempfile::tempdir;

    #[test]
    fn remote_queue_launch_command_escapes_prompt() {
        let cmd = launch_command_for(WarpAgentKind::Cursor, "it's fine");
        assert!(cmd.contains("agent -p"));
        assert!(cmd.contains("'it'\\''s fine'"));
    }

    #[test]
    fn poll_remote_queue_returns_next_item() {
        let dir = tempdir().unwrap();
        enqueue_prompt(dir.path(), "t1", WarpAgentKind::Codex, "hello").unwrap();
        let dispatch = poll_remote_queue(dir.path()).expect("queue item");
        assert_eq!(dispatch.item.id, "t1");
        assert!(dispatch.command.contains("exec --full-auto"));
    }

    #[test]
    fn cursor_agent_error_reports_missing_agent() {
        let dir = tempdir().unwrap();
        let err = cursor_agent_error_for_path(Some(dir.path().as_os_str()), None)
            .expect("missing agent should be reported");
        assert!(err.contains("agent"));
        assert!(err.contains("PATH"));
    }

    #[test]
    fn cursor_agent_error_accepts_agent_on_path() {
        let dir = tempdir().unwrap();
        let agent_path = dir.path().join("agent");
        fs::write(&agent_path, "").unwrap();
        #[cfg(unix)]
        {
            let mut permissions = fs::metadata(&agent_path).unwrap().permissions();
            permissions.set_mode(0o755);
            fs::set_permissions(&agent_path, permissions).unwrap();
        }
        assert!(cursor_agent_error_for_path(Some(dir.path().as_os_str()), None).is_none());
    }

    #[test]
    fn poll_remote_queue_marks_missing_cursor_agent_failed_once() {
        let queue_dir = tempdir().unwrap();
        let empty_path = tempdir().unwrap();
        enqueue_prompt(
            queue_dir.path(),
            "cursor-missing",
            WarpAgentKind::Cursor,
            "run",
        )
        .unwrap();

        let dispatch = poll_remote_queue_with_path(
            queue_dir.path(),
            Some(empty_path.path().as_os_str()),
            None,
        );
        let state = agent_core::load_task_state(queue_dir.path(), "cursor-missing").unwrap();
        let (events, _) = agent_core::read_task_events_page(queue_dir.path(), "cursor-missing", 0);
        let error_events = events
            .iter()
            .filter(|event| event.message.contains("agent"))
            .count();

        assert!(dispatch.is_none());
        assert_eq!(state.status, agent_core::TaskStatus::Failed);
        assert_eq!(error_events, 1);
    }
}
