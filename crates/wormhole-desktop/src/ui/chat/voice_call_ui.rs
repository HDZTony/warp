//! Shared chat voice/video call UI helpers (header, profile panel, incoming banner, calls panel).

use crate::ui::chat::shell_state::SharedChatShellState;
use crate::ui::core_handle::CoreHandle;
use crate::ui::panel_primitives::StatusTone;
use wormhole_desktop_core::call_history::{
    call_history_put, expires_at_from_end, new_call_id, CallHistoryDirection, CallHistoryKind,
    CallHistoryParticipant, CallHistoryRecord, CallHistoryStatus,
};
use wormhole_desktop_core::chat_commands::chat_list_conversations;
use wormhole_desktop_core::chat_rtc_call::{
    chat_video_call_accept, chat_video_call_cancel, chat_video_call_decline, chat_video_call_end,
    chat_video_call_invite, chat_video_call_status, chat_voice_call_accept, chat_voice_call_cancel,
    chat_voice_call_decline, chat_voice_call_end, chat_voice_call_invite, chat_voice_call_status,
    load_video_device_prefs, load_voice_device_prefs, ChatRtcCallStatusDto,
    ChatVideoCallDeviceParams, ChatVideoCallInviteParams, ChatVoiceCallConvParams,
    ChatVoiceCallInviteParams, VideoDevicePrefs, VoiceDevicePrefs,
};

pub fn devices_from_voice_prefs(prefs: &VoiceDevicePrefs) -> ChatVideoCallDeviceParams {
    ChatVideoCallDeviceParams {
        camera_device_id: None,
        microphone_device_id: Some(prefs.mic_id.clone()).filter(|s| !s.is_empty()),
        muted: Some(prefs.muted),
    }
}

pub fn devices_from_video_prefs(prefs: &VideoDevicePrefs) -> ChatVideoCallDeviceParams {
    ChatVideoCallDeviceParams {
        camera_device_id: Some(prefs.camera_id.clone()).filter(|s| !s.is_empty()),
        microphone_device_id: Some(prefs.mic_id.clone()).filter(|s| !s.is_empty()),
        muted: Some(prefs.muted),
    }
}

pub fn invite_params(conv_id: impl Into<String>) -> ChatVoiceCallInviteParams {
    ChatVoiceCallInviteParams {
        conv_id: conv_id.into(),
        devices: ChatVideoCallDeviceParams::default(),
    }
}

pub fn invite_params_with_voice(
    conv_id: impl Into<String>,
    prefs: &VoiceDevicePrefs,
) -> ChatVoiceCallInviteParams {
    ChatVoiceCallInviteParams {
        conv_id: conv_id.into(),
        devices: devices_from_voice_prefs(prefs),
    }
}

pub fn invite_params_with_video(
    conv_id: impl Into<String>,
    prefs: &VideoDevicePrefs,
) -> ChatVideoCallInviteParams {
    ChatVideoCallInviteParams {
        conv_id: conv_id.into(),
        devices: devices_from_video_prefs(prefs),
    }
}

pub fn conv_params(conv_id: impl Into<String>) -> ChatVoiceCallConvParams {
    ChatVoiceCallConvParams {
        conv_id: conv_id.into(),
    }
}

fn loaded_voice_prefs(core: &CoreHandle) -> VoiceDevicePrefs {
    load_voice_device_prefs(&core.runtime().state.data_dir)
}

fn loaded_video_prefs(core: &CoreHandle) -> VideoDevicePrefs {
    load_video_device_prefs(&core.runtime().state.data_dir)
}

pub async fn fetch_status(
    core: &CoreHandle,
    conv_id: &str,
) -> Result<ChatRtcCallStatusDto, String> {
    let runtime = core.runtime();
    chat_voice_call_status(&runtime.state, conv_params(conv_id)).await
}

pub async fn fetch_video_status(
    core: &CoreHandle,
    conv_id: &str,
) -> Result<ChatRtcCallStatusDto, String> {
    let runtime = core.runtime();
    chat_video_call_status(&runtime.state, conv_params(conv_id)).await
}

pub async fn invite(core: &CoreHandle, conv_id: &str) -> Result<ChatRtcCallStatusDto, String> {
    let prefs = loaded_voice_prefs(core);
    invite_with_prefs(core, conv_id, &prefs).await
}

pub async fn invite_with_prefs(
    core: &CoreHandle,
    conv_id: &str,
    prefs: &VoiceDevicePrefs,
) -> Result<ChatRtcCallStatusDto, String> {
    let runtime = core.runtime();
    let app = runtime.ctx.as_ref();
    let status =
        chat_voice_call_invite(app, &runtime.state, invite_params_with_voice(conv_id, prefs)).await?;
    record_outgoing_call_history(
        core,
        conv_id,
        status.call_id.as_deref(),
        CallHistoryKind::Voice,
        CallHistoryStatus::Ringing,
        None,
    )
    .await;
    Ok(status)
}

pub async fn invite_video(core: &CoreHandle, conv_id: &str) -> Result<ChatRtcCallStatusDto, String> {
    let prefs = loaded_video_prefs(core);
    invite_video_with_prefs(core, conv_id, &prefs).await
}

pub async fn invite_video_with_prefs(
    core: &CoreHandle,
    conv_id: &str,
    prefs: &VideoDevicePrefs,
) -> Result<ChatRtcCallStatusDto, String> {
    let runtime = core.runtime();
    let app = runtime.ctx.as_ref();
    let status =
        chat_video_call_invite(app, &runtime.state, invite_params_with_video(conv_id, prefs)).await?;
    record_outgoing_call_history(
        core,
        conv_id,
        status.call_id.as_deref(),
        CallHistoryKind::Video,
        CallHistoryStatus::Ringing,
        None,
    )
    .await;
    Ok(status)
}

async fn record_outgoing_call_history(
    core: &CoreHandle,
    conv_id: &str,
    call_id: Option<&str>,
    kind: CallHistoryKind,
    status: CallHistoryStatus,
    room_id: Option<String>,
) {
    let runtime = core.runtime();
    let app = runtime.ctx.as_ref();
    let Ok(list) = chat_list_conversations(app, &runtime.state).await else {
        return;
    };
    let Some(conv) = list.into_iter().find(|c| c.id == conv_id) else {
        return;
    };
    let now = unix_now();
    let ended = matches!(
        status,
        CallHistoryStatus::Ended
            | CallHistoryStatus::Missed
            | CallHistoryStatus::Declined
            | CallHistoryStatus::Cancelled
            | CallHistoryStatus::Failed
    );
    let record = CallHistoryRecord {
        call_id: call_id.map(str::to_string).unwrap_or_else(new_call_id),
        kind,
        direction: CallHistoryDirection::Outgoing,
        status,
        participants: vec![CallHistoryParticipant {
            endpoint_id: conv.peer_endpoint,
            display_name: conv.peer_display_name.unwrap_or_default(),
        }],
        started_at: now,
        ended_at: if ended { Some(now) } else { None },
        expires_at: expires_at_from_end(now),
        room_id,
        conv_id: Some(conv_id.to_string()),
    };
    let _ = call_history_put(&runtime.state, record).await;
}

/// Best-effort status update for an existing call (end / decline / cancel / accept).
pub async fn update_call_history_status(
    core: &CoreHandle,
    conv_id: &str,
    call_id: Option<&str>,
    kind: CallHistoryKind,
    status: CallHistoryStatus,
    direction: CallHistoryDirection,
) {
    let Some(call_id) = call_id.filter(|s| !s.is_empty()) else {
        return;
    };
    let runtime = core.runtime();
    let app = runtime.ctx.as_ref();
    let peer = chat_list_conversations(app, &runtime.state)
        .await
        .ok()
        .and_then(|list| list.into_iter().find(|c| c.id == conv_id))
        .map(|c| (c.peer_endpoint, c.peer_display_name.unwrap_or_default()));
    let Some((endpoint_id, display_name)) = peer else {
        return;
    };
    let now = unix_now();
    let ended = !matches!(
        status,
        CallHistoryStatus::Ringing | CallHistoryStatus::Active
    );
    let record = CallHistoryRecord {
        call_id: call_id.to_string(),
        kind,
        direction,
        status,
        participants: vec![CallHistoryParticipant {
            endpoint_id,
            display_name,
        }],
        started_at: now.saturating_sub(1),
        ended_at: if ended { Some(now) } else { None },
        expires_at: expires_at_from_end(now),
        room_id: None,
        conv_id: Some(conv_id.to_string()),
    };
    let _ = call_history_put(&runtime.state, record).await;
}

/// Write an incoming ringing row when the peer invites us (idempotent by call_id).
pub async fn record_incoming_ringing(
    core: &CoreHandle,
    conv_id: &str,
    call_id: Option<&str>,
    kind: CallHistoryKind,
) {
    update_call_history_status(
        core,
        conv_id,
        call_id,
        kind,
        CallHistoryStatus::Ringing,
        CallHistoryDirection::Incoming,
    )
    .await;
}

fn unix_now() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

pub async fn accept(core: &CoreHandle, conv_id: &str) -> Result<ChatRtcCallStatusDto, String> {
    let prefs = loaded_voice_prefs(core);
    let runtime = core.runtime();
    let app = runtime.ctx.as_ref();
    let status =
        chat_voice_call_accept(app, &runtime.state, invite_params_with_voice(conv_id, &prefs))
            .await?;
    update_call_history_status(
        core,
        conv_id,
        status.call_id.as_deref(),
        CallHistoryKind::Voice,
        CallHistoryStatus::Active,
        CallHistoryDirection::Incoming,
    )
    .await;
    Ok(status)
}

pub async fn accept_video(core: &CoreHandle, conv_id: &str) -> Result<ChatRtcCallStatusDto, String> {
    let prefs = loaded_video_prefs(core);
    let runtime = core.runtime();
    let app = runtime.ctx.as_ref();
    let status =
        chat_video_call_accept(app, &runtime.state, invite_params_with_video(conv_id, &prefs))
            .await?;
    update_call_history_status(
        core,
        conv_id,
        status.call_id.as_deref(),
        CallHistoryKind::Video,
        CallHistoryStatus::Active,
        CallHistoryDirection::Incoming,
    )
    .await;
    Ok(status)
}

pub async fn decline(core: &CoreHandle, conv_id: &str) -> Result<ChatRtcCallStatusDto, String> {
    let runtime = core.runtime();
    let app = runtime.ctx.as_ref();
    let status = chat_voice_call_decline(app, &runtime.state, conv_params(conv_id)).await?;
    update_call_history_status(
        core,
        conv_id,
        status.call_id.as_deref(),
        CallHistoryKind::Voice,
        CallHistoryStatus::Declined,
        CallHistoryDirection::Incoming,
    )
    .await;
    Ok(status)
}

pub async fn decline_video(core: &CoreHandle, conv_id: &str) -> Result<ChatRtcCallStatusDto, String> {
    let runtime = core.runtime();
    let app = runtime.ctx.as_ref();
    let status = chat_video_call_decline(app, &runtime.state, conv_params(conv_id)).await?;
    update_call_history_status(
        core,
        conv_id,
        status.call_id.as_deref(),
        CallHistoryKind::Video,
        CallHistoryStatus::Declined,
        CallHistoryDirection::Incoming,
    )
    .await;
    Ok(status)
}

pub async fn cancel(core: &CoreHandle, conv_id: &str) -> Result<ChatRtcCallStatusDto, String> {
    let runtime = core.runtime();
    let app = runtime.ctx.as_ref();
    let status = chat_voice_call_cancel(app, &runtime.state, conv_params(conv_id)).await?;
    update_call_history_status(
        core,
        conv_id,
        status.call_id.as_deref(),
        CallHistoryKind::Voice,
        CallHistoryStatus::Cancelled,
        CallHistoryDirection::Outgoing,
    )
    .await;
    Ok(status)
}

pub async fn cancel_video(core: &CoreHandle, conv_id: &str) -> Result<ChatRtcCallStatusDto, String> {
    let runtime = core.runtime();
    let app = runtime.ctx.as_ref();
    let status = chat_video_call_cancel(app, &runtime.state, conv_params(conv_id)).await?;
    update_call_history_status(
        core,
        conv_id,
        status.call_id.as_deref(),
        CallHistoryKind::Video,
        CallHistoryStatus::Cancelled,
        CallHistoryDirection::Outgoing,
    )
    .await;
    Ok(status)
}

pub async fn end(core: &CoreHandle, conv_id: &str) -> Result<ChatRtcCallStatusDto, String> {
    let runtime = core.runtime();
    let app = runtime.ctx.as_ref();
    let status = chat_voice_call_end(app, &runtime.state, conv_params(conv_id)).await?;
    update_call_history_status(
        core,
        conv_id,
        status.call_id.as_deref(),
        CallHistoryKind::Voice,
        CallHistoryStatus::Ended,
        CallHistoryDirection::Outgoing,
    )
    .await;
    Ok(status)
}

pub async fn end_video(core: &CoreHandle, conv_id: &str) -> Result<ChatRtcCallStatusDto, String> {
    let runtime = core.runtime();
    let app = runtime.ctx.as_ref();
    let status = chat_video_call_end(app, &runtime.state, conv_params(conv_id)).await?;
    update_call_history_status(
        core,
        conv_id,
        status.call_id.as_deref(),
        CallHistoryKind::Video,
        CallHistoryStatus::Ended,
        CallHistoryDirection::Outgoing,
    )
    .await;
    Ok(status)
}

pub fn apply_voice_status(shell_state: &SharedChatShellState, status: &ChatRtcCallStatusDto) {
    if let Ok(mut state) = shell_state.lock() {
        state.set_voice_call_phase(status.phase.clone());
        if status.phase != "active" {
            state.voice_live_peer = None;
        }
    }
}

pub fn voice_status_to_header_line(phase: &str, peer_online: bool) -> String {
    match phase {
        "ringing" => "正在呼叫…".into(),
        "incoming" => "来电".into(),
        "active" => "通话 · 已连接".into(),
        _ if peer_online => "在线".into(),
        _ => "离线".into(),
    }
}

pub fn video_status_to_header_line(phase: &str, peer_online: bool) -> String {
    match phase {
        "ringing" => "正在视频呼叫…".into(),
        "incoming" => "视频来电".into(),
        "active" => "视频通话 · 已连接".into(),
        _ if peer_online => "在线".into(),
        _ => "离线".into(),
    }
}

pub fn voice_error_toast(err: &str) -> (String, StatusTone) {
    let lower = err.to_lowercase();
    if lower.contains("offline") || err.contains("离线") {
        ("终端离线，无法发起语音通话".into(), StatusTone::Muted)
    } else if lower.contains("microphone") || err.contains("麦克风") {
        (
            "无法访问麦克风，请检查权限与设备".into(),
            StatusTone::Danger,
        )
    } else if lower.contains("opus") {
        ("Opus 音频不可用，请检查编解码".into(), StatusTone::Danger)
    } else if err.contains("冲突") || lower.contains("busy") || lower.contains("session") {
        ("已有进行中的通话或远程会话".into(), StatusTone::Danger)
    } else {
        (format!("语音通话失败: {err}"), StatusTone::Danger)
    }
}

pub fn video_error_toast(err: &str) -> (String, StatusTone) {
    let lower = err.to_lowercase();
    if lower.contains("offline") || err.contains("离线") {
        ("终端离线，无法发起视频通话".into(), StatusTone::Muted)
    } else if lower.contains("camera") || err.contains("摄像头") {
        (
            "无法访问摄像头，请检查权限与设备".into(),
            StatusTone::Danger,
        )
    } else if err.contains("冲突") || lower.contains("busy") || lower.contains("session") {
        ("已有进行中的通话或远程会话".into(), StatusTone::Danger)
    } else if err.contains("REALTIME_SFU") || err.contains("503") || lower.contains("sfu") {
        (
            "群视频服务未配置（Realtime SFU），请联系管理员".into(),
            StatusTone::Danger,
        )
    } else {
        (format!("视频通话失败: {err}"), StatusTone::Danger)
    }
}
