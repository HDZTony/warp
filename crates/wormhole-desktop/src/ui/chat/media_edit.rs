//! Telegram Desktop–aligned photo editor (Transform + Paint).
//!
//! Layout numbers match tdesktop `editor.style`. Pixel pipeline on Done:
//! overlay layers (strokes / stickers / text / shapes) → flip → rotate → crop.

use std::path::{Path, PathBuf};

use image::{DynamicImage, ImageFormat, Rgb, RgbImage};
use pathfinder_color::ColorU;
use pathfinder_geometry::vector::{vec2f, Vector2F};
use storage_core::decode_chat_image_to_rgb;
use warpui::elements::{
    Align, AutomationTarget, Border, ChildAnchor, ConstrainedBox, Container, CornerRadius,
    CrossAxisAlignment, DispatchEventResult, EventHandler, Flex, MainAxisAlignment,
    MainAxisSize, OffsetPositioning, ParentAnchor, ParentElement, ParentOffsetBounds, Radius,
    SavePosition, Stack,
};
use warpui::fonts::FamilyId;
use warpui::Element;

use crate::ui::icons;
use crate::ui::theme;
use crate::ui_text;

/// tdesktop `photoEditorControlsHeight`
pub const CONTROLS_HEIGHT: f32 = 146.0;
pub const CONTROLS_BOTTOM_SKIP: f32 = 20.0;
pub const CONTROLS_CENTER_SKIP: f32 = 6.0;
pub const BUTTON_BAR_HEIGHT: f32 = 48.0;
/// Wider than tdesktop 422 so mid icons + TOOL_GAP + edge pad still fit.
pub const BUTTON_BAR_WIDTH: f32 = 460.0;
pub const CONTENT_MARGIN: f32 = CONTROLS_BOTTOM_SKIP;
pub const ICON_BTN: f32 = BUTTON_BAR_HEIGHT;
pub const HANDLE_HIT: f32 = 18.0;
pub const CROP_MIN: f32 = 0.05;
pub const TOOL_BTN: f32 = 36.0;
pub const TOOL_GAP: f32 = 12.0;
/// Horizontal inset inside the pill so Cancel/Done / Undo/Redo are not flush to the curve.
pub const BUTTON_BAR_EDGE_PAD: f32 = 16.0;
pub const BRUSH_SIZE_MIN: f32 = 0.004;
pub const BRUSH_SIZE_MAX: f32 = 0.04;
/// Discrete steps for brush-size drag (limits full-UI notify rate).
pub const BRUSH_SIZE_STEPS: f32 = 20.0;
/// tdesktop `photoEditorBrushSizeControlHeight` (slightly shorter for our chrome).
pub const BRUSH_SIZE_RAIL_H: f32 = 240.0;
pub const BRUSH_SIZE_HIT_PAD: f32 = 24.0;
pub const BRUSH_SIZE_TRACK_W: f32 = 6.0;
/// `SavePosition` id for converting window mouse coords → canvas-local.
pub const MEDIA_EDIT_CANVAS_POS: &str = "chat:media_edit_canvas_pos";
/// `SavePosition` id for brush-size rail local Y.
pub const MEDIA_EDIT_BRUSH_SIZE_POS: &str = "chat:media_edit_brush_size_pos";

/// Convert a window-space mouse position into canvas-local coords (0..box size).
///
/// Warp `EventHandler` callbacks receive **window-global** positions; callers that
/// divide by `box_w`/`box_h` without subtracting the canvas origin clamp every
/// point to the far edge and painting appears broken.
pub fn canvas_local_from_window(
    window_pos: Vector2F,
    canvas_origin: Vector2F,
) -> Vector2F {
    vec2f(
        window_pos.x() - canvas_origin.x(),
        window_pos.y() - canvas_origin.y(),
    )
}

/// Normalize a canvas-local point into 0–1 image space.
pub fn normalize_canvas_point(local: Vector2F, box_w: f32, box_h: f32) -> (f32, f32) {
    let w = box_w.max(1.0);
    let h = box_h.max(1.0);
    (
        (local.x() / w).clamp(0.0, 1.0),
        (local.y() / h).clamp(0.0, 1.0),
    )
}

/// Brush size ratio `t` in 0..=1 from rail-local Y. Top = thick (1), bottom = thin (0).
pub fn brush_t_from_local_y(local_y: f32, rail_h: f32) -> f32 {
    let top = BRUSH_SIZE_HIT_PAD;
    let bottom = (rail_h - BRUSH_SIZE_HIT_PAD).max(top + 1.0);
    let y = local_y.clamp(top, bottom);
    (1.0 - (y - top) / (bottom - top)).clamp(0.0, 1.0)
}

pub fn brush_width_from_t(t: f32) -> f32 {
    let t = t.clamp(0.0, 1.0);
    BRUSH_SIZE_MIN + t * (BRUSH_SIZE_MAX - BRUSH_SIZE_MIN)
}

pub fn brush_t_from_width(width: f32) -> f32 {
    ((width - BRUSH_SIZE_MIN) / (BRUSH_SIZE_MAX - BRUSH_SIZE_MIN).max(1e-6)).clamp(0.0, 1.0)
}

/// Snap brush ratio to a small number of steps so drag does not rebuild UI every pixel.
pub fn quantize_brush_t(t: f32) -> f32 {
    let t = t.clamp(0.0, 1.0);
    (t * BRUSH_SIZE_STEPS).round() / BRUSH_SIZE_STEPS
}

/// Strokes currently in the edit stack (for pending ink overlay).
pub fn strokes_from_layers(layers: &[EditLayer]) -> Vec<PaintStroke> {
    layers
        .iter()
        .filter_map(|layer| match layer {
            EditLayer::Stroke(stroke) => Some(stroke.clone()),
            _ => None,
        })
        .collect()
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum MediaEditMode {
    #[default]
    Transform,
    Paint,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum CropRatio {
    #[default]
    Free,
    Original,
    Square,
    R3x2,
    R16x9,
    R3x4,
    R9x16,
}

impl CropRatio {
    pub fn label_key(self) -> &'static str {
        match self {
            Self::Free => "chat.media_upload.edit_ratio_free",
            Self::Original => "chat.media_upload.edit_ratio_original",
            Self::Square => "chat.media_upload.edit_ratio_square",
            Self::R3x2 => "chat.media_upload.edit_ratio_3_2",
            Self::R16x9 => "chat.media_upload.edit_ratio_16_9",
            Self::R3x4 => "chat.media_upload.edit_ratio_3_4",
            Self::R9x16 => "chat.media_upload.edit_ratio_9_16",
        }
    }

    pub fn all() -> &'static [CropRatio] {
        &[
            Self::Original,
            Self::Square,
            Self::R3x2,
            Self::R16x9,
            Self::R3x4,
            Self::R9x16,
            Self::Free,
        ]
    }

    pub fn aspect(self, original_w: u32, original_h: u32) -> Option<f32> {
        match self {
            Self::Free => None,
            Self::Original => {
                let w = original_w.max(1) as f32;
                let h = original_h.max(1) as f32;
                Some(w / h)
            }
            Self::Square => Some(1.0),
            Self::R3x2 => Some(3.0 / 2.0),
            Self::R16x9 => Some(16.0 / 9.0),
            Self::R3x4 => Some(3.0 / 4.0),
            Self::R9x16 => Some(9.0 / 16.0),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum BrushTool {
    #[default]
    Pen,
    Arrow,
    Marker,
    Blur,
    Eraser,
}

impl BrushTool {
    pub fn all() -> &'static [BrushTool] {
        &[
            Self::Pen,
            Self::Arrow,
            Self::Marker,
            Self::Blur,
            Self::Eraser,
        ]
    }

    pub fn icon_path(self) -> &'static str {
        match self {
            Self::Pen => "media-edit-tool-pen.svg",
            Self::Arrow => "media-edit-shape-arrow.svg",
            Self::Marker => "media-edit-tool-marker.svg",
            Self::Blur => "media-edit-tool-blur.svg",
            Self::Eraser => "media-edit-tool-eraser.svg",
        }
    }

    pub fn automation_id(self) -> &'static str {
        match self {
            Self::Pen => "chat:media_edit_tool_pen",
            Self::Arrow => "chat:media_edit_tool_arrow",
            Self::Marker => "chat:media_edit_tool_marker",
            Self::Blur => "chat:media_edit_tool_blur",
            Self::Eraser => "chat:media_edit_tool_eraser",
        }
    }

    pub fn label_key(self) -> &'static str {
        match self {
            Self::Pen => "chat.media_upload.edit_tool_pen",
            Self::Arrow => "chat.media_upload.edit_tool_arrow",
            Self::Marker => "chat.media_upload.edit_tool_marker",
            Self::Blur => "chat.media_upload.edit_tool_blur",
            Self::Eraser => "chat.media_upload.edit_tool_eraser",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ShapeKind {
    Rectangle,
    Circle,
    Arrow,
    Star,
    Bubble,
}

impl ShapeKind {
    pub fn all() -> &'static [ShapeKind] {
        &[
            Self::Rectangle,
            Self::Circle,
            Self::Arrow,
            Self::Star,
            Self::Bubble,
        ]
    }

    pub fn icon_path(self, filled: bool) -> &'static str {
        match (self, filled) {
            (Self::Rectangle, false) => "media-edit-shape-rectangle.svg",
            (Self::Rectangle, true) => "media-edit-shape-rectangle-fill.svg",
            (Self::Circle, false) => "media-edit-shape-circle.svg",
            (Self::Circle, true) => "media-edit-shape-circle-fill.svg",
            (Self::Arrow, _) => "media-edit-shape-arrow.svg",
            (Self::Star, false) => "media-edit-shape-star.svg",
            (Self::Star, true) => "media-edit-shape-star-fill.svg",
            (Self::Bubble, false) => "media-edit-shape-bubble.svg",
            (Self::Bubble, true) => "media-edit-shape-bubble-fill.svg",
        }
    }

    pub fn label_key(self) -> &'static str {
        match self {
            Self::Rectangle => "chat.media_upload.edit_shape_rect",
            Self::Circle => "chat.media_upload.edit_shape_circle",
            Self::Arrow => "chat.media_upload.edit_shape_arrow",
            Self::Star => "chat.media_upload.edit_shape_star",
            Self::Bubble => "chat.media_upload.edit_shape_bubble",
        }
    }
}

/// Normalized crop rect in oriented image space (0–1).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct NormCropRect {
    pub x0: f32,
    pub y0: f32,
    pub x1: f32,
    pub y1: f32,
}

impl Default for NormCropRect {
    fn default() -> Self {
        Self {
            x0: 0.0,
            y0: 0.0,
            x1: 1.0,
            y1: 1.0,
        }
    }
}

impl NormCropRect {
    pub fn clamp(self) -> Self {
        let x0 = self.x0.clamp(0.0, 1.0).min(self.x1);
        let y0 = self.y0.clamp(0.0, 1.0).min(self.y1);
        let x1 = self.x1.clamp(0.0, 1.0).max(x0);
        let y1 = self.y1.clamp(0.0, 1.0).max(y0);
        let mut out = Self { x0, y0, x1, y1 };
        if out.x1 - out.x0 < CROP_MIN {
            out.x1 = (out.x0 + CROP_MIN).min(1.0);
            out.x0 = (out.x1 - CROP_MIN).max(0.0);
        }
        if out.y1 - out.y0 < CROP_MIN {
            out.y1 = (out.y0 + CROP_MIN).min(1.0);
            out.y0 = (out.y1 - CROP_MIN).max(0.0);
        }
        out
    }

    pub fn is_identity(self) -> bool {
        self.x0 <= 0.001 && self.y0 <= 0.001 && self.x1 >= 0.999 && self.y1 >= 0.999
    }

    /// Max inscribed aspect rect, horizontally centered, pinned to top (ratio menu).
    pub fn fit_aspect_top(aspect: Option<f32>) -> Self {
        let Some(aspect) = aspect.filter(|a| *a > 0.01) else {
            return Self::default();
        };
        let mut new_w = 1.0_f32;
        let mut new_h = new_w / aspect;
        if new_h > 1.0 {
            new_h = 1.0;
            new_w = new_h * aspect;
        }
        Self {
            x0: ((1.0 - new_w) * 0.5).max(0.0),
            y0: 0.0,
            x1: ((1.0 - new_w) * 0.5 + new_w).min(1.0),
            y1: new_h.min(1.0),
        }
        .clamp()
    }

    /// Keep current crop roughly, constrain to aspect while dragging.
    pub fn enforce_aspect(self, aspect: Option<f32>) -> Self {
        let Some(aspect) = aspect.filter(|a| *a > 0.01) else {
            return self.clamp();
        };
        let mut c = self.clamp();
        let w = c.x1 - c.x0;
        let h = c.y1 - c.y0;
        if w <= 0.0 || h <= 0.0 {
            return c;
        }
        let cur = w / h;
        if (cur - aspect).abs() < 0.001 {
            return c;
        }
        let cx = (c.x0 + c.x1) * 0.5;
        let cy = (c.y0 + c.y1) * 0.5;
        let (nw, nh) = if cur > aspect {
            (h * aspect, h)
        } else {
            (w, w / aspect)
        };
        c.x0 = (cx - nw * 0.5).max(0.0);
        c.x1 = (cx + nw * 0.5).min(1.0);
        c.y0 = (cy - nh * 0.5).max(0.0);
        c.y1 = (cy + nh * 0.5).min(1.0);
        let w = c.x1 - c.x0;
        let h = c.y1 - c.y0;
        if w / h > aspect {
            let nw = h * aspect;
            let cx = (c.x0 + c.x1) * 0.5;
            c.x0 = cx - nw * 0.5;
            c.x1 = cx + nw * 0.5;
        } else {
            let nh = w / aspect;
            let cy = (c.y0 + c.y1) * 0.5;
            c.y0 = cy - nh * 0.5;
            c.y1 = cy + nh * 0.5;
        }
        c.clamp()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RotateQuarter {
    Cw,
    Ccw,
}

pub fn rotate_quarter_delta(dir: RotateQuarter) -> i32 {
    match dir {
        RotateQuarter::Cw => 1,
        RotateQuarter::Ccw => -1,
    }
}

#[derive(Debug, Clone)]
pub struct PaintStroke {
    pub points: Vec<(f32, f32)>,
    pub color: [u8; 3],
    pub width: f32,
    pub tool: BrushTool,
}

impl Default for PaintStroke {
    fn default() -> Self {
        Self::new_default()
    }
}

impl PaintStroke {
    pub fn new_default() -> Self {
        Self {
            points: Vec::new(),
            color: [255, 59, 48],
            width: 0.012,
            tool: BrushTool::Pen,
        }
    }

    pub fn with_brush(tool: BrushTool, color: [u8; 3], width: f32) -> Self {
        let width = match tool {
            BrushTool::Marker => (width * 2.2).clamp(BRUSH_SIZE_MIN, BRUSH_SIZE_MAX),
            BrushTool::Eraser | BrushTool::Blur => (width * 1.6).clamp(BRUSH_SIZE_MIN, BRUSH_SIZE_MAX),
            _ => width.clamp(BRUSH_SIZE_MIN, BRUSH_SIZE_MAX),
        };
        Self {
            points: Vec::new(),
            color,
            width,
            tool,
        }
    }
}

#[derive(Debug, Clone)]
pub enum EditLayer {
    Stroke(PaintStroke),
    Sticker {
        emoji: String,
        x: f32,
        y: f32,
        size: f32,
    },
    Text {
        text: String,
        x: f32,
        y: f32,
        color: [u8; 3],
    },
    Shape {
        kind: ShapeKind,
        filled: bool,
        x0: f32,
        y0: f32,
        x1: f32,
        y1: f32,
        color: [u8; 3],
        width: f32,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CropHandle {
    Move,
    Nw,
    Ne,
    Sw,
    Se,
    N,
    S,
    E,
    W,
}

pub const BRUSH_PRESET_COLORS: [[u8; 3]; 8] = [
    [255, 59, 48],
    [255, 149, 0],
    [255, 204, 0],
    [52, 199, 89],
    [0, 199, 190],
    [50, 173, 230],
    [0, 122, 255],
    [175, 82, 222],
];

pub const STICKER_EMOJIS: &[&str] = &[
    "😀", "😂", "🥰", "😎", "🤔", "👍", "🔥", "❤️", "⭐", "🎉", "🚀", "👀",
];

pub fn hit_test_crop(
    crop: NormCropRect,
    local: Vector2F,
    box_size: Vector2F,
) -> Option<CropHandle> {
    if box_size.x() <= 1.0 || box_size.y() <= 1.0 {
        return None;
    }
    let nx = (local.x() / box_size.x()).clamp(0.0, 1.0);
    let ny = (local.y() / box_size.y()).clamp(0.0, 1.0);
    let hx = HANDLE_HIT / box_size.x();
    let hy = HANDLE_HIT / box_size.y();
    let near = |a: f32, b: f32, t: f32| (a - b).abs() <= t;
    let in_x = nx >= crop.x0 - hx && nx <= crop.x1 + hx;
    let in_y = ny >= crop.y0 - hy && ny <= crop.y1 + hy;
    if near(nx, crop.x0, hx) && near(ny, crop.y0, hy) {
        return Some(CropHandle::Nw);
    }
    if near(nx, crop.x1, hx) && near(ny, crop.y0, hy) {
        return Some(CropHandle::Ne);
    }
    if near(nx, crop.x0, hx) && near(ny, crop.y1, hy) {
        return Some(CropHandle::Sw);
    }
    if near(nx, crop.x1, hx) && near(ny, crop.y1, hy) {
        return Some(CropHandle::Se);
    }
    if near(ny, crop.y0, hy) && in_x {
        return Some(CropHandle::N);
    }
    if near(ny, crop.y1, hy) && in_x {
        return Some(CropHandle::S);
    }
    if near(nx, crop.x0, hx) && in_y {
        return Some(CropHandle::W);
    }
    if near(nx, crop.x1, hx) && in_y {
        return Some(CropHandle::E);
    }
    if nx >= crop.x0 && nx <= crop.x1 && ny >= crop.y0 && ny <= crop.y1 {
        return Some(CropHandle::Move);
    }
    None
}

pub fn apply_crop_drag(
    start: NormCropRect,
    handle: CropHandle,
    origin: Vector2F,
    pos: Vector2F,
    box_size: Vector2F,
    aspect: Option<f32>,
) -> NormCropRect {
    if box_size.x() <= 1.0 || box_size.y() <= 1.0 {
        return start;
    }
    let dx = (pos.x() - origin.x()) / box_size.x();
    let dy = (pos.y() - origin.y()) / box_size.y();
    let mut c = start;
    match handle {
        CropHandle::Move => {
            let w = c.x1 - c.x0;
            let h = c.y1 - c.y0;
            c.x0 = (c.x0 + dx).clamp(0.0, 1.0 - w);
            c.y0 = (c.y0 + dy).clamp(0.0, 1.0 - h);
            c.x1 = c.x0 + w;
            c.y1 = c.y0 + h;
        }
        CropHandle::Nw => {
            c.x0 += dx;
            c.y0 += dy;
        }
        CropHandle::Ne => {
            c.x1 += dx;
            c.y0 += dy;
        }
        CropHandle::Sw => {
            c.x0 += dx;
            c.y1 += dy;
        }
        CropHandle::Se => {
            c.x1 += dx;
            c.y1 += dy;
        }
        CropHandle::N => c.y0 += dy,
        CropHandle::S => c.y1 += dy,
        CropHandle::W => c.x0 += dx,
        CropHandle::E => c.x1 += dx,
    }
    c.enforce_aspect(aspect)
}

fn draw_thick_line(img: &mut RgbImage, x0: i32, y0: i32, x1: i32, y1: i32, rgb: Rgb<u8>, r: i32) {
    let mut x0 = x0;
    let mut y0 = y0;
    let dx = (x1 - x0).abs();
    let sx = if x0 < x1 { 1 } else { -1 };
    let dy = -(y1 - y0).abs();
    let sy = if y0 < y1 { 1 } else { -1 };
    let mut err = dx + dy;
    loop {
        for oy in -r..=r {
            for ox in -r..=r {
                if ox * ox + oy * oy <= r * r {
                    let px = x0 + ox;
                    let py = y0 + oy;
                    if px >= 0 && py >= 0 && (px as u32) < img.width() && (py as u32) < img.height()
                    {
                        img.put_pixel(px as u32, py as u32, rgb);
                    }
                }
            }
        }
        if x0 == x1 && y0 == y1 {
            break;
        }
        let e2 = 2 * err;
        if e2 >= dy {
            err += dy;
            x0 += sx;
        }
        if e2 <= dx {
            err += dx;
            y0 += sy;
        }
    }
}

fn blend_pixel(dst: &mut Rgb<u8>, src: Rgb<u8>, alpha: f32) {
    let a = alpha.clamp(0.0, 1.0);
    dst.0[0] = ((1.0 - a) * dst.0[0] as f32 + a * src.0[0] as f32) as u8;
    dst.0[1] = ((1.0 - a) * dst.0[1] as f32 + a * src.0[1] as f32) as u8;
    dst.0[2] = ((1.0 - a) * dst.0[2] as f32 + a * src.0[2] as f32) as u8;
}

fn draw_thick_line_blend(
    img: &mut RgbImage,
    x0: i32,
    y0: i32,
    x1: i32,
    y1: i32,
    rgb: Rgb<u8>,
    r: i32,
    alpha: f32,
) {
    let mut x0 = x0;
    let mut y0 = y0;
    let dx = (x1 - x0).abs();
    let sx = if x0 < x1 { 1 } else { -1 };
    let dy = -(y1 - y0).abs();
    let sy = if y0 < y1 { 1 } else { -1 };
    let mut err = dx + dy;
    loop {
        for oy in -r..=r {
            for ox in -r..=r {
                if ox * ox + oy * oy <= r * r {
                    let px = x0 + ox;
                    let py = y0 + oy;
                    if px >= 0 && py >= 0 && (px as u32) < img.width() && (py as u32) < img.height()
                    {
                        let p = img.get_pixel_mut(px as u32, py as u32);
                        blend_pixel(p, rgb, alpha);
                    }
                }
            }
        }
        if x0 == x1 && y0 == y1 {
            break;
        }
        let e2 = 2 * err;
        if e2 >= dy {
            err += dy;
            x0 += sx;
        }
        if e2 <= dx {
            err += dx;
            y0 += sy;
        }
    }
}

fn erase_line(
    img: &mut RgbImage,
    original: &RgbImage,
    x0: i32,
    y0: i32,
    x1: i32,
    y1: i32,
    r: i32,
) {
    let mut x0 = x0;
    let mut y0 = y0;
    let dx = (x1 - x0).abs();
    let sx = if x0 < x1 { 1 } else { -1 };
    let dy = -(y1 - y0).abs();
    let sy = if y0 < y1 { 1 } else { -1 };
    let mut err = dx + dy;
    loop {
        for oy in -r..=r {
            for ox in -r..=r {
                if ox * ox + oy * oy <= r * r {
                    let px = x0 + ox;
                    let py = y0 + oy;
                    if px >= 0
                        && py >= 0
                        && (px as u32) < img.width()
                        && (py as u32) < img.height()
                        && (px as u32) < original.width()
                        && (py as u32) < original.height()
                    {
                        img.put_pixel(px as u32, py as u32, *original.get_pixel(px as u32, py as u32));
                    }
                }
            }
        }
        if x0 == x1 && y0 == y1 {
            break;
        }
        let e2 = 2 * err;
        if e2 >= dy {
            err += dy;
            x0 += sx;
        }
        if e2 <= dx {
            err += dx;
            y0 += sy;
        }
    }
}

fn blur_disk(img: &mut RgbImage, cx: i32, cy: i32, r: i32) {
    if r <= 0 {
        return;
    }
    let w = img.width() as i32;
    let h = img.height() as i32;
    let mut samples: Vec<(u32, u32, Rgb<u8>)> = Vec::new();
    for oy in -r..=r {
        for ox in -r..=r {
            if ox * ox + oy * oy > r * r {
                continue;
            }
            let px = cx + ox;
            let py = cy + oy;
            if px < 0 || py < 0 || px >= w || py >= h {
                continue;
            }
            let mut sr = 0u32;
            let mut sg = 0u32;
            let mut sb = 0u32;
            let mut n = 0u32;
            let br = (r / 2).max(1);
            for by in -br..=br {
                for bx in -br..=br {
                    let qx = px + bx;
                    let qy = py + by;
                    if qx < 0 || qy < 0 || qx >= w || qy >= h {
                        continue;
                    }
                    let p = img.get_pixel(qx as u32, qy as u32);
                    sr += p.0[0] as u32;
                    sg += p.0[1] as u32;
                    sb += p.0[2] as u32;
                    n += 1;
                }
            }
            if n > 0 {
                samples.push((
                    px as u32,
                    py as u32,
                    Rgb([(sr / n) as u8, (sg / n) as u8, (sb / n) as u8]),
                ));
            }
        }
    }
    for (x, y, rgb) in samples {
        img.put_pixel(x, y, rgb);
    }
}

fn paint_stroke_on(img: &mut RgbImage, original: &RgbImage, stroke: &PaintStroke) {
    let w = img.width().max(1) as f32;
    let h = img.height().max(1) as f32;
    if stroke.points.is_empty() {
        return;
    }
    let radius = ((stroke.width * w.min(h)) * 0.5).round().max(1.0) as i32;
    let rgb = Rgb(stroke.color);
    // Single click: stamp a disk so a tap still leaves ink.
    if stroke.points.len() == 1 {
        let (x, y) = stroke.points[0];
        let px = (x * w).round() as i32;
        let py = (y * h).round() as i32;
        match stroke.tool {
            BrushTool::Eraser => erase_line(img, original, px, py, px, py, radius),
            BrushTool::Blur => blur_disk(img, px, py, radius),
            BrushTool::Marker => draw_thick_line_blend(img, px, py, px, py, rgb, radius, 0.35),
            BrushTool::Pen | BrushTool::Arrow => {
                draw_thick_line(img, px, py, px, py, rgb, radius);
            }
        }
        return;
    }
    match stroke.tool {
        BrushTool::Pen | BrushTool::Arrow => {
            for pair in stroke.points.windows(2) {
                draw_thick_line(
                    img,
                    (pair[0].0 * w).round() as i32,
                    (pair[0].1 * h).round() as i32,
                    (pair[1].0 * w).round() as i32,
                    (pair[1].1 * h).round() as i32,
                    rgb,
                    radius,
                );
            }
            if stroke.tool == BrushTool::Arrow {
                if let (Some(&(x0, y0)), Some(&(x1, y1))) =
                    (stroke.points.first(), stroke.points.last())
                {
                    let ax0 = (x0 * w) as f32;
                    let ay0 = (y0 * h) as f32;
                    let ax1 = (x1 * w) as f32;
                    let ay1 = (y1 * h) as f32;
                    let dx = ax1 - ax0;
                    let dy = ay1 - ay0;
                    let len = (dx * dx + dy * dy).sqrt().max(1.0);
                    let ux = dx / len;
                    let uy = dy / len;
                    let head = (radius as f32 * 4.0).max(10.0);
                    let lx = ax1 - ux * head + (-uy) * head * 0.45;
                    let ly = ay1 - uy * head + ux * head * 0.45;
                    let rx = ax1 - ux * head - (-uy) * head * 0.45;
                    let ry = ay1 - uy * head - ux * head * 0.45;
                    draw_thick_line(
                        img,
                        ax1 as i32,
                        ay1 as i32,
                        lx as i32,
                        ly as i32,
                        rgb,
                        radius,
                    );
                    draw_thick_line(
                        img,
                        ax1 as i32,
                        ay1 as i32,
                        rx as i32,
                        ry as i32,
                        rgb,
                        radius,
                    );
                }
            }
        }
        BrushTool::Marker => {
            for pair in stroke.points.windows(2) {
                draw_thick_line_blend(
                    img,
                    (pair[0].0 * w).round() as i32,
                    (pair[0].1 * h).round() as i32,
                    (pair[1].0 * w).round() as i32,
                    (pair[1].1 * h).round() as i32,
                    rgb,
                    radius,
                    0.35,
                );
            }
        }
        BrushTool::Eraser => {
            for pair in stroke.points.windows(2) {
                erase_line(
                    img,
                    original,
                    (pair[0].0 * w).round() as i32,
                    (pair[0].1 * h).round() as i32,
                    (pair[1].0 * w).round() as i32,
                    (pair[1].1 * h).round() as i32,
                    radius,
                );
            }
        }
        BrushTool::Blur => {
            for &(x, y) in &stroke.points {
                blur_disk(
                    img,
                    (x * w).round() as i32,
                    (y * h).round() as i32,
                    radius,
                );
            }
        }
    }
}

fn fill_rect(img: &mut RgbImage, x0: i32, y0: i32, x1: i32, y1: i32, rgb: Rgb<u8>, filled: bool) {
    let (xa, xb) = (x0.min(x1), x0.max(x1));
    let (ya, yb) = (y0.min(y1), y0.max(y1));
    if filled {
        for y in ya..=yb {
            for x in xa..=xb {
                if x >= 0 && y >= 0 && (x as u32) < img.width() && (y as u32) < img.height() {
                    img.put_pixel(x as u32, y as u32, rgb);
                }
            }
        }
    } else {
        for x in xa..=xb {
            if ya >= 0 && (ya as u32) < img.height() && x >= 0 && (x as u32) < img.width() {
                img.put_pixel(x as u32, ya as u32, rgb);
            }
            if yb >= 0 && (yb as u32) < img.height() && x >= 0 && (x as u32) < img.width() {
                img.put_pixel(x as u32, yb as u32, rgb);
            }
        }
        for y in ya..=yb {
            if xa >= 0 && (xa as u32) < img.width() && y >= 0 && (y as u32) < img.height() {
                img.put_pixel(xa as u32, y as u32, rgb);
            }
            if xb >= 0 && (xb as u32) < img.width() && y >= 0 && (y as u32) < img.height() {
                img.put_pixel(xb as u32, y as u32, rgb);
            }
        }
    }
}

fn fill_ellipse(img: &mut RgbImage, cx: i32, cy: i32, rx: i32, ry: i32, rgb: Rgb<u8>, filled: bool) {
    let rx = rx.max(1);
    let ry = ry.max(1);
    for y in (cy - ry)..=(cy + ry) {
        for x in (cx - rx)..=(cx + rx) {
            let nx = (x - cx) as f32 / rx as f32;
            let ny = (y - cy) as f32 / ry as f32;
            let d = nx * nx + ny * ny;
            let hit = if filled {
                d <= 1.0
            } else {
                d <= 1.0 && d >= 0.82
            };
            if hit && x >= 0 && y >= 0 && (x as u32) < img.width() && (y as u32) < img.height() {
                img.put_pixel(x as u32, y as u32, rgb);
            }
        }
    }
}

fn draw_glyph_block(img: &mut RgbImage, cx: f32, cy: f32, size: f32, color: [u8; 3], filled: bool) {
    // Simple placeholder block for emoji/text (monochrome stamp).
    let w = img.width().max(1) as f32;
    let h = img.height().max(1) as f32;
    let px = (cx * w).round() as i32;
    let py = (cy * h).round() as i32;
    let r = ((size * w.min(h)) * 0.5).round().max(4.0) as i32;
    let rgb = Rgb(color);
    if filled {
        fill_ellipse(img, px, py, r, r, rgb, true);
    } else {
        fill_rect(img, px - r, py - r / 2, px + r, py + r / 2, rgb, true);
    }
}

fn apply_layer(img: &mut RgbImage, original: &RgbImage, layer: &EditLayer) {
    let w = img.width().max(1) as f32;
    let h = img.height().max(1) as f32;
    match layer {
        EditLayer::Stroke(stroke) => paint_stroke_on(img, original, stroke),
        EditLayer::Sticker { x, y, size, .. } => {
            draw_glyph_block(img, *x, *y, *size, [255, 220, 80], true);
        }
        EditLayer::Text { x, y, color, .. } => {
            draw_glyph_block(img, *x, *y, 0.08, *color, false);
        }
        EditLayer::Shape {
            kind,
            filled,
            x0,
            y0,
            x1,
            y1,
            color,
            width,
        } => {
            let xa = (*x0 * w).round() as i32;
            let ya = (*y0 * h).round() as i32;
            let xb = (*x1 * w).round() as i32;
            let yb = (*y1 * h).round() as i32;
            let rgb = Rgb(*color);
            let stroke_r = ((*width * w.min(h)) * 0.5).round().max(1.0) as i32;
            match kind {
                ShapeKind::Rectangle | ShapeKind::Bubble => {
                    if *filled {
                        fill_rect(img, xa, ya, xb, yb, rgb, true);
                    } else {
                        for t in 0..=stroke_r {
                            fill_rect(img, xa - t, ya - t, xb + t, yb + t, rgb, false);
                        }
                    }
                }
                ShapeKind::Circle => {
                    let cx = (xa + xb) / 2;
                    let cy = (ya + yb) / 2;
                    let rx = ((xb - xa).abs() / 2).max(1);
                    let ry = ((yb - ya).abs() / 2).max(1);
                    fill_ellipse(img, cx, cy, rx, ry, rgb, *filled);
                }
                ShapeKind::Arrow => {
                    draw_thick_line(img, xa, ya, xb, yb, rgb, stroke_r);
                    let dx = (xb - xa) as f32;
                    let dy = (yb - ya) as f32;
                    let len = (dx * dx + dy * dy).sqrt().max(1.0);
                    let ux = dx / len;
                    let uy = dy / len;
                    let head = (stroke_r as f32 * 5.0).max(12.0);
                    let lx = xb as f32 - ux * head + (-uy) * head * 0.4;
                    let ly = yb as f32 - uy * head + ux * head * 0.4;
                    let rx = xb as f32 - ux * head - (-uy) * head * 0.4;
                    let ry = yb as f32 - uy * head - ux * head * 0.4;
                    draw_thick_line(img, xb, yb, lx as i32, ly as i32, rgb, stroke_r);
                    draw_thick_line(img, xb, yb, rx as i32, ry as i32, rgb, stroke_r);
                }
                ShapeKind::Star => {
                    let cx = (xa + xb) / 2;
                    let cy = (ya + yb) / 2;
                    let rx = ((xb - xa).abs() / 2).max(2);
                    let ry = ((yb - ya).abs() / 2).max(2);
                    // Five spikes approximated as diamond + cross.
                    fill_ellipse(img, cx, cy, rx, ry / 2, rgb, *filled);
                    fill_ellipse(img, cx, cy, rx / 2, ry, rgb, *filled);
                }
            }
        }
    }
}

pub fn render_edited_rgb(
    source: &Path,
    layers: &[EditLayer],
    flipped: bool,
    rotate_quarters_cw: i32,
    crop: Option<NormCropRect>,
) -> Result<RgbImage, String> {
    let rgb = decode_chat_image_to_rgb(source).map_err(|e| e.to_string())?;
    let original = rgb.clone();
    let mut img = DynamicImage::ImageRgb8(rgb);
    {
        let mut rgb8 = img.to_rgb8();
        for layer in layers {
            apply_layer(&mut rgb8, &original, layer);
        }
        img = DynamicImage::ImageRgb8(rgb8);
    }
    if flipped {
        img = img.fliph();
    }
    let turns = rotate_quarters_cw.rem_euclid(4);
    for _ in 0..turns {
        img = img.rotate90();
    }
    if let Some(crop) = crop {
        let crop = crop.clamp();
        if !crop.is_identity() {
            let w = img.width();
            let h = img.height();
            let x = (crop.x0 * w as f32).round() as u32;
            let y = (crop.y0 * h as f32).round() as u32;
            let cw = ((crop.x1 - crop.x0) * w as f32).round() as u32;
            let ch = ((crop.y1 - crop.y0) * h as f32).round() as u32;
            let cw = cw.max(1).min(w.saturating_sub(x));
            let ch = ch.max(1).min(h.saturating_sub(y));
            img = img.crop_imm(x, y, cw, ch);
        }
    }
    Ok(img.to_rgb8())
}

pub fn apply_image_edit(
    source: &Path,
    staging_dir: &Path,
    rotate_quarters_cw: i32,
    crop: NormCropRect,
    flipped: bool,
    layers: &[EditLayer],
) -> Result<PathBuf, String> {
    let rgb = render_edited_rgb(source, layers, flipped, rotate_quarters_cw, Some(crop))?;
    std::fs::create_dir_all(staging_dir).map_err(|e| e.to_string())?;
    let out = staging_dir.join(format!("edit-{}.png", uuid::Uuid::new_v4()));
    DynamicImage::ImageRgb8(rgb)
        .save_with_format(&out, ImageFormat::Png)
        .map_err(|e| e.to_string())?;
    Ok(out)
}

fn bar_bg() -> ColorU {
    ColorU::new(40, 40, 40, 230)
}

fn edge_btn(
    label: String,
    id: &str,
    font: FamilyId,
    accent: bool,
    on_click: impl Fn(&mut warpui::elements::EventContext) + 'static,
) -> Box<dyn Element> {
    let color = if accent {
        theme::accent_cool()
    } else {
        ColorU::new(255, 255, 255, 230)
    };
    AutomationTarget::new(
        EventHandler::new(
            ConstrainedBox::new(
                Align::new(
                    Container::new(
                        ui_text::body(label.clone(), font)
                            .with_color(color)
                            .finish(),
                    )
                    .with_padding_left(22.0)
                    .with_padding_right(22.0)
                    .finish(),
                )
                .finish(),
            )
            .with_height(BUTTON_BAR_HEIGHT)
            .finish(),
        )
        .on_left_mouse_down(move |ctx, _, _| {
            on_click(ctx);
            DispatchEventResult::StopPropagation
        })
        .finish(),
    )
    .with_label(label)
    .with_id(id)
    .finish()
}

fn svg_icon_btn(
    path: &'static str,
    label: String,
    id: &str,
    active: bool,
    on_click: impl Fn(&mut warpui::elements::EventContext) + 'static,
) -> Box<dyn Element> {
    let fg = if active {
        theme::accent_cool()
    } else {
        ColorU::new(240, 240, 240, 220)
    };
    // ConstrainedBox must wrap the whole hit target (not only the glyph inside Align),
    // otherwise mid-bar icons collapse to ~22px and look cramped.
    AutomationTarget::new(
        EventHandler::new(
            ConstrainedBox::new(Align::new(icons::icon(path, 22.0, fg)).finish())
                .with_width(ICON_BTN)
                .with_height(ICON_BTN)
                .finish(),
        )
        .on_left_mouse_down(move |ctx, _, _| {
            on_click(ctx);
            DispatchEventResult::StopPropagation
        })
        .finish(),
    )
    .with_label(label)
    .with_id(id)
    .finish()
}

fn text_glyph_btn(
    glyph: &str,
    label: String,
    id: &str,
    font: FamilyId,
    active: bool,
    on_click: impl Fn(&mut warpui::elements::EventContext) + 'static,
) -> Box<dyn Element> {
    let fg = if active {
        theme::accent_cool()
    } else {
        ColorU::new(240, 240, 240, 220)
    };
    AutomationTarget::new(
        EventHandler::new(
            ConstrainedBox::new(
                Align::new(
                    ui_text::section_title(glyph.to_string(), font)
                        .with_color(fg)
                        .finish(),
                )
                .finish(),
            )
            .with_width(ICON_BTN)
            .with_height(ICON_BTN)
            .finish(),
        )
        .on_left_mouse_down(move |ctx, _, _| {
            on_click(ctx);
            DispatchEventResult::StopPropagation
        })
        .finish(),
    )
    .with_label(label)
    .with_id(id)
    .finish()
}

fn button_bar(children: Vec<Box<dyn Element>>, width: f32) -> Box<dyn Element> {
    let mut row = Flex::row()
        .with_main_axis_size(MainAxisSize::Max)
        .with_cross_axis_alignment(CrossAxisAlignment::Center)
        .with_main_axis_alignment(MainAxisAlignment::SpaceBetween);
    for child in children {
        row.add_child(child);
    }
    ConstrainedBox::new(
        Container::new(row.finish())
            .with_background(bar_bg())
            .with_corner_radius(CornerRadius::with_all(Radius::Pixels(BUTTON_BAR_HEIGHT / 2.0)))
            .with_padding_left(BUTTON_BAR_EDGE_PAD)
            .with_padding_right(BUTTON_BAR_EDGE_PAD)
            .finish(),
    )
    .with_width(width)
    .with_height(BUTTON_BAR_HEIGHT)
    .finish()
}

fn gapped(gap: f32, child: Box<dyn Element>) -> Box<dyn Element> {
    Container::new(child).with_padding_left(gap).finish()
}

fn tool_circle_btn(
    path: &'static str,
    label: String,
    id: &str,
    active: bool,
    on_click: impl Fn(&mut warpui::elements::EventContext) + 'static,
) -> Box<dyn Element> {
    let fg = if active {
        theme::accent_cool()
    } else {
        ColorU::new(240, 240, 240, 230)
    };
    let bg = if active {
        ColorU::new(60, 60, 60, 255)
    } else {
        ColorU::new(48, 48, 48, 230)
    };
    AutomationTarget::new(
        EventHandler::new(
            ConstrainedBox::new(
                Container::new(Align::new(icons::icon(path, 18.0, fg)).finish())
                    .with_background(bg)
                    .with_corner_radius(CornerRadius::with_all(Radius::Pixels(TOOL_BTN / 2.0)))
                    .finish(),
            )
            .with_width(TOOL_BTN)
            .with_height(TOOL_BTN)
            .finish(),
        )
        .on_left_mouse_down(move |ctx, _, _| {
            on_click(ctx);
            DispatchEventResult::StopPropagation
        })
        .finish(),
    )
    .with_label(label)
    .with_id(id)
    .finish()
}

fn color_ring_btn(
    color: [u8; 3],
    active: bool,
    on_click: impl Fn(&mut warpui::elements::EventContext) + 'static,
) -> Box<dyn Element> {
    let ring = if active {
        theme::accent_cool()
    } else {
        ColorU::new(255, 80, 80, 255)
    };
    AutomationTarget::new(
        EventHandler::new(
            ConstrainedBox::new(
                Container::new(
                    Align::new(
                        ConstrainedBox::new(
                            Container::new(Flex::column().finish())
                                .with_background(ColorU::new(color[0], color[1], color[2], 255))
                                .with_corner_radius(CornerRadius::with_all(Radius::Pixels(9.0)))
                                .finish(),
                        )
                        .with_width(18.0)
                        .with_height(18.0)
                        .finish(),
                    )
                    .finish(),
                )
                .with_background(ring)
                .with_corner_radius(CornerRadius::with_all(Radius::Pixels(TOOL_BTN / 2.0)))
                .with_border(Border::all(2.0).with_border_color(ring))
                .finish(),
            )
            .with_width(TOOL_BTN)
            .with_height(TOOL_BTN)
            .finish(),
        )
        .on_left_mouse_down(move |ctx, _, _| {
            on_click(ctx);
            DispatchEventResult::StopPropagation
        })
        .finish(),
    )
    .with_label(wormhole_i18n::t("chat.media_upload.edit_tool_color"))
    .with_id("chat:media_edit_tool_color")
    .finish()
}

/// Transform bottom bar: Cancel | Flip Rotate Paint Ratio | Done
pub fn transform_controls_bar(
    font: FamilyId,
    flipped: bool,
    on_cancel: impl Fn(&mut warpui::elements::EventContext) + 'static,
    on_flip: impl Fn(&mut warpui::elements::EventContext) + 'static,
    on_rotate: impl Fn(&mut warpui::elements::EventContext) + 'static,
    on_paint: impl Fn(&mut warpui::elements::EventContext) + 'static,
    on_ratio: impl Fn(&mut warpui::elements::EventContext) + 'static,
    on_done: impl Fn(&mut warpui::elements::EventContext) + 'static,
) -> Box<dyn Element> {
    let mid = Flex::row()
        .with_main_axis_size(MainAxisSize::Min)
        .with_cross_axis_alignment(CrossAxisAlignment::Center)
        .with_child(svg_icon_btn(
            "media-edit-flip.svg",
            wormhole_i18n::t("chat.media_upload.edit_flip"),
            "chat:media_edit_flip",
            flipped,
            on_flip,
        ))
        .with_child(gapped(
            TOOL_GAP,
            svg_icon_btn(
                "media-edit-rotate.svg",
                wormhole_i18n::t("chat.media_upload.edit_rotate_cw"),
                "chat:media_edit_rotate_cw",
                false,
                on_rotate,
            ),
        ))
        .with_child(gapped(
            TOOL_GAP,
            svg_icon_btn(
                "media-edit-paint.svg",
                wormhole_i18n::t("chat.media_upload.edit_paint"),
                "chat:media_edit_paint",
                false,
                on_paint,
            ),
        ))
        .with_child(gapped(
            TOOL_GAP,
            svg_icon_btn(
                "media-edit-ratio.svg",
                wormhole_i18n::t("chat.media_upload.edit_ratio"),
                "chat:media_edit_ratio",
                false,
                on_ratio,
            ),
        ))
        .finish();

    Align::new(button_bar(
        vec![
            edge_btn(
                wormhole_i18n::t("chat.media_upload.edit_cancel"),
                "chat:media_edit_cancel",
                font,
                false,
                on_cancel,
            ),
            mid,
            edge_btn(
                wormhole_i18n::t("chat.media_upload.edit_apply"),
                "chat:media_edit_apply",
                font,
                true,
                on_done,
            ),
        ],
        BUTTON_BAR_WIDTH,
    ))
    .finish()
}

pub fn brush_size_rail(
    size: f32,
    on_size_t: impl Fn(f32, &mut warpui::elements::EventContext) + Clone + 'static,
) -> Box<dyn Element> {
    let t = brush_t_from_width(size);
    let rail_h = BRUSH_SIZE_RAIL_H;
    let track_w = BRUSH_SIZE_TRACK_W;
    let handle_d = 6.0 + t * 16.0;
    let top = BRUSH_SIZE_HIT_PAD;
    let bottom = rail_h - BRUSH_SIZE_HIT_PAD;
    let cy = top + (1.0 - t) * (bottom - top);
    let white = ColorU::new(255, 255, 255, 244);
    let track = ColorU::new(255, 255, 255, 140);

    let mut stack = Stack::new();
    // Vertical track (approx collapsed wedge as a thin bar).
    stack.add_positioned_overlay_child(
        ConstrainedBox::new(
            Container::new(Flex::column().finish())
                .with_background(track)
                .with_corner_radius(CornerRadius::with_all(Radius::Pixels(track_w * 0.5)))
                .finish(),
        )
        .with_width(track_w)
        .with_height((bottom - top).max(1.0))
        .finish(),
        OffsetPositioning::offset_from_parent(
            vec2f(22.0 - track_w * 0.5, top),
            ParentOffsetBounds::Unbounded,
            ParentAnchor::TopLeft,
            ChildAnchor::TopLeft,
        ),
    );
    // White circular handle.
    stack.add_positioned_overlay_child(
        ConstrainedBox::new(
            Container::new(Flex::column().finish())
                .with_background(white)
                .with_corner_radius(CornerRadius::with_all(Radius::Pixels(handle_d * 0.5)))
                .finish(),
        )
        .with_width(handle_d)
        .with_height(handle_d)
        .finish(),
        OffsetPositioning::offset_from_parent(
            vec2f(22.0 - handle_d * 0.5, cy - handle_d * 0.5),
            ParentOffsetBounds::Unbounded,
            ParentAnchor::TopLeft,
            ChildAnchor::TopLeft,
        ),
    );

    let hit = ConstrainedBox::new(stack.finish())
        .with_width(44.0)
        .with_height(rail_h)
        .finish();

    let on_down = on_size_t.clone();
    let on_drag = on_size_t;
    AutomationTarget::new(
        EventHandler::new(SavePosition::new(hit, MEDIA_EDIT_BRUSH_SIZE_POS).finish())
            .on_left_mouse_down(move |ctx, _, pos| {
                let origin = ctx
                    .element_position_by_id(MEDIA_EDIT_BRUSH_SIZE_POS)
                    .map(|r| vec2f(r.origin().x(), r.origin().y()))
                    .unwrap_or_else(|| vec2f(0.0, 0.0));
                let local = canvas_local_from_window(pos, origin);
                on_down(brush_t_from_local_y(local.y(), rail_h), ctx);
                DispatchEventResult::StopPropagation
            })
            .on_mouse_dragged(move |ctx, _, pos| {
                let origin = ctx
                    .element_position_by_id(MEDIA_EDIT_BRUSH_SIZE_POS)
                    .map(|r| vec2f(r.origin().x(), r.origin().y()))
                    .unwrap_or_else(|| vec2f(0.0, 0.0));
                let local = canvas_local_from_window(pos, origin);
                on_drag(brush_t_from_local_y(local.y(), rail_h), ctx);
                DispatchEventResult::StopPropagation
            })
            .finish(),
    )
    .with_label(wormhole_i18n::t("chat.media_upload.edit_brush_size"))
    .with_id("chat:media_edit_brush_size")
    .finish()
}

/// Paint dual bars + ColorPicker tools between undo/redo.
pub fn paint_controls_bars(
    font: FamilyId,
    brush: BrushTool,
    color: [u8; 3],
    on_undo: impl Fn(&mut warpui::elements::EventContext) + 'static,
    on_redo: impl Fn(&mut warpui::elements::EventContext) + 'static,
    on_color: impl Fn(&mut warpui::elements::EventContext) + 'static,
    on_tool: impl Fn(BrushTool, &mut warpui::elements::EventContext) + Clone + 'static,
    on_cancel: impl Fn(&mut warpui::elements::EventContext) + 'static,
    on_paint: impl Fn(&mut warpui::elements::EventContext) + 'static,
    on_sticker: impl Fn(&mut warpui::elements::EventContext) + 'static,
    on_text: impl Fn(&mut warpui::elements::EventContext) + 'static,
    on_shape: impl Fn(&mut warpui::elements::EventContext) + 'static,
    on_done: impl Fn(&mut warpui::elements::EventContext) + 'static,
) -> Box<dyn Element> {
    let mut tools = Flex::row()
        .with_main_axis_size(MainAxisSize::Min)
        .with_cross_axis_alignment(CrossAxisAlignment::Center)
        .with_child(color_ring_btn(color, true, on_color));
    for tool in BrushTool::all() {
        let t = *tool;
        let on_tool = on_tool.clone();
        tools.add_child(
            Container::new(tool_circle_btn(
                t.icon_path(),
                wormhole_i18n::t(t.label_key()),
                t.automation_id(),
                brush == t,
                move |ctx| on_tool(t, ctx),
            ))
            .with_padding_left(TOOL_GAP)
            .finish(),
        );
    }

    let top_width = BUTTON_BAR_WIDTH + 100.0;
    let top = Align::new(button_bar(
        vec![
            svg_icon_btn(
                "media-edit-undo.svg",
                wormhole_i18n::t("chat.media_upload.edit_undo"),
                "chat:media_edit_undo",
                false,
                on_undo,
            ),
            tools.finish(),
            svg_icon_btn(
                "media-edit-redo.svg",
                wormhole_i18n::t("chat.media_upload.edit_redo"),
                "chat:media_edit_redo",
                false,
                on_redo,
            ),
        ],
        top_width,
    ))
    .finish();

    let mid = Flex::row()
        .with_main_axis_size(MainAxisSize::Min)
        .with_cross_axis_alignment(CrossAxisAlignment::Center)
        .with_child(svg_icon_btn(
            "media-edit-paint.svg",
            wormhole_i18n::t("chat.media_upload.edit_paint"),
            "chat:media_edit_paint",
            true,
            on_paint,
        ))
        .with_child(gapped(
            TOOL_GAP,
            svg_icon_btn(
                "media-edit-stickers.svg",
                wormhole_i18n::t("chat.media_upload.edit_sticker"),
                "chat:media_edit_sticker",
                false,
                on_sticker,
            ),
        ))
        .with_child(gapped(
            TOOL_GAP,
            text_glyph_btn(
                "A",
                wormhole_i18n::t("chat.media_upload.edit_text"),
                "chat:media_edit_text",
                font,
                false,
                on_text,
            ),
        ))
        .with_child(gapped(
            TOOL_GAP,
            svg_icon_btn(
                "media-edit-shapes.svg",
                wormhole_i18n::t("chat.media_upload.edit_shape"),
                "chat:media_edit_shape",
                false,
                on_shape,
            ),
        ))
        .finish();

    let bottom = Align::new(button_bar(
        vec![
            edge_btn(
                wormhole_i18n::t("chat.media_upload.edit_cancel"),
                "chat:media_edit_cancel",
                font,
                true,
                on_cancel,
            ),
            mid,
            edge_btn(
                wormhole_i18n::t("chat.media_upload.edit_apply"),
                "chat:media_edit_apply",
                font,
                true,
                on_done,
            ),
        ],
        BUTTON_BAR_WIDTH,
    ))
    .finish();

    Flex::column()
        .with_main_axis_size(MainAxisSize::Max)
        .with_cross_axis_alignment(CrossAxisAlignment::Center)
        .with_child(top)
        .with_child(
            ConstrainedBox::new(Flex::column().finish())
                .with_height(CONTROLS_CENTER_SKIP)
                .finish(),
        )
        .with_child(bottom)
        .finish()
}

pub fn controls_footer(
    font: FamilyId,
    mode: MediaEditMode,
    flipped: bool,
    brush: BrushTool,
    color: [u8; 3],
    on_transform_cancel: impl Fn(&mut warpui::elements::EventContext) + 'static,
    on_flip: impl Fn(&mut warpui::elements::EventContext) + 'static,
    on_rotate: impl Fn(&mut warpui::elements::EventContext) + 'static,
    on_enter_paint: impl Fn(&mut warpui::elements::EventContext) + 'static,
    on_ratio: impl Fn(&mut warpui::elements::EventContext) + 'static,
    on_transform_done: impl Fn(&mut warpui::elements::EventContext) + 'static,
    on_undo: impl Fn(&mut warpui::elements::EventContext) + 'static,
    on_redo: impl Fn(&mut warpui::elements::EventContext) + 'static,
    on_color: impl Fn(&mut warpui::elements::EventContext) + 'static,
    on_tool: impl Fn(BrushTool, &mut warpui::elements::EventContext) + Clone + 'static,
    on_paint_cancel: impl Fn(&mut warpui::elements::EventContext) + 'static,
    on_paint_tool: impl Fn(&mut warpui::elements::EventContext) + 'static,
    on_sticker: impl Fn(&mut warpui::elements::EventContext) + 'static,
    on_text: impl Fn(&mut warpui::elements::EventContext) + 'static,
    on_shape: impl Fn(&mut warpui::elements::EventContext) + 'static,
    on_paint_done: impl Fn(&mut warpui::elements::EventContext) + 'static,
) -> Box<dyn Element> {
    let inner = match mode {
        MediaEditMode::Transform => transform_controls_bar(
            font,
            flipped,
            on_transform_cancel,
            on_flip,
            on_rotate,
            on_enter_paint,
            on_ratio,
            on_transform_done,
        ),
        MediaEditMode::Paint => paint_controls_bars(
            font,
            brush,
            color,
            on_undo,
            on_redo,
            on_color,
            on_tool,
            on_paint_cancel,
            on_paint_tool,
            on_sticker,
            on_text,
            on_shape,
            on_paint_done,
        ),
    };
    ConstrainedBox::new(
        Container::new(Align::new(inner).finish())
            .with_padding_bottom(CONTROLS_BOTTOM_SKIP)
            .finish(),
    )
    .with_height(CONTROLS_HEIGHT)
    .finish()
}

pub fn ratio_menu(
    font: FamilyId,
    current: CropRatio,
    on_pick: impl Fn(CropRatio, &mut warpui::elements::EventContext) + Clone + 'static,
) -> Box<dyn Element> {
    let mut col = Flex::column().with_cross_axis_alignment(CrossAxisAlignment::Stretch);
    for ratio in CropRatio::all() {
        let label = wormhole_i18n::t(ratio.label_key());
        let selected = *ratio == current;
        let on_pick = on_pick.clone();
        let r = *ratio;
        col.add_child(
            AutomationTarget::new(
                EventHandler::new(
                    Container::new(
                        ui_text::body(label.clone(), font)
                            .with_color(if selected {
                                theme::accent_cool()
                            } else {
                                theme::text()
                            })
                            .finish(),
                    )
                    .with_uniform_padding(10.0)
                    .finish(),
                )
                .on_left_mouse_down(move |ctx, _, _| {
                    on_pick(r, ctx);
                    DispatchEventResult::StopPropagation
                })
                .finish(),
            )
            .with_label(label)
            .with_id(format!("chat:media_edit_ratio_{:?}", r).to_lowercase())
            .finish(),
        );
    }
    Container::new(col.finish())
        .with_background(theme::panel())
        .with_corner_radius(CornerRadius::with_all(Radius::Pixels(10.0)))
        .with_border(Border::all(1.0).with_border_color(theme::border()))
        .finish()
}

pub fn sticker_panel(
    font: FamilyId,
    on_pick: impl Fn(String, &mut warpui::elements::EventContext) + Clone + 'static,
) -> Box<dyn Element> {
    let mut row = Flex::row().with_main_axis_size(MainAxisSize::Min);
    for emoji in STICKER_EMOJIS {
        let e = (*emoji).to_string();
        let on_pick = on_pick.clone();
        let label = e.clone();
        row.add_child(
            AutomationTarget::new(
                EventHandler::new(
                    Container::new(
                        ui_text::section_title(e.clone(), font)
                            .with_color(ColorU::white())
                            .finish(),
                    )
                    .with_uniform_padding(8.0)
                    .finish(),
                )
                .on_left_mouse_down(move |ctx, _, _| {
                    on_pick(e.clone(), ctx);
                    DispatchEventResult::StopPropagation
                })
                .finish(),
            )
            .with_label(label)
            .with_id(format!("chat:media_edit_sticker_emoji_{emoji}"))
            .finish(),
        );
    }
    Container::new(row.finish())
        .with_background(bar_bg())
        .with_corner_radius(CornerRadius::with_all(Radius::Pixels(12.0)))
        .with_uniform_padding(6.0)
        .finish()
}

pub fn shape_menu(
    font: FamilyId,
    filled: bool,
    on_pick: impl Fn(ShapeKind, &mut warpui::elements::EventContext) + Clone + 'static,
    on_toggle_fill: impl Fn(&mut warpui::elements::EventContext) + 'static,
) -> Box<dyn Element> {
    let mut col = Flex::column().with_cross_axis_alignment(CrossAxisAlignment::Stretch);
    for kind in ShapeKind::all() {
        let k = *kind;
        let on_pick = on_pick.clone();
        let label = wormhole_i18n::t(k.label_key());
        col.add_child(
            AutomationTarget::new(
                EventHandler::new(
                    Flex::row()
                        .with_cross_axis_alignment(CrossAxisAlignment::Center)
                        .with_child(icons::icon(k.icon_path(filled), 20.0, ColorU::white()))
                        .with_child(
                            Container::new(
                                ui_text::body(label.clone(), font)
                                    .with_color(theme::text())
                                    .finish(),
                            )
                            .with_padding_left(8.0)
                            .finish(),
                        )
                        .finish(),
                )
                .on_left_mouse_down(move |ctx, _, _| {
                    on_pick(k, ctx);
                    DispatchEventResult::StopPropagation
                })
                .finish(),
            )
            .with_label(label)
            .with_id(format!("chat:media_edit_shape_{:?}", k).to_lowercase())
            .finish(),
        );
    }
    let fill_label = if filled {
        wormhole_i18n::t("chat.media_upload.edit_shape_outline")
    } else {
        wormhole_i18n::t("chat.media_upload.edit_shape_fill")
    };
    col.add_child(
        AutomationTarget::new(
            EventHandler::new(
                Container::new(
                    ui_text::body(fill_label.clone(), font)
                        .with_color(theme::accent_cool())
                        .finish(),
                )
                .with_uniform_padding(10.0)
                .finish(),
            )
            .on_left_mouse_down(move |ctx, _, _| {
                on_toggle_fill(ctx);
                DispatchEventResult::StopPropagation
            })
            .finish(),
        )
        .with_label(fill_label)
        .with_id("chat:media_edit_shape_fill_toggle")
        .finish(),
    );
    Container::new(col.finish())
        .with_background(theme::panel())
        .with_corner_radius(CornerRadius::with_all(Radius::Pixels(10.0)))
        .with_border(Border::all(1.0).with_border_color(theme::border()))
        .with_uniform_padding(6.0)
        .finish()
}

pub fn color_palette(
    current: [u8; 3],
    on_pick: impl Fn([u8; 3], &mut warpui::elements::EventContext) + Clone + 'static,
) -> Box<dyn Element> {
    let mut row = Flex::row().with_main_axis_size(MainAxisSize::Min);
    for c in BRUSH_PRESET_COLORS {
        let on_pick = on_pick.clone();
        let selected = c == current;
        row.add_child(
            AutomationTarget::new(
                EventHandler::new(
                    ConstrainedBox::new(
                        Container::new(Flex::column().finish())
                            .with_background(ColorU::new(c[0], c[1], c[2], 255))
                            .with_corner_radius(CornerRadius::with_all(Radius::Pixels(10.0)))
                            .with_border(
                                Border::all(if selected { 2.0 } else { 1.0 })
                                    .with_border_color(if selected {
                                        theme::accent_cool()
                                    } else {
                                        ColorU::new(255, 255, 255, 80)
                                    }),
                            )
                            .finish(),
                    )
                    .with_width(20.0)
                    .with_height(20.0)
                    .finish(),
                )
                .on_left_mouse_down(move |ctx, _, _| {
                    on_pick(c, ctx);
                    DispatchEventResult::StopPropagation
                })
                .finish(),
            )
            .with_label(format!("#{:02x}{:02x}{:02x}", c[0], c[1], c[2]))
            .with_id(format!("chat:media_edit_color_{:02x}{:02x}{:02x}", c[0], c[1], c[2]))
            .finish(),
        );
        row.add_child(
            ConstrainedBox::new(Flex::row().finish())
                .with_width(6.0)
                .finish(),
        );
    }
    Container::new(row.finish())
        .with_background(bar_bg())
        .with_corner_radius(CornerRadius::with_all(Radius::Pixels(12.0)))
        .with_uniform_padding(8.0)
        .finish()
}

pub fn crop_overlay_layers(
    crop: NormCropRect,
    box_w: f32,
    box_h: f32,
    show_grid: bool,
) -> Vec<(Box<dyn Element>, Vector2F)> {
    let crop = crop.clamp();
    let fade = ColorU::new(0, 0, 0, 140);
    let line = ColorU::new(255, 255, 255, 230);
    let x0 = crop.x0 * box_w;
    let y0 = crop.y0 * box_h;
    let x1 = crop.x1 * box_w;
    let y1 = crop.y1 * box_h;
    let cw = (x1 - x0).max(1.0);
    let ch = (y1 - y0).max(1.0);
    let mut layers = Vec::new();
    let sized = |w: f32, h: f32, child: Box<dyn Element>| {
        ConstrainedBox::new(child).with_width(w).with_height(h).finish()
    };

    if y0 > 0.5 {
        layers.push((
            sized(
                box_w,
                y0,
                Container::new(Flex::column().finish())
                    .with_background(fade)
                    .finish(),
            ),
            vec2f(0.0, 0.0),
        ));
    }
    if box_h - y1 > 0.5 {
        layers.push((
            sized(
                box_w,
                box_h - y1,
                Container::new(Flex::column().finish())
                    .with_background(fade)
                    .finish(),
            ),
            vec2f(0.0, y1),
        ));
    }
    if x0 > 0.5 {
        layers.push((
            sized(
                x0,
                ch,
                Container::new(Flex::column().finish())
                    .with_background(fade)
                    .finish(),
            ),
            vec2f(0.0, y0),
        ));
    }
    if box_w - x1 > 0.5 {
        layers.push((
            sized(
                box_w - x1,
                ch,
                Container::new(Flex::column().finish())
                    .with_background(fade)
                    .finish(),
            ),
            vec2f(x1, y0),
        ));
    }
    layers.push((
        sized(
            cw,
            ch,
            Container::new(Flex::column().finish())
                .with_border(Border::all(1.5).with_border_color(line))
                .finish(),
        ),
        vec2f(x0, y0),
    ));
    let arm = 14.0_f32;
    let thick = 3.0_f32;
    let corners = [
        (x0, y0, arm, thick),
        (x0, y0, thick, arm),
        (x1 - arm, y0, arm, thick),
        (x1 - thick, y0, thick, arm),
        (x0, y1 - thick, arm, thick),
        (x0, y1 - arm, thick, arm),
        (x1 - arm, y1 - thick, arm, thick),
        (x1 - thick, y1 - arm, thick, arm),
    ];
    for (lx, ly, w, h) in corners {
        layers.push((
            sized(
                w,
                h,
                Container::new(Flex::column().finish())
                    .with_background(line)
                    .finish(),
            ),
            vec2f(lx, ly),
        ));
    }
    if show_grid {
        for i in 1..3 {
            let gx = x0 + cw * (i as f32) / 3.0;
            let gy = y0 + ch * (i as f32) / 3.0;
            layers.push((
                sized(
                    1.0,
                    ch,
                    Container::new(Flex::column().finish())
                        .with_background(ColorU::new(255, 255, 255, 120))
                        .finish(),
                ),
                vec2f(gx, y0),
            ));
            layers.push((
                sized(
                    cw,
                    1.0,
                    Container::new(Flex::column().finish())
                        .with_background(ColorU::new(255, 255, 255, 120))
                        .finish(),
                ),
                vec2f(x0, gy),
            ));
        }
    }
    layers
}

/// Lightweight in-progress paint overlay (segments as small colored rects).
/// Used so drag ink appears before async `rebuild_edit_preview` finishes.
pub fn live_stroke_overlay_layers(
    stroke: &PaintStroke,
    box_w: f32,
    box_h: f32,
) -> Vec<(Box<dyn Element>, Vector2F)> {
    if stroke.points.is_empty() {
        return Vec::new();
    }
    let color = match stroke.tool {
        BrushTool::Eraser => ColorU::new(200, 200, 200, 160),
        BrushTool::Blur => ColorU::new(180, 180, 220, 140),
        _ => ColorU::new(stroke.color[0], stroke.color[1], stroke.color[2], 220),
    };
    let thickness = ((stroke.width * box_w.min(box_h)).max(3.0)).min(18.0);
    let sized = |w: f32, h: f32, child: Box<dyn Element>| {
        ConstrainedBox::new(child).with_width(w).with_height(h).finish()
    };
    let mut layers = Vec::new();
    let pts: Vec<(f32, f32)> = if stroke.points.len() == 1 {
        let p = stroke.points[0];
        vec![p, p]
    } else {
        stroke.points.clone()
    };
    for pair in pts.windows(2) {
        let (x0, y0) = (pair[0].0 * box_w, pair[0].1 * box_h);
        let (x1, y1) = (pair[1].0 * box_w, pair[1].1 * box_h);
        let dx = x1 - x0;
        let dy = y1 - y0;
        let len = (dx * dx + dy * dy).sqrt().max(1.0);
        let steps = ((len / (thickness * 0.5)).ceil() as i32).clamp(1, 48);
        for i in 0..=steps {
            let t = i as f32 / steps as f32;
            let x = x0 + dx * t - thickness * 0.5;
            let y = y0 + dy * t - thickness * 0.5;
            layers.push((
                sized(
                    thickness,
                    thickness,
                    Container::new(Flex::column().finish())
                        .with_background(color)
                        .with_corner_radius(CornerRadius::with_all(Radius::Pixels(thickness * 0.5)))
                        .finish(),
                ),
                vec2f(x, y),
            ));
        }
    }
    layers
}

/// In-progress shape rubber-band as a border rect.
pub fn live_shape_overlay_layers(
    x0: f32,
    y0: f32,
    x1: f32,
    y1: f32,
    color: [u8; 3],
    box_w: f32,
    box_h: f32,
) -> Vec<(Box<dyn Element>, Vector2F)> {
    let xa = x0.min(x1) * box_w;
    let ya = y0.min(y1) * box_h;
    let xb = x0.max(x1) * box_w;
    let yb = y0.max(y1) * box_h;
    let cw = (xb - xa).max(2.0);
    let ch = (yb - ya).max(2.0);
    let line = ColorU::new(color[0], color[1], color[2], 230);
    vec![(
        ConstrainedBox::new(
            Container::new(Flex::column().finish())
                .with_border(Border::all(2.0).with_border_color(line))
                .finish(),
        )
        .with_width(cw)
        .with_height(ch)
        .finish(),
        vec2f(xa, ya),
    )]
}

pub fn inset_crop_10() -> NormCropRect {
    NormCropRect {
        x0: 0.1,
        y0: 0.1,
        x1: 0.9,
        y1: 0.9,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn write_test_png(dir: &Path, name: &str, w: u32, h: u32) -> PathBuf {
        let path = dir.join(name);
        let mut img = RgbImage::new(w, h);
        for (x, y, pixel) in img.enumerate_pixels_mut() {
            *pixel = Rgb([(x % 256) as u8, (y % 256) as u8, 80]);
        }
        DynamicImage::ImageRgb8(img)
            .save_with_format(&path, ImageFormat::Png)
            .unwrap();
        path
    }

    #[test]
    fn rotate_flip_crop_writes_readable_png() {
        let dir = tempfile::tempdir().unwrap();
        let src = write_test_png(dir.path(), "src.png", 40, 20);
        let stroke = PaintStroke {
            points: vec![(0.1, 0.1), (0.9, 0.9)],
            color: [255, 0, 0],
            width: 0.05,
            tool: BrushTool::Pen,
        };
        let out = apply_image_edit(
            &src,
            dir.path(),
            1,
            inset_crop_10(),
            true,
            &[EditLayer::Stroke(stroke)],
        )
        .unwrap();
        assert!(out.exists());
        let rgb = decode_chat_image_to_rgb(&out).unwrap();
        assert!(rgb.width() >= 10);
        assert!(rgb.height() >= 10);
    }

    #[test]
    fn fit_aspect_top_pins_y0_and_max_area() {
        let square = NormCropRect::fit_aspect_top(Some(1.0));
        assert!(square.y0 < 0.001);
        assert!((square.x1 - square.x0 - (square.y1 - square.y0)).abs() < 0.02);
        assert!((square.x1 - square.x0 - 1.0).abs() < 0.02 || (square.y1 - square.y0 - 1.0).abs() < 0.02);

        let wide = NormCropRect::fit_aspect_top(Some(16.0 / 9.0));
        assert!(wide.y0 < 0.001);
        assert!(((wide.x1 - wide.x0) / (wide.y1 - wide.y0) - 16.0 / 9.0).abs() < 0.05);
        assert!((wide.x1 - wide.x0 - 1.0).abs() < 0.02);
    }

    #[test]
    fn marker_and_blur_layers_write() {
        let dir = tempfile::tempdir().unwrap();
        let src = write_test_png(dir.path(), "src.png", 32, 32);
        let layers = vec![
            EditLayer::Stroke(PaintStroke {
                points: vec![(0.2, 0.2), (0.8, 0.8)],
                color: [0, 255, 0],
                width: 0.03,
                tool: BrushTool::Marker,
            }),
            EditLayer::Stroke(PaintStroke {
                points: vec![(0.5, 0.5), (0.6, 0.6)],
                color: [0, 0, 0],
                width: 0.04,
                tool: BrushTool::Blur,
            }),
            EditLayer::Text {
                text: "Hi".into(),
                x: 0.3,
                y: 0.3,
                color: [255, 255, 255],
            },
        ];
        let out = apply_image_edit(&src, dir.path(), 0, NormCropRect::default(), false, &layers)
            .unwrap();
        assert!(out.exists());
    }

    #[test]
    fn hit_test_corners() {
        let crop = NormCropRect {
            x0: 0.2,
            y0: 0.2,
            x1: 0.8,
            y1: 0.8,
        };
        let size = vec2f(100.0, 100.0);
        assert_eq!(
            hit_test_crop(crop, vec2f(20.0, 20.0), size),
            Some(CropHandle::Nw)
        );
        assert_eq!(
            hit_test_crop(crop, vec2f(50.0, 50.0), size),
            Some(CropHandle::Move)
        );
    }

    #[test]
    fn square_ratio_enforced_on_drag() {
        let c = NormCropRect {
            x0: 0.0,
            y0: 0.0,
            x1: 0.5,
            y1: 1.0,
        }
        .enforce_aspect(Some(1.0));
        assert!((c.x1 - c.x0) - (c.y1 - c.y0) < 0.02);
    }

    #[test]
    fn window_coords_normalize_relative_to_canvas_origin() {
        // Canvas at (540, 284), size 200×200 — clicking center of canvas.
        let origin = vec2f(540.0, 284.0);
        let window = vec2f(640.0, 384.0);
        let local = canvas_local_from_window(window, origin);
        assert!((local.x() - 100.0).abs() < 0.01);
        assert!((local.y() - 100.0).abs() < 0.01);
        let (nx, ny) = normalize_canvas_point(local, 200.0, 200.0);
        assert!((nx - 0.5).abs() < 0.01);
        assert!((ny - 0.5).abs() < 0.01);

        // Bug regression: treating window coords as local clamps everything to 1.0.
        let (bad_x, bad_y) = normalize_canvas_point(window, 200.0, 200.0);
        assert!((bad_x - 1.0).abs() < 0.01);
        assert!((bad_y - 1.0).abs() < 0.01);
    }

    #[test]
    fn single_point_stroke_writes_ink() {
        let dir = tempfile::tempdir().unwrap();
        let src = write_test_png(dir.path(), "src.png", 32, 32);
        let stroke = PaintStroke {
            points: vec![(0.5, 0.5)],
            color: [255, 0, 0],
            width: 0.08,
            tool: BrushTool::Pen,
        };
        let out = apply_image_edit(
            &src,
            dir.path(),
            0,
            NormCropRect::default(),
            false,
            &[EditLayer::Stroke(stroke)],
        )
        .unwrap();
        let rgb = decode_chat_image_to_rgb(&out).unwrap();
        let px = rgb.get_pixel(16, 16);
        assert_eq!(px.0[0], 255);
        assert_eq!(px.0[1], 0);
        assert_eq!(px.0[2], 0);
    }

    #[test]
    fn brush_size_y_maps_top_thick_bottom_thin() {
        let h = BRUSH_SIZE_RAIL_H;
        let top_t = brush_t_from_local_y(BRUSH_SIZE_HIT_PAD, h);
        let bot_t = brush_t_from_local_y(h - BRUSH_SIZE_HIT_PAD, h);
        let mid_t = brush_t_from_local_y(h * 0.5, h);
        assert!((top_t - 1.0).abs() < 0.02);
        assert!(bot_t < 0.05);
        assert!((mid_t - 0.5).abs() < 0.08);
        assert!((brush_width_from_t(0.0) - BRUSH_SIZE_MIN).abs() < 1e-6);
        assert!((brush_width_from_t(1.0) - BRUSH_SIZE_MAX).abs() < 1e-6);
        assert!((brush_t_from_width(brush_width_from_t(0.35)) - 0.35).abs() < 1e-5);
    }

    #[test]
    fn quantize_brush_t_snaps_to_steps() {
        assert!((quantize_brush_t(0.0) - 0.0).abs() < 1e-6);
        assert!((quantize_brush_t(1.0) - 1.0).abs() < 1e-6);
        let q = quantize_brush_t(0.37);
        assert!((q * BRUSH_SIZE_STEPS - (q * BRUSH_SIZE_STEPS).round()).abs() < 1e-5);
        // Neighbors within one step collapse → fewer UI rebuilds while dragging.
        assert_eq!(quantize_brush_t(0.352), quantize_brush_t(0.358));
    }

    #[test]
    fn strokes_from_layers_collects_paint_only() {
        let layers = vec![
            EditLayer::Stroke(PaintStroke {
                points: vec![(0.1, 0.1)],
                color: [1, 2, 3],
                width: 0.01,
                tool: BrushTool::Pen,
            }),
            EditLayer::Text {
                text: "x".into(),
                x: 0.5,
                y: 0.5,
                color: [0, 0, 0],
            },
            EditLayer::Stroke(PaintStroke {
                points: vec![(0.9, 0.9)],
                color: [9, 8, 7],
                width: 0.02,
                tool: BrushTool::Blur,
            }),
        ];
        let strokes = strokes_from_layers(&layers);
        assert_eq!(strokes.len(), 2);
        assert_eq!(strokes[0].color, [1, 2, 3]);
        assert_eq!(strokes[1].tool, BrushTool::Blur);
    }
}
