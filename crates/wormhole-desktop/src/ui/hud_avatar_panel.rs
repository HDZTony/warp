//! Tab-bar account avatar + balance popover (HTML `.hud-avatar-*` / `.hud-balance-*`).

use pathfinder_color::ColorU;
use warpui::Element;
use warpui::elements::Fill;
use warpui::elements::{
    Align, Border, ClippedScrollStateHandle, ClippedScrollable, ConstrainedBox, Container,
    CornerRadius, CrossAxisAlignment, DispatchEventResult, EventHandler, Expanded, Flex, Image,
    MainAxisAlignment, MainAxisSize, ParentElement, Radius, ScrollbarWidth,
};
use warpui::fonts::FamilyId;
use warpui_core::assets::asset_cache::AssetSource;
use warpui_core::image_cache::CacheOption;

use crate::ui::app_shell::{AppShellAction, RedeemTab};
use crate::ui::desktop_prefs::{format_account_balance, format_redeem_amount, RedeemHistoryEntry};
use crate::ui::panel_primitives::{HUD_RADIUS, StatusTone, status_line};
use crate::ui::text_field_input::{
    TextFieldInput, render_field_with_caret, wrap_text_field_focus_on_click,
};
use crate::ui::theme;
use crate::ui_text;

pub const AVATAR_SIZE: f32 = 34.0;
pub const PANEL_WIDTH: f32 = 260.0;

const REDEEM_MODAL_WIDTH: f32 = 480.0;
const REDEEM_JOIN_BTN_HEIGHT: f32 = 34.0;
const REDEEM_ACTION_BTN_HEIGHT: f32 = 36.0;
const REDEEM_HISTORY_MAX_HEIGHT: f32 = 280.0;
const PURCHASE_MODAL_WIDTH: f32 = 560.0;
const QR_SIZE: f32 = 128.0;

#[derive(Clone, Debug)]
pub struct PurchaseProductUi {
    pub product_id: String,
    pub label: String,
    pub pay_url: String,
    pub qr_asset_id: String,
    pub qr_loaded: bool,
}

pub fn avatar_letter(authenticated: bool, device_id: Option<&str>) -> char {
    if !authenticated {
        return 'W';
    }
    device_id
        .and_then(|id| id.chars().next())
        .map(|ch| ch.to_uppercase().next().unwrap_or('W'))
        .unwrap_or('W')
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
    balance_credits: i64,
    busy: bool,
    balance_feedback: Option<&str>,
    font: FamilyId,
    mono: FamilyId,
) -> Box<dyn Element> {
    let balance_text = if busy {
        "同步中…".to_string()
    } else {
        format_account_balance(balance_credits)
    };
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
            ui_text::cluster_ctrl("账户余额".to_string(), mono)
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

pub fn build_purchase_modal(
    products: &[PurchaseProductUi],
    busy: bool,
    feedback: Option<&str>,
    font: FamilyId,
    mono: FamilyId,
) -> Box<dyn Element> {
    let mut dialog = Flex::column().with_cross_axis_alignment(CrossAxisAlignment::Stretch);
    dialog.add_child(
        ui_text::title("购买金额", font)
            .with_color(theme::text())
            .finish(),
    );
    dialog.add_child(
        Container::new(
            ui_text::body(
                "选择金额后扫码或打开链接付款，付款后使用商家提供的卡密兑换到账户余额。",
                font,
            )
            .with_color(theme::muted())
            .finish(),
        )
        .with_vertical_margin(8.0)
        .finish(),
    );

    if busy && products.is_empty() {
        dialog.add_child(
            Container::new(status_line(
                "正在读取商品配置…",
                font,
                StatusTone::Placeholder,
            ))
            .with_margin_top(12.0)
            .finish(),
        );
    } else if products.is_empty() {
        dialog.add_child(
            Container::new(status_line("暂无可购买商品", font, StatusTone::Warn))
                .with_margin_top(12.0)
                .finish(),
        );
    } else {
        let mut grid = Flex::row()
            .with_cross_axis_alignment(CrossAxisAlignment::Stretch)
            .with_main_axis_size(MainAxisSize::Max);
        for (index, product) in products.iter().enumerate() {
            if index > 0 {
                grid.add_child(
                    Container::new(Flex::column().finish())
                        .with_horizontal_margin(6.0)
                        .finish(),
                );
            }
            grid.add_child(Expanded::new(1.0, purchase_product_card(product, font, mono)).finish());
        }
        dialog.add_child(Container::new(grid.finish()).with_margin_top(16.0).finish());
    }

    if let Some(msg) = feedback {
        dialog.add_child(
            Container::new(status_line(msg.to_string(), font, StatusTone::Neutral))
                .with_margin_top(12.0)
                .finish(),
        );
    }

    let mut actions = Flex::row()
        .with_cross_axis_alignment(CrossAxisAlignment::Center)
        .with_main_axis_alignment(MainAxisAlignment::End)
        .with_main_axis_size(MainAxisSize::Max);
    actions.add_child(redeem_button(
        "刷新",
        false,
        font,
        AppShellAction::RefreshCreditProducts,
        REDEEM_ACTION_BTN_HEIGHT,
    ));
    actions.add_child(
        Container::new(Flex::column().finish())
            .with_horizontal_margin(8.0)
            .finish(),
    );
    actions.add_child(redeem_button(
        "关闭",
        true,
        font,
        AppShellAction::ClosePurchaseModal,
        REDEEM_ACTION_BTN_HEIGHT,
    ));
    dialog.add_child(
        Container::new(actions.finish())
            .with_margin_top(16.0)
            .finish(),
    );

    let panel = EventHandler::new(
        Container::new(
            ConstrainedBox::new(dialog.finish())
                .with_width(PURCHASE_MODAL_WIDTH)
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
            ctx.dispatch_typed_action(AppShellAction::ClosePurchaseModal);
            DispatchEventResult::StopPropagation
        })
        .finish()
}

fn purchase_product_card(
    product: &PurchaseProductUi,
    font: FamilyId,
    mono: FamilyId,
) -> Box<dyn Element> {
    let mut card = Flex::column()
        .with_cross_axis_alignment(CrossAxisAlignment::Stretch)
        .with_main_axis_size(MainAxisSize::Min);
    card.add_child(
        ui_text::cluster_label(product.label.clone(), mono)
            .with_color(theme::accent_cool())
            .finish(),
    );
    card.add_child(
        Container::new(
            ui_text::mono(product.product_id.clone(), mono)
                .with_color(theme::muted())
                .finish(),
        )
        .with_margin_top(4.0)
        .finish(),
    );

    let qr_content: Box<dyn Element> = if product.qr_loaded {
        Image::new(
            AssetSource::Raw {
                id: product.qr_asset_id.clone(),
            },
            CacheOption::BySize,
        )
        .finish()
    } else {
        Align::new(
            ui_text::body("二维码待同步".to_string(), font)
                .with_color(theme::muted())
                .finish(),
        )
        .finish()
    };
    card.add_child(
        Container::new(
            ConstrainedBox::new(qr_content)
                .with_width(QR_SIZE)
                .with_height(QR_SIZE)
                .finish(),
        )
        .with_uniform_padding(8.0)
        .with_margin_top(12.0)
        .with_background(theme::canvas())
        .with_border(Border::all(1.0).with_border_fill(theme::border()))
        .with_corner_radius(CornerRadius::with_all(Radius::Pixels(HUD_RADIUS)))
        .finish(),
    );
    card.add_child(
        Container::new(
            ui_text::mono(product.pay_url.clone(), mono)
                .with_color(theme::muted())
                .finish(),
        )
        .with_margin_top(8.0)
        .finish(),
    );

    let mut actions = Flex::row()
        .with_cross_axis_alignment(CrossAxisAlignment::Stretch)
        .with_main_axis_size(MainAxisSize::Max);
    actions.add_child(
        Expanded::new(
            1.0,
            redeem_button(
                "复制",
                false,
                font,
                AppShellAction::CopyPurchaseLink(product.pay_url.clone()),
                32.0,
            ),
        )
        .finish(),
    );
    actions.add_child(
        Container::new(Flex::column().finish())
            .with_horizontal_margin(5.0)
            .finish(),
    );
    actions.add_child(
        Expanded::new(
            1.0,
            redeem_button(
                "打开",
                true,
                font,
                AppShellAction::OpenPurchaseLink(product.pay_url.clone()),
                32.0,
            ),
        )
        .finish(),
    );
    card.add_child(
        Container::new(actions.finish())
            .with_margin_top(12.0)
            .finish(),
    );

    Container::new(card.finish())
        .with_uniform_padding(14.0)
        .with_background(theme::panel_elevated())
        .with_border(Border::all(1.0).with_border_fill(theme::border()))
        .with_corner_radius(CornerRadius::with_all(Radius::Pixels(HUD_RADIUS)))
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
        (
            theme::border_bright(),
            ColorU::new(0, 0, 0, 0),
            theme::text(),
        )
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
    active_tab: RedeemTab,
    draft: &str,
    marked: &str,
    focused: bool,
    caret_visible: bool,
    feedback: Option<&str>,
    feedback_tone: StatusTone,
    busy: bool,
    history_busy: bool,
    history_error: Option<&str>,
    history: &[RedeemHistoryEntry],
    history_scroll: &ClippedScrollStateHandle,
    font: FamilyId,
    mono: FamilyId,
) -> Box<dyn Element> {
    let mut dialog = Flex::column().with_cross_axis_alignment(CrossAxisAlignment::Stretch);
    dialog.add_child(redeem_tab_bar(active_tab, font));

    match active_tab {
        RedeemTab::Redeem => dialog.add_child(redeem_balance_panel(
            draft,
            marked,
            focused,
            caret_visible,
            feedback,
            feedback_tone,
            busy,
            font,
            mono,
        )),
        RedeemTab::History => dialog.add_child(redeem_history_panel(
            history,
            history_scroll,
            history_busy,
            history_error,
            font,
            mono,
        )),
    }

    let panel = EventHandler::new(
        Container::new(
            ConstrainedBox::new(dialog.finish())
                .with_width(REDEEM_MODAL_WIDTH)
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

fn redeem_tab_bar(active_tab: RedeemTab, font: FamilyId) -> Box<dyn Element> {
    let mut bar = Flex::row()
        .with_cross_axis_alignment(CrossAxisAlignment::Stretch)
        .with_main_axis_size(MainAxisSize::Max);
    bar.add_child(
        Expanded::new(
            1.0,
            redeem_tab_button("兑换余额", active_tab == RedeemTab::Redeem, RedeemTab::Redeem, font),
        )
        .finish(),
    );
    bar.add_child(
        Container::new(Flex::column().finish())
            .with_horizontal_margin(4.0)
            .finish(),
    );
    bar.add_child(
        Expanded::new(
            1.0,
            redeem_tab_button("兑换记录", active_tab == RedeemTab::History, RedeemTab::History, font),
        )
        .finish(),
    );
    Container::new(bar.finish())
        .with_uniform_padding(3.0)
        .with_margin_bottom(16.0)
        .with_background(theme::canvas())
        .with_border(Border::all(1.0).with_border_fill(theme::border()))
        .with_corner_radius(CornerRadius::with_all(Radius::Pixels(HUD_RADIUS)))
        .finish()
}

fn redeem_tab_button(
    label: &str,
    active: bool,
    tab: RedeemTab,
    font: FamilyId,
) -> Box<dyn Element> {
    let (bg, color, border) = if active {
        (
            theme::accent_cool_bg_default(),
            theme::accent_cool(),
            theme::accent_cool(),
        )
    } else {
        (
            ColorU::new(0, 0, 0, 0),
            theme::muted(),
            ColorU::new(0, 0, 0, 0),
        )
    };
    let mut container = Container::new(
        ConstrainedBox::new(
            Flex::row()
                .with_cross_axis_alignment(CrossAxisAlignment::Center)
                .with_main_axis_alignment(MainAxisAlignment::Center)
                .with_child(
                    ui_text::body(label.to_string(), font)
                        .with_color(color)
                        .finish(),
                )
                .finish(),
        )
        .with_height(34.0)
        .finish(),
    )
    .with_vertical_padding(8.0)
    .with_horizontal_padding(12.0)
    .with_background(bg)
    .with_corner_radius(CornerRadius::with_all(Radius::Pixels(HUD_RADIUS)));
    if active {
        container = container.with_border(Border::all(1.0).with_border_fill(border));
    }
    EventHandler::new(container.finish())
        .on_left_mouse_down(move |ctx, _, _| {
            ctx.dispatch_typed_action(AppShellAction::SwitchRedeemTab(tab));
            DispatchEventResult::StopPropagation
        })
        .finish()
}

fn redeem_balance_panel(
    draft: &str,
    marked: &str,
    focused: bool,
    caret_visible: bool,
    feedback: Option<&str>,
    feedback_tone: StatusTone,
    busy: bool,
    font: FamilyId,
    mono: FamilyId,
) -> Box<dyn Element> {
    let field = render_field_with_caret(
        draft,
        marked,
        "WORMHOLE-XXXX 或 WH-XXXX",
        mono,
        focused,
        false,
        caret_visible,
    );
    let input = wrap_text_field_focus_on_click(
        TextFieldInput::builder(field, |ctx, action| {
            ctx.dispatch_typed_action(AppShellAction::RedeemCodeEdit(action));
        })
        .focused(focused)
        .ime_preedit(!marked.is_empty())
        .on_keydown(|ctx, keystroke| match keystroke.key.as_str() {
            "enter" | "return" => {
                ctx.dispatch_typed_action(AppShellAction::SubmitRedeem);
                DispatchEventResult::StopPropagation
            }
            "escape" => {
                ctx.dispatch_typed_action(AppShellAction::CloseRedeemModal);
                DispatchEventResult::StopPropagation
            }
            _ => DispatchEventResult::PropagateToParent,
        })
        .finish(),
        |ctx| ctx.dispatch_typed_action(AppShellAction::FocusRedeemCode),
    );

    let mut panel = Flex::column().with_cross_axis_alignment(CrossAxisAlignment::Stretch);
    panel.add_child(
        ui_text::title("兑换余额", font)
            .with_color(theme::text())
            .finish(),
    );
    panel.add_child(
        Container::new(
            ui_text::body("输入兑换码，确认后将充值到账户剩余金额。", font)
                .with_color(theme::muted())
                .finish(),
        )
        .with_margin_bottom(16.0)
        .finish(),
    );
    panel.add_child(
        Container::new(
            ui_text::cluster_ctrl("兑换码".to_string(), mono)
                .with_color(theme::muted())
                .finish(),
        )
        .with_margin_bottom(6.0)
        .finish(),
    );
    panel.add_child(
        Container::new(input)
            .with_vertical_padding(10.0)
            .with_horizontal_padding(12.0)
            .with_background(theme::canvas())
            .with_border(Border::all(1.0).with_border_fill(theme::border()))
            .with_corner_radius(CornerRadius::with_all(Radius::Pixels(HUD_RADIUS)))
            .finish(),
    );

    let submit_label = if busy { "验证中…" } else { "确定" };
    let mut actions = Flex::row()
        .with_cross_axis_alignment(CrossAxisAlignment::Center)
        .with_main_axis_alignment(MainAxisAlignment::End)
        .with_main_axis_size(MainAxisSize::Max);
    actions.add_child(modal_cancel_button(
        "取消",
        font,
        AppShellAction::CloseRedeemModal,
        true,
    ));
    actions.add_child(
        Container::new(Flex::column().finish())
            .with_horizontal_margin(8.0)
            .finish(),
    );
    actions.add_child(modal_submit_button(
        submit_label,
        mono,
        AppShellAction::SubmitRedeem,
        !busy,
    ));
    panel.add_child(
        Container::new(actions.finish())
            .with_margin_top(18.0)
            .finish(),
    );
    if let Some(msg) = feedback {
        panel.add_child(
            Container::new(status_line(msg.to_string(), font, feedback_tone))
                .with_margin_top(12.0)
                .finish(),
        );
    }
    panel.finish()
}

fn redeem_history_panel(
    history: &[RedeemHistoryEntry],
    history_scroll: &ClippedScrollStateHandle,
    history_busy: bool,
    history_error: Option<&str>,
    font: FamilyId,
    mono: FamilyId,
) -> Box<dyn Element> {
    let mut panel = Flex::column().with_cross_axis_alignment(CrossAxisAlignment::Stretch);
    panel.add_child(
        ui_text::title("兑换记录", font)
            .with_color(theme::text())
            .finish(),
    );
    panel.add_child(
        Container::new(
            ui_text::body("查看历史兑换时间、卡号与到账金额。", font)
                .with_color(theme::muted())
                .finish(),
        )
        .with_margin_bottom(16.0)
        .finish(),
    );
    if history_busy {
        panel.add_child(
            Container::new(status_line(
                "正在同步兑换记录…",
                font,
                StatusTone::Placeholder,
            ))
            .with_margin_bottom(12.0)
            .finish(),
        );
    } else if let Some(err) = history_error {
        panel.add_child(
            Container::new(status_line(err.to_string(), font, StatusTone::Danger))
                .with_margin_bottom(12.0)
                .finish(),
        );
    }
    panel.add_child(redeem_history_table(
        history,
        history_scroll,
        history_busy,
        history_error.is_some(),
        mono,
    ));
    panel.add_child(
        Container::new(
            Flex::row()
                .with_main_axis_alignment(MainAxisAlignment::End)
                .with_child(modal_cancel_button(
                    "关闭",
                    font,
                    AppShellAction::CloseRedeemModal,
                    true,
                ))
                .finish(),
        )
        .with_margin_top(16.0)
        .finish(),
    );
    panel.finish()
}

fn redeem_history_table(
    history: &[RedeemHistoryEntry],
    history_scroll: &ClippedScrollStateHandle,
    history_busy: bool,
    history_error: bool,
    mono: FamilyId,
) -> Box<dyn Element> {
    let mut table = Flex::column().with_cross_axis_alignment(CrossAxisAlignment::Stretch);
    table.add_child(redeem_history_header_row(mono));
    if history_busy {
        table.add_child(
            Container::new(
                Align::new(
                    ui_text::body("正在同步兑换记录…".to_string(), mono)
                        .with_color(theme::muted())
                        .finish(),
                )
                .finish(),
            )
            .with_vertical_padding(28.0)
            .with_horizontal_padding(12.0)
            .finish(),
        );
    } else if history.is_empty() && !history_error {
        table.add_child(
            Container::new(
                Align::new(
                    ui_text::body("暂无兑换记录".to_string(), mono)
                        .with_color(theme::muted())
                        .finish(),
                )
                .finish(),
            )
            .with_vertical_padding(28.0)
            .with_horizontal_padding(12.0)
            .finish(),
        );
    } else {
        for entry in history {
            table.add_child(redeem_history_row(entry, mono));
        }
    }

    let scroll = ClippedScrollable::vertical(
        history_scroll.clone(),
        ConstrainedBox::new(table.finish())
            .with_max_height(REDEEM_HISTORY_MAX_HEIGHT)
            .finish(),
        ScrollbarWidth::Auto,
        Fill::None,
        Fill::None,
        Fill::None,
    )
    .finish();

    Container::new(scroll)
        .with_margin_top(4.0)
        .with_background(theme::canvas())
        .with_border(Border::all(1.0).with_border_fill(theme::border()))
        .with_corner_radius(CornerRadius::with_all(Radius::Pixels(HUD_RADIUS)))
        .finish()
}

fn redeem_history_header_row(mono: FamilyId) -> Box<dyn Element> {
    Flex::row()
        .with_cross_axis_alignment(CrossAxisAlignment::Center)
        .with_main_axis_size(MainAxisSize::Max)
        .with_child(
            Expanded::new(
                1.1,
                history_cell("兑换时间", mono, theme::muted(), CrossAxisAlignment::Start),
            )
            .finish(),
        )
        .with_child(
            Expanded::new(
                1.4,
                history_cell("兑换卡号", mono, theme::muted(), CrossAxisAlignment::Start),
            )
            .finish(),
        )
        .with_child(
            Expanded::new(
                0.8,
                history_cell("金额", mono, theme::muted(), CrossAxisAlignment::End),
            )
            .finish(),
        )
        .finish()
}

fn redeem_history_row(entry: &RedeemHistoryEntry, mono: FamilyId) -> Box<dyn Element> {
    Container::new(
        Flex::row()
            .with_cross_axis_alignment(CrossAxisAlignment::Center)
            .with_main_axis_size(MainAxisSize::Max)
            .with_child(
                Expanded::new(
                    1.1,
                    history_cell(&entry.time, mono, theme::muted(), CrossAxisAlignment::Start),
                )
                .finish(),
            )
            .with_child(
                Expanded::new(
                    1.4,
                    history_cell(&entry.code, mono, theme::accent_cool(), CrossAxisAlignment::Start),
                )
                .finish(),
            )
            .with_child(
                Expanded::new(
                    0.8,
                    history_cell(
                        &format_redeem_amount(entry.amount_credits),
                        mono,
                        theme::accent(),
                        CrossAxisAlignment::End,
                    ),
                )
                .finish(),
            )
            .finish(),
    )
    .with_vertical_padding(10.0)
    .with_horizontal_padding(8.0)
    .with_border(Border::bottom(1.0).with_border_fill(theme::border()))
    .finish()
}

fn history_cell(
    label: &str,
    mono: FamilyId,
    color: ColorU,
    align: CrossAxisAlignment,
) -> Box<dyn Element> {
    let row = Flex::row()
        .with_cross_axis_alignment(CrossAxisAlignment::Center)
        .with_main_axis_size(MainAxisSize::Max)
        .with_main_axis_alignment(match align {
            CrossAxisAlignment::End => MainAxisAlignment::End,
            _ => MainAxisAlignment::Start,
        })
        .with_child(
            ui_text::mono(label.to_string(), mono)
                .with_color(color)
                .finish(),
        );
    row.finish()
}

fn modal_cancel_button(
    label: &str,
    font: FamilyId,
    action: AppShellAction,
    enabled: bool,
) -> Box<dyn Element> {
    let color = if enabled {
        theme::muted()
    } else {
        theme::placeholder()
    };
    let border = if enabled {
        theme::border()
    } else {
        theme::border()
    };
    let button = EventHandler::new(
        Container::new(
            ConstrainedBox::new(
                Flex::row()
                    .with_cross_axis_alignment(CrossAxisAlignment::Center)
                    .with_main_axis_alignment(MainAxisAlignment::Center)
                    .with_child(
                        ui_text::body(label.to_string(), font)
                            .with_color(color)
                            .finish(),
                    )
                    .finish(),
            )
            .with_height(REDEEM_JOIN_BTN_HEIGHT)
            .with_min_width(72.0)
            .finish(),
        )
        .with_horizontal_padding(14.0)
        .with_background(ColorU::new(0, 0, 0, 0))
        .with_border(Border::all(1.0).with_border_fill(border))
        .with_corner_radius(CornerRadius::with_all(Radius::Pixels(HUD_RADIUS)))
        .finish(),
    );
    if enabled {
        button
            .on_left_mouse_down(move |ctx, _, _| {
                ctx.dispatch_typed_action(action.clone());
                DispatchEventResult::StopPropagation
            })
            .finish()
    } else {
        button.finish()
    }
}

fn modal_submit_button(
    label: &str,
    mono: FamilyId,
    action: AppShellAction,
    enabled: bool,
) -> Box<dyn Element> {
    let (border, bg, color) = if enabled {
        (
            theme::accent(),
            theme::accent_bg(40),
            theme::accent(),
        )
    } else {
        (
            theme::border(),
            theme::panel(),
            theme::placeholder(),
        )
    };
    let button = EventHandler::new(
        Container::new(
            ConstrainedBox::new(
                Flex::row()
                    .with_cross_axis_alignment(CrossAxisAlignment::Center)
                    .with_main_axis_alignment(MainAxisAlignment::Center)
                    .with_child(
                        ui_text::cluster_ctrl(label.to_string(), mono)
                            .with_color(color)
                            .finish(),
                    )
                    .finish(),
            )
            .with_height(REDEEM_JOIN_BTN_HEIGHT)
            .with_min_width(80.0)
            .finish(),
        )
        .with_horizontal_padding(16.0)
        .with_background(bg)
        .with_border(Border::all(1.0).with_border_fill(border))
        .with_corner_radius(CornerRadius::with_all(Radius::Pixels(HUD_RADIUS)))
        .finish(),
    );
    if enabled {
        button
            .on_left_mouse_down(move |ctx, _, _| {
                ctx.dispatch_typed_action(action.clone());
                DispatchEventResult::StopPropagation
            })
            .finish()
    } else {
        button.finish()
    }
}

fn redeem_button(
    label: &str,
    primary: bool,
    font: FamilyId,
    action: AppShellAction,
    height: f32,
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
                Flex::row()
                    .with_cross_axis_alignment(CrossAxisAlignment::Center)
                    .with_main_axis_alignment(MainAxisAlignment::Center)
                    .with_child(
                        ui_text::body(label.to_string(), font)
                            .with_color(color)
                            .finish(),
                    )
                    .finish(),
            )
            .with_min_width(72.0)
            .with_height(height)
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
