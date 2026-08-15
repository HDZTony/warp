//! Telegram-style 「上传媒体」 confirmation dialog.

use std::path::PathBuf;

use pathfinder_color::ColorU;
use pathfinder_geometry::vector::{vec2f, Vector2F};
use warpui::elements::{
    Align, AutomationTarget, Border, ChildAnchor, ClippedScrollStateHandle, ClippedScrollable,
    ConstrainedBox, Container, CornerRadius, CrossAxisAlignment, DispatchEventResult, EventHandler,
    Expanded, Fill, Flex, Image, MainAxisAlignment, MainAxisSize, OffsetPositioning, ParentAnchor,
    ParentElement, ParentOffsetBounds, Radius, SavePosition, ScrollbarWidth, Stack,
};
use warpui::fonts::FamilyId;
use warpui::{AppContext, Element, Entity, TypedActionView, View, ViewContext};
use warpui_core::assets::asset_cache::AssetSource;
use warpui_core::image_cache::CacheOption;
use warpui_core::platform::file_picker::FilePickerConfiguration;

use crate::ui::chat::attach_panel::{
    build_upload_send_batches, picker_config_for_kind, prepare_staged_file,
    should_persist_media_upload_as_file, show_media_upload_as_file_option,
    show_media_upload_group_option, show_media_upload_remember_option,
    sim_use_attach_paths_from_env, AttachKind, PreparedStagedFile,
};
use crate::ui::chat::bubble::format_file_size;
use crate::ui::chat::image_asset::{
    decode_image_asset_from_path, insert_attachment_image_payload, DecodedImageAsset,
};
use crate::ui::chat::layout::fit_attachment_preview;
use crate::ui::chat::media_edit::{
    apply_crop_drag, apply_image_edit, brush_size_rail, brush_width_from_t,
    canvas_local_from_window, color_palette, controls_footer, crop_overlay_layers, hit_test_crop,
    live_shape_overlay_layers, live_stroke_overlay_layers, normalize_canvas_point, quantize_brush_t,
    ratio_menu, render_edited_rgb, rotate_quarter_delta, shape_menu, sticker_panel, CONTENT_MARGIN,
    MEDIA_EDIT_CANVAS_POS, BrushTool, CropHandle, CropRatio, EditLayer, MediaEditMode, NormCropRect,
    PaintStroke, RotateQuarter, ShapeKind,
};
use crate::ui::chat::shell::ConversationSelection;
use crate::ui::chat::shell_state::{
    MediaUploadDraft, MediaUploadItem, PendingOutgoingAttachment, SharedChatShellState,
};
use crate::ui::core_handle::CoreHandle;
use crate::ui::panel_primitives::StatusTone;
use crate::ui::text_field_input::{
    render_field_with_caret, sync_caret_blink, CaretBlink, CaretBlinkHost, TextFieldEditAction,
    TextFieldInput, TextFieldState,
};
use crate::ui::theme;
use crate::ui_text;
use wormhole_desktop_core::chat_commands::{
    chat_send_message, SendChatAttachmentDto, SendChatMessageParams,
};
use wormhole_desktop_core::chat_ui_prefs::{load_chat_ui_prefs_sync, save_media_upload_prefs};

const DIALOG_WIDTH: f32 = 420.0;
const DIALOG_MAX_HEIGHT: f32 = 560.0;
const THUMB: f32 = 56.0;
const ROW_GAP: f32 = 8.0;

#[derive(Debug, Clone)]
pub enum MediaUploadAction {
    Close,
    ToggleGroup,
    ToggleAsFile,
    ToggleRemember,
    Remove(String),
    Edit(String),
    EditRotate(RotateQuarter),
    EditFlip,
    EditEnterPaint,
    EditExitPaint,
    EditApply,
    EditCancel,
    EditToggleRatioMenu,
    EditPickRatio(CropRatio),
    EditUndo,
    EditRedo,
    EditSticker,
    EditText,
    EditShape,
    EditPickSticker(String),
    EditPickShape(ShapeKind),
    EditToggleShapeFill,
    EditPickColor([u8; 3]),
    EditSetBrush(BrushTool),
    EditCycleColor,
    EditBrushSize { t: f32 },
    EditToggleColorPalette,
    EditToggleStickerPanel,
    EditToggleShapeMenu,
    EditPointerDown { x: f32, y: f32 },
    EditPointerMove { x: f32, y: f32 },
    EditPointerUp,
    EditPreviewReady {
        asset_id: String,
        width: u32,
        height: u32,
    },
    CaptionEdit(TextFieldEditAction),
    FocusCaption,
    AddMore,
    StagePicked(Vec<PathBuf>),
    PickerFailed(String),
    PreviewReady { id: String, asset_id: String },
    Send,
}

pub struct MediaUploadModalView {
    core: CoreHandle,
    selection: ConversationSelection,
    shell_state: SharedChatShellState,
    font: FamilyId,
    scroll: ClippedScrollStateHandle,
    caption_field: TextFieldState,
    caption_focused: bool,
    caret_blink: CaretBlink,
    sending: bool,
    picking: bool,
    status: String,
    edit_rotate_cw: i32,
    edit_flipped: bool,
    edit_crop: NormCropRect,
    edit_ratio: CropRatio,
    edit_ratio_menu: bool,
    edit_layers: Vec<EditLayer>,
    edit_redo: Vec<EditLayer>,
    edit_active_stroke: Option<PaintStroke>,
    /// Ink committed but not yet in `edit_preview_asset` (async rebuild). Cleared when gen lands.
    edit_pending_ink: Vec<PaintStroke>,
    edit_shape_drag: Option<(ShapeKind, f32, f32, f32, f32)>,
    edit_brush: BrushTool,
    edit_brush_color: [u8; 3],
    edit_brush_width: f32,
    edit_shapes_filled: bool,
    edit_color_palette: bool,
    edit_sticker_panel: bool,
    edit_shape_menu: bool,
    edit_place_mode: PlaceMode,
    edit_drag: Option<(CropHandle, NormCropRect, Vector2F)>,
    edit_box: Vector2F,
    edit_preview_asset: Option<String>,
    edit_preview_w: u32,
    edit_preview_h: u32,
    edit_source_w: u32,
    edit_source_h: u32,
    edit_rebuild_gen: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
enum PlaceMode {
    #[default]
    Brush,
    WaitingSticker(String),
    Text,
    Shape(ShapeKind),
}

impl MediaUploadModalView {
    pub fn new(
        ctx: &mut ViewContext<Self>,
        core: CoreHandle,
        selection: ConversationSelection,
        shell_state: SharedChatShellState,
    ) -> Self {
        let font = crate::ui::fonts::load_ui_font(ctx);
        Self {
            core,
            selection,
            shell_state,
            font,
            scroll: ClippedScrollStateHandle::new(),
            caption_field: TextFieldState::new(),
            caption_focused: false,
            caret_blink: CaretBlink::new(),
            sending: false,
            picking: false,
            status: String::new(),
            edit_rotate_cw: 0,
            edit_flipped: false,
            edit_crop: NormCropRect::default(),
            edit_ratio: CropRatio::Free,
            edit_ratio_menu: false,
            edit_layers: Vec::new(),
            edit_redo: Vec::new(),
            edit_active_stroke: None,
            edit_pending_ink: Vec::new(),
            edit_shape_drag: None,
            edit_brush: BrushTool::Pen,
            edit_brush_color: [255, 59, 48],
            edit_brush_width: 0.012,
            edit_shapes_filled: false,
            edit_color_palette: false,
            edit_sticker_panel: false,
            edit_shape_menu: false,
            edit_place_mode: PlaceMode::Brush,
            edit_drag: None,
            edit_box: vec2f(1.0, 1.0),
            edit_preview_asset: None,
            edit_preview_w: 0,
            edit_preview_h: 0,
            edit_source_w: 0,
            edit_source_h: 0,
            edit_rebuild_gen: 0,
        }
    }

    fn is_open(&self) -> bool {
        self.shell_state
            .lock()
            .map(|s| s.media_upload_open)
            .unwrap_or(false)
    }

    fn data_dir(&self) -> PathBuf {
        self.core.data_dir()
    }

    fn staging_dir(&self) -> PathBuf {
        self.data_dir().join("chat").join("compose-staging")
    }

    pub fn queue_missing_previews(&mut self, ctx: &mut ViewContext<Self>) {
        let pending: Vec<(String, PathBuf)> = self
            .shell_state
            .lock()
            .ok()
            .map(|state| {
                state
                    .media_upload
                    .items
                    .iter()
                    .filter(|i| i.kind == "image" && i.preview_asset_id.is_none())
                    .map(|i| (i.id.clone(), i.path.clone()))
                    .collect()
            })
            .unwrap_or_default();
        for (id, path) in pending {
            self.queue_preview(id, path, ctx);
        }
    }

    fn queue_preview(&mut self, id: String, path: PathBuf, ctx: &mut ViewContext<Self>) {
        ctx.spawn(
            async move {
                tokio::task::spawn_blocking(move || decode_image_asset_from_path(&path))
                    .await
                    .ok()
                    .and_then(Result::ok)
            },
            move |view, decoded, ctx| {
                if let Some(decoded) = decoded {
                    let asset_id = insert_attachment_image_payload(ctx, &id, decoded.payload);
                    if let Ok(mut state) = view.shell_state.lock() {
                        if let Some(item) =
                            state.media_upload.items.iter_mut().find(|i| i.id == id)
                        {
                            item.preview_asset_id = Some(asset_id);
                        }
                        state.bump_overlay_tick();
                    }
                }
                ctx.notify();
            },
        );
    }

    fn append_prepared(
        &mut self,
        files: Vec<PreparedStagedFile>,
        delete_on_clear: bool,
        ctx: &mut ViewContext<Self>,
    ) {
        if files.is_empty() {
            return;
        }
        let mut previews = Vec::new();
        if let Ok(mut state) = self.shell_state.lock() {
            for file in files {
                let id = uuid::Uuid::new_v4().to_string();
                if file.kind == "image" {
                    previews.push((id.clone(), file.path.clone()));
                }
                state.media_upload.items.push(MediaUploadItem {
                    id,
                    path: file.path,
                    kind: file.kind,
                    name: file.name,
                    size: file.size,
                    preview_asset_id: None,
                    delete_on_clear,
                });
            }
            state.bump_overlay_tick();
        }
        for (id, path) in previews {
            self.queue_preview(id, path, ctx);
        }
        ctx.notify();
    }

    fn open_add_picker(&mut self, ctx: &mut ViewContext<Self>) {
        let sim_paths = sim_use_attach_paths_from_env();
        if !sim_paths.is_empty() {
            ctx.dispatch_typed_action(&MediaUploadAction::StagePicked(sim_paths));
            return;
        }
        let Some(config) = picker_config_for_kind(AttachKind::Media) else {
            return;
        };
        self.picking = true;
        ctx.notify();
        let config: FilePickerConfiguration = config;
        ctx.open_file_picker(
            move |result, ctx| {
                let action = match result {
                    Ok(paths) => MediaUploadAction::StagePicked(
                        paths.into_iter().map(PathBuf::from).collect(),
                    ),
                    Err(err) => MediaUploadAction::PickerFailed(err.to_string()),
                };
                ctx.dispatch_typed_action(&action);
            },
            config,
        );
    }

    fn reset_edit_session(&mut self) {
        self.edit_rotate_cw = 0;
        self.edit_flipped = false;
        self.edit_crop = NormCropRect::default();
        self.edit_ratio = CropRatio::Free;
        self.edit_ratio_menu = false;
        self.edit_layers.clear();
        self.edit_redo.clear();
        self.edit_active_stroke = None;
        self.edit_pending_ink.clear();
        self.edit_shape_drag = None;
        self.edit_brush = BrushTool::Pen;
        self.edit_brush_color = [255, 59, 48];
        self.edit_brush_width = 0.012;
        self.edit_shapes_filled = false;
        self.edit_color_palette = false;
        self.edit_sticker_panel = false;
        self.edit_shape_menu = false;
        self.edit_place_mode = PlaceMode::Brush;
        self.edit_drag = None;
        self.edit_preview_asset = None;
        self.edit_preview_w = 0;
        self.edit_preview_h = 0;
        self.edit_source_w = 0;
        self.edit_source_h = 0;
    }

    fn edit_mode(&self) -> MediaEditMode {
        if self
            .shell_state
            .lock()
            .map(|s| s.media_upload.edit_paint_mode)
            .unwrap_or(false)
        {
            MediaEditMode::Paint
        } else {
            MediaEditMode::Transform
        }
    }

    fn set_paint_mode(&mut self, paint: bool) {
        if let Ok(mut state) = self.shell_state.lock() {
            state.media_upload.edit_paint_mode = paint;
            state.bump_overlay_tick();
        }
    }

    fn start_edit(&mut self, item_id: String, ctx: &mut ViewContext<Self>) {
        self.reset_edit_session();
        let path = {
            let Ok(mut state) = self.shell_state.lock() else {
                return;
            };
            let is_image = state
                .media_upload
                .items
                .iter()
                .any(|i| i.id == item_id && i.kind == "image");
            if !is_image {
                return;
            }
            let path = state
                .media_upload
                .items
                .iter()
                .find(|i| i.id == item_id)
                .map(|i| i.path.clone());
            state.media_upload.editing_item_id = Some(item_id);
            state.media_upload.edit_paint_mode = false;
            state.bump_overlay_tick();
            path
        };
        if let Some(path) = path {
            if let Ok(decoded) = decode_image_asset_from_path(&path) {
                self.edit_source_w = decoded.width;
                self.edit_source_h = decoded.height;
            }
            self.rebuild_edit_preview(ctx);
        }
        ctx.notify();
    }

    fn rebuild_edit_preview(&mut self, ctx: &mut ViewContext<Self>) {
        let path = {
            let Ok(state) = self.shell_state.lock() else {
                return;
            };
            let Some(id) = state.media_upload.editing_item_id.clone() else {
                return;
            };
            state
                .media_upload
                .items
                .iter()
                .find(|i| i.id == id)
                .map(|i| i.path.clone())
        };
        let Some(path) = path else {
            return;
        };
        let layers = self.edit_layers.clone();
        let flipped = self.edit_flipped;
        let rotate = self.edit_rotate_cw;
        self.edit_rebuild_gen = self.edit_rebuild_gen.saturating_add(1);
        let gen = self.edit_rebuild_gen;
        ctx.spawn(
            async move {
                tokio::task::spawn_blocking(move || {
                    let rgb = render_edited_rgb(&path, &layers, flipped, rotate, None)?;
                    let (width, height) = rgb.dimensions();
                    let payload = warpui_core::image_cache::CustomImageHeader::prepend_custom_header(
                        rgb.into_raw(),
                        width,
                        height,
                        warpui_core::image_cache::CustomImageFormat::Rgb,
                    )
                    .map_err(|e| format!("{e:?}"))?;
                    Ok::<DecodedImageAsset, String>(DecodedImageAsset {
                        payload,
                        width,
                        height,
                    })
                })
                .await
                .map_err(|e| e.to_string())
                .and_then(|r| r)
            },
            move |view, result, ctx| {
                if gen != view.edit_rebuild_gen {
                    return;
                }
                if let Ok(decoded) = result {
                    let asset_id = insert_attachment_image_payload(
                        ctx,
                        &format!("media-edit-{}", gen),
                        decoded.payload,
                    );
                    view.edit_preview_asset = Some(asset_id);
                    view.edit_preview_w = decoded.width;
                    view.edit_preview_h = decoded.height;
                    // Raster preview now includes all layers — drop vector pending ink.
                    view.edit_pending_ink.clear();
                }
                ctx.notify();
            },
        );
    }

    fn commit_stroke(&mut self, stroke: PaintStroke, ctx: &mut ViewContext<Self>) {
        self.edit_pending_ink.push(stroke.clone());
        self.edit_layers.push(EditLayer::Stroke(stroke));
        self.edit_redo.clear();
        self.rebuild_edit_preview(ctx);
    }

    fn flush_active_stroke(&mut self, ctx: &mut ViewContext<Self>) {
        if let Some(stroke) = self.edit_active_stroke.take() {
            if !stroke.points.is_empty() {
                self.commit_stroke(stroke, ctx);
            }
        }
    }

    fn apply_edit(&mut self, ctx: &mut ViewContext<Self>) {
        let (item_id, path) = {
            let Ok(state) = self.shell_state.lock() else {
                return;
            };
            let Some(id) = state.media_upload.editing_item_id.clone() else {
                return;
            };
            let Some(item) = state.media_upload.items.iter().find(|i| i.id == id) else {
                return;
            };
            (id, item.path.clone())
        };
        let staging = self.staging_dir();
        let rotate = self.edit_rotate_cw;
        let crop = self.edit_crop;
        let flipped = self.edit_flipped;
        let layers = self.edit_layers.clone();
        ctx.spawn(
            async move {
                tokio::task::spawn_blocking(move || {
                    apply_image_edit(&path, &staging, rotate, crop, flipped, &layers)
                })
                .await
                .map_err(|e| e.to_string())
                .and_then(|r| r)
            },
            move |view, result, ctx| match result {
                Ok(new_path) => {
                    let meta = std::fs::metadata(&new_path).ok();
                    let size = meta.map(|m| m.len()).unwrap_or(0);
                    let name = new_path
                        .file_name()
                        .map(|n| n.to_string_lossy().into_owned())
                        .unwrap_or_else(|| "edit.png".into());
                    let mut old_delete: Option<PathBuf> = None;
                    if let Ok(mut state) = view.shell_state.lock() {
                        if let Some(item) =
                            state.media_upload.items.iter_mut().find(|i| i.id == item_id)
                        {
                            if item.delete_on_clear {
                                old_delete = Some(item.path.clone());
                            }
                            item.path = new_path.clone();
                            item.name = name;
                            item.size = size;
                            item.kind = "image".into();
                            item.delete_on_clear = true;
                            item.preview_asset_id = None;
                        }
                        state.media_upload.editing_item_id = None;
                        state.media_upload.edit_paint_mode = false;
                        state.bump_overlay_tick();
                    }
                    if let Some(old) = old_delete {
                        let _ = std::fs::remove_file(old);
                    }
                    view.reset_edit_session();
                    view.queue_preview(item_id, new_path, ctx);
                    ctx.notify();
                }
                Err(err) => {
                    view.status = err;
                    ctx.notify();
                }
            },
        );
    }

    fn cancel_edit(&mut self, ctx: &mut ViewContext<Self>) {
        self.reset_edit_session();
        if let Ok(mut state) = self.shell_state.lock() {
            state.media_upload.editing_item_id = None;
            state.media_upload.edit_paint_mode = false;
            state.bump_overlay_tick();
        }
        ctx.notify();
    }

    fn aspect_for_edit(&self) -> Option<f32> {
        self.edit_ratio
            .aspect(self.edit_preview_w.max(1), self.edit_preview_h.max(1))
    }

    fn send(&mut self, ctx: &mut ViewContext<Self>) {
        if self.sending {
            return;
        }
        let conv_id = self
            .selection
            .lock()
            .ok()
            .and_then(|g| g.clone())
            .or_else(|| {
                self.shell_state
                    .lock()
                    .ok()
                    .and_then(|s| s.media_upload.conv_id.clone())
            });
        let Some(conv_id) = conv_id else {
            self.status = wormhole_i18n::t("chat.compose.select_conversation");
            ctx.notify();
            return;
        };

        let (
            items,
            caption,
            group,
            as_file,
            remember,
            initial_group,
            initial_as_file,
            forced_as_file,
            reply_to,
        ) = {
            let Ok(mut state) = self.shell_state.lock() else {
                return;
            };
            if state.media_upload.items.is_empty() {
                return;
            }
            let reply_to = state.reply_draft.take().map(|d| d.message_id);
            (
                state.media_upload.items.clone(),
                state.media_upload.caption.clone(),
                state.media_upload.group_items,
                state.media_upload.send_as_files,
                state.media_upload.remember,
                state.media_upload.initial_group_items,
                state.media_upload.initial_send_as_files,
                state.media_upload.forced_as_file,
                reply_to,
            )
        };

        if remember
            && show_media_upload_remember_option(group, as_file, initial_group, initial_as_file)
        {
            let show_group = show_media_upload_group_option(items.len());
            let show_as_file =
                show_media_upload_as_file_option(items.iter().map(|i| i.kind.as_str()));
            let persist_group = show_group.then_some(group);
            let persist_as_file = should_persist_media_upload_as_file(
                show_as_file,
                forced_as_file,
                as_file,
                initial_as_file,
            )
            .then_some(as_file);
            if persist_group.is_some() || persist_as_file.is_some() {
                let _ =
                    save_media_upload_prefs(&self.data_dir(), persist_group, persist_as_file);
            }
        }

        let pairs: Vec<(PathBuf, String)> = items
            .iter()
            .map(|i| (i.path.clone(), i.name.clone()))
            .collect();
        let batches = build_upload_send_batches(&pairs, &caption, group, as_file);
        if batches.is_empty() {
            return;
        }

        self.sending = true;
        self.status = wormhole_i18n::t("chat.compose.sending");
        ctx.notify();

        let delete_paths: Vec<PathBuf> = items
            .iter()
            .filter(|i| i.delete_on_clear)
            .map(|i| i.path.clone())
            .collect();

        let sent_at = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_millis() as u64)
            .unwrap_or(0);
        let mut client_ids = Vec::new();
        if let Ok(mut state) = self.shell_state.lock() {
            for (idx, batch) in batches.iter().enumerate() {
                let client_id = format!("pending:{}", uuid::Uuid::new_v4());
                let pending_atts: Vec<_> = batch
                    .attachments
                    .iter()
                    .map(|(kind, path)| {
                        let name = path
                            .file_name()
                            .map(|n| n.to_string_lossy().into_owned())
                            .unwrap_or_else(|| "file".into());
                        let size = items
                            .iter()
                            .find(|i| i.path == *path)
                            .map(|i| i.size)
                            .unwrap_or(0);
                        PendingOutgoingAttachment {
                            kind: kind.clone(),
                            name,
                            size,
                            local_path: Some(path.to_string_lossy().into_owned()),
                        }
                    })
                    .collect();
                state.push_pending_outgoing(
                    client_id.clone(),
                    conv_id.clone(),
                    batch.body.clone(),
                    sent_at + idx as u64,
                    pending_atts,
                );
                client_ids.push(client_id);
            }
            // Close immediately (Telegram-style); keep pending bubbles until the
            // async send finishes. Staging files are deleted only on success.
            state.dismiss_media_upload_after_send();
            state.bump_message_tick();
        }

        let core = self.core.clone();
        let shell_state = self.shell_state.clone();
        let batches_for_send = batches;
        let reply_first = reply_to;
        ctx.spawn(
            async move {
                let runtime = core.runtime();
                let mut first = true;
                for batch in batches_for_send {
                    let attachments: Vec<_> = batch
                        .attachments
                        .iter()
                        .map(|(kind, path)| SendChatAttachmentDto {
                            kind: kind.clone(),
                            path: path.to_string_lossy().into_owned(),
                        })
                        .collect();
                    let params = SendChatMessageParams {
                        conv_id: conv_id.clone(),
                        body: batch.body,
                        backend: None,
                        peer_bootstrap_addrs: Vec::new(),
                        sticker: None,
                        attachments,
                        reply_to: if first {
                            first = false;
                            reply_first.clone()
                        } else {
                            None
                        },
                        forwarded_from: None,
                    };
                    chat_send_message(runtime.ctx.as_ref(), &runtime.state, params)
                        .await
                        .map_err(|e| e.to_string())?;
                }
                Ok::<(), String>(())
            },
            move |view, output, ctx| {
                view.sending = false;
                for id in &client_ids {
                    if let Ok(mut state) = shell_state.lock() {
                        state.remove_pending(id);
                    }
                }
                match output {
                    Ok(()) => {
                        for path in &delete_paths {
                            let _ = std::fs::remove_file(path);
                        }
                        if let Ok(mut state) = shell_state.lock() {
                            state.bump_message_tick();
                        }
                        view.status.clear();
                    }
                    Err(err) => {
                        view.status = wormhole_i18n::t_args(
                            "chat.attachment_send_failed",
                            &[("err", &err)],
                        );
                        if let Ok(mut state) = shell_state.lock() {
                            state.show_toast(view.status.clone(), StatusTone::Danger);
                            state.bump_message_tick();
                        }
                    }
                }
                ctx.notify();
            },
        );
    }

    fn title_text(items: &[MediaUploadItem]) -> String {
        let n = items.len();
        let images = items.iter().filter(|i| i.kind == "image").count();
        let videos = items.iter().filter(|i| i.kind == "video").count();
        if images == n && n > 0 {
            wormhole_i18n::t_args("chat.media_upload.title_images", &[("n", &n.to_string())])
        } else if videos == n && n > 0 {
            wormhole_i18n::t_args("chat.media_upload.title_videos", &[("n", &n.to_string())])
        } else {
            wormhole_i18n::t_args("chat.media_upload.title_files", &[("n", &n.to_string())])
        }
    }

    fn checkbox_row(
        &self,
        checked: bool,
        label: String,
        id: &str,
        action: MediaUploadAction,
    ) -> Box<dyn Element> {
        let mark = if checked { "☑" } else { "☐" };
        AutomationTarget::new(
            EventHandler::new(
                Flex::row()
                    .with_cross_axis_alignment(CrossAxisAlignment::Center)
                    .with_child(
                        ui_text::body(format!("{mark}  {label}"), self.font)
                            .with_color(theme::text())
                            .finish(),
                    )
                    .finish(),
            )
            .on_left_mouse_down(move |ctx, _, _| {
                ctx.dispatch_typed_action(action.clone());
                DispatchEventResult::StopPropagation
            })
            .finish(),
        )
        .with_label(label)
        .with_id(id)
        .finish()
    }

    /// Telegram `setupSendWayControls` / `updateSendWayControls`: show Group / as-file /
    /// Remember only when applicable.
    fn send_way_controls(&self, draft: &MediaUploadDraft) -> Box<dyn Element> {
        let show_group = show_media_upload_group_option(draft.items.len());
        let show_as_file =
            show_media_upload_as_file_option(draft.items.iter().map(|i| i.kind.as_str()));
        let show_remember = show_media_upload_remember_option(
            draft.group_items,
            draft.send_as_files,
            draft.initial_group_items,
            draft.initial_send_as_files,
        );
        let as_file_label = if draft.items.len() == 1 {
            wormhole_i18n::t("chat.media_upload.as_file_one")
        } else {
            wormhole_i18n::t("chat.media_upload.as_file")
        };

        let mut col = Flex::column().with_cross_axis_alignment(CrossAxisAlignment::Stretch);
        let mut first = true;
        if show_group {
            col.add_child(self.checkbox_row(
                draft.group_items,
                wormhole_i18n::t("chat.media_upload.group"),
                "chat:media_upload_group",
                MediaUploadAction::ToggleGroup,
            ));
            first = false;
        }
        if show_as_file {
            let row = self.checkbox_row(
                draft.send_as_files,
                as_file_label,
                "chat:media_upload_as_file",
                MediaUploadAction::ToggleAsFile,
            );
            col.add_child(if first {
                row
            } else {
                Container::new(row).with_padding_top(6.0).finish()
            });
            first = false;
        }
        if show_remember {
            let row = self.checkbox_row(
                draft.remember,
                wormhole_i18n::t("chat.media_upload.remember"),
                "chat:media_upload_remember",
                MediaUploadAction::ToggleRemember,
            );
            col.add_child(if first {
                row
            } else {
                Container::new(row).with_padding_top(6.0).finish()
            });
        }
        Container::new(col.finish())
            .with_padding_bottom(10.0)
            .finish()
    }

    fn sync_remember_visibility(draft: &mut MediaUploadDraft) {
        if !show_media_upload_remember_option(
            draft.group_items,
            draft.send_as_files,
            draft.initial_group_items,
            draft.initial_send_as_files,
        ) {
            draft.remember = false;
        }
    }

    fn item_row(&self, item: &MediaUploadItem) -> Box<dyn Element> {
        let id = item.id.clone();
        let id_edit = item.id.clone();
        let can_edit = item.kind == "image";
        let thumb: Box<dyn Element> = if let Some(asset_id) = item.preview_asset_id.clone() {
            ConstrainedBox::new(
                Container::new(
                    Image::new(
                        AssetSource::Raw { id: asset_id },
                        CacheOption::BySize,
                    )
                    .finish(),
                )
                .with_corner_radius(CornerRadius::with_all(Radius::Pixels(6.0)))
                .finish(),
            )
            .with_width(THUMB)
            .with_height(THUMB)
            .finish()
        } else {
            ConstrainedBox::new(
                Container::new(
                    Flex::row()
                        .with_main_axis_alignment(MainAxisAlignment::Center)
                        .with_cross_axis_alignment(CrossAxisAlignment::Center)
                        .with_child(
                            ui_text::body("…", self.font)
                                .with_color(theme::muted())
                                .finish(),
                        )
                        .finish(),
                )
                .with_background(theme::panel_elevated())
                .finish(),
            )
            .with_width(THUMB)
            .with_height(THUMB)
            .finish()
        };

        let info = Flex::column()
            .with_cross_axis_alignment(CrossAxisAlignment::Start)
            .with_child(
                ui_text::body(item.name.clone(), self.font)
                    .with_color(theme::text())
                    .finish(),
            )
            .with_child(
                ui_text::chat_bubble_meta(format_file_size(item.size), self.font)
                    .with_color(theme::muted())
                    .finish(),
            )
            .finish();

        let remove = AutomationTarget::new(
            EventHandler::new(
                Container::new(
                    ui_text::body("×".to_string(), self.font)
                        .with_color(theme::muted())
                        .finish(),
                )
                .with_uniform_padding(6.0)
                .finish(),
            )
            .on_left_mouse_down(move |ctx, _, _| {
                ctx.dispatch_typed_action(MediaUploadAction::Remove(id.clone()));
                DispatchEventResult::StopPropagation
            })
            .finish(),
        )
        .with_label(wormhole_i18n::t("chat.compose.remove_attachment"))
        .with_id(format!("chat:media_upload_remove_{}", item.id))
        .finish();

        let row = Flex::row()
            .with_main_axis_size(MainAxisSize::Max)
            .with_cross_axis_alignment(CrossAxisAlignment::Center)
            .with_child(thumb)
            .with_child(
                Expanded::new(
                    1.0,
                    Container::new(info).with_padding_left(ROW_GAP).finish(),
                )
                .finish(),
            )
            .with_child(remove)
            .finish();

        let row_id = format!("chat:media_upload_item_{}", item.id);
        let labeled = AutomationTarget::new(
            EventHandler::new(row)
                .on_left_mouse_down(move |ctx, _, _| {
                    if can_edit {
                        ctx.dispatch_typed_action(MediaUploadAction::Edit(id_edit.clone()));
                    }
                    DispatchEventResult::StopPropagation
                })
                .finish(),
        )
        .with_label(item.name.clone())
        .with_id(row_id)
        .finish();

        Container::new(labeled)
            .with_padding_bottom(ROW_GAP)
            .finish()
    }

    fn caption_field(&self, caption: &str) -> Box<dyn Element> {
        let field = render_field_with_caret(
            caption,
            &self.caption_field.marked_text,
            &wormhole_i18n::t("chat.media_upload.caption"),
            self.font,
            self.caption_focused,
            self.sending,
            self.caret_blink.visible,
            self.caption_field.cursor,
        );
        let input = TextFieldInput::builder(field, |ctx, action| {
            ctx.dispatch_typed_action(MediaUploadAction::CaptionEdit(action));
        })
        .focused(self.caption_focused)
        .disabled(self.sending)
        .ime_preedit(!self.caption_field.marked_text.is_empty())
        .finish();

        AutomationTarget::new(
            EventHandler::new(
                Container::new(input)
                    .with_padding_top(4.0)
                    .with_padding_bottom(4.0)
                    .with_border(Border::bottom(1.0).with_border_color(theme::accent_cool()))
                    .finish(),
            )
            .on_left_mouse_down(|ctx, _, _| {
                ctx.dispatch_typed_action(MediaUploadAction::FocusCaption);
                DispatchEventResult::StopPropagation
            })
            .finish(),
        )
        .with_label(wormhole_i18n::t("chat.media_upload.caption"))
        .with_id("chat:media_upload_caption")
        .finish()
    }

    fn footer(&self) -> Box<dyn Element> {
        let add = text_action_btn(
            wormhole_i18n::t("chat.media_upload.add"),
            "chat:media_upload_add",
            MediaUploadAction::AddMore,
            self.font,
        );
        let cancel = text_action_btn(
            wormhole_i18n::t("chat.media_upload.cancel"),
            "chat:media_upload_cancel",
            MediaUploadAction::Close,
            self.font,
        );
        let send = text_action_btn(
            wormhole_i18n::t("chat.media_upload.send"),
            "chat:media_upload_send",
            MediaUploadAction::Send,
            self.font,
        );
        Flex::row()
            .with_main_axis_size(MainAxisSize::Max)
            .with_cross_axis_alignment(CrossAxisAlignment::Center)
            .with_child(add)
            .with_child(Expanded::new(1.0, Flex::row().finish()).finish())
            .with_child(cancel)
            .with_child(Container::new(send).with_padding_left(12.0).finish())
            .finish()
    }

    fn dialog_body(&self, draft: &MediaUploadDraft) -> Box<dyn Element> {
        let mut list = Flex::column().with_cross_axis_alignment(CrossAxisAlignment::Stretch);
        for item in &draft.items {
            list.add_child(self.item_row(item));
        }
        let scroll = ClippedScrollable::vertical(
            self.scroll.clone(),
            ConstrainedBox::new(list.finish())
                .with_max_height(220.0)
                .finish(),
            ScrollbarWidth::Auto,
            Fill::None,
            Fill::None,
            Fill::None,
        )
        .finish();

        let mut col = Flex::column()
            .with_cross_axis_alignment(CrossAxisAlignment::Stretch)
            .with_child(
                ui_text::section_title(Self::title_text(&draft.items), self.font)
                    .with_color(theme::text())
                    .finish(),
            )
            .with_child(Container::new(scroll).with_padding_top(12.0).finish())
            .with_child(
                Container::new(
                    ui_text::chat_bubble_meta(
                        wormhole_i18n::t("chat.media_upload.edit_hint"),
                        self.font,
                    )
                    .with_color(theme::muted())
                    .finish(),
                )
                .with_padding_top(4.0)
                .with_padding_bottom(8.0)
                .finish(),
            )
            .with_child(self.send_way_controls(draft))
            .with_child(
                ui_text::chat_bubble_meta(wormhole_i18n::t("chat.media_upload.caption"), self.font)
                    .with_color(theme::accent_cool())
                    .finish(),
            )
            .with_child(self.caption_field(&draft.caption));

        if !self.status.is_empty() {
            col.add_child(
                Container::new(
                    ui_text::body(self.status.clone(), self.font)
                        .with_color(theme::danger())
                        .finish(),
                )
                .with_padding_top(8.0)
                .finish(),
            );
        }

        col.add_child(
            Container::new(self.footer())
                .with_padding_top(16.0)
                .finish(),
        );

        Container::new(col.finish())
            .with_uniform_padding(16.0)
            .with_background(theme::panel())
            .with_corner_radius(CornerRadius::with_all(Radius::Pixels(12.0)))
            .with_border(Border::all(1.0).with_border_color(theme::border()))
            .finish()
    }

    fn edit_fullscreen(&self) -> Box<dyn Element> {
        let mode = self.edit_mode();
        let (box_w, box_h) = if self.edit_preview_w > 0 && self.edit_preview_h > 0 {
            let (w, h) =
                fit_attachment_preview(self.edit_preview_w, self.edit_preview_h, 720.0, 480.0);
            (w.max(200.0), h.max(200.0))
        } else {
            (320.0, 240.0)
        };
        let img: Box<dyn Element> = if let Some(asset_id) = &self.edit_preview_asset {
            ConstrainedBox::new(
                Image::new(
                    AssetSource::Raw {
                        id: asset_id.clone(),
                    },
                    CacheOption::BySize,
                )
                .finish(),
            )
            .with_width(box_w)
            .with_height(box_h)
            .finish()
        } else {
            ConstrainedBox::new(
                Align::new(
                    ui_text::body("…", self.font)
                        .with_color(theme::muted())
                        .finish(),
                )
                .finish(),
            )
            .with_width(box_w)
            .with_height(box_h)
            .finish()
        };

        let mut canvas = Stack::new();
        canvas.add_child(img);
        if matches!(mode, MediaEditMode::Transform) {
            for (layer, offset) in
                crop_overlay_layers(self.edit_crop, box_w, box_h, self.edit_drag.is_some())
            {
                canvas.add_positioned_overlay_child(
                    layer,
                    OffsetPositioning::offset_from_parent(
                        offset,
                        ParentOffsetBounds::Unbounded,
                        ParentAnchor::TopLeft,
                        ChildAnchor::TopLeft,
                    ),
                );
            }
        }
        if matches!(mode, MediaEditMode::Paint) {
            for stroke in &self.edit_pending_ink {
                for (layer, offset) in live_stroke_overlay_layers(stroke, box_w, box_h) {
                    canvas.add_positioned_overlay_child(
                        layer,
                        OffsetPositioning::offset_from_parent(
                            offset,
                            ParentOffsetBounds::Unbounded,
                            ParentAnchor::TopLeft,
                            ChildAnchor::TopLeft,
                        ),
                    );
                }
            }
            if let Some(stroke) = &self.edit_active_stroke {
                for (layer, offset) in live_stroke_overlay_layers(stroke, box_w, box_h) {
                    canvas.add_positioned_overlay_child(
                        layer,
                        OffsetPositioning::offset_from_parent(
                            offset,
                            ParentOffsetBounds::Unbounded,
                            ParentAnchor::TopLeft,
                            ChildAnchor::TopLeft,
                        ),
                    );
                }
            }
            if let Some((_, x0, y0, x1, y1)) = self.edit_shape_drag {
                for (layer, offset) in
                    live_shape_overlay_layers(x0, y0, x1, y1, self.edit_brush_color, box_w, box_h)
                {
                    canvas.add_positioned_overlay_child(
                        layer,
                        OffsetPositioning::offset_from_parent(
                            offset,
                            ParentOffsetBounds::Unbounded,
                            ParentAnchor::TopLeft,
                            ChildAnchor::TopLeft,
                        ),
                    );
                }
            }
        }

        let canvas = AutomationTarget::new(
            EventHandler::new(
                SavePosition::new(
                    ConstrainedBox::new(canvas.finish())
                        .with_width(box_w)
                        .with_height(box_h)
                        .finish(),
                    MEDIA_EDIT_CANVAS_POS,
                )
                .finish(),
            )
            .on_left_mouse_down(|ctx, _, pos| {
                let origin = ctx
                    .element_position_by_id(MEDIA_EDIT_CANVAS_POS)
                    .map(|r| vec2f(r.origin().x(), r.origin().y()))
                    .unwrap_or_else(|| vec2f(0.0, 0.0));
                let local = canvas_local_from_window(pos, origin);
                ctx.dispatch_typed_action(MediaUploadAction::EditPointerDown {
                    x: local.x(),
                    y: local.y(),
                });
                DispatchEventResult::StopPropagation
            })
            .on_mouse_dragged(|ctx, _, pos| {
                let origin = ctx
                    .element_position_by_id(MEDIA_EDIT_CANVAS_POS)
                    .map(|r| vec2f(r.origin().x(), r.origin().y()))
                    .unwrap_or_else(|| vec2f(0.0, 0.0));
                let local = canvas_local_from_window(pos, origin);
                ctx.dispatch_typed_action(MediaUploadAction::EditPointerMove {
                    x: local.x(),
                    y: local.y(),
                });
                DispatchEventResult::StopPropagation
            })
            .on_left_mouse_up(|ctx, _, _| {
                ctx.dispatch_typed_action(MediaUploadAction::EditPointerUp);
                DispatchEventResult::StopPropagation
            })
            .finish(),
        )
        .with_label(wormhole_i18n::t("chat.media_upload.edit_title"))
        .with_id("chat:media_edit_canvas")
        .finish();

        let content = if matches!(mode, MediaEditMode::Paint) {
            let rail = brush_size_rail(self.edit_brush_width, |t, ctx| {
                ctx.dispatch_typed_action(MediaUploadAction::EditBrushSize { t });
            });
            Flex::row()
                .with_main_axis_size(MainAxisSize::Max)
                .with_cross_axis_alignment(CrossAxisAlignment::Center)
                .with_child(
                    Container::new(rail)
                        .with_padding_left(12.0)
                        .with_padding_right(8.0)
                        .finish(),
                )
                .with_child(Expanded::new(1.0, Align::new(canvas).finish()).finish())
                .finish()
        } else {
            Align::new(canvas).finish()
        };

        let content = Container::new(content)
            .with_padding_left(CONTENT_MARGIN)
            .with_padding_right(CONTENT_MARGIN)
            .with_padding_top(CONTENT_MARGIN)
            .finish();

        let footer = controls_footer(
            self.font,
            mode,
            self.edit_flipped,
            self.edit_brush,
            self.edit_brush_color,
            |ctx| ctx.dispatch_typed_action(MediaUploadAction::EditCancel),
            |ctx| ctx.dispatch_typed_action(MediaUploadAction::EditFlip),
            |ctx| {
                ctx.dispatch_typed_action(MediaUploadAction::EditRotate(RotateQuarter::Cw))
            },
            |ctx| ctx.dispatch_typed_action(MediaUploadAction::EditEnterPaint),
            |ctx| ctx.dispatch_typed_action(MediaUploadAction::EditToggleRatioMenu),
            |ctx| ctx.dispatch_typed_action(MediaUploadAction::EditApply),
            |ctx| ctx.dispatch_typed_action(MediaUploadAction::EditUndo),
            |ctx| ctx.dispatch_typed_action(MediaUploadAction::EditRedo),
            |ctx| ctx.dispatch_typed_action(MediaUploadAction::EditToggleColorPalette),
            |tool, ctx| ctx.dispatch_typed_action(MediaUploadAction::EditSetBrush(tool)),
            |ctx| ctx.dispatch_typed_action(MediaUploadAction::EditExitPaint),
            |ctx| ctx.dispatch_typed_action(MediaUploadAction::EditEnterPaint),
            |ctx| ctx.dispatch_typed_action(MediaUploadAction::EditToggleStickerPanel),
            |ctx| ctx.dispatch_typed_action(MediaUploadAction::EditText),
            |ctx| ctx.dispatch_typed_action(MediaUploadAction::EditToggleShapeMenu),
            |ctx| ctx.dispatch_typed_action(MediaUploadAction::EditExitPaint),
        );

        let mut col = Flex::column()
            .with_cross_axis_alignment(CrossAxisAlignment::Stretch)
            .with_main_axis_size(MainAxisSize::Max)
            .with_child(Expanded::new(1.0, content).finish())
            .with_child(footer);

        if self.edit_ratio_menu && matches!(mode, MediaEditMode::Transform) {
            let menu = ratio_menu(self.font, self.edit_ratio, |ratio, ctx| {
                ctx.dispatch_typed_action(MediaUploadAction::EditPickRatio(ratio));
            });
            col.add_child(
                Align::new(Container::new(menu).with_padding_bottom(8.0).finish()).finish(),
            );
        }
        if self.edit_color_palette && matches!(mode, MediaEditMode::Paint) {
            let palette = color_palette(self.edit_brush_color, |c, ctx| {
                ctx.dispatch_typed_action(MediaUploadAction::EditPickColor(c));
            });
            col.add_child(
                Align::new(Container::new(palette).with_padding_bottom(8.0).finish()).finish(),
            );
        }
        if self.edit_sticker_panel && matches!(mode, MediaEditMode::Paint) {
            let panel = sticker_panel(self.font, |emoji, ctx| {
                ctx.dispatch_typed_action(MediaUploadAction::EditPickSticker(emoji));
            });
            col.add_child(
                Align::new(Container::new(panel).with_padding_bottom(8.0).finish()).finish(),
            );
        }
        if self.edit_shape_menu && matches!(mode, MediaEditMode::Paint) {
            let menu = shape_menu(
                self.font,
                self.edit_shapes_filled,
                |kind, ctx| ctx.dispatch_typed_action(MediaUploadAction::EditPickShape(kind)),
                |ctx| ctx.dispatch_typed_action(MediaUploadAction::EditToggleShapeFill),
            );
            col.add_child(
                Align::new(Container::new(menu).with_padding_bottom(8.0).finish()).finish(),
            );
        }

        AutomationTarget::new(
            Container::new(col.finish())
                .with_background(ColorU::new(16, 16, 16, 220))
                .finish(),
        )
        .with_label(wormhole_i18n::t("chat.media_upload.edit_title"))
        .with_id("chat:media_edit_dialog")
        .finish()
    }
}

fn text_action_btn(
    label: String,
    id: &str,
    action: MediaUploadAction,
    font: FamilyId,
) -> Box<dyn Element> {
    AutomationTarget::new(
        EventHandler::new(
            Container::new(
                ui_text::body(label.clone(), font)
                    .with_color(theme::accent_cool())
                    .finish(),
            )
            .with_uniform_padding(8.0)
            .finish(),
        )
        .on_left_mouse_down(move |ctx, _, _| {
            ctx.dispatch_typed_action(action.clone());
            DispatchEventResult::StopPropagation
        })
        .finish(),
    )
    .with_label(label)
    .with_id(id)
    .finish()
}

impl Entity for MediaUploadModalView {
    type Event = ();
}

impl CaretBlinkHost for MediaUploadModalView {
    fn caret_blink(&mut self) -> &mut CaretBlink {
        &mut self.caret_blink
    }

    fn caret_input_focused(&self) -> bool {
        self.caption_focused
    }
}

impl TypedActionView for MediaUploadModalView {
    type Action = MediaUploadAction;

    fn handle_action(&mut self, action: &MediaUploadAction, ctx: &mut ViewContext<Self>) {
        match action {
            MediaUploadAction::Close => {
                if let Ok(mut state) = self.shell_state.lock() {
                    state.close_media_upload();
                }
                self.status.clear();
                ctx.notify();
            }
            MediaUploadAction::ToggleGroup => {
                if let Ok(mut state) = self.shell_state.lock() {
                    state.media_upload.group_items = !state.media_upload.group_items;
                    Self::sync_remember_visibility(&mut state.media_upload);
                    state.bump_overlay_tick();
                }
                ctx.notify();
            }
            MediaUploadAction::ToggleAsFile => {
                if let Ok(mut state) = self.shell_state.lock() {
                    state.media_upload.send_as_files = !state.media_upload.send_as_files;
                    Self::sync_remember_visibility(&mut state.media_upload);
                    state.bump_overlay_tick();
                }
                ctx.notify();
            }
            MediaUploadAction::ToggleRemember => {
                if let Ok(mut state) = self.shell_state.lock() {
                    state.media_upload.remember = !state.media_upload.remember;
                    state.bump_overlay_tick();
                }
                ctx.notify();
            }
            MediaUploadAction::Remove(id) => {
                if let Ok(mut state) = self.shell_state.lock() {
                    if let Some(pos) = state.media_upload.items.iter().position(|i| i.id == *id) {
                        let item = state.media_upload.items.remove(pos);
                        if item.delete_on_clear {
                            let _ = std::fs::remove_file(&item.path);
                        }
                    }
                    if state.media_upload.items.is_empty() {
                        state.close_media_upload();
                    } else {
                        state.bump_overlay_tick();
                    }
                }
                ctx.notify();
            }
            MediaUploadAction::Edit(id) => self.start_edit(id.clone(), ctx),
            MediaUploadAction::EditRotate(dir) => {
                self.edit_rotate_cw =
                    (self.edit_rotate_cw + rotate_quarter_delta(*dir)).rem_euclid(4);
                self.edit_crop = NormCropRect::default();
                self.rebuild_edit_preview(ctx);
                ctx.notify();
            }
            MediaUploadAction::EditFlip => {
                self.edit_flipped = !self.edit_flipped;
                self.edit_crop = NormCropRect::default();
                self.rebuild_edit_preview(ctx);
                ctx.notify();
            }
            MediaUploadAction::EditEnterPaint => {
                self.set_paint_mode(true);
                self.edit_ratio_menu = false;
                ctx.notify();
            }
            MediaUploadAction::EditExitPaint => {
                self.set_paint_mode(false);
                self.edit_place_mode = PlaceMode::Brush;
                self.edit_sticker_panel = false;
                self.edit_shape_menu = false;
                self.edit_color_palette = false;
                if let Some(stroke) = self.edit_active_stroke.take() {
                    if !stroke.points.is_empty() {
                        self.commit_stroke(stroke, ctx);
                    }
                }
                ctx.notify();
            }
            MediaUploadAction::EditApply => {
                if matches!(self.edit_mode(), MediaEditMode::Paint) {
                    self.set_paint_mode(false);
                    self.edit_place_mode = PlaceMode::Brush;
                    ctx.notify();
                } else {
                    self.apply_edit(ctx);
                }
            }
            MediaUploadAction::EditCancel => {
                if matches!(self.edit_mode(), MediaEditMode::Paint) {
                    self.set_paint_mode(false);
                    self.edit_place_mode = PlaceMode::Brush;
                    ctx.notify();
                } else {
                    self.cancel_edit(ctx);
                }
            }
            MediaUploadAction::EditToggleRatioMenu => {
                self.edit_ratio_menu = !self.edit_ratio_menu;
                ctx.notify();
            }
            MediaUploadAction::EditPickRatio(ratio) => {
                self.edit_ratio = *ratio;
                self.edit_crop = NormCropRect::fit_aspect_top(self.aspect_for_edit());
                self.edit_ratio_menu = false;
                ctx.notify();
            }
            MediaUploadAction::EditUndo => {
                if let Some(layer) = self.edit_layers.pop() {
                    self.edit_redo.push(layer);
                    self.edit_pending_ink.clear();
                    self.rebuild_edit_preview(ctx);
                }
                ctx.notify();
            }
            MediaUploadAction::EditRedo => {
                if let Some(layer) = self.edit_redo.pop() {
                    if let EditLayer::Stroke(ref stroke) = layer {
                        self.edit_pending_ink.push(stroke.clone());
                    }
                    self.edit_layers.push(layer);
                    self.rebuild_edit_preview(ctx);
                }
                ctx.notify();
            }
            MediaUploadAction::EditSticker => {
                self.edit_sticker_panel = !self.edit_sticker_panel;
                self.edit_shape_menu = false;
                self.edit_color_palette = false;
                ctx.notify();
            }
            MediaUploadAction::EditToggleStickerPanel => {
                self.edit_sticker_panel = !self.edit_sticker_panel;
                self.edit_shape_menu = false;
                self.edit_color_palette = false;
                ctx.notify();
            }
            MediaUploadAction::EditText => {
                self.edit_place_mode = PlaceMode::Text;
                self.edit_sticker_panel = false;
                self.edit_shape_menu = false;
                ctx.notify();
            }
            MediaUploadAction::EditShape | MediaUploadAction::EditToggleShapeMenu => {
                self.edit_shape_menu = !self.edit_shape_menu;
                self.edit_sticker_panel = false;
                self.edit_color_palette = false;
                ctx.notify();
            }
            MediaUploadAction::EditPickSticker(emoji) => {
                self.edit_place_mode = PlaceMode::WaitingSticker(emoji.clone());
                self.edit_sticker_panel = false;
                ctx.notify();
            }
            MediaUploadAction::EditPickShape(kind) => {
                self.edit_place_mode = PlaceMode::Shape(*kind);
                self.edit_shape_menu = false;
                ctx.notify();
            }
            MediaUploadAction::EditToggleShapeFill => {
                self.edit_shapes_filled = !self.edit_shapes_filled;
                ctx.notify();
            }
            MediaUploadAction::EditPickColor(c) => {
                self.edit_brush_color = *c;
                self.edit_color_palette = false;
                ctx.notify();
            }
            MediaUploadAction::EditSetBrush(tool) => {
                self.edit_brush = *tool;
                self.edit_place_mode = PlaceMode::Brush;
                ctx.notify();
            }
            MediaUploadAction::EditCycleColor => {
                self.edit_color_palette = !self.edit_color_palette;
                ctx.notify();
            }
            MediaUploadAction::EditToggleColorPalette => {
                self.edit_color_palette = !self.edit_color_palette;
                self.edit_sticker_panel = false;
                self.edit_shape_menu = false;
                ctx.notify();
            }
            MediaUploadAction::EditBrushSize { t } => {
                let width = brush_width_from_t(quantize_brush_t(*t));
                if (width - self.edit_brush_width).abs() < 1e-6 {
                    return;
                }
                self.edit_brush_width = width;
                ctx.notify();
            }
            MediaUploadAction::EditPointerDown { x, y } => {
                // `x`/`y` are canvas-local (window pos minus SavePosition origin).
                let pos = vec2f(*x, *y);
                let (box_w, box_h) = if self.edit_preview_w > 0 && self.edit_preview_h > 0 {
                    let (w, h) = fit_attachment_preview(
                        self.edit_preview_w,
                        self.edit_preview_h,
                        720.0,
                        480.0,
                    );
                    (w.max(200.0), h.max(200.0))
                } else {
                    (320.0, 240.0)
                };
                self.edit_box = vec2f(box_w, box_h);
                let (nx, ny) = normalize_canvas_point(pos, box_w, box_h);
                match self.edit_mode() {
                    MediaEditMode::Paint => match self.edit_place_mode.clone() {
                        PlaceMode::WaitingSticker(emoji) => {
                            self.edit_layers.push(EditLayer::Sticker {
                                emoji,
                                x: nx,
                                y: ny,
                                size: 0.12,
                            });
                            self.edit_redo.clear();
                            self.edit_place_mode = PlaceMode::Brush;
                            self.rebuild_edit_preview(ctx);
                        }
                        PlaceMode::Text => {
                            self.edit_layers.push(EditLayer::Text {
                                text: wormhole_i18n::t("chat.media_upload.edit_text_default"),
                                x: nx,
                                y: ny,
                                color: self.edit_brush_color,
                            });
                            self.edit_redo.clear();
                            self.edit_place_mode = PlaceMode::Brush;
                            self.rebuild_edit_preview(ctx);
                        }
                        PlaceMode::Shape(kind) => {
                            self.edit_shape_drag = Some((kind, nx, ny, nx, ny));
                        }
                        PlaceMode::Brush => {
                            self.flush_active_stroke(ctx);
                            let mut stroke = PaintStroke::with_brush(
                                self.edit_brush,
                                self.edit_brush_color,
                                self.edit_brush_width,
                            );
                            stroke.points.push((nx, ny));
                            self.edit_active_stroke = Some(stroke);
                        }
                    },
                    MediaEditMode::Transform => {
                        if let Some(handle) = hit_test_crop(self.edit_crop, pos, self.edit_box) {
                            self.edit_drag = Some((handle, self.edit_crop, pos));
                        }
                    }
                }
                ctx.notify();
            }
            MediaUploadAction::EditPointerMove { x, y } => {
                let pos = vec2f(*x, *y);
                match self.edit_mode() {
                    MediaEditMode::Paint => {
                        if let Some(stroke) = self.edit_active_stroke.as_mut() {
                            stroke
                                .points
                                .push(normalize_canvas_point(pos, self.edit_box.x(), self.edit_box.y()));
                        }
                        if let Some((kind, x0, y0, _, _)) = self.edit_shape_drag {
                            let (x1, y1) =
                                normalize_canvas_point(pos, self.edit_box.x(), self.edit_box.y());
                            self.edit_shape_drag = Some((kind, x0, y0, x1, y1));
                        }
                    }
                    MediaEditMode::Transform => {
                        if let Some((handle, start, origin)) = self.edit_drag {
                            self.edit_crop = apply_crop_drag(
                                start,
                                handle,
                                origin,
                                pos,
                                self.edit_box,
                                self.aspect_for_edit(),
                            );
                        }
                    }
                }
                ctx.notify();
            }
            MediaUploadAction::EditPointerUp => {
                if matches!(self.edit_mode(), MediaEditMode::Paint) {
                    if let Some(stroke) = self.edit_active_stroke.take() {
                        if !stroke.points.is_empty() {
                            self.commit_stroke(stroke, ctx);
                        }
                    }
                    if let Some((kind, x0, y0, x1, y1)) = self.edit_shape_drag.take() {
                        self.edit_layers.push(EditLayer::Shape {
                            kind,
                            filled: self.edit_shapes_filled,
                            x0,
                            y0,
                            x1,
                            y1,
                            color: self.edit_brush_color,
                            width: self.edit_brush_width,
                        });
                        self.edit_redo.clear();
                        self.edit_place_mode = PlaceMode::Brush;
                        self.rebuild_edit_preview(ctx);
                    }
                }
                self.edit_drag = None;
                ctx.notify();
            }
            MediaUploadAction::EditPreviewReady {
                asset_id,
                width,
                height,
            } => {
                self.edit_preview_asset = Some(asset_id.clone());
                self.edit_preview_w = *width;
                self.edit_preview_h = *height;
                ctx.notify();
            }
            MediaUploadAction::FocusCaption => {
                self.caption_focused = true;
                sync_caret_blink(self, ctx);
                ctx.notify();
            }
            MediaUploadAction::CaptionEdit(edit) => {
                if let Ok(mut state) = self.shell_state.lock() {
                    self.caption_field
                        .apply(&mut state.media_upload.caption, edit);
                }
                self.caption_focused = true;
                sync_caret_blink(self, ctx);
                ctx.notify();
            }
            MediaUploadAction::AddMore => self.open_add_picker(ctx),
            MediaUploadAction::StagePicked(paths) => {
                self.picking = false;
                let mut files = Vec::new();
                for path in paths {
                    match prepare_staged_file(path.clone(), AttachKind::Media) {
                        Ok(f) => files.push(f),
                        Err(err) => self.status = err.message(),
                    }
                }
                self.append_prepared(files, false, ctx);
            }
            MediaUploadAction::PickerFailed(err) => {
                self.picking = false;
                self.status = err.clone();
                ctx.notify();
            }
            MediaUploadAction::PreviewReady { id, asset_id } => {
                if let Ok(mut state) = self.shell_state.lock() {
                    if let Some(item) = state.media_upload.items.iter_mut().find(|i| i.id == *id) {
                        item.preview_asset_id = Some(asset_id.clone());
                    }
                    state.bump_overlay_tick();
                }
                ctx.notify();
            }
            MediaUploadAction::Send => self.send(ctx),
        }
    }
}

impl View for MediaUploadModalView {
    fn ui_name() -> &'static str {
        "MediaUploadModalView"
    }

    fn render(&self, _app: &AppContext) -> Box<dyn Element> {
        if !self.is_open() {
            return Flex::column().finish();
        }
        let draft = self
            .shell_state
            .lock()
            .map(|s| s.media_upload.clone())
            .unwrap_or_default();

        if draft.editing_item_id.is_some() {
            return self.edit_fullscreen();
        }

        let scrim = EventHandler::new(
            Container::new(Flex::column().finish())
                .with_background(ColorU::new(0, 0, 0, 140))
                .finish(),
        )
        .with_automation_label(wormhole_i18n::t("chat.media_upload.cancel"))
        .with_automation_id("chat:media_upload_scrim")
        .on_left_mouse_down(|ctx, _, _| {
            ctx.dispatch_typed_action(MediaUploadAction::Close);
            DispatchEventResult::StopPropagation
        })
        .finish();

        let dialog = ConstrainedBox::new(
            EventHandler::new(self.dialog_body(&draft))
                .on_left_mouse_down(|_, _, _| DispatchEventResult::StopPropagation)
                .finish(),
        )
        .with_width(DIALOG_WIDTH)
        .with_max_height(DIALOG_MAX_HEIGHT)
        .finish();

        let dialog = AutomationTarget::new(dialog)
            .with_label(wormhole_i18n::t("chat.media_upload.dialog"))
            .with_id("chat:media_upload_dialog")
            .finish();

        Stack::new()
            .with_child(scrim)
            .with_child(Align::new(dialog).finish())
            .finish()
    }
}

/// Open the media upload modal with prepared files (from compose / drop / paste).
pub fn open_media_upload_with_files(
    shell_state: &SharedChatShellState,
    data_dir: &std::path::Path,
    conv_id: Option<String>,
    files: Vec<PreparedStagedFile>,
    delete_flags: Vec<bool>,
    force_as_file_default: bool,
) {
    if files.is_empty() {
        return;
    }
    let prefs = load_chat_ui_prefs_sync(data_dir);
    let group_items = prefs.media_upload_group_or_default();
    let send_as_files = if force_as_file_default {
        true
    } else {
        prefs.media_upload_as_file_or_default()
    };
    let mut draft = MediaUploadDraft {
        conv_id,
        items: Vec::new(),
        caption: String::new(),
        group_items,
        send_as_files,
        remember: false,
        initial_group_items: group_items,
        initial_send_as_files: send_as_files,
        forced_as_file: force_as_file_default,
        editing_item_id: None,
        edit_paint_mode: false,
    };
    for (idx, file) in files.into_iter().enumerate() {
        let delete_on_clear = delete_flags.get(idx).copied().unwrap_or(false);
        draft.items.push(MediaUploadItem {
            id: uuid::Uuid::new_v4().to_string(),
            path: file.path,
            kind: file.kind,
            name: file.name,
            size: file.size,
            preview_asset_id: None,
            delete_on_clear,
        });
    }
    if let Ok(mut state) = shell_state.lock() {
        if state.media_upload_open {
            state.media_upload.items.extend(draft.items);
            if state.media_upload.conv_id.is_none() {
                state.media_upload.conv_id = draft.conv_id;
            }
            state.bump_overlay_tick();
        } else {
            state.open_media_upload(draft);
        }
    }
}
