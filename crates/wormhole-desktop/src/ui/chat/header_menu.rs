use std::sync::{Arc, Mutex};

use pathfinder_color::ColorU;
use warpui::elements::{
    Align, Container, CrossAxisAlignment, DispatchEventResult, EventHandler, Flex, Hoverable,
    MainAxisSize, MouseState, MouseStateHandle, ParentElement,
};
use warpui::fonts::FamilyId;
use warpui::Element;

use crate::ui::chat::header::ChatHeaderAction;
use crate::ui::panel_primitives::{
    popover_menu_separator, popover_plain_item, popover_plain_item_with_id, popover_shell,
};
use crate::ui::theme;

const MENU_WIDTH: f32 = 220.0;
const FLYOUT_WIDTH: f32 = 200.0;
const MUTE_ONE_HOUR_SECS: u64 = 3600;
const MUTE_EIGHT_HOUR_SECS: u64 = 8 * 3600;

/// Telegram top-bar actions: flat icon (no circular fill).
pub fn header_button_colors(active: bool, hovered: bool) -> (ColorU, ColorU) {
    if active || hovered {
        (ColorU::transparent_black(), theme::accent_cool())
    } else {
        (ColorU::transparent_black(), theme::muted())
    }
}

pub fn header_menu_panel(
    font: FamilyId,
    mute_flyout_open: bool,
    has_custom_wallpaper: bool,
    muted: bool,
    profile_open: bool,
) -> Box<dyn Element> {
    let mut col = Flex::column()
        .with_main_axis_size(MainAxisSize::Min)
        .with_cross_axis_alignment(CrossAxisAlignment::Stretch);
    if muted {
        col.add_child(popover_plain_item(
            font,
            &wormhole_i18n::t("chat.context.unmute"),
            false,
            false,
            |ctx, _, _| {
                ctx.dispatch_typed_action(ChatHeaderAction::Unmute);
                DispatchEventResult::StopPropagation
            },
        ));
    } else {
        col.add_child(popover_plain_item(
            font,
            &wormhole_i18n::t("chat.context.mute"),
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
    }
    col.add_child(popover_menu_separator());
    col.add_child(popover_plain_item_with_id(
        font,
        &wormhole_i18n::t("chat.menu.video_call"),
        Some("chat:video_call"),
        false,
        false,
        |ctx, _, _| {
            ctx.dispatch_typed_action(ChatHeaderAction::VideoCallPrimary);
            DispatchEventResult::StopPropagation
        },
    ));
    col.add_child(popover_plain_item_with_id(
        font,
        &wormhole_i18n::t("chat.menu.remote_desktop"),
        Some("chat:remote_desktop"),
        false,
        false,
        |ctx, _, _| {
            ctx.dispatch_typed_action(ChatHeaderAction::OpenRemoteDesktop);
            DispatchEventResult::StopPropagation
        },
    ));
    if !profile_open {
        col.add_child(popover_plain_item_with_id(
            font,
            &wormhole_i18n::t("chat.menu.view_profile"),
            Some("chat:profile"),
            false,
            false,
            |ctx, _, _| {
                ctx.dispatch_typed_action(ChatHeaderAction::OpenProfile);
                DispatchEventResult::StopPropagation
            },
        ));
    }
    col.add_child(popover_plain_item(
        font,
        &wormhole_i18n::t("chat.menu.set_wallpaper"),
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
            &wormhole_i18n::t("chat.menu.clear_wallpaper"),
            false,
            false,
            |ctx, _, _| {
                ctx.dispatch_typed_action(ChatHeaderAction::ClearWallpaper);
                DispatchEventResult::StopPropagation
            },
        ));
    }
    col.add_child(popover_menu_separator());
    col.add_child(popover_plain_item(
        font,
        &wormhole_i18n::t("chat.menu.clear_history"),
        false,
        false,
        |ctx, _, _| {
            ctx.dispatch_typed_action(ChatHeaderAction::ClearHistory);
            DispatchEventResult::StopPropagation
        },
    ));
    col.add_child(popover_plain_item(
        font,
        &wormhole_i18n::t("chat.context.delete"),
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
    col.add_child(popover_plain_item(
        font,
        &wormhole_i18n::t("chat.mute.one_hour"),
        false,
        false,
        |ctx, _, _| {
            ctx.dispatch_typed_action(ChatHeaderAction::MuteFor {
                seconds: MUTE_ONE_HOUR_SECS,
            });
            DispatchEventResult::StopPropagation
        },
    ));
    col.add_child(popover_plain_item(
        font,
        &wormhole_i18n::t("chat.mute.eight_hours"),
        false,
        false,
        |ctx, _, _| {
            ctx.dispatch_typed_action(ChatHeaderAction::MuteFor {
                seconds: MUTE_EIGHT_HOUR_SECS,
            });
            DispatchEventResult::StopPropagation
        },
    ));
    col.add_child(popover_plain_item(
        font,
        &wormhole_i18n::t("chat.mute.forever"),
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
        .finish()
    })
    .finish();

    let (automation_label, automation_id) = match &action {
        ChatHeaderAction::ToggleThreadSearch => ("搜索消息", "chat:thread_search"),
        ChatHeaderAction::VoiceCallPrimary => ("语音通话", "chat:voice_call"),
        ChatHeaderAction::ToggleProfile => ("会话资料", "chat:info_toggle"),
        ChatHeaderAction::ToggleHeaderMenu => ("更多", "chat:header_menu"),
        _ => ("聊天操作", "chat:header_action"),
    };
    let _ = size;
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
    fn active_uses_transparent_bg() {
        let (bg, icon) = header_button_colors(true, false);
        assert_eq!(bg, ColorU::transparent_black());
        assert_eq!(icon, theme::accent_cool());
    }
}
