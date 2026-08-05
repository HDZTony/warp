//! AI idle capability marquee (`.agent-capability-marquee` in `desktop-current.html`).
//!
//! Shown only when no sidebar session is selected. Chips fill the composer draft
//! via [`super::AgentPanelAction::ApplyCapabilityPrompt`].
//!
//! Hover pauses scroll; mouse leave resumes from the same virtual time (pause
//! clock lives in [`MarqueeAnimState`] so Element rebuilds cannot reset offset).
//!
//! Chip hover / dismiss uses [`ChipHoverBank`]: `Hoverable` builds its child once
//! (`FnOnce`), so hover UI only appears after `notify` rebuild if `MouseState`
//! survives across View renders.

use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use pathfinder_color::ColorU;
use pathfinder_geometry::rect::RectF;
use pathfinder_geometry::vector::{vec2f, Vector2F};

use warpui::elements::{
    AfterLayoutContext, Align, AppContext, Border, Clipped, ConstrainedBox, Container,
    CrossAxisAlignment, DispatchEventResult, Element, EventContext, EventHandler, Flex, Hoverable,
    LayoutContext, LiveElement, MainAxisSize, MouseState, MouseStateHandle, PaintContext,
    ParentElement, Point, SizeConstraint, Text,
};
use warpui::fonts::FamilyId;
use warpui::ClipBounds;
use warpui_core::event::DispatchedEvent;

use super::AgentPanelAction;
use crate::ui::theme;

/// Matches `.agent-capability-row` height.
pub const ROW_HEIGHT: f32 = 44.0;
/// Matches marquee column `gap: 10px`.
pub const ROW_GAP: f32 = 10.0;
/// Matches `.agent-capability-track` `gap: 6px`.
pub const CHIP_GAP: f32 = 6.0;
/// Matches `.agent-capability-chip` horizontal padding.
pub const CHIP_PAD_X: f32 = 10.0;
/// Matches `.agent-capability-chip` vertical padding.
pub const CHIP_PAD_Y: f32 = 4.0;
/// Matches `.agent-capability-chip` `font-size: 14px`.
pub const CHIP_FONT_SIZE: f32 = 14.0;
/// Secondary dismiss label size (slightly smaller than chip).
pub const DISMISS_FONT_SIZE: f32 = 12.0;
/// HTML clones rows until at least 8 are present.
pub const TARGET_ROWS: usize = 8;
/// Matches marquee vertical padding in `.is-new-session`.
const MARQUEE_PAD_Y: f32 = 28.0;
const REPAINT_INTERVAL: Duration = Duration::from_millis(33);

/// Design-source capability prompts (5 rows × 6 chips) from `desktop-current.html`.
pub const CAPABILITY_ROW_PROMPTS: &[&[&str]] = &[
    &[
        "分析代码并定位错误",
        "整理文档并生成摘要",
        "对比终端同步状态",
        "规划并执行重复任务",
        "搜索项目文件并回答问题",
        "生成测试并提出修复建议",
    ],
    &[
        "梳理项目结构并提出改进",
        "检查日志并总结异常",
        "生成发布前检查清单",
        "解释陌生代码的工作方式",
        "找出重复文件与可清理项",
        "将需求拆分为开发任务",
    ],
    &[
        "同步跨终端文件并报告差异",
        "为当前改动生成测试用例",
        "分析性能瓶颈并给出建议",
        "编写可执行的技术方案",
        "检索项目资料并回答问题",
        "审查改动并发现潜在风险",
    ],
    &[
        "生成测试并提出修复建议",
        "搜索项目文件并回答问题",
        "规划并执行重复任务",
        "对比终端同步状态",
        "整理文档并生成摘要",
        "分析代码并定位错误",
    ],
    &[
        "制作变更摘要供团队审阅",
        "提取文件中的待办与决策",
        "为问题建立复现步骤",
        "检查配置是否存在冲突",
        "生成清晰的操作说明",
        "汇总跨项目的工作进度",
    ],
];

/// Per-row `(duration_secs, negative_delay_secs)` from HTML keyframes.
const ROW_ANIM: [(f32, f32); TARGET_ROWS] = [
    (32.0, 0.0),
    (36.0, 16.0),
    (29.0, 28.0),
    (39.0, 9.0),
    (33.0, 36.0),
    (35.0, 21.0),
    (30.0, 12.0),
    (38.0, 31.0),
];

/// Shared animation clock: survives Element rebuilds so pause/resume keeps position.
#[derive(Debug)]
pub struct MarqueeAnimState {
    started: Instant,
    pause_accum: Mutex<Duration>,
    paused_since: Mutex<Option<Instant>>,
}

impl MarqueeAnimState {
    pub fn new() -> Arc<Self> {
        Arc::new(Self {
            started: Instant::now(),
            pause_accum: Mutex::new(Duration::ZERO),
            paused_since: Mutex::new(None),
        })
    }

    /// Virtual elapsed seconds excluding time spent paused.
    pub fn elapsed_secs(&self) -> f32 {
        let pause_accum = self
            .pause_accum
            .lock()
            .map(|g| *g)
            .unwrap_or(Duration::ZERO);
        let paused_extra = self
            .paused_since
            .lock()
            .ok()
            .and_then(|g| g.map(|since| since.elapsed()))
            .unwrap_or(Duration::ZERO);
        self.started
            .elapsed()
            .saturating_sub(pause_accum)
            .saturating_sub(paused_extra)
            .as_secs_f32()
    }

    pub fn set_paused(&self, paused: bool) {
        let Ok(mut paused_since) = self.paused_since.lock() else {
            return;
        };
        let Ok(mut pause_accum) = self.pause_accum.lock() else {
            return;
        };
        if paused {
            if paused_since.is_none() {
                *paused_since = Some(Instant::now());
            }
        } else if let Some(since) = paused_since.take() {
            *pause_accum = pause_accum.saturating_add(since.elapsed());
        }
    }

    pub fn is_paused(&self) -> bool {
        self.paused_since
            .lock()
            .map(|g| g.is_some())
            .unwrap_or(false)
    }
}

/// Expand the 5 design rows to 8 by cycling (same as HTML JS clone loop).
pub fn expanded_capability_rows() -> Vec<&'static [&'static str]> {
    let mut rows = Vec::with_capacity(TARGET_ROWS);
    let mut i = 0usize;
    while rows.len() < TARGET_ROWS {
        rows.push(CAPABILITY_ROW_PROMPTS[i % CAPABILITY_ROW_PROMPTS.len()]);
        i += 1;
    }
    rows
}

fn chip_idle_color() -> ColorU {
    // CSS: color-mix(in srgb, var(--muted) 82%, var(--text))
    let muted = theme::muted();
    let text = theme::text();
    let mix = |a: u8, b: u8| -> u8 { (f32::from(a) * 0.82 + f32::from(b) * 0.18).round() as u8 };
    ColorU::new(mix(muted.r, text.r), mix(muted.g, text.g), mix(muted.b, text.b), 255)
}

/// Stable per-chip [`MouseState`] so hover (and dismiss) survives View rebuilds.
#[derive(Debug, Default)]
pub struct ChipHoverBank {
    states: Mutex<Vec<MouseStateHandle>>,
}

impl ChipHoverBank {
    pub fn new() -> Arc<Self> {
        Arc::new(Self {
            states: Mutex::new(Vec::new()),
        })
    }

    pub fn get(&self, index: usize) -> MouseStateHandle {
        let Ok(mut states) = self.states.lock() else {
            return Arc::new(Mutex::new(MouseState::default()));
        };
        while states.len() <= index {
            states.push(Arc::new(Mutex::new(MouseState::default())));
        }
        Arc::clone(&states[index])
    }
}

fn capability_chip(
    font: FamilyId,
    prompt: &str,
    mouse: MouseStateHandle,
) -> Box<dyn Element> {
    let prompt_owned = prompt.to_string();
    let label = prompt.to_string();
    Hoverable::new(mouse, move |state| {
        let hovered = state.is_hovered();
        let color = if hovered {
            theme::text()
        } else {
            chip_idle_color()
        };
        let prompt_text = Text::new(label.clone(), font, CHIP_FONT_SIZE)
            .with_color(color)
            .finish();
        let prompt_pad = Container::new(prompt_text)
            .with_padding_left(CHIP_PAD_X)
            .with_padding_right(if hovered { 6.0 } else { CHIP_PAD_X })
            .with_padding_top(CHIP_PAD_Y)
            .with_padding_bottom(CHIP_PAD_Y)
            .finish();

        let mut row = Flex::row()
            .with_cross_axis_alignment(CrossAxisAlignment::Center)
            .with_main_axis_size(MainAxisSize::Min)
            .with_child(prompt_pad);

        if hovered {
            let dismiss = EventHandler::new(
                Container::new(
                    Text::new("以后不再显示".to_string(), font, DISMISS_FONT_SIZE)
                        .with_color(theme::muted())
                        .finish(),
                )
                .with_padding_right(CHIP_PAD_X)
                .with_padding_top(CHIP_PAD_Y)
                .with_padding_bottom(CHIP_PAD_Y)
                .finish(),
            )
            .with_automation_label("以后不再显示")
            .with_automation_id("ai:capability_marquee_dismiss")
            .on_left_mouse_down(move |ctx, _, _| {
                ctx.dispatch_typed_action(AgentPanelAction::DismissCapabilityMarquee);
                DispatchEventResult::StopPropagation
            })
            .finish();
            row.add_child(dismiss);
        }

        let mut box_ = Container::new(row.finish());
        if hovered {
            box_ = box_.with_border(Border::bottom(1.0).with_border_fill(theme::accent_cool()));
        }
        box_.finish()
    })
    // Prefer Hoverable press over nested EventHandler so hit-testing uses
    // child_max_z_index (Stack-safe). Defer so dismiss can stop Apply.
    .with_defer_events_to_children()
    .on_mouse_down(move |ctx, _, _| {
        ctx.dispatch_typed_action(AgentPanelAction::ApplyCapabilityPrompt(
            prompt_owned.clone(),
        ));
    })
    .finish()
}

fn track_content(
    font: FamilyId,
    prompts: &[&str],
    chip_hovers: &ChipHoverBank,
    row_index: usize,
) -> Box<dyn Element> {
    let mut row = Flex::row()
        .with_cross_axis_alignment(CrossAxisAlignment::Center)
        .with_main_axis_size(MainAxisSize::Min);
    // Duplicate the sequence so translateX(-50%) loops seamlessly.
    let chips_per_pass = prompts.len();
    for pass in 0..2 {
        for (i, prompt) in prompts.iter().enumerate() {
            if pass > 0 || i > 0 {
                row.add_child(
                    ConstrainedBox::new(warpui::elements::Empty::new().finish())
                        .with_width(CHIP_GAP)
                        .with_height(1.0)
                        .finish(),
                );
            }
            let chip_index = row_index * chips_per_pass * 2 + pass * chips_per_pass + i;
            row.add_child(capability_chip(
                font,
                prompt,
                chip_hovers.get(chip_index),
            ));
        }
    }
    row.finish()
}

/// Horizontally scrolling track: paints duplicated chip row with `translateX` offset.
struct MarqueeRowTrack {
    child: Box<dyn Element>,
    clock: Arc<MarqueeAnimState>,
    duration_secs: f32,
    delay_secs: f32,
    viewport: Option<Vector2F>,
    content_width: f32,
    origin: Option<Point>,
}

impl MarqueeRowTrack {
    fn live(
        child: Box<dyn Element>,
        clock: Arc<MarqueeAnimState>,
        duration_secs: f32,
        delay_secs: f32,
    ) -> Box<dyn Element> {
        Box::new(LiveElement::new(
            Box::new(Self {
                child,
                clock,
                duration_secs,
                delay_secs,
                viewport: None,
                content_width: 0.0,
                origin: None,
            }),
            REPAINT_INTERVAL,
        ))
    }

    fn scroll_offset(&self) -> f32 {
        let cycle = self.content_width * 0.5;
        if cycle <= 0.0 || self.duration_secs <= 0.0 {
            return 0.0;
        }
        let elapsed = self.clock.elapsed_secs() + self.delay_secs;
        let progress = (elapsed / self.duration_secs).rem_euclid(1.0);
        progress * cycle
    }
}

impl Element for MarqueeRowTrack {
    fn layout(
        &mut self,
        constraint: SizeConstraint,
        ctx: &mut LayoutContext,
        app: &AppContext,
    ) -> Vector2F {
        let height = ROW_HEIGHT;
        let width = if constraint.max.x().is_finite() {
            constraint.max.x()
        } else {
            0.0
        };
        let child_constraint =
            SizeConstraint::new(vec2f(0.0, height), vec2f(f32::INFINITY, height));
        let child_size = self.child.layout(child_constraint, ctx, app);
        self.content_width = child_size.x();
        let size = vec2f(width, height);
        self.viewport = Some(size);
        size
    }

    fn after_layout(&mut self, ctx: &mut AfterLayoutContext, app: &AppContext) {
        self.child.after_layout(ctx, app);
    }

    fn paint(&mut self, origin: Vector2F, ctx: &mut PaintContext, app: &AppContext) {
        let size = self.viewport.unwrap_or_else(|| vec2f(1.0, ROW_HEIGHT));
        self.origin = Some(Point::from_vec2f(origin, ctx.scene.z_index()));
        let offset = self.scroll_offset();
        ctx.scene
            .start_layer(ClipBounds::BoundedBy(RectF::new(origin, size)));
        self.child.paint(origin - vec2f(offset, 0.0), ctx, app);
        ctx.scene.stop_layer();
        // Keep scheduling repaints while paused so leave→resume does not depend on a
        // one-shot notify; offset stays frozen via the shared clock.
        ctx.repaint_after(REPAINT_INTERVAL);
    }

    fn dispatch_event(
        &mut self,
        event: &DispatchedEvent,
        ctx: &mut EventContext,
        app: &AppContext,
    ) -> bool {
        self.child.origin().is_some() && self.child.dispatch_event(event, ctx, app)
    }

    fn size(&self) -> Option<Vector2F> {
        self.viewport
    }

    fn origin(&self) -> Option<Point> {
        self.origin
    }
}

fn marquee_row(
    font: FamilyId,
    prompts: &[&str],
    clock: Arc<MarqueeAnimState>,
    chip_hovers: &ChipHoverBank,
    row_index: usize,
    duration_secs: f32,
    delay_secs: f32,
) -> Box<dyn Element> {
    let track = MarqueeRowTrack::live(
        track_content(font, prompts, chip_hovers, row_index),
        clock,
        duration_secs,
        delay_secs,
    );
    ConstrainedBox::new(Clipped::new(track).finish())
        .with_height(ROW_HEIGHT)
        .finish()
}

/// Full-bleed idle marquee: 8 scrolling rows, vertical center; hover pauses.
pub fn render(
    font: FamilyId,
    clock: Arc<MarqueeAnimState>,
    hover_state: MouseStateHandle,
    chip_hovers: Arc<ChipHoverBank>,
) -> Box<dyn Element> {
    let rows = expanded_capability_rows();
    let mut col = Flex::column()
        .with_cross_axis_alignment(CrossAxisAlignment::Stretch)
        .with_main_axis_size(MainAxisSize::Min);
    for (i, prompts) in rows.iter().enumerate() {
        if i > 0 {
            col.add_child(
                ConstrainedBox::new(warpui::elements::Empty::new().finish())
                    .with_height(ROW_GAP)
                    .finish(),
            );
        }
        let (duration_secs, delay_secs) = ROW_ANIM[i];
        col.add_child(marquee_row(
            font,
            prompts,
            Arc::clone(&clock),
            chip_hovers.as_ref(),
            i,
            duration_secs,
            delay_secs,
        ));
    }
    let body = Container::new(col.finish())
        .with_padding_top(MARQUEE_PAD_Y)
        .with_padding_bottom(MARQUEE_PAD_Y)
        .finish();

    Hoverable::new(hover_state, move |_| Align::new(body).finish())
        .on_hover(move |is_hovered, _, _, _| {
            clock.set_paused(is_hovered);
        })
        .finish()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn expands_design_rows_to_eight() {
        let rows = expanded_capability_rows();
        assert_eq!(rows.len(), TARGET_ROWS);
        assert_eq!(rows[0], CAPABILITY_ROW_PROMPTS[0]);
        assert_eq!(rows[5], CAPABILITY_ROW_PROMPTS[0]);
        assert_eq!(rows[6], CAPABILITY_ROW_PROMPTS[1]);
        assert_eq!(rows[7], CAPABILITY_ROW_PROMPTS[2]);
    }

    #[test]
    fn each_design_row_has_six_chips() {
        for row in CAPABILITY_ROW_PROMPTS {
            assert_eq!(row.len(), 6);
            for prompt in *row {
                assert!(!prompt.is_empty());
            }
        }
        assert_eq!(CAPABILITY_ROW_PROMPTS.len(), 5);
    }

    #[test]
    fn chip_hover_bank_reuses_same_mouse_state() {
        let bank = ChipHoverBank::new();
        let a = bank.get(3);
        let b = bank.get(3);
        assert!(Arc::ptr_eq(&a, &b));
        let other = bank.get(4);
        assert!(!Arc::ptr_eq(&a, &other));
    }

    #[test]
    fn pause_freezes_elapsed_and_resume_continues() {
        let clock = MarqueeAnimState::new();
        std::thread::sleep(Duration::from_millis(30));
        let before = clock.elapsed_secs();
        assert!(before > 0.0);

        clock.set_paused(true);
        std::thread::sleep(Duration::from_millis(40));
        let paused = clock.elapsed_secs();
        assert!((paused - before).abs() < 0.02, "paused={paused} before={before}");
        assert!(clock.is_paused());

        clock.set_paused(false);
        std::thread::sleep(Duration::from_millis(30));
        let after = clock.elapsed_secs();
        assert!(after > paused + 0.01, "after={after} paused={paused}");
        assert!(!clock.is_paused());
    }
}
