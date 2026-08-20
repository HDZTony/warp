//! Telegram-style outgoing call confirmation / ringing overlay
//! (`Calls::Panel` WaitingUserConfirmation + `PanelBackground`).

use pathfinder_color::ColorU;
use pathfinder_geometry::rect::RectF;
use pathfinder_geometry::vector::{vec2f, Vector2F};
use warpui::elements::{
    AfterLayoutContext, Align, AutomationTarget, ConstrainedBox, Container, CornerRadius,
    CrossAxisAlignment, DispatchEventResult, EventContext, EventHandler, Flex, LayoutContext,
    MainAxisAlignment, MainAxisSize, PaintContext, ParentElement, Point, Radius, SizeConstraint,
    Stack,
};
use warpui::event::DispatchedEvent;
use warpui::fonts::FamilyId;
use warpui::ClipBounds;
use warpui::{AppContext, Element, Entity, TypedActionView, View, ViewContext};

use crate::ui::chat::labels::chat_avatar_for_os;
use crate::ui::chat::shell_state::{OutgoingCallUi, SharedChatShellState};
use crate::ui::icons;
use crate::ui::panel_primitives::tg_avatar;
use crate::ui::theme;
use crate::ui_text;

const AVATAR_SIZE: f32 = 160.0;
const BTN_SIZE: f32 = 64.0;
const BTN_GAP: f32 = 28.0;

#[derive(Debug, Clone)]
pub enum OutgoingCallAction {
    StartVoice,
    StartVideo,
    Cancel,
}

#[derive(Debug, Clone)]
pub enum OutgoingCallEvent {
    StartVoice,
    StartVideo,
    Cancel,
}

pub struct OutgoingCallPanelView {
    shell_state: SharedChatShellState,
    font: FamilyId,
    last_overlay_tick: u64,
}

impl OutgoingCallPanelView {
    pub fn new(
        ctx: &mut ViewContext<Self>,
        shell_state: SharedChatShellState,
        font: FamilyId,
    ) -> Self {
        let view = Self {
            shell_state,
            font,
            last_overlay_tick: 0,
        };
        view.start_overlay_poll(ctx);
        view
    }

    fn start_overlay_poll(&self, ctx: &mut ViewContext<Self>) {
        ctx.spawn(
            async move {
                tokio::time::sleep(std::time::Duration::from_millis(200)).await;
            },
            |view, _, ctx| {
                let tick = view
                    .shell_state
                    .lock()
                    .map(|state| state.overlay_tick)
                    .unwrap_or(0);
                if tick != view.last_overlay_tick {
                    view.last_overlay_tick = tick;
                    ctx.notify();
                }
                view.start_overlay_poll(ctx);
            },
        );
    }

    fn ui(&self) -> Option<OutgoingCallUi> {
        self.shell_state
            .lock()
            .ok()
            .and_then(|state| state.outgoing_call_ui.clone())
    }
}

impl Entity for OutgoingCallPanelView {
    type Event = OutgoingCallEvent;
}

impl View for OutgoingCallPanelView {
    fn ui_name() -> &'static str {
        "OutgoingCallPanelView"
    }

    fn render(&self, _app: &AppContext) -> Box<dyn Element> {
        let Some(ui) = self.ui() else {
            return Container::new(Flex::row().finish()).finish();
        };
        render_panel(self.font, &ui)
    }
}

impl TypedActionView for OutgoingCallPanelView {
    type Action = OutgoingCallAction;

    fn handle_action(&mut self, action: &OutgoingCallAction, ctx: &mut ViewContext<Self>) {
        match action {
            OutgoingCallAction::StartVoice => ctx.emit(OutgoingCallEvent::StartVoice),
            OutgoingCallAction::StartVideo => ctx.emit(OutgoingCallEvent::StartVideo),
            OutgoingCallAction::Cancel => ctx.emit(OutgoingCallEvent::Cancel),
        }
    }
}

fn render_panel(font: FamilyId, ui: &OutgoingCallUi) -> Box<dyn Element> {
    let ringing = ui.is_ringing();
    let video_ringing = ui.is_video_ringing();
    let title = ui.peer_title().to_string();
    let avatar = chat_avatar_for_os(ui.avatar_os());

    let status = if ringing {
        if video_ringing {
            wormhole_i18n::t("chat.call.outgoing_video_ringing")
        } else {
            wormhole_i18n::t("chat.call.outgoing_ringing")
        }
    } else {
        wormhole_i18n::t("chat.call.confirm_tip")
    };

    let mut col = Flex::column()
        .with_main_axis_size(MainAxisSize::Min)
        .with_cross_axis_alignment(CrossAxisAlignment::Center);
    col.add_child(tg_avatar(avatar, font, AVATAR_SIZE));
    col.add_child(
        Container::new(
            ui_text::chat_header_title(title, font)
                .with_color(theme::call_panel_text())
                .finish(),
        )
        .with_margin_top(20.0)
        .finish(),
    );
    col.add_child(
        Container::new(
            ui_text::body(status, font)
                .with_color(theme::call_panel_muted())
                .finish(),
        )
        .with_margin_top(10.0)
        .finish(),
    );

    let mut buttons = Flex::row()
        .with_main_axis_alignment(MainAxisAlignment::Center)
        .with_cross_axis_alignment(CrossAxisAlignment::Center)
        .with_main_axis_size(MainAxisSize::Min);

    if !ringing {
        buttons.add_child(call_round_btn(
            font,
            Some("chat-header-video.svg"),
            theme::success(),
            theme::chat_bubble_text(),
            wormhole_i18n::t("chat.call.start_video"),
            "chat:call_start_video",
            OutgoingCallAction::StartVideo,
            false,
        ));
        buttons.add_child(
            Container::new(call_round_btn(
                font,
                None,
                theme::call_panel_text(),
                theme::call_panel_bg(),
                wormhole_i18n::t("chat.call.cancel"),
                "chat:call_cancel",
                OutgoingCallAction::Cancel,
                true,
            ))
            .with_margin_left(BTN_GAP)
            .finish(),
        );
        buttons.add_child(
            Container::new(call_round_btn(
                font,
                Some("chat-header-phone.svg"),
                theme::success(),
                theme::chat_bubble_text(),
                wormhole_i18n::t("chat.call.start_call"),
                "chat:call_start",
                OutgoingCallAction::StartVoice,
                false,
            ))
            .with_margin_left(BTN_GAP)
            .finish(),
        );
    } else {
        buttons.add_child(call_round_btn(
            font,
            None,
            theme::call_panel_text(),
            theme::call_panel_bg(),
            wormhole_i18n::t("chat.call.cancel"),
            "chat:call_cancel",
            OutgoingCallAction::Cancel,
            true,
        ));
    }

    col.add_child(
        Container::new(buttons.finish())
            .with_margin_top(36.0)
            .finish(),
    );

    let mut body = Stack::new();
    body.add_child(Align::new(radial_glow(520.0, 16)).finish());
    body.add_child(Align::new(radial_glow(360.0, 24)).finish());
    body.add_child(Align::new(radial_glow(220.0, 32)).finish());
    body.add_child(Align::new(col.finish()).finish());

    let swallow = EventHandler::new(Align::new(body.finish()).finish())
        .skip_automation()
        .on_left_mouse_down(|_ctx, _, _| DispatchEventResult::StopPropagation)
        .finish();

    OutgoingCallBackdrop::new(
        AutomationTarget::new(swallow)
            .with_label(wormhole_i18n::t("chat.call.outgoing_panel"))
            .with_id("chat:outgoing_call")
            .finish(),
    )
}

fn radial_glow(size: f32, alpha: u8) -> Box<dyn Element> {
    ConstrainedBox::new(
        Container::new(Flex::row().finish())
            .with_background(theme::with_alpha(theme::accent_cool(), alpha))
            .with_corner_radius(CornerRadius::with_all(Radius::Percentage(50.0)))
            .finish(),
    )
    .with_width(size)
    .with_height(size)
    .with_min_width(size)
    .with_max_width(size)
    .with_min_height(size)
    .with_max_height(size)
    .finish()
}

fn call_round_btn(
    font: FamilyId,
    icon: Option<&'static str>,
    bg: ColorU,
    fg: ColorU,
    label: String,
    automation_id: &'static str,
    action: OutgoingCallAction,
    cancel_x: bool,
) -> Box<dyn Element> {
    let glyph: Box<dyn Element> = if let Some(path) = icon {
        icons::chat_header_icon(path, fg)
    } else if cancel_x {
        Align::new(
            ui_text::chat_header_title("×".to_string(), font)
                .with_color(fg)
                .finish(),
        )
        .finish()
    } else {
        Align::new(Flex::row().finish()).finish()
    };

    let circle = ConstrainedBox::new(
        Container::new(Align::new(glyph).finish())
            .with_background(bg)
            .with_corner_radius(CornerRadius::with_all(Radius::Percentage(50.0)))
            .finish(),
    )
    .with_width(BTN_SIZE)
    .with_height(BTN_SIZE)
    .with_min_width(BTN_SIZE)
    .with_max_width(BTN_SIZE)
    .with_min_height(BTN_SIZE)
    .with_max_height(BTN_SIZE)
    .finish();

    let mut col = Flex::column()
        .with_main_axis_size(MainAxisSize::Min)
        .with_cross_axis_alignment(CrossAxisAlignment::Center);
    col.add_child(circle);
    col.add_child(
        Container::new(
            ui_text::device_meta(label.clone(), font)
                .with_color(theme::call_panel_muted())
                .finish(),
        )
        .with_margin_top(8.0)
        .finish(),
    );

    AutomationTarget::new(
        EventHandler::new(col.finish())
            .skip_automation()
            .on_left_mouse_down(move |ctx, _, _| {
                ctx.dispatch_typed_action(action.clone());
                DispatchEventResult::StopPropagation
            })
            .finish(),
    )
    .with_label(label)
    .with_id(automation_id)
    .finish()
}

/// Full-size opaque void fill so chat doodle cannot show through
/// (tdesktop `PanelBackground` / `st::callBgOpaque`).
struct OutgoingCallBackdrop {
    child: Box<dyn Element>,
    size: Option<Vector2F>,
    origin: Option<Point>,
}

impl OutgoingCallBackdrop {
    fn new(child: Box<dyn Element>) -> Box<dyn Element> {
        Box::new(Self {
            child,
            size: None,
            origin: None,
        })
    }
}

impl Element for OutgoingCallBackdrop {
    fn layout(
        &mut self,
        constraint: SizeConstraint,
        ctx: &mut LayoutContext,
        app: &AppContext,
    ) -> Vector2F {
        let child_size = self.child.layout(constraint, ctx, app);
        let max = constraint.max;
        let size = vec2f(
            if max.x().is_finite() {
                max.x()
            } else {
                child_size.x()
            },
            if max.y().is_finite() {
                max.y()
            } else {
                child_size.y()
            },
        );
        self.size = Some(size);
        size
    }

    fn after_layout(&mut self, ctx: &mut AfterLayoutContext, app: &AppContext) {
        self.child.after_layout(ctx, app);
    }

    fn paint(&mut self, origin: Vector2F, ctx: &mut PaintContext, app: &AppContext) {
        self.origin = Some(Point::from_vec2f(origin, ctx.scene.z_index()));
        let size = self.size.unwrap_or_else(|| vec2f(1.0, 1.0));
        ctx.scene
            .start_layer(ClipBounds::BoundedBy(RectF::new(origin, size)));
        ctx.scene
            .draw_rect_without_hit_recording(RectF::new(origin, size))
            .with_background(theme::call_panel_bg());
        ctx.scene.stop_layer();
        self.child.paint(origin, ctx, app);
    }

    fn dispatch_event(
        &mut self,
        event: &DispatchedEvent,
        ctx: &mut EventContext,
        app: &AppContext,
    ) -> bool {
        self.child.dispatch_event(event, ctx, app)
    }

    fn size(&self) -> Option<Vector2F> {
        self.size
    }

    fn origin(&self) -> Option<Point> {
        self.origin
    }
}
