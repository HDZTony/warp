use pathfinder_color::ColorU;
use pathfinder_geometry::vector::vec2f;
use warpui::elements::{
    Align, Border, ChildAnchor, ConstrainedBox, Container, CornerRadius, CrossAxisAlignment, Empty,
    Flex, Hoverable, MainAxisAlignment, MainAxisSize, MouseStateHandle, OffsetPositioning,
    ParentAnchor, ParentElement, ParentOffsetBounds, Radius, Rect, Stack, Text,
};
use warpui::fonts::FamilyId;
use warpui::platform::FullscreenState;
use warpui::{Action, AppContext, Element, Entity, ModelContext, SingletonEntity, WindowId};

use crate::ui::theme;
use crate::ui_text;

/// Height of the integrated title row (tabs + caption). Must match [`super::app_shell`] layout.
pub const CHROME_ROW_HEIGHT: f32 = 54.0;

#[cfg(windows)]
const WINDOWS_TRAFFIC_LIGHT_WIDTH: f32 = 190.0;

const TAB_BAR_PADDING_LEFT: f32 = 16.0;
const BUTTON_ICON_SIZE: f32 = 22.0;
const WINDOWS_BUTTON_PADDING_VERTICAL: f32 = 6.0;
const WINDOWS_BUTTON_PADDING_HORIZONTAL: f32 = 12.0;
const WINDOWS_ICON_WIDTH: f32 = 46.0;
const WINDOWS_ICON_FONT_SIZE: f32 = 10.0;
const WINDOWS_GOLDEN_RATIO: f32 = 1.618_034;
const WINDOWS_BRIGHT_RED: ColorU = ColorU {
    r: 232,
    g: 17,
    b: 32,
    a: u8::MAX,
};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TrafficLightSide {
    Left,
    Right,
}

#[derive(Default)]
pub struct TrafficLightMouseStates {
    pub minimize_window_button: MouseStateHandle,
    pub maximize_window_button: MouseStateHandle,
    pub close_window_button: MouseStateHandle,
    #[cfg(windows)]
    pub ccswitch_intercept_toggle: MouseStateHandle,
}

/// Windows title-bar toggle: when enabled, Wormhole handles `ccswitch://` without forwarding to CC Switch.
#[cfg(windows)]
#[derive(Clone, Copy)]
pub struct CcswitchInterceptToggle<A: Action + Copy> {
    pub enabled: bool,
    pub toggle: A,
}

#[derive(Clone, Debug)]
pub struct TrafficLightData {
    width: f32,
    pub side: TrafficLightSide,
    scales_with_zoom: bool,
}

impl TrafficLightData {
    pub fn width(&self, zoom_factor: f32) -> f32 {
        if self.scales_with_zoom {
            self.width
        } else {
            self.width / zoom_factor
        }
    }
}

pub fn traffic_light_data(ctx: &AppContext, window_id: WindowId) -> Option<TrafficLightData> {
    if ctx
        .windows()
        .platform_window(window_id)
        .is_some_and(|window| window.uses_native_window_decorations())
    {
        return None;
    }

    if cfg!(target_os = "macos") {
        Some(TrafficLightData {
            width: 64.0,
            side: TrafficLightSide::Left,
            scales_with_zoom: false,
        })
    } else if cfg!(any(target_os = "linux", target_os = "freebsd"))
        && !ctx.windows().is_tiling_window_manager()
    {
        Some(TrafficLightData {
            width: 116.0,
            side: TrafficLightSide::Right,
            scales_with_zoom: true,
        })
    } else if cfg!(target_os = "windows") {
        Some(TrafficLightData {
            width: WINDOWS_TRAFFIC_LIGHT_WIDTH,
            side: TrafficLightSide::Right,
            scales_with_zoom: true,
        })
    } else {
        None
    }
}

pub fn should_reserve_traffic_light_space_in_tab_bar(side: TrafficLightSide) -> bool {
    side == TrafficLightSide::Right
}

pub fn tab_bar_left_padding(
    traffic_light_data: Option<&TrafficLightData>,
    zoom_factor: f32,
    is_fullscreen: bool,
) -> f32 {
    if is_fullscreen && cfg!(target_os = "macos") {
        TAB_BAR_PADDING_LEFT
    } else {
        traffic_light_data
            .filter(|data| data.side == TrafficLightSide::Left)
            .map(|data| data.width(zoom_factor))
            .unwrap_or(0.0)
            + TAB_BAR_PADDING_LEFT
    }
}

pub fn left_padding_spacer(width: f32) -> Box<dyn Element> {
    ConstrainedBox::new(Empty::new().finish())
        .with_width(width)
        .finish()
}

pub fn traffic_light_spacer(data: &TrafficLightData, zoom_factor: f32) -> Option<Box<dyn Element>> {
    if should_reserve_traffic_light_space_in_tab_bar(data.side) {
        Some(left_padding_spacer(data.width(zoom_factor)))
    } else {
        None
    }
}

#[derive(Clone, Copy)]
pub struct TrafficLightActions<A: Action + Copy> {
    pub minimize: A,
    pub toggle_maximize: A,
    pub close: A,
}

pub fn render_traffic_lights<A: Action + Copy + 'static>(
    window_id: WindowId,
    app: &AppContext,
    mouse_states: &TrafficLightMouseStates,
    actions: TrafficLightActions<A>,
    ui_font: FamilyId,
    #[cfg(windows)] ccswitch_intercept: Option<CcswitchInterceptToggle<A>>,
) -> Box<dyn Element> {
    let Some(data) = traffic_light_data(app, window_id) else {
        return Empty::new().finish();
    };

    if cfg!(target_os = "macos") {
        return Empty::new().finish();
    }

    let fullscreen_state = app
        .windows()
        .platform_window(window_id)
        .map(|window| window.fullscreen_state())
        .unwrap_or_default();

    #[cfg(target_os = "windows")]
    {
        return data.render_windows(
            fullscreen_state,
            mouse_states,
            actions,
            app,
            ui_font,
            ccswitch_intercept,
        );
    }

    #[cfg(any(target_os = "linux", target_os = "freebsd"))]
    {
        return data.render_linux(fullscreen_state, mouse_states, actions);
    }

    #[cfg(not(any(windows, target_os = "linux", target_os = "freebsd")))]
    {
        let _ = (data, fullscreen_state, mouse_states, actions);
        Empty::new().finish()
    }
}

#[cfg(target_os = "macos")]
pub fn sync_window_button_visibility(ctx: &mut warpui::ViewContext<impl warpui::View>) {
    use warpui::platform::mac::WindowExt;

    if let Some(platform_window) = ctx.windows().platform_window(ctx.window_id()) {
        platform_window.as_ref().set_window_buttons(true);
    }
}

#[cfg(not(target_os = "macos"))]
pub fn sync_window_button_visibility(_ctx: &mut warpui::ViewContext<impl warpui::View>) {}

#[cfg(windows)]
pub struct WindowsSymbolFontState {
    icon_font_family: Option<FamilyId>,
}

#[cfg(windows)]
impl WindowsSymbolFontState {
    pub fn new(ctx: &mut ModelContext<Self>) -> Self {
        let icon_font_family = if windows_version::OsVersion::current().build >= 22000 {
            Self::load_symbol_font("Segoe Fluent Icons", ctx)
                .or_else(|| Self::load_symbol_font("Segoe MDL2 Assets", ctx))
        } else {
            Self::load_symbol_font("Segoe MDL2 Assets", ctx)
        };
        Self { icon_font_family }
    }

    fn load_symbol_font(symbol_font: &str, ctx: &mut AppContext) -> Option<FamilyId> {
        warpui::fonts::Cache::handle(ctx).update(ctx, |font_cache, _| {
            match font_cache.get_or_load_system_font(symbol_font) {
                Ok(family) => Some(family),
                Err(err) => {
                    log::warn!("Failed to load windows symbol font {symbol_font}: {err:?}");
                    None
                }
            }
        })
    }

    pub fn icon_font_family(&self) -> Option<FamilyId> {
        self.icon_font_family
    }
}

#[cfg(windows)]
impl Entity for WindowsSymbolFontState {
    type Event = ();
}

#[cfg(windows)]
impl SingletonEntity for WindowsSymbolFontState {}

#[cfg(windows)]
#[derive(Copy, Clone)]
enum WindowsTrafficLightIcon {
    Close,
    Minimize,
    Maximize,
    Restore,
}

#[cfg(windows)]
impl WindowsTrafficLightIcon {
    fn unicode_code_point(self) -> &'static str {
        match self {
            Self::Minimize => "\u{e921}",
            Self::Restore => "\u{e923}",
            Self::Maximize => "\u{e922}",
            Self::Close => "\u{e8bb}",
        }
    }

    fn background_hover_color(self) -> ColorU {
        match self {
            Self::Close => WINDOWS_BRIGHT_RED,
            Self::Minimize | Self::Maximize | Self::Restore => theme::panel_elevated(),
        }
    }

    fn icon_color(self, hovered: bool) -> ColorU {
        match self {
            Self::Close if hovered => ColorU::white(),
            _ => theme::text(),
        }
    }

    fn render<A: Action + Copy + 'static>(
        self,
        mouse_state_handle: MouseStateHandle,
        icon_font_family: FamilyId,
        action: A,
    ) -> Box<dyn Element> {
        let hoverable = Hoverable::new(mouse_state_handle, move |state| {
            let hovered = state.is_hovered();
            let icon_color = self.icon_color(hovered);
            let icon = Text::new(
                self.unicode_code_point(),
                icon_font_family,
                WINDOWS_ICON_FONT_SIZE,
            )
            .with_color(icon_color)
            .with_line_height_ratio(WINDOWS_GOLDEN_RATIO)
            .finish();
            let icon = Align::new(icon).finish();
            if hovered {
                Container::new(icon)
                    .with_background(self.background_hover_color())
                    .finish()
            } else {
                icon
            }
        })
        .on_click(move |ctx, _, _| {
            ctx.dispatch_typed_action(action);
        })
        .finish();

        ConstrainedBox::new(hoverable)
            .with_width(WINDOWS_ICON_WIDTH)
            .finish()
    }
}

impl TrafficLightData {
    #[cfg(windows)]
    fn render_windows<A: Action + Copy + 'static>(
        &self,
        fullscreen_state: FullscreenState,
        mouse_states: &TrafficLightMouseStates,
        actions: TrafficLightActions<A>,
        app: &AppContext,
        ui_font: FamilyId,
        ccswitch_intercept: Option<CcswitchInterceptToggle<A>>,
    ) -> Box<dyn Element> {
        if let Some(icon_font_family) = WindowsSymbolFontState::handle(app)
            .as_ref(app)
            .icon_font_family()
        {
            return self.render_windows_with_glyph_icons(
                fullscreen_state,
                mouse_states,
                actions,
                icon_font_family,
                ccswitch_intercept,
                ui_font,
            );
        }

        log::warn!("Unable to load Windows symbol font; using rect icons for traffic lights");
        self.render_windows_with_rect_icons(
            fullscreen_state,
            mouse_states,
            actions,
            ui_font,
            ccswitch_intercept,
        )
    }

    #[cfg(windows)]
    fn render_windows_with_glyph_icons<A: Action + Copy + 'static>(
        &self,
        fullscreen_state: FullscreenState,
        mouse_states: &TrafficLightMouseStates,
        actions: TrafficLightActions<A>,
        icon_font_family: FamilyId,
        ccswitch_intercept: Option<CcswitchInterceptToggle<A>>,
        ui_font: FamilyId,
    ) -> Box<dyn Element> {
        let maximize_icon = if fullscreen_state == FullscreenState::Normal {
            WindowsTrafficLightIcon::Maximize
        } else {
            WindowsTrafficLightIcon::Restore
        };
        let mut row = Flex::row()
            .with_main_axis_size(MainAxisSize::Min)
            .with_main_axis_alignment(MainAxisAlignment::End)
            .with_cross_axis_alignment(CrossAxisAlignment::Stretch);
        if let Some(toggle) = ccswitch_intercept {
            row.add_child(Self::render_ccswitch_intercept_toggle(
                mouse_states.ccswitch_intercept_toggle.clone(),
                toggle,
                ui_font,
            ));
        }
        row.add_children([
            WindowsTrafficLightIcon::Minimize.render(
                mouse_states.minimize_window_button.clone(),
                icon_font_family,
                actions.minimize,
            ),
            maximize_icon.render(
                mouse_states.maximize_window_button.clone(),
                icon_font_family,
                actions.toggle_maximize,
            ),
            WindowsTrafficLightIcon::Close.render(
                mouse_states.close_window_button.clone(),
                icon_font_family,
                actions.close,
            ),
        ]);
        let flex = row.finish();

        ConstrainedBox::new(flex)
            .with_height(CHROME_ROW_HEIGHT)
            .finish()
    }

    #[cfg(windows)]
    fn render_windows_with_rect_icons<A: Action + Copy + 'static>(
        &self,
        fullscreen_state: FullscreenState,
        mouse_states: &TrafficLightMouseStates,
        actions: TrafficLightActions<A>,
        ui_font: FamilyId,
        ccswitch_intercept: Option<CcswitchInterceptToggle<A>>,
    ) -> Box<dyn Element> {
        let fg_color = theme::text();
        let mut row = Flex::row();
        if let Some(toggle) = ccswitch_intercept {
            row.add_child(Self::render_ccswitch_intercept_toggle(
                mouse_states.ccswitch_intercept_toggle.clone(),
                toggle,
                ui_font,
            ));
        }
        row.add_children([
            Container::new(
                Self::windows_rect_button(
                    mouse_states.minimize_window_button.clone(),
                    Self::windows_minimize_icon(fg_color),
                    theme::panel_elevated(),
                    actions.minimize,
                )
                .finish(),
            )
            .finish(),
            Self::windows_rect_button(
                mouse_states.maximize_window_button.clone(),
                Self::windows_maximize_icon(fg_color, fullscreen_state),
                theme::panel_elevated(),
                actions.toggle_maximize,
            )
            .finish(),
            Container::new(
                Self::windows_close_button(
                    mouse_states.close_window_button.clone(),
                    ui_font,
                    actions.close,
                )
                .finish(),
            )
            .finish(),
        ]);
        ConstrainedBox::new(Align::new(row.finish()).finish())
            .with_max_height(CHROME_ROW_HEIGHT)
            .with_width(self.width)
            .finish()
    }

    #[cfg(windows)]
    fn render_ccswitch_intercept_toggle<A: Action + Copy + 'static>(
        mouse_state: MouseStateHandle,
        toggle: CcswitchInterceptToggle<A>,
        ui_font: FamilyId,
    ) -> Box<dyn Element> {
        let enabled = toggle.enabled;
        let action = toggle.toggle;
        let label = if enabled { "拦截 CC" } else { "转发 CC" };
        Hoverable::new(mouse_state, move |state| {
            let hovered = state.is_hovered();
            let (border, bg, color) = if enabled {
                (
                    theme::accent_cool(),
                    if hovered {
                        theme::accent_cool_bg(64)
                    } else {
                        theme::accent_cool_bg(40)
                    },
                    theme::accent_cool(),
                )
            } else if hovered {
                (
                    theme::border_bright(),
                    theme::panel_elevated(),
                    theme::text(),
                )
            } else {
                (
                    theme::border_bright(),
                    theme::panel(),
                    theme::muted(),
                )
            };
            Container::new(
                Align::new(
                    ui_text::cluster_ctrl(label, ui_font)
                        .with_color(color)
                        .finish(),
                )
                .finish(),
            )
            .with_vertical_padding(WINDOWS_BUTTON_PADDING_VERTICAL)
            .with_horizontal_padding(8.0)
            .with_background(bg)
            .with_border(Border::all(1.0).with_border_fill(border))
            .with_corner_radius(CornerRadius::with_all(Radius::Pixels(4.0)))
            .with_margin_right(4.0)
            .finish()
        })
        .on_click(move |ctx, _, _| {
            ctx.dispatch_typed_action(action);
        })
        .finish()
    }

    #[cfg(windows)]
    fn windows_minimize_icon(fg_color: ColorU) -> Box<dyn Element> {
        ConstrainedBox::new(Rect::new().with_background_color(fg_color).finish())
            .with_height(1.0)
            .with_width(12.0)
            .finish()
    }

    #[cfg(windows)]
    fn windows_maximize_icon(
        fg_color: ColorU,
        fullscreen_state: FullscreenState,
    ) -> Box<dyn Element> {
        let mut maximize_button_icon = ConstrainedBox::new(
            Rect::new()
                .with_border(Border::all(1.0).with_border_color(fg_color))
                .with_corner_radius(CornerRadius::with_all(Radius::Pixels(2.0)))
                .finish(),
        )
        .with_width(10.0)
        .with_height(10.0)
        .finish();

        if fullscreen_state != FullscreenState::Normal {
            let mut stack = Stack::new();
            stack.add_positioned_child(
                maximize_button_icon,
                OffsetPositioning::offset_from_parent(
                    vec2f(0.0, 0.0),
                    ParentOffsetBounds::Unbounded,
                    ParentAnchor::BottomLeft,
                    ChildAnchor::BottomLeft,
                ),
            );
            stack.add_positioned_child(
                ConstrainedBox::new(
                    Rect::new()
                        .with_border(
                            Border::new(1.0)
                                .with_sides(true, false, false, true)
                                .with_border_color(fg_color),
                        )
                        .with_corner_radius(CornerRadius::with_all(Radius::Pixels(2.0)))
                        .finish(),
                )
                .with_width(10.0)
                .with_height(10.0)
                .finish(),
                OffsetPositioning::offset_from_parent(
                    vec2f(0.0, 0.0),
                    ParentOffsetBounds::Unbounded,
                    ParentAnchor::TopRight,
                    ChildAnchor::TopRight,
                ),
            );
            maximize_button_icon = ConstrainedBox::new(stack.finish())
                .with_width(12.0)
                .with_height(12.0)
                .finish();
        }

        maximize_button_icon
    }

    #[cfg(windows)]
    fn windows_close_icon(icon_color: ColorU, font: FamilyId) -> Box<dyn Element> {
        ConstrainedBox::new(
            Align::new(Text::new("×", font, 16.0).with_color(icon_color).finish()).finish(),
        )
        .with_height(BUTTON_ICON_SIZE)
        .with_width(BUTTON_ICON_SIZE)
        .finish()
    }

    #[cfg(windows)]
    fn windows_close_button<A: Action + Copy + 'static>(
        mouse_state: MouseStateHandle,
        font: FamilyId,
        action: A,
    ) -> Hoverable {
        Hoverable::new(mouse_state, move |state| {
            let (background_color, icon_color) = if state.is_hovered() {
                (WINDOWS_BRIGHT_RED, ColorU::white())
            } else {
                (ColorU::transparent_black(), theme::text())
            };

            Container::new(
                ConstrainedBox::new(
                    Align::new(Self::windows_close_icon(icon_color, font)).finish(),
                )
                .with_width(BUTTON_ICON_SIZE)
                .with_height(BUTTON_ICON_SIZE)
                .finish(),
            )
            .with_vertical_padding(WINDOWS_BUTTON_PADDING_VERTICAL)
            .with_horizontal_padding(WINDOWS_BUTTON_PADDING_HORIZONTAL)
            .with_background_color(background_color)
            .finish()
        })
        .on_click(move |ctx, _, _| {
            ctx.dispatch_typed_action(action);
        })
    }

    #[cfg(windows)]
    fn windows_rect_button<A: Action + Copy + 'static>(
        mouse_state: MouseStateHandle,
        child: Box<dyn Element>,
        hover_color: ColorU,
        action: A,
    ) -> Hoverable {
        Hoverable::new(mouse_state, move |state| {
            let background_color = if state.is_hovered() {
                hover_color
            } else {
                ColorU::transparent_black()
            };
            Container::new(
                ConstrainedBox::new(Align::new(child).finish())
                    .with_width(BUTTON_ICON_SIZE)
                    .finish(),
            )
            .with_vertical_padding(WINDOWS_BUTTON_PADDING_VERTICAL)
            .with_horizontal_padding(WINDOWS_BUTTON_PADDING_HORIZONTAL)
            .with_background_color(background_color)
            .finish()
        })
        .on_click(move |ctx, _, _| {
            ctx.dispatch_typed_action(action);
        })
    }

    #[cfg(any(target_os = "linux", target_os = "freebsd"))]
    fn render_linux<A: Action + Copy + 'static>(
        &self,
        fullscreen_state: FullscreenState,
        mouse_states: &TrafficLightMouseStates,
        actions: TrafficLightActions<A>,
    ) -> Box<dyn Element> {
        let fg_color = theme::text();
        let maximize_button_icon = Self::linux_maximize_icon(fg_color, fullscreen_state);

        ConstrainedBox::new(
            Align::new(
                Flex::row()
                    .with_children([
                        Container::new(
                            Self::linux_circle_button(
                                Arc::clone(&mouse_states.minimize_window_button),
                                ConstrainedBox::new(
                                    Rect::new().with_background_color(fg_color).finish(),
                                )
                                .with_height(2.0)
                                .with_width(8.0)
                                .finish(),
                                actions.minimize,
                            )
                            .finish(),
                        )
                        .with_margin_right(16.0)
                        .finish(),
                        Self::linux_circle_button(
                            Arc::clone(&mouse_states.maximize_window_button),
                            maximize_button_icon,
                            actions.toggle_maximize,
                        )
                        .finish(),
                        Container::new(
                            Self::linux_circle_button(
                                Arc::clone(&mouse_states.close_window_button),
                                Self::linux_close_icon(fg_color),
                                actions.close,
                            )
                            .finish(),
                        )
                        .with_margin_left(16.0)
                        .with_margin_right(12.0)
                        .finish(),
                    ])
                    .finish(),
            )
            .finish(),
        )
        .with_max_height(CHROME_ROW_HEIGHT)
        .with_width(self.width)
        .finish()
    }

    #[cfg(any(target_os = "linux", target_os = "freebsd"))]
    fn linux_close_icon(fg_color: ColorU) -> Box<dyn Element> {
        ConstrainedBox::new(
            Rect::new()
                .with_border(Border::all(1.5).with_border_color(fg_color))
                .finish(),
        )
        .with_height(8.0)
        .with_width(8.0)
        .finish()
    }

    #[cfg(any(target_os = "linux", target_os = "freebsd"))]
    fn linux_maximize_icon(
        fg_color: ColorU,
        fullscreen_state: FullscreenState,
    ) -> Box<dyn Element> {
        let mut maximize_button_icon = ConstrainedBox::new(
            Rect::new()
                .with_border(Border::all(2.0).with_border_color(fg_color))
                .finish(),
        )
        .with_width(6.0)
        .with_height(6.0)
        .finish();

        if fullscreen_state != FullscreenState::Normal {
            let mut stack = Stack::new();
            stack.add_positioned_child(
                maximize_button_icon,
                OffsetPositioning::offset_from_parent(
                    vec2f(0.0, 0.0),
                    ParentOffsetBounds::Unbounded,
                    ParentAnchor::BottomLeft,
                    ChildAnchor::BottomLeft,
                ),
            );
            stack.add_positioned_child(
                ConstrainedBox::new(
                    Rect::new()
                        .with_border(
                            Border::new(1.0)
                                .with_sides(true, false, false, true)
                                .with_border_color(fg_color),
                        )
                        .with_corner_radius(CornerRadius::with_all(Radius::Pixels(1.0)))
                        .finish(),
                )
                .with_width(6.0)
                .with_height(6.0)
                .finish(),
                OffsetPositioning::offset_from_parent(
                    vec2f(0.0, 0.0),
                    ParentOffsetBounds::Unbounded,
                    ParentAnchor::TopRight,
                    ChildAnchor::TopRight,
                ),
            );
            maximize_button_icon = ConstrainedBox::new(stack.finish())
                .with_width(8.0)
                .with_height(8.0)
                .finish();
        }

        maximize_button_icon
    }

    #[cfg(any(target_os = "linux", target_os = "freebsd"))]
    fn linux_circle_button<A: Action + Copy + 'static>(
        mouse_state: MouseStateHandle,
        child: Box<dyn Element>,
        action: A,
    ) -> Hoverable {
        Hoverable::new(mouse_state, |state| {
            let background_color = if state.is_hovered() {
                theme::panel_elevated()
            } else {
                theme::panel()
            };
            Container::new(
                ConstrainedBox::new(Align::new(child).finish())
                    .with_width(BUTTON_ICON_SIZE)
                    .with_height(BUTTON_ICON_SIZE)
                    .finish(),
            )
            .with_background(background_color)
            .with_corner_radius(CornerRadius::with_all(Radius::Percentage(50.0)))
            .finish()
        })
        .on_click(move |ctx, _, _| {
            ctx.dispatch_typed_action(action);
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reserve_space_only_for_right_side_traffic_lights() {
        assert!(!should_reserve_traffic_light_space_in_tab_bar(
            TrafficLightSide::Left
        ));
        assert!(should_reserve_traffic_light_space_in_tab_bar(
            TrafficLightSide::Right
        ));
    }

    #[test]
    fn macos_left_padding_includes_traffic_light_width() {
        let data = TrafficLightData {
            width: 64.0,
            side: TrafficLightSide::Left,
            scales_with_zoom: false,
        };
        assert_eq!(tab_bar_left_padding(Some(&data), 1.0, false), 80.0);
    }

    #[test]
    fn macos_fullscreen_left_padding_falls_back() {
        let data = TrafficLightData {
            width: 64.0,
            side: TrafficLightSide::Left,
            scales_with_zoom: false,
        };
        if cfg!(target_os = "macos") {
            assert_eq!(tab_bar_left_padding(Some(&data), 1.0, true), 16.0);
        } else {
            assert_eq!(tab_bar_left_padding(Some(&data), 1.0, true), 80.0);
        }
    }

    #[test]
    fn traffic_light_width_scales_with_zoom_when_enabled() {
        let data = TrafficLightData {
            width: 136.0,
            side: TrafficLightSide::Right,
            scales_with_zoom: true,
        };
        assert_eq!(data.width(2.0), 136.0);

        let mac_data = TrafficLightData {
            width: 64.0,
            side: TrafficLightSide::Left,
            scales_with_zoom: false,
        };
        assert_eq!(mac_data.width(2.0), 32.0);
    }
}
