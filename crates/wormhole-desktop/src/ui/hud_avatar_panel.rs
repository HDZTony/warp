//! Tab-bar account avatar + balance popover (HTML `.hud-avatar-*` / `.hud-balance-*`).

use pathfinder_color::ColorU;
use warpui::elements::{
    Align, Border, ConstrainedBox, Container, CornerRadius, CrossAxisAlignment,
    DispatchEventResult, EventHandler, Expanded, Flex, MainAxisAlignment, MainAxisSize,
    ParentElement, Radius,
};
use warpui::fonts::FamilyId;
use warpui::Element;

use crate::ui::app_shell::AppShellAction;
use crate::ui::desktop_prefs::format_balance_yuan;
use crate::ui::panel_primitives::{status_line, StatusTone, HUD_RADIUS};
use crate::ui::theme;
use crate::ui_text;

pub const AVATAR_SIZE: f32 = 34.0;
pub const PANEL_WIDTH: f32 = 260.0;

const PURCHASE_ADD_CENTS: i64 = 10_000;

pub fn avatar_letter(authenticated: bool, device_id: Option<&str>) -> char {
    if !authenticated {
        return 'W';
    }
    device_id
        .and_then(|id| id.chars().next())
        .map(|ch| ch.to_uppercase().next().unwrap_or('W'))
        .unwrap_or('W')
}

/// HTML redeem rules: `wormhole`/`wh-` prefix → ¥50; length ≥ 8 → ¥20.
pub fn redeem_cents_for_code(code: &str) -> Option<i64> {
    let trimmed = code.trim();
    if trimmed.is_empty() {
        return None;
    }
    let lower = trimmed.to_lowercase();
    if lower.starts_with("wormhole") || lower.starts_with("wh-") {
        return Some(5_000);
    }
    if trimmed.len() >= 8 {
        return Some(2_000);
    }
    None
}

pub fn purchase_add_cents() -> i64 {
    PURCHASE_ADD_CENTS
}

pub fn build_avatar_slot(
    authenticated: bool,
    device_id: Option<&str>,
    panel_open: bool,
    font: FamilyId,
) -> Box<dyn Element> {
    let letter = avatar_letter(authenticated, device_id);

    let circle = Container::new(
        ConstrainedBox::new(
            Flex::row()
                .with_cross_axis_alignment(CrossAxisAlignment::Center)
                .with_main_axis_alignment(MainAxisAlignment::Center)
                .with_main_axis_size(MainAxisSize::Max)
                .with_child(
                    ui_text::body(letter.to_string(), font)
                        .with_color(theme::accent())
                        .finish(),
                )
                .finish(),
        )
        .with_width(AVATAR_SIZE)
        .with_height(AVATAR_SIZE)
        .finish(),
    )
    .with_background(theme::panel_elevated())
    .with_border(Border::all(1.0).with_border_fill(if panel_open {
        theme::accent_cool()
    } else {
        theme::border()
    }))
    .with_corner_radius(CornerRadius::with_all(Radius::Percentage(50.0)))
    .finish();

    let btn = EventHandler::new(circle)
        .on_left_mouse_down(move |ctx, _, _| {
            if authenticated {
                ctx.dispatch_typed_action(AppShellAction::ToggleAvatarPanel);
            } else {
                ctx.dispatch_typed_action(AppShellAction::OpenLogin);
            }
            DispatchEventResult::StopPropagation
        })
        .finish();

    Container::new(
        ConstrainedBox::new(btn)
            .with_min_width(AVATAR_SIZE)
            .finish(),
    )
    .with_horizontal_padding(8.0)
    .with_border(Border::left(1.0).with_border_fill(theme::border()))
    .finish()
}

pub fn build_avatar_panel(
    device_id: Option<&str>,
    balance_cents: i64,
    balance_feedback: Option<&str>,
    font: FamilyId,
    mono: FamilyId,
) -> Box<dyn Element> {
    let balance_text = format_balance_yuan(balance_cents);
    let user_label = device_id.unwrap_or("未登录").to_string();

    let mut panel = Flex::column()
        .with_cross_axis_alignment(CrossAxisAlignment::Stretch)
        .with_main_axis_size(MainAxisSize::Min);

    let mut head = Flex::column()
        .with_cross_axis_alignment(CrossAxisAlignment::Stretch)
        .with_main_axis_size(MainAxisSize::Min);
    head.add_child(
        ui_text::body("Wormhole 账户", font)
            .with_color(theme::text())
            .finish(),
    );
    head.add_child(
        Container::new(
            ui_text::mono(user_label, mono)
                .with_color(theme::muted())
                .finish(),
        )
        .with_vertical_margin(4.0)
        .finish(),
    );
    panel.add_child(
        Container::new(head.finish())
            .with_padding_bottom(12.0)
            .with_border(Border::bottom(1.0).with_border_fill(theme::border()))
            .finish(),
    );

    let balance_row = Flex::row()
        .with_cross_axis_alignment(CrossAxisAlignment::Center)
        .with_main_axis_size(MainAxisSize::Max)
        .with_main_axis_alignment(MainAxisAlignment::SpaceBetween)
        .with_child(
            ui_text::cluster_ctrl("剩余金额".to_string(), mono)
                .with_color(theme::muted())
                .finish(),
        )
        .with_child(
            ui_text::cluster_label(balance_text, mono)
                .with_color(theme::text())
                .finish(),
        );
    panel.add_child(
        Container::new(balance_row.finish())
            .with_vertical_margin(14.0)
            .finish(),
    );

    let mut actions = Flex::row()
        .with_cross_axis_alignment(CrossAxisAlignment::Stretch)
        .with_main_axis_size(MainAxisSize::Max);
    actions.add_child(
        Expanded::new(
            1.0,
            balance_action_button("购买", true, font, AppShellAction::PurchaseBalance),
        )
        .finish(),
    );
    actions.add_child(
        Container::new(Flex::column().finish())
            .with_horizontal_margin(8.0)
            .finish(),
    );
    actions.add_child(
        Expanded::new(
            1.0,
            balance_action_button("兑换", false, font, AppShellAction::OpenRedeemModal),
        )
        .finish(),
    );
    panel.add_child(
        ConstrainedBox::new(actions.finish())
            .with_width(PANEL_WIDTH)
            .with_height(36.0)
            .finish(),
    );

    if let Some(msg) = balance_feedback {
        panel.add_child(
            Container::new(status_line(msg.to_string(), font, StatusTone::Neutral))
                .with_vertical_margin(10.0)
                .finish(),
        );
    }

    EventHandler::new(
        Container::new(
            ConstrainedBox::new(panel.finish())
                .with_width(PANEL_WIDTH)
                .finish(),
        )
        .with_uniform_padding(16.0)
        .with_background(theme::panel())
        .with_border(Border::all(1.0).with_border_fill(theme::border()))
        .with_corner_radius(CornerRadius::with_all(Radius::Pixels(HUD_RADIUS + 2.0)))
        .finish(),
    )
    .on_left_mouse_down(|_, _, _| DispatchEventResult::StopPropagation)
    .finish()
}

fn balance_action_button(
    label: &str,
    primary: bool,
    font: FamilyId,
    action: AppShellAction,
) -> Box<dyn Element> {
    let (border, bg, color) = if primary {
        (
            theme::accent_cool(),
            theme::accent_cool_bg(24),
            theme::accent_cool(),
        )
    } else {
        (theme::border_bright(), ColorU::new(0, 0, 0, 0), theme::text())
    };
    EventHandler::new(
        Container::new(
            ConstrainedBox::new(
                Flex::row()
                    .with_cross_axis_alignment(CrossAxisAlignment::Center)
                    .with_main_axis_alignment(MainAxisAlignment::Center)
                    .with_main_axis_size(MainAxisSize::Max)
                    .with_child(
                        ui_text::body(label.to_string(), font)
                            .with_color(color)
                            .finish(),
                    )
                    .finish(),
            )
            .with_height(36.0)
            .finish(),
        )
        .with_background(bg)
        .with_border(Border::all(1.0).with_border_fill(border))
        .with_corner_radius(CornerRadius::with_all(Radius::Pixels(HUD_RADIUS)))
        .finish(),
    )
    .on_left_mouse_down(move |ctx, _, _| {
        ctx.dispatch_typed_action(action.clone());
        DispatchEventResult::StopPropagation
    })
    .finish()
}

pub fn build_redeem_modal(
    draft: &str,
    feedback: Option<&str>,
    font: FamilyId,
    mono: FamilyId,
) -> Box<dyn Element> {
    let preview = if draft.is_empty() {
        "输入或粘贴兑换码".to_string()
    } else {
        draft.to_string()
    };

    let mut dialog = Flex::column().with_cross_axis_alignment(CrossAxisAlignment::Stretch);
    dialog.add_child(
        ui_text::title("兑换", font)
            .with_color(theme::text())
            .finish(),
    );
    dialog.add_child(
        Container::new(
            ui_text::body("输入兑换码以增加余额。", font)
                .with_color(theme::muted())
                .finish(),
        )
        .with_vertical_margin(8.0)
        .finish(),
    );
    dialog.add_child(
        Container::new(
            ui_text::mono(preview, mono)
                .with_color(theme::text())
                .finish(),
        )
        .with_uniform_padding(10.0)
        .with_vertical_margin(6.0)
        .with_background(theme::canvas())
        .with_border(Border::all(1.0).with_border_fill(theme::border()))
        .with_corner_radius(CornerRadius::with_all(Radius::Pixels(HUD_RADIUS)))
        .finish(),
    );
    dialog.add_child(
        Container::new(
            Flex::row()
                .with_main_axis_size(MainAxisSize::Max)
                .with_child(
                    Expanded::new(
                        1.0,
                        redeem_button("从剪贴板粘贴", false, font, AppShellAction::PasteRedeemCode),
                    )
                    .finish(),
                )
                .finish(),
        )
        .with_vertical_margin(10.0)
        .finish(),
    );

    let mut actions = Flex::row()
        .with_cross_axis_alignment(CrossAxisAlignment::Center)
        .with_main_axis_alignment(MainAxisAlignment::End)
        .with_main_axis_size(MainAxisSize::Max);
    actions.add_child(redeem_button("取消", false, font, AppShellAction::CloseRedeemModal));
    actions.add_child(
        Container::new(Flex::column().finish())
            .with_horizontal_margin(8.0)
            .finish(),
    );
    actions.add_child(redeem_button("兑换", true, font, AppShellAction::SubmitRedeem));
    dialog.add_child(
        Container::new(actions.finish())
            .with_vertical_margin(12.0)
            .finish(),
    );
    if let Some(msg) = feedback {
        dialog.add_child(status_line(msg.to_string(), font, StatusTone::Warn));
    }

    let panel = EventHandler::new(
        Container::new(
            ConstrainedBox::new(dialog.finish())
                .with_width(400.0)
                .finish(),
        )
        .with_uniform_padding(24.0)
        .with_background(theme::panel())
        .with_border(Border::all(1.0).with_border_fill(theme::border_bright()))
        .with_corner_radius(CornerRadius::with_all(Radius::Pixels(HUD_RADIUS)))
        .finish(),
    )
    .on_left_mouse_down(|_, _, _| DispatchEventResult::StopPropagation)
    .finish();

    let scrim = Container::new(
        Align::new(Container::new(panel).with_uniform_padding(24.0).finish()).finish(),
    )
    .with_background(ColorU::new(8, 7, 11, 180))
    .finish();

    EventHandler::new(scrim)
        .on_left_mouse_down(|ctx, _, _| {
            ctx.dispatch_typed_action(AppShellAction::CloseRedeemModal);
            DispatchEventResult::StopPropagation
        })
        .finish()
}

fn redeem_button(
    label: &str,
    primary: bool,
    font: FamilyId,
    action: AppShellAction,
) -> Box<dyn Element> {
    let (border, bg, color) = if primary {
        (
            theme::accent_cool(),
            theme::accent_cool_bg(40),
            theme::accent_cool(),
        )
    } else {
        (theme::border_bright(), theme::panel(), theme::text())
    };
    EventHandler::new(
        Container::new(
            ConstrainedBox::new(
                ui_text::body(label.to_string(), font)
                    .with_color(color)
                    .finish(),
            )
            .with_min_width(72.0)
            .with_height(36.0)
            .finish(),
        )
        .with_horizontal_padding(12.0)
        .with_background(bg)
        .with_border(Border::all(1.0).with_border_fill(border))
        .with_corner_radius(CornerRadius::with_all(Radius::Pixels(HUD_RADIUS)))
        .finish(),
    )
    .on_left_mouse_down(move |ctx, _, _| {
        ctx.dispatch_typed_action(action.clone());
        DispatchEventResult::StopPropagation
    })
    .finish()
}
