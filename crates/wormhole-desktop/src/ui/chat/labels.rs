use wormhole_desktop_core::chat_commands::ChatConversationDto;
use wormhole_desktop_core::cluster_commands::{ClusterNodeDto, ClusterStatusDto};

pub fn find_cluster_node<'a>(
    conv: &ChatConversationDto,
    cluster: Option<&'a ClusterStatusDto>,
) -> Option<&'a ClusterNodeDto> {
    cluster?.nodes.iter().find(|node| {
        node.chat_endpoint_id.as_deref() == Some(conv.peer_endpoint.as_str())
            || node.node_id == conv.peer_endpoint
    })
}

/// Fixed device label for sidebar title and chat header (`OS · hostname` when in cluster).
pub fn conversation_device_title(
    conv: &ChatConversationDto,
    cluster: Option<&ClusterStatusDto>,
) -> String {
    if conv.kind == "cluster_group" {
        return conv
            .title
            .clone()
            .filter(|title| !title.is_empty())
            .unwrap_or_else(|| "群聊".to_string());
    }
    if let Some(node) = find_cluster_node(conv, cluster) {
        return format!("{} · {}", node.os, node.hostname);
    }
    conv.peer_display_name
        .clone()
        .filter(|name| !name.is_empty())
        .or_else(|| conv.title.clone())
        .unwrap_or_else(|| "未知设备".to_string())
}

/// Sidebar preview line: last message snippet, not the device name.
pub fn conversation_preview(
    conv: &ChatConversationDto,
    cluster: Option<&ClusterStatusDto>,
) -> String {
    if let Some(preview) = conv
        .last_message_preview
        .as_ref()
        .map(|value| value.trim())
        .filter(|value| !value.is_empty())
    {
        return preview.to_string();
    }
    if conv.last_message_at.is_some() {
        return "有新消息".to_string();
    }
    if let Some(node) = find_cluster_node(conv, cluster) {
        return if node.online {
            "在线 · 等待消息…".to_string()
        } else {
            "离线".to_string()
        };
    }
    "等待消息…".to_string()
}

#[cfg(test)]
mod tests {
    use super::*;
    use wormhole_desktop_core::cluster_commands::ClusterStatusDto;

    fn sample_conv(kind: &str, peer: &str, title: Option<&str>, preview: Option<&str>) -> ChatConversationDto {
        ChatConversationDto {
            id: "conv-1".into(),
            backend: "wormhole".into(),
            kind: kind.into(),
            title: title.map(str::to_string),
            cluster_id: None,
            members: Vec::new(),
            created_by: None,
            membership_version: 0,
            peer_endpoint: peer.into(),
            peer_display_name: Some("wormhole".into()),
            peer_bootstrap_addrs: Vec::new(),
            doc_ticket: String::new(),
            created_at: 0,
            last_message_at: None,
            last_message_preview: preview.map(str::to_string),
        }
    }

    fn cluster_with_node(peer: &str, os: &str, hostname: &str) -> ClusterStatusDto {
        ClusterStatusDto {
            configured: true,
            cluster_id: None,
            clusters: Vec::new(),
            joined_at: None,
            device_id: None,
            local_node_id: String::new(),
            transport: String::new(),
            nodes: vec![ClusterNodeDto {
                node_id: peer.into(),
                device_id: None,
                chat_endpoint_id: Some(peer.into()),
                chat_bootstrap_addrs: Vec::new(),
                hostname: hostname.into(),
                os: os.into(),
                roles: Vec::new(),
                online: true,
                presence_status: String::new(),
                cpu_cores: 0,
                memory_total: 0,
                storage_total: 0,
                storage_free: 0,
                billing_node_score: None,
                billing_expired: false,
                role: String::new(),
                removable: false,
                revoked: false,
                server_member_confirmed: false,
            }],
            storage_volumes: Vec::new(),
            normal_replica_target: 0,
            photo_video_replica_target: 0,
            build_cache_replica_target: 0,
            normal_replica_degraded: false,
            photo_video_replica_degraded: false,
            syncing: false,
            auth_required: false,
            device_bootstrap_required: false,
            device_bootstrap_error: None,
        }
    }

    #[test]
    fn conversation_device_title_prefers_cluster_label() {
        let conv = sample_conv("direct", "peer-1", None, None);
        let cluster = cluster_with_node("peer-1", "Windows", "DESKTOP-VHCQ89I");
        assert_eq!(
            conversation_device_title(&conv, Some(&cluster)),
            "Windows · DESKTOP-VHCQ89I"
        );
    }

    #[test]
    fn conversation_device_title_uses_group_title() {
        let conv = sample_conv("cluster_group", "peer-1", Some("家庭群"), None);
        assert_eq!(conversation_device_title(&conv, None), "家庭群");
    }

    #[test]
    fn conversation_preview_uses_last_message_preview() {
        let conv = sample_conv("direct", "peer-1", None, Some("你好啊"));
        assert_eq!(conversation_preview(&conv, None), "你好啊");
    }

    #[test]
    fn conversation_preview_does_not_repeat_device_name() {
        let conv = sample_conv("direct", "peer-1", None, None);
        let cluster = cluster_with_node("peer-1", "Windows", "DESKTOP-VHCQ89I");
        assert_eq!(
            conversation_preview(&conv, Some(&cluster)),
            "在线 · 等待消息…"
        );
    }
}
