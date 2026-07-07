use pathfinder_color::ColorU;
use warpui::elements::{
    Border, ConstrainedBox, Container, CornerRadius, CrossAxisAlignment, DispatchEventResult,
    EventHandler, Flex, MainAxisSize, ParentElement, Radius,
};
use warpui::fonts::FamilyId;
use warpui::Element;

use crate::ui::chat::header::ChatHeaderAction;
use crate::ui::panel_primitives::StatusTone;
use crate::ui::theme;
use crate::ui_text;

const MENU_WIDTH: f32 = 220.0;
const FLYOUT_WIDTH: f32 = 200.0;

pub fn header_menu_panel(
    font: FamilyId,
    mute_flyout_open: bool,
) -> Box<dyn Element> {
    let mut col = Flex::column().with_main_axis_size(MainAxisSize::Min);
    col.add_child(menu_item(
        font,
        "消息免打扰",
        false,
        ChatHeaderAction::ToggleMuteFlyout,
        true,
    ));
    if mute_flyout_open {
        col.add_child(mute_flyout(font));
    }
    col.add_child(menu_separator());
    col.add_child(menu_item(
        font,
        "查看个人资料",
        false,
        ChatHeaderAction::OpenProfile,
        false,
    ));
    col.add_child(menu_item(
        font,
        "设置壁纸",
        false,
        ChatHeaderAction::MenuToast("壁纸设置（演示）".into(), StatusTone::Muted),
        false,
    ));
    col.add_child(menu_item(
        font,
        "禁用文件分享",
        false,
        ChatHeaderAction::MenuToast("已禁用此终端的文件分享（演示）".into(), StatusTone::Muted),
        false,
    ));
    col.add_child(menu_item(
        font,
        "清空历史记录",
        false,
        ChatHeaderAction::ClearHistory,
        false,
    ));
    col.add_child(menu_item(
        font,
        "删除聊天",
        true,
        ChatHeaderAction::DeleteChat,
        false,
    ));

    Container::new(
        ConstrainedBox::new(col.finish())
            .with_min_width(MENU_WIDTH)
            .finish(),
    )
        .with_background(theme::panel())
        .with_border(Border::all(1.0).with_border_fill(theme::border()))
        .with_corner_radius(CornerRadius::with_all(Radius::Pixels(12.0)))
        .finish()
}

fn mute_flyout(font: FamilyId) -> Box<dyn Element> {
    let mut col = Flex::column().with_main_axis_size(MainAxisSize::Min);
    for (label, toast) in [
        ("设置铃声", "铃声设置（演示）"),
        ("关闭通知音", "已关闭此终端通知音"),
        ("设置静音时长…", "静音 1 小时（演示）"),
    ] {
        col.add_child(menu_item(
            font,
            label,
            false,
            ChatHeaderAction::MenuToast(toast.into(), StatusTone::Muted),
            false,
        ));
    }
    col.add_child(menu_item(
        font,
        "永久静音",
        true,
        ChatHeaderAction::MuteForever,
        false,
    ));
    Container::new(
        ConstrainedBox::new(col.finish())
            .with_min_width(FLYOUT_WIDTH)
            .finish(),
    )
    .with_uniform_padding(6.0)
        .with_margin_left(8.0)
        .with_background(theme::panel())
        .with_border(Border::all(1.0).with_border_fill(theme::border()))
        .with_corner_radius(CornerRadius::with_all(Radius::Pixels(12.0)))
        .finish()
}

fn menu_separator() -> Box<dyn Element> {
    ConstrainedBox::new(
        Container::new(Flex::row().finish())
            .with_vertical_margin(4.0)
            .with_horizontal_margin(8.0)
            .with_background(theme::border())
            .finish(),
    )
    .with_height(1.0)
    .finish()
}

fn menu_item(
    font: FamilyId,
    label: &str,
    danger: bool,
    action: ChatHeaderAction,
    has_flyout: bool,
) -> Box<dyn Element> {
    let color = if danger { theme::danger() } else { theme::text() };
    let mut row = Flex::row()
        .with_cross_axis_alignment(CrossAxisAlignment::Center)
        .with_main_axis_size(MainAxisSize::Max);
    row.add_child(
        ui_text::body(label.to_string(), font)
            .with_color(color)
            .finish(),
    );
    if has_flyout {
        row.add_child(
            Container::new(
                ui_text::body("›", font)
                    .with_color(theme::muted())
                    .finish(),
            )
            .with_margin_left(8.0)
            .finish(),
        );
    }
    EventHandler::new(
        Container::new(row.finish())
            .with_uniform_padding(10.0)
            .finish(),
    )
    .on_left_mouse_down(move |ctx, _, _| {
        ctx.dispatch_typed_action(action.clone());
        DispatchEventResult::StopPropagation
    })
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
