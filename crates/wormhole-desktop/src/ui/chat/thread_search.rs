use warpui::accessibility::{AccessibilityContent, ActionAccessibilityContent, WarpA11yRole};
use warpui::elements::{
    Align, Border, ConstrainedBox, Container, CrossAxisAlignment, DispatchEventResult,
    EventHandler, Expanded, Flex, MainAxisSize, ParentElement,
};
use warpui::fonts::FamilyId;
use warpui::{AccessibilityData, AppContext, Element, Entity, TypedActionView, View, ViewContext};

use crate::ui::chat::shell_state::SharedChatShellState;
use crate::ui::icons;
use crate::ui::panel_primitives::chat_search_pill;
use crate::ui::text_field_input::{
    render_field_with_caret, sync_caret_blink, wrap_text_field_focus_on_click, CaretBlink,
    CaretBlinkHost, TextFieldEditAction, TextFieldInput, TextFieldState,
};
use crate::ui::theme;
use crate::ui_text;

const TG_THREAD_SEARCH_CLOSE: f32 = 32.0;

#[derive(Debug, Clone)]
pub enum ChatThreadSearchAction {
    QueryEdit(TextFieldEditAction),
    FocusQuery,
    ActivateQuery,
    Close,
}

pub struct ChatThreadSearchView {
    shell_state: SharedChatShellState,
    font: FamilyId,
    query: String,
    field_state: TextFieldState,
    focused: bool,
    caret_blink: CaretBlink,
}

impl ChatThreadSearchView {
    pub fn new(ctx: &mut ViewContext<Self>, shell_state: SharedChatShellState) -> Self {
        let font = crate::ui::fonts::load_ui_font(ctx);
        Self {
            shell_state,
            font,
            query: String::new(),
            field_state: TextFieldState::new(),
            focused: false,
            caret_blink: CaretBlink::new(),
        }
    }
}

impl Entity for ChatThreadSearchView {
    type Event = ();
}

impl View for ChatThreadSearchView {
    fn ui_name() -> &'static str {
        "ChatThreadSearchView"
    }

    fn render(&self, _app: &AppContext) -> Box<dyn Element> {
        let open = self
            .shell_state
            .lock()
            .map(|state| state.thread_search_open)
            .unwrap_or(false);
        if !open {
            return Flex::row().finish();
        }

        let field = render_field_with_caret(
            &self.query,
            &self.field_state.marked_text,
            "搜索此对话…",
            self.font,
            self.focused,
            false,
            self.caret_blink.visible,
        );
        let input = TextFieldInput::builder(field, |ctx, action| {
            ctx.dispatch_typed_action(ChatThreadSearchAction::QueryEdit(action));
        })
        .focused(self.focused)
        .ime_preedit(!self.field_state.marked_text.is_empty())
        .finish();
        let input = wrap_text_field_focus_on_click(input, |ctx| {
            ctx.dispatch_typed_action(ChatThreadSearchAction::ActivateQuery);
        });

        let row = Flex::row()
            .with_main_axis_size(MainAxisSize::Max)
            .with_cross_axis_alignment(CrossAxisAlignment::Center)
            .with_child(
                Container::new(icons::chat_sidebar_search_icon(theme::muted()))
                    .with_horizontal_margin(2.0)
                    .finish(),
            )
            .with_child(Expanded::new(1.0, input).finish())
            .finish();

        let wrap = chat_search_pill(row, theme::canvas(), theme::border(), 6.0, 10.0, 999.0);

        let close_btn = EventHandler::new(
            ConstrainedBox::new(
                Align::new(
                    ui_text::body("×", self.font)
                        .with_color(theme::muted())
                        .finish(),
                )
                .finish(),
            )
            .with_width(TG_THREAD_SEARCH_CLOSE)
            .with_height(TG_THREAD_SEARCH_CLOSE)
            .finish(),
        )
        .on_left_mouse_down(|ctx, _, _| {
            ctx.dispatch_typed_action(ChatThreadSearchAction::Close);
            DispatchEventResult::StopPropagation
        })
        .finish();

        Container::new(
            Flex::row()
                .with_cross_axis_alignment(CrossAxisAlignment::Center)
                .with_main_axis_size(MainAxisSize::Max)
                .with_child(Expanded::new(1.0, wrap).finish())
                .with_child(close_btn)
                .finish(),
        )
        .with_padding_left(12.0)
        .with_padding_right(12.0)
        .with_padding_top(6.0)
        .with_padding_bottom(6.0)
        .with_background(theme::panel())
        .with_border(Border::bottom(1.0).with_border_fill(theme::border()))
        .finish()
    }
}

impl TypedActionView for ChatThreadSearchView {
    type Action = ChatThreadSearchAction;

    fn handle_action(&mut self, action: &ChatThreadSearchAction, ctx: &mut ViewContext<Self>) {
        match action {
            ChatThreadSearchAction::QueryEdit(edit) => {
                self.field_state.apply(&mut self.query, edit);
                if let Ok(mut state) = self.shell_state.lock() {
                    state.thread_search_query = self.query.clone();
                }
                self.focused = true;
                sync_caret_blink(self, ctx);
                ctx.notify();
            }
            ChatThreadSearchAction::FocusQuery | ChatThreadSearchAction::ActivateQuery => {
                self.focused = true;
                sync_caret_blink(self, ctx);
                ctx.notify();
            }
            ChatThreadSearchAction::Close => {
                if let Ok(mut state) = self.shell_state.lock() {
                    state.close_thread_search();
                }
                self.query.clear();
                self.field_state.clear_marked();
                self.focused = false;
                sync_caret_blink(self, ctx);
                ctx.notify();
            }
        }
    }

    fn action_accessibility_contents(
        &mut self,
        action: &ChatThreadSearchAction,
        _ctx: &mut ViewContext<Self>,
    ) -> ActionAccessibilityContent {
        let content = match action {
            ChatThreadSearchAction::Close => {
                AccessibilityContent::new_without_help("关闭会话搜索", WarpA11yRole::ButtonRole)
            }
            _ => AccessibilityContent::new_without_help("搜索此对话", WarpA11yRole::TextfieldRole),
        };
        ActionAccessibilityContent::Custom(content)
    }
}

impl CaretBlinkHost for ChatThreadSearchView {
    fn caret_blink(&mut self) -> &mut CaretBlink {
        &mut self.caret_blink
    }

    fn caret_input_focused(&self) -> bool {
        self.focused
    }
}
