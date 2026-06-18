//! Embedded-mode agent switcher shown above the Warp tab bar.

use pathfinder_color::ColorU;
use warp_core::ui::theme::color::internal_colors;
use warpui::elements::{
    Container, CrossAxisAlignment, DispatchEventResult, EventHandler, Flex, MainAxisSize,
    ParentElement, Text,
};
use warpui::fonts::FamilyId;
use warpui::Element;
use wormhole_embed::{self, PreferredAgent};

use crate::appearance::Appearance;
use crate::workspace::WorkspaceAction;

const TOOLBAR_HEIGHT: f32 = 36.0;

pub fn render_agent_switcher(
    font: FamilyId,
    selected: PreferredAgent,
    appearance: &Appearance,
) -> Box<dyn Element> {
    let theme = appearance.theme();
    let mut row = Flex::row()
        .with_cross_axis_alignment(CrossAxisAlignment::Center)
        .with_main_axis_size(MainAxisSize::Min);

    for (agent, label) in [
        (PreferredAgent::Codex, "Codex"),
        (PreferredAgent::Cursor, "Cursor"),
    ] {
        let is_selected = selected == agent;
        let bg = if is_selected {
            internal_colors::fg_overlay_2(theme)
        } else {
            ColorU::new(0, 0, 0, 0)
        };
        let fg = if is_selected {
            theme.accent()
        } else {
            theme.foreground()
        };
        let label_el = Text::new(label, font, 13.).with_color(fg).finish();
        let button = Container::new(
            EventHandler::new(label_el)
                .on_left_mouse_down(move |ctx, _, _| {
                    ctx.dispatch_typed_action(WorkspaceAction::WormholeEmbedSelectAgent(agent));
                    DispatchEventResult::StopPropagation
                })
                .finish(),
        )
        .with_uniform_padding(8.0)
        .with_background(bg)
        .finish();
        row.add_child(button);
    }

    let hint = Text::new("切换将在新终端 tab 启动对应 CLI", font, 12.)
        .with_color(internal_colors::fg_overlay_3(theme))
        .finish();

    let body = Flex::row()
        .with_cross_axis_alignment(CrossAxisAlignment::Center)
        .with_child(row.finish())
        .with_child(Container::new(hint).with_left_padding(12.0).finish())
        .finish();

    Container::new(body)
        .with_height(TOOLBAR_HEIGHT)
        .with_left_padding(12.0)
        .with_right_padding(12.0)
        .with_background(theme.background())
        .finish()
}

pub fn current_preferred_agent() -> PreferredAgent {
    wormhole_embed::preferred_agent()
}

pub fn persist_preferred_agent(agent: PreferredAgent) {
    if let Some(data_dir) = wormhole_embed::data_dir() {
        let _ = wormhole_embed::prefs::set_preferred_agent(&data_dir, agent);
    }
}

pub fn launch_command_for(agent: PreferredAgent) -> String {
    match agent {
        PreferredAgent::Codex => wormhole_embed::default_codex_launch_command(),
        PreferredAgent::Cursor => wormhole_embed::default_cursor_launch_command(),
    }
}
