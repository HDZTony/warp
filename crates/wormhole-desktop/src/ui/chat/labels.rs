use wormhole_desktop_core::chat_commands::ChatConversationDto;
use wormhole_desktop_core::cluster_commands::{ClusterNodeDto, ClusterStatusDto};

/// Match a conversation peer to a cluster node by endpoint, node id, or display name.
pub fn find_cluster_node<'a>(
    conv: &ChatConversationDto,
    cluster: Option<&'a ClusterStatusDto>,
) -> Option<&'a ClusterNodeDto> {
    let cluster = cluster?;
    let peer = conv.peer_endpoint.as_str();
    if let Some(node) = cluster
        .nodes
        .iter()
        .find(|node| node.chat_endpoint_id.as_deref() == Some(peer) || node.node_id == peer)
    {
        return Some(node);
    }
    let display = conv
        .peer_display_name
        .as_deref()
        .map(str::trim)
        .filter(|name| !name.is_empty())?;
    cluster.nodes.iter().find(|node| {
        node.hostname.eq_ignore_ascii_case(display)
            || format!("{} · {}", node.os, node.hostname).eq_ignore_ascii_case(display)
    })
}

/// Return only an iroh endpoint identity suitable for RDP resolution, never a cluster UUID.
pub fn remote_desktop_peer_identity(
    conv: Option<&ChatConversationDto>,
    node: Option<&ClusterNodeDto>,
) -> Option<String> {
    conv.map(|conv| conv.peer_endpoint.trim())
        .filter(|endpoint| !endpoint.is_empty())
        .map(str::to_string)
        .or_else(|| {
            node.and_then(|node| node.chat_endpoint_id.as_deref())
                .map(str::trim)
                .filter(|endpoint| !endpoint.is_empty())
                .map(str::to_string)
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
            .unwrap_or_else(|| wormhole_i18n::t("chat.group"));
    }
    if let Some(node) = find_cluster_node(conv, cluster) {
        return format!("{} · {}", node.os, node.hostname);
    }
    conv.peer_display_name
        .clone()
        .filter(|name| !name.is_empty())
        .or_else(|| conv.title.clone())
        .unwrap_or_else(|| wormhole_i18n::t("chat.unknown_device"))
}

/// Sidebar / header avatar initials aligned with HTML `chatAvatarForDevice`.
pub fn chat_avatar_for_os(os: &str) -> String {
    let os = os.trim();
    if os.is_empty() {
        return "WH".to_string();
    }
    if os.eq_ignore_ascii_case("Windows")
        || os.eq_ignore_ascii_case("macOS")
        || os.eq_ignore_ascii_case("Linux")
        || os.to_ascii_lowercase().contains("windows")
        || os.to_ascii_lowercase().contains("macos")
        || os.to_ascii_lowercase().contains("linux")
    {
        return "PC".to_string();
    }
    if os.to_ascii_lowercase().contains("ipad") {
        return "iP".to_string();
    }
    if os.to_ascii_lowercase().contains("ios") || os.to_ascii_lowercase().contains("android") {
        return "iOS".to_string();
    }
    let compact: String = os
        .chars()
        .filter(|c| c.is_ascii_alphanumeric())
        .take(3)
        .collect::<String>()
        .to_uppercase();
    if compact.is_empty() {
        "DEV".to_string()
    } else {
        compact
    }
}

/// OS string for avatar: cluster node when known, else empty (caller may fall back).
pub fn conversation_os_label(
    conv: &ChatConversationDto,
    cluster: Option<&ClusterStatusDto>,
) -> String {
    find_cluster_node(conv, cluster)
        .map(|node| node.os.clone())
        .unwrap_or_default()
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
        return wormhole_i18n::t("chat.new_message");
    }
    if let Some(node) = find_cluster_node(conv, cluster) {
        return if node.online {
            wormhole_i18n::t("chat.waiting")
        } else {
            wormhole_i18n::t("common.offline")
        };
    }
    wormhole_i18n::t("chat.waiting_short")
}

#[cfg(test)]
mod tests {
    use super::*;
    use wormhole_desktop_core::cluster_commands::ClusterStatusDto;

    fn sample_conv(
        kind: &str,
        peer: &str,
        title: Option<&str>,
        preview: Option<&str>,
        peer_display_name: Option<&str>,
    ) -> ChatConversationDto {
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
            peer_display_name: peer_display_name.map(str::to_string),
            peer_bootstrap_addrs: Vec::new(),
            peer_user_id: None,
            contact_conv_id: None,
            description: None,
            avatar_path: None,
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
                same_account: false,
                user_id: None,
                account_display_name: None,
                pending_handshake: false,
                handshake_error: None,
                share_volumes: Vec::new(),
                has_local_share_replicas: false,
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
            role_stale: false,
        }
    }

    #[test]
    fn remote_desktop_identity_prefers_conversation_endpoint_and_never_node_id() {
        let conv = sample_conv("direct", "conversation-endpoint", None, None, None);
        let cluster = cluster_with_node("node-uuid", "windows", "office-pc");
        let node = &cluster.nodes[0];
        assert_eq!(
            remote_desktop_peer_identity(Some(&conv), Some(node)).as_deref(),
            Some("conversation-endpoint")
        );

        let mut endpoint_node = node.clone();
        endpoint_node.chat_endpoint_id = Some("cluster-endpoint".into());
        assert_eq!(
            remote_desktop_peer_identity(None, Some(&endpoint_node)).as_deref(),
            Some("cluster-endpoint")
        );

        endpoint_node.chat_endpoint_id = None;
        assert_eq!(
            remote_desktop_peer_identity(None, Some(&endpoint_node)),
            None
        );
    }

    #[test]
    fn conversation_device_title_prefers_cluster_label() {
        let conv = sample_conv("direct", "peer-1", None, None, Some("wormhole"));
        let cluster = cluster_with_node("peer-1", "Windows", "DESKTOP-VHCQ89I");
        assert_eq!(
            conversation_device_title(&conv, Some(&cluster)),
            "Windows · DESKTOP-VHCQ89I"
        );
    }

    #[test]
    fn find_cluster_node_by_hostname_display_name() {
        let conv = sample_conv(
            "direct",
            "stale-endpoint",
            None,
            None,
            Some("DESKTOP-KDSVGM5"),
        );
        let cluster = cluster_with_node("node-real", "Windows", "DESKTOP-KDSVGM5");
        assert!(find_cluster_node(&conv, Some(&cluster)).is_some());
        assert_eq!(
            conversation_device_title(&conv, Some(&cluster)),
            "Windows · DESKTOP-KDSVGM5"
        );
    }

    #[test]
    fn find_cluster_node_by_os_hostname_display_name() {
        let conv = sample_conv(
            "direct",
            "stale-endpoint",
            None,
            None,
            Some("Windows · DESKTOP-KDSVGM5"),
        );
        let cluster = cluster_with_node("node-real", "Windows", "DESKTOP-KDSVGM5");
        assert_eq!(
            conversation_device_title(&conv, Some(&cluster)),
            "Windows · DESKTOP-KDSVGM5"
        );
    }

    #[test]
    fn conversation_device_title_uses_group_title() {
        let conv = sample_conv("cluster_group", "peer-1", Some("家庭群"), None, None);
        assert_eq!(conversation_device_title(&conv, None), "家庭群");
    }

    #[test]
    fn conversation_preview_uses_last_message_preview() {
        let conv = sample_conv("direct", "peer-1", None, Some("你好啊"), Some("wormhole"));
        assert_eq!(conversation_preview(&conv, None), "你好啊");
    }

    #[test]
    fn conversation_preview_does_not_repeat_device_name() {
        wormhole_i18n::set_locale("zh-CN");
        let conv = sample_conv("direct", "peer-1", None, None, Some("wormhole"));
        let cluster = cluster_with_node("peer-1", "Windows", "DESKTOP-VHCQ89I");
        assert_eq!(
            conversation_preview(&conv, Some(&cluster)),
            wormhole_i18n::t("chat.waiting")
        );
    }

    #[test]
    fn chat_avatar_for_os_maps_common_platforms() {
        assert_eq!(chat_avatar_for_os("Windows"), "PC");
        assert_eq!(chat_avatar_for_os("macOS"), "PC");
        assert_eq!(chat_avatar_for_os("Linux"), "PC");
        assert_eq!(chat_avatar_for_os("iPadOS 18"), "iP");
        assert_eq!(chat_avatar_for_os("iOS 18"), "iOS");
        assert_eq!(chat_avatar_for_os("Android 14"), "iOS");
        assert_eq!(chat_avatar_for_os(""), "WH");
        assert_eq!(chat_avatar_for_os("??"), "DEV");
    }
}
