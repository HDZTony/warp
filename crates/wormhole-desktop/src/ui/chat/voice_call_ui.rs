//! Shared chat voice-call UI helpers (header, profile panel, incoming banner).

use crate::ui::chat::shell_state::SharedChatShellState;
use crate::ui::core_handle::CoreHandle;
use crate::ui::panel_primitives::StatusTone;
use wormhole_desktop_core::chat_rtc_call::{
    chat_voice_call_accept, chat_voice_call_cancel, chat_voice_call_decline, chat_voice_call_end,
    chat_voice_call_invite, chat_voice_call_status, ChatRtcCallStatusDto,
    ChatVideoCallDeviceParams, ChatVoiceCallConvParams, ChatVoiceCallInviteParams,
};

pub fn invite_params(conv_id: impl Into<String>) -> ChatVoiceCallInviteParams {
    ChatVoiceCallInviteParams {
        conv_id: conv_id.into(),
        devices: ChatVideoCallDeviceParams::default(),
    }
}

pub fn conv_params(conv_id: impl Into<String>) -> ChatVoiceCallConvParams {
    ChatVoiceCallConvParams {
        conv_id: conv_id.into(),
    }
}

pub async fn fetch_status(
    core: &CoreHandle,
    conv_id: &str,
) -> Result<ChatRtcCallStatusDto, String> {
    let runtime = core.runtime();
    chat_voice_call_status(&runtime.state, conv_params(conv_id)).await
}

pub async fn invite(core: &CoreHandle, conv_id: &str) -> Result<ChatRtcCallStatusDto, String> {
    let runtime = core.runtime();
    let app = runtime.ctx.as_ref();
    chat_voice_call_invite(app, &runtime.state, invite_params(conv_id)).await
}

pub async fn accept(core: &CoreHandle, conv_id: &str) -> Result<ChatRtcCallStatusDto, String> {
    let runtime = core.runtime();
    let app = runtime.ctx.as_ref();
    chat_voice_call_accept(app, &runtime.state, invite_params(conv_id)).await
}

pub async fn decline(core: &CoreHandle, conv_id: &str) -> Result<ChatRtcCallStatusDto, String> {
    let runtime = core.runtime();
    let app = runtime.ctx.as_ref();
    chat_voice_call_decline(app, &runtime.state, conv_params(conv_id)).await
}

pub async fn cancel(core: &CoreHandle, conv_id: &str) -> Result<ChatRtcCallStatusDto, String> {
    let runtime = core.runtime();
    let app = runtime.ctx.as_ref();
    chat_voice_call_cancel(app, &runtime.state, conv_params(conv_id)).await
}

pub async fn end(core: &CoreHandle, conv_id: &str) -> Result<ChatRtcCallStatusDto, String> {
    let runtime = core.runtime();
    let app = runtime.ctx.as_ref();
    chat_voice_call_end(app, &runtime.state, conv_params(conv_id)).await
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
        "active" => "语音通话 · 已连接".into(),
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
        ("Opus 音频不可用，请检查编译依赖".into(), StatusTone::Danger)
    } else if err.contains("冲突") || lower.contains("busy") || lower.contains("session") {
        ("已有进行中的通话或远程会话".into(), StatusTone::Danger)
    } else {
        (format!("语音通话失败: {err}"), StatusTone::Danger)
    }
}
