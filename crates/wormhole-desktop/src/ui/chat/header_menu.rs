use std::sync::{Arc, Mutex};

use pathfinder_color::ColorU;
use warpui::elements::{
    Align, Container, CornerRadius, CrossAxisAlignment, DispatchEventResult, EventHandler, Flex,
    Hoverable, MainAxisSize, MouseState, MouseStateHandle, ParentElement, Radius,
};
use warpui::fonts::FamilyId;
use warpui::Element;

use crate::ui::chat::header::ChatHeaderAction;
use crate::ui::panel_primitives::{
    popover_menu_separator, popover_plain_item, popover_shell, StatusTone,
};
use crate::ui::theme;

const MENU_WIDTH: f32 = 220.0;
const FLYOUT_WIDTH: f32 = 200.0;

pub fn header_button_colors(active: bool, hovered: bool) -> (ColorU, ColorU) {
    if active {
        (theme::accent_cool_bg(32), theme::accent_cool())
    } else if hovered {
        (theme::accent_bg_default(), theme::accent_cool())
    } else {
        (ColorU::transparent_black(), theme::muted())
    }
}

pub fn header_menu_panel(
    font: FamilyId,
    mute_flyout_open: bool,
    has_custom_wallpaper: bool,
) -> Box<dyn Element> {
    let mut col = Flex::column()
        .with_main_axis_size(MainAxisSize::Min)
        .with_cross_axis_alignment(CrossAxisAlignment::Stretch);
    col.add_child(popover_plain_item(
        font,
        "消息免打扰",
        false,
        true,
        |ctx, _, _| {
            ctx.dispatch_typed_action(ChatHeaderAction::ToggleMuteFlyout);
            DispatchEventResult::StopPropagation
        },
    ));
    if mute_flyout_open {
        col.add_child(mute_flyout(font));
    }
    col.add_child(popover_menu_separator());
    col.add_child(popover_plain_item(
        font,
        "查看个人资料",
        false,
        false,
        |ctx, _, _| {
            ctx.dispatch_typed_action(ChatHeaderAction::OpenProfile);
            DispatchEventResult::StopPropagation
        },
    ));
    col.add_child(popover_plain_item(
        font,
        "设置壁纸",
        false,
        false,
        |ctx, _, _| {
            ctx.dispatch_typed_action(ChatHeaderAction::SetWallpaper);
            DispatchEventResult::StopPropagation
        },
    ));
    if has_custom_wallpaper {
        col.add_child(popover_plain_item(
            font,
            "恢复默认壁纸",
            false,
            false,
            |ctx, _, _| {
                ctx.dispatch_typed_action(ChatHeaderAction::ClearWallpaper);
                DispatchEventResult::StopPropagation
            },
        ));
    }
    col.add_child(popover_plain_item(
        font,
        "禁用文件分享",
        false,
        false,
        |ctx, _, _| {
            ctx.dispatch_typed_action(ChatHeaderAction::MenuToast(
                "已禁用此终端的文件分享（演示）".into(),
                StatusTone::Muted,
            ));
            DispatchEventResult::StopPropagation
        },
    ));
    col.add_child(popover_menu_separator());
    col.add_child(popover_plain_item(
        font,
        "清空历史记录",
        false,
        false,
        |ctx, _, _| {
            ctx.dispatch_typed_action(ChatHeaderAction::ClearHistory);
            DispatchEventResult::StopPropagation
        },
    ));
    col.add_child(popover_plain_item(
        font,
        "删除聊天",
        true,
        false,
        |ctx, _, _| {
            ctx.dispatch_typed_action(ChatHeaderAction::DeleteChat);
            DispatchEventResult::StopPropagation
        },
    ));

    popover_shell(
        MENU_WIDTH,
        Container::new(col.finish())
            .with_uniform_padding(6.0)
            .finish(),
    )
}

fn mute_flyout(font: FamilyId) -> Box<dyn Element> {
    let mut col = Flex::column()
        .with_main_axis_size(MainAxisSize::Min)
        .with_cross_axis_alignment(CrossAxisAlignment::Stretch);
    for (label, toast) in [
        ("设置铃声", "铃声设置（演示）"),
        ("关闭通知音", "已关闭此终端通知音"),
        ("设置静音时长…", "静音 1 小时（演示）"),
    ] {
        let toast = toast.to_string();
        col.add_child(popover_plain_item(
            font,
            label,
            false,
            false,
            move |ctx, _, _| {
                ctx.dispatch_typed_action(ChatHeaderAction::MenuToast(
                    toast.clone(),
                    StatusTone::Muted,
                ));
                DispatchEventResult::StopPropagation
            },
        ));
    }
    col.add_child(popover_plain_item(
        font,
        "永久静音",
        true,
        false,
        |ctx, _, _| {
            ctx.dispatch_typed_action(ChatHeaderAction::MuteForever);
            DispatchEventResult::StopPropagation
        },
    ));
    Container::new(popover_shell(
        FLYOUT_WIDTH,
        Container::new(col.finish())
            .with_uniform_padding(6.0)
            .finish(),
    ))
    .with_margin_left(8.0)
    .finish()
}

pub fn header_button(
    icon_path: &'static str,
    color: ColorU,
    active: bool,
    action: ChatHeaderAction,
    size: f32,
) -> Box<dyn Element> {
    let mouse_state: MouseStateHandle = Arc::new(Mutex::new(MouseState::default()));
    let icon_path_static = icon_path;
    let action_clone = action.clone();
    let hoverable = Hoverable::new(mouse_state, move |state| {
        let (bg, icon_color) = header_button_colors(active, state.is_hovered());
        let resolved_icon = if active || state.is_hovered() {
            icon_color
        } else {
            color
        };
        Container::new(
            Align::new(crate::ui::icons::chat_header_icon(
                icon_path_static,
                resolved_icon,
            ))
            .finish(),
        )
        .with_background(bg)
        .with_corner_radius(CornerRadius::with_all(Radius::Pixels(size / 2.0)))
        .finish()
    })
    .finish();

    let (automation_label, automation_id) = match &action {
        ChatHeaderAction::ToggleThreadSearch => ("搜索消息", "chat:thread_search"),
        ChatHeaderAction::VoiceCallPrimary => ("语音通话", "chat:voice_call"),
        ChatHeaderAction::VideoCallPrimary => ("视频通话", "chat:video_call"),
        ChatHeaderAction::OpenRemoteDesktop => ("远程桌面", "chat:remote_desktop"),
        ChatHeaderAction::ToggleProfile => ("个人资料", "chat:profile"),
        ChatHeaderAction::ToggleHeaderMenu => ("更多", "chat:header_menu"),
        _ => ("聊天操作", "chat:header_action"),
    };
    EventHandler::new(hoverable)
        .with_automation_label(automation_label)
        .with_automation_id(automation_id)
        .on_left_mouse_down(move |ctx, _, _| {
            ctx.dispatch_typed_action(action_clone.clone());
            DispatchEventResult::StopPropagation
        })
        .finish()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn active_beats_hover_for_header_button_colors() {
        let (bg, _) = header_button_colors(true, false);
        assert_ne!(bg, ColorU::transparent_black());
        let (bg_hover, _) = header_button_colors(true, true);
        assert_eq!(bg, bg_hover);
    }
}
