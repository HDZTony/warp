use pathfinder_color::ColorU;
use warpui::elements::{
    Container, CornerRadius, CrossAxisAlignment, DispatchEventResult, EventHandler, Flex,
    MainAxisSize, ParentElement, Radius,
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

pub fn header_menu_panel(
    font: FamilyId,
    mute_flyout_open: bool,
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
            ctx.dispatch_typed_action(ChatHeaderAction::MenuToast(
                "壁纸设置（演示）".into(),
                StatusTone::Muted,
            ));
            DispatchEventResult::StopPropagation
        },
    ));
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
    Container::new(
        popover_shell(
            FLYOUT_WIDTH,
            Container::new(col.finish())
                .with_uniform_padding(6.0)
                .finish(),
        ),
    )
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
    let bg = if active {
        theme::accent_cool_bg(32)
    } else {
        ColorU::transparent_black()
    };
    let icon_color = if active {
        theme::accent_cool()
    } else {
        color
    };
    EventHandler::new(
        Container::new(
            warpui::elements::Align::new(crate::ui::icons::chat_header_icon(icon_path, icon_color))
                .finish(),
        )
        .with_background(bg)
        .with_corner_radius(CornerRadius::with_all(Radius::Pixels(size / 2.0)))
        .finish(),
    )
    .on_left_mouse_down(move |ctx, _, _| {
        ctx.dispatch_typed_action(action.clone());
        DispatchEventResult::StopPropagation
    })
    .finish()
}
