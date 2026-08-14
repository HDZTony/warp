//! Telegram-style image lightbox + context menu for chat attachments.

use std::path::PathBuf;

use pathfinder_color::ColorU;
use warpui::elements::{
    Align, AutomationTarget, ConstrainedBox, Container, CornerRadius, CrossAxisAlignment,
    DispatchEventResult, EventHandler, Expanded, Flex, Image, MainAxisAlignment, MainAxisSize,
    ParentElement, Radius,
};
use warpui::fonts::FamilyId;
use warpui::Element;
use warpui_core::assets::asset_cache::AssetSource;
use warpui_core::image_cache::CacheOption;

use crate::ui::chat::layout::{fit_attachment_preview, ATTACH_PREVIEW_MAX_EDGE};
use crate::ui::chat::shell_state::{
    ChatShellState, ForwardDraft, ImageContextMenu, ImageViewerState, ReplyDraft,
    SharedChatShellState,
};
use crate::ui::clipboard::write_clipboard_image_from_path;
use crate::ui::panel_primitives::{popover_plain_item, popover_shell, StatusTone};
use crate::ui::theme;
use crate::ui_text;
use wormhole_desktop_core::cluster_commands::reveal_path_in_folder;

pub fn image_viewer_overlay(
    font: FamilyId,
    viewer: &ImageViewerState,
    shell_state: SharedChatShellState,
) -> Box<dyn Element> {
    let max_w = 720.0_f32;
    let max_h = 520.0_f32;
    let (w, h) = match (viewer.source_width, viewer.source_height) {
        (Some(sw), Some(sh)) => fit_attachment_preview(sw, sh, max_w, max_h),
        _ => (ATTACH_PREVIEW_MAX_EDGE, ATTACH_PREVIEW_MAX_EDGE),
    };

    let image: Box<dyn Element> = if let Some(asset_id) = &viewer.asset_id {
        ConstrainedBox::new(
            Image::new(
                AssetSource::Raw {
                    id: asset_id.clone(),
                },
                CacheOption::BySize,
            )
            .finish(),
        )
        .with_width(w)
        .with_height(h)
        .finish()
    } else {
        ConstrainedBox::new(
            Align::new(
                ui_text::body(wormhole_i18n::t("chat.image.not_ready"), font)
                    .with_color(theme::muted())
                    .finish(),
            )
            .finish(),
        )
        .with_width(w)
        .with_height(h.min(160.0))
        .finish()
    };

    let toolbar = Flex::row()
        .with_main_axis_alignment(MainAxisAlignment::End)
        .with_cross_axis_alignment(CrossAxisAlignment::Center)
        .with_main_axis_size(MainAxisSize::Max)
        .with_child(action_btn(
            font,
            wormhole_i18n::t("chat.image.save"),
            "chat:image_save",
            shell_state.clone(),
            save_image,
        ))
        .with_child(action_btn(
            font,
            wormhole_i18n::t("chat.image.copy"),
            "chat:image_copy",
            shell_state.clone(),
            copy_image,
        ))
        .with_child(action_btn(
            font,
            wormhole_i18n::t("chat.image.reveal"),
            "chat:image_reveal",
            shell_state.clone(),
            reveal_image,
        ))
        .with_child(action_btn(
            font,
            wormhole_i18n::t("chat.image.reply"),
            "chat:image_reply",
            shell_state.clone(),
            reply_image,
        ))
        .with_child(action_btn(
            font,
            wormhole_i18n::t("chat.image.forward"),
            "chat:image_forward",
            shell_state.clone(),
            forward_image,
        ))
        .with_child(action_btn(
            font,
            wormhole_i18n::t("chat.image.close"),
            "chat:image_close",
            shell_state.clone(),
            close_viewer,
        ))
        .finish();

    let content = Flex::column()
        .with_main_axis_size(MainAxisSize::Max)
        .with_cross_axis_alignment(CrossAxisAlignment::Stretch)
        .with_child(Container::new(toolbar).with_uniform_padding(12.0).finish())
        .with_child(Expanded::new(1.0, Align::new(image).finish()).finish())
        .finish();

    let shell_for_close = shell_state;
    let backdrop = EventHandler::new(
        Container::new(content)
            .with_background(ColorU::new(0, 0, 0, 210))
            .finish(),
    )
    .skip_automation()
    .on_left_mouse_down(move |ctx, _, _| {
        if let Ok(mut state) = shell_for_close.lock() {
            state.close_image_viewer();
        }
        ctx.notify();
        DispatchEventResult::StopPropagation
    })
    .finish();

    AutomationTarget::new(backdrop)
        .with_label(wormhole_i18n::t("chat.image.viewer"))
        .with_id("chat:image_viewer")
        .finish()
}

fn action_btn(
    font: FamilyId,
    label: String,
    id: &str,
    shell_state: SharedChatShellState,
    on_click: fn(&mut ChatShellState),
) -> Box<dyn Element> {
    AutomationTarget::new(
        EventHandler::new(
            Container::new(
                ui_text::chat_bubble_meta(label.clone(), font)
                    .with_color(ColorU::white())
                    .finish(),
            )
            .with_horizontal_padding(10.0)
            .with_vertical_padding(6.0)
            .with_margin_left(6.0)
            .with_background(ColorU::new(255, 255, 255, 28))
            .with_corner_radius(CornerRadius::with_all(Radius::Pixels(8.0)))
            .finish(),
        )
        .skip_automation()
        .on_left_mouse_down(move |ctx, _, _| {
            if let Ok(mut state) = shell_state.lock() {
                on_click(&mut state);
            }
            ctx.notify();
            DispatchEventResult::StopPropagation
        })
        .finish(),
    )
    .with_label(label)
    .with_id(id.to_string())
    .finish()
}

pub fn image_context_menu_overlay(
    font: FamilyId,
    menu: &ImageContextMenu,
    shell_state: SharedChatShellState,
) -> Box<dyn Element> {
    let mut col = Flex::column()
        .with_main_axis_size(MainAxisSize::Min)
        .with_cross_axis_alignment(CrossAxisAlignment::Stretch);

    for (label, on_click) in [
        (wormhole_i18n::t("chat.image.save"), save_image as fn(&mut ChatShellState)),
        (wormhole_i18n::t("chat.image.copy"), copy_image),
        (wormhole_i18n::t("chat.image.reveal"), reveal_image),
        (wormhole_i18n::t("chat.image.reply"), reply_image),
        (wormhole_i18n::t("chat.image.forward"), forward_image),
    ] {
        let shell_state = shell_state.clone();
        col.add_child(popover_plain_item(
            font,
            &label,
            false,
            false,
            move |ctx, _, _| {
                if let Ok(mut state) = shell_state.lock() {
                    on_click(&mut state);
                }
                ctx.notify();
                DispatchEventResult::StopPropagation
            },
        ));
    }

    let panel = popover_shell(220.0, col.finish());
    Container::new(panel)
        .with_margin_left(menu.x)
        .with_margin_top(menu.y)
        .finish()
}

fn current_local_path(state: &ChatShellState) -> Option<String> {
    state
        .image_viewer
        .as_ref()
        .and_then(|v| v.local_path.clone())
        .or_else(|| {
            state
                .image_context_menu
                .as_ref()
                .and_then(|m| m.local_path.clone())
        })
}

fn current_name(state: &ChatShellState) -> String {
    state
        .image_viewer
        .as_ref()
        .map(|v| v.name.clone())
        .or_else(|| state.image_context_menu.as_ref().map(|m| m.name.clone()))
        .unwrap_or_else(|| "image.png".into())
}

fn current_message_id(state: &ChatShellState) -> Option<String> {
    state
        .image_viewer
        .as_ref()
        .map(|v| v.message_id.clone())
        .or_else(|| {
            state
                .image_context_menu
                .as_ref()
                .map(|m| m.message_id.clone())
        })
}

fn close_viewer(state: &mut ChatShellState) {
    state.close_image_viewer();
}

fn save_image(state: &mut ChatShellState) {
    let Some(path) = current_local_path(state) else {
        state.show_toast(wormhole_i18n::t("chat.image.not_ready"), StatusTone::Danger);
        return;
    };
    let default_name = current_name(state);
    let dest = rfd::FileDialog::new()
        .set_file_name(&default_name)
        .save_file();
    if let Some(dest) = dest {
        match std::fs::copy(&path, &dest) {
            Ok(_) => {
                state.show_toast(wormhole_i18n::t("chat.image.saved"), StatusTone::Success);
                state.image_context_menu = None;
            }
            Err(err) => {
                state.show_toast(
                    format!("{}: {err}", wormhole_i18n::t("chat.image.save_failed")),
                    StatusTone::Danger,
                );
            }
        }
    }
}

fn copy_image(state: &mut ChatShellState) {
    let Some(path) = current_local_path(state) else {
        state.show_toast(wormhole_i18n::t("chat.image.not_ready"), StatusTone::Danger);
        return;
    };
    match write_clipboard_image_from_path(PathBuf::from(path).as_path()) {
        Ok(()) => {
            state.show_toast(wormhole_i18n::t("chat.image.copied"), StatusTone::Success);
            state.image_context_menu = None;
        }
        Err(err) => {
            state.show_toast(
                format!("{}: {err}", wormhole_i18n::t("chat.image.copy_failed")),
                StatusTone::Danger,
            );
        }
    }
}

fn reveal_image(state: &mut ChatShellState) {
    let Some(path) = current_local_path(state) else {
        state.show_toast(wormhole_i18n::t("chat.image.not_ready"), StatusTone::Danger);
        return;
    };
    match reveal_path_in_folder(PathBuf::from(path).as_path()) {
        Ok(()) => {
            state.image_context_menu = None;
        }
        Err(err) => state.show_toast(err, StatusTone::Danger),
    }
}

fn reply_image(state: &mut ChatShellState) {
    let Some(message_id) = current_message_id(state) else {
        return;
    };
    state.set_reply_draft(ReplyDraft {
        message_id,
        preview: wormhole_i18n::t("chat.preview.image"),
        has_image: true,
    });
}

fn forward_image(state: &mut ChatShellState) {
    let Some(path) = current_local_path(state) else {
        state.show_toast(wormhole_i18n::t("chat.image.not_ready"), StatusTone::Danger);
        return;
    };
    let name = current_name(state);
    let forwarded_from = state
        .selected_summary
        .as_ref()
        .map(|s| s.title.clone())
        .unwrap_or_else(|| "Wormhole".into());
    state.set_forward_draft(ForwardDraft {
        local_path: path,
        name,
        kind: "image".into(),
        size: 0,
        forwarded_from,
    });
}
