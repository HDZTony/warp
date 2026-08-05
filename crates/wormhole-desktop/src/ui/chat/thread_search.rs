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
    PreviousResult,
    NextResult,
    ToggleCalendar,
    PreviousMonth,
    NextMonth,
    SelectDate { year: i32, month: u32, day: u32 },
    Close,
}

pub struct ChatThreadSearchView {
    shell_state: SharedChatShellState,
    font: FamilyId,
    query: String,
    field_state: TextFieldState,
    focused: bool,
    caret_blink: CaretBlink,
    calendar_open: bool,
    calendar_month: NaiveDate,
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
            calendar_open: false,
            calendar_month: Local::now()
                .date_naive()
                .with_day(1)
                .expect("first day of month"),
        }
    }

    fn action_button(&self, label: String, action: ChatThreadSearchAction) -> Box<dyn Element> {
        let automation_id = format!("chat:thread_search:{label}");
        EventHandler::new(
            ConstrainedBox::new(
                Align::new(
                    ui_text::body(label.clone(), self.font)
                        .with_color(theme::muted())
                        .finish(),
                )
                .finish(),
            )
            .with_width(TG_THREAD_SEARCH_CLOSE)
            .with_height(TG_THREAD_SEARCH_CLOSE)
            .finish(),
        )
        .with_automation_label(label)
        .with_automation_id(automation_id)
        .on_left_mouse_down(move |ctx, _, _| {
            ctx.dispatch_typed_action(action.clone());
            DispatchEventResult::StopPropagation
        })
        .finish()
    }

    fn calendar(&self) -> Box<dyn Element> {
        let month = self.calendar_month;
        let next_month = if month.month() == 12 {
            NaiveDate::from_ymd_opt(month.year() + 1, 1, 1)
        } else {
            NaiveDate::from_ymd_opt(month.year(), month.month() + 1, 1)
        }
        .expect("next month");
        let days = (next_month - Duration::days(1)).day();
        let leading = month.weekday().num_days_from_monday();
        let mut calendar = Flex::column().with_main_axis_size(MainAxisSize::Min);
        calendar.add_child(
            Flex::row()
                .with_cross_axis_alignment(CrossAxisAlignment::Center)
                .with_child(self.action_button("<".into(), ChatThreadSearchAction::PreviousMonth))
                .with_child(
                    Expanded::new(
                        1.0,
                        Align::new(
                            ui_text::body(
                                format!("{} 年 {} 月", month.year(), month.month()),
                                self.font,
                            )
                            .with_color(theme::text())
                            .finish(),
                        )
                        .finish(),
                    )
                    .finish(),
                )
                .with_child(self.action_button(">".into(), ChatThreadSearchAction::NextMonth))
                .finish(),
        );
        for week in 0..6 {
            let mut row = Flex::row().with_main_axis_size(MainAxisSize::Max);
            for weekday in 0..7 {
                let slot = week * 7 + weekday;
                let day = slot as i32 - leading as i32 + 1;
                if day < 1 || day > days as i32 {
                    row.add_child(Expanded::new(1.0, Flex::row().finish()).finish());
                } else {
                    row.add_child(
                        Expanded::new(
                            1.0,
                            self.action_button(
                                day.to_string(),
                                ChatThreadSearchAction::SelectDate {
                                    year: month.year(),
                                    month: month.month(),
                                    day: day as u32,
                                },
                            ),
                        )
                        .finish(),
                    );
                }
            }
            calendar.add_child(row.finish());
        }
        Container::new(calendar.finish())
            .with_padding_left(8.0)
            .with_padding_right(8.0)
            .with_padding_bottom(8.0)
            .with_background(theme::panel())
            .finish()
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
        let (open, current, total, loading) = self
            .shell_state
            .lock()
            .map(|state| {
                (
                    state.thread_search_open,
                    state.thread_search_current,
                    state.thread_search_total,
                    state.thread_search_loading,
                )
            })
            .unwrap_or((false, 0, 0, false));
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
            self.field_state.cursor,
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
        .with_automation_label("关闭搜索")
        .with_automation_id("chat:thread_search_close")
        .on_left_mouse_down(|ctx, _, _| {
            ctx.dispatch_typed_action(ChatThreadSearchAction::Close);
            DispatchEventResult::StopPropagation
        })
        .finish();

        let count = if loading {
            "…".into()
        } else if total == 0 {
            "0/0".into()
        } else {
            format!("{}/{}", current.saturating_add(1).min(total), total)
        };
        let toolbar = Container::new(
            Flex::row()
                .with_cross_axis_alignment(CrossAxisAlignment::Center)
                .with_main_axis_size(MainAxisSize::Max)
                .with_child(Expanded::new(1.0, wrap).finish())
                .with_child(
                    self.action_button("日期".into(), ChatThreadSearchAction::ToggleCalendar),
                )
                .with_child(
                    Container::new(
                        ui_text::body(count, self.font)
                            .with_color(theme::muted())
                            .finish(),
                    )
                    .with_horizontal_padding(6.0)
                    .finish(),
                )
                .with_child(self.action_button("↑".into(), ChatThreadSearchAction::PreviousResult))
                .with_child(self.action_button("↓".into(), ChatThreadSearchAction::NextResult))
                .with_child(close_btn)
                .finish(),
        )
        .with_padding_left(12.0)
        .with_padding_right(12.0)
        .with_padding_top(6.0)
        .with_padding_bottom(6.0)
        .with_background(theme::panel())
        .with_border(Border::bottom(1.0).with_border_fill(theme::border()))
        .finish();

        let mut root = Flex::column().with_main_axis_size(MainAxisSize::Min);
        root.add_child(toolbar);
        if self.calendar_open {
            root.add_child(self.calendar());
        }
        root.finish()
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
                    state.request_thread_search();
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
            ChatThreadSearchAction::PreviousResult => {
                if let Ok(mut state) = self.shell_state.lock() {
                    state.navigate_thread_search(-1);
                }
                ctx.notify();
            }
            ChatThreadSearchAction::NextResult => {
                if let Ok(mut state) = self.shell_state.lock() {
                    state.navigate_thread_search(1);
                }
                ctx.notify();
            }
            ChatThreadSearchAction::ToggleCalendar => {
                self.calendar_open = !self.calendar_open;
                ctx.notify();
            }
            ChatThreadSearchAction::PreviousMonth => {
                self.calendar_month = if self.calendar_month.month() == 1 {
                    NaiveDate::from_ymd_opt(self.calendar_month.year() - 1, 12, 1)
                } else {
                    NaiveDate::from_ymd_opt(
                        self.calendar_month.year(),
                        self.calendar_month.month() - 1,
                        1,
                    )
                }
                .expect("previous month");
                ctx.notify();
            }
            ChatThreadSearchAction::NextMonth => {
                self.calendar_month = if self.calendar_month.month() == 12 {
                    NaiveDate::from_ymd_opt(self.calendar_month.year() + 1, 1, 1)
                } else {
                    NaiveDate::from_ymd_opt(
                        self.calendar_month.year(),
                        self.calendar_month.month() + 1,
                        1,
                    )
                }
                .expect("next month");
                ctx.notify();
            }
            ChatThreadSearchAction::SelectDate { year, month, day } => {
                if let Some(date) = NaiveDate::from_ymd_opt(*year, *month, *day) {
                    let start = Local
                        .from_local_datetime(&date.and_hms_opt(0, 0, 0).expect("midnight"))
                        .earliest();
                    let next = date + Duration::days(1);
                    let end = Local
                        .from_local_datetime(&next.and_hms_opt(0, 0, 0).expect("midnight"))
                        .latest();
                    if let (Some(start), Some(end)) = (start, end) {
                        if let Ok(mut state) = self.shell_state.lock() {
                            state.thread_search_from_ms = Some(start.timestamp_millis() as u64);
                            state.thread_search_to_ms = Some(end.timestamp_millis() as u64);
                            state.request_thread_search();
                        }
                    }
                }
                self.calendar_open = false;
                ctx.notify();
            }
            ChatThreadSearchAction::Close => {
                if let Ok(mut state) = self.shell_state.lock() {
                    state.close_thread_search();
                }
                self.query.clear();
                self.field_state.clear_marked();
                self.focused = false;
                self.calendar_open = false;
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
            ChatThreadSearchAction::PreviousResult | ChatThreadSearchAction::NextResult => {
                AccessibilityContent::new_without_help("浏览搜索结果", WarpA11yRole::ButtonRole)
            }
            ChatThreadSearchAction::ToggleCalendar
            | ChatThreadSearchAction::PreviousMonth
            | ChatThreadSearchAction::NextMonth
            | ChatThreadSearchAction::SelectDate { .. } => {
                AccessibilityContent::new_without_help("按日期搜索", WarpA11yRole::ButtonRole)
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
use chrono::{Datelike, Duration, Local, NaiveDate, TimeZone};
