use pathfinder_color::ColorU;
use warpui::elements::{
    Align, Border, ConstrainedBox, Container, CornerRadius, CrossAxisAlignment, DispatchEventResult,
    EventHandler, Expanded, Flex, MainAxisAlignment, MainAxisSize, ParentElement, Radius,
};
use warpui::fonts::FamilyId;
use warpui::keymap::Keystroke;
use warpui::{AppContext, Element, Entity, TypedActionView, UpdateView, View, ViewContext};

use crate::ui::core_handle::CoreHandle;
use crate::ui::panel_primitives::{status_line, StatusTone};
use crate::ui::text_field_input::{
    render_field_with_caret, sync_caret_blink, wrap_text_field_focus_on_click, CaretBlink,
    CaretBlinkHost, TextFieldEditAction, TextFieldInput, TextFieldState,
};
use crate::ui::theme;
use crate::ui_text;
use wormhole_desktop_core::device_identity::device_bootstrap;
use wormhole_desktop_core::{
    cloud_auth_status, supabase_password_login, SupabasePasswordLoginParams,
};

#[derive(Debug, Clone)]
pub enum LoginModalAction {
    Open,
    Close,
    Submit,
    EmailEdit(TextFieldEditAction),
    PasswordEdit(TextFieldEditAction),
    FocusEmail,
    FocusPassword,
}

#[derive(Debug, Clone)]
pub enum LoginModalEvent {
    AuthChanged {
        authenticated: bool,
        user_id: Option<String>,
    },
    Dismissed,
}

pub struct LoginModalView {
    core: CoreHandle,
    font: FamilyId,
    open: bool,
    email: String,
    password: String,
    email_field: TextFieldState,
    password_field: TextFieldState,
    email_focused: bool,
    password_focused: bool,
    caret_blink: CaretBlink,
    busy: bool,
    status: String,
    status_tone: StatusTone,
}

impl LoginModalView {
    pub fn new(ctx: &mut ViewContext<Self>, core: CoreHandle) -> Self {
        Self {
            core,
            font: crate::ui::fonts::load_ui_font(ctx),
            open: false,
            email: String::new(),
            password: String::new(),
            email_field: TextFieldState::new(),
            password_field: TextFieldState::new(),
            email_focused: true,
            password_focused: false,
            caret_blink: CaretBlink::new(),
            busy: false,
            status: String::new(),
            status_tone: StatusTone::Placeholder,
        }
    }

    pub fn is_open(&self) -> bool {
        self.open
    }

    pub fn open(&mut self, ctx: &mut ViewContext<Self>) {
        self.open = true;
        self.status.clear();
        self.email_focused = true;
        self.password_focused = false;
        sync_caret_blink(self, ctx);
        ctx.notify();
    }

    pub fn close(&mut self, ctx: &mut ViewContext<Self>) {
        self.open = false;
        self.busy = false;
        self.email_focused = false;
        self.password_focused = false;
        sync_caret_blink(self, ctx);
        ctx.emit(LoginModalEvent::Dismissed);
        ctx.notify();
    }

    fn any_field_focused(&self) -> bool {
        self.email_focused || self.password_focused
    }

    fn sync_caret(&mut self, ctx: &mut ViewContext<Self>) {
        sync_caret_blink(self, ctx);
    }

    fn consumes_shell_navigation(keystroke: &Keystroke) -> bool {
        if keystroke.alt || keystroke.meta {
            return false;
        }
        if keystroke.ctrl {
            return matches!(keystroke.key.as_str(), "1" | "2" | "3" | "4" | "5");
        }
        matches!(
            keystroke.key.as_str(),
            "left" | "right" | "home" | "end"
        )
    }

    fn field_keydown(
        ctx: &mut warpui::elements::EventContext,
        keystroke: &Keystroke,
        tab_action: LoginModalAction,
    ) -> DispatchEventResult {
        if Self::consumes_shell_navigation(keystroke) {
            return DispatchEventResult::StopPropagation;
        }
        match keystroke.key.as_str() {
            "tab" => {
                ctx.dispatch_typed_action(tab_action);
                DispatchEventResult::StopPropagation
            }
            "escape" => {
                ctx.dispatch_typed_action(LoginModalAction::Close);
                DispatchEventResult::StopPropagation
            }
            "enter" | "return" => {
                ctx.dispatch_typed_action(LoginModalAction::Submit);
                DispatchEventResult::StopPropagation
            }
            _ => DispatchEventResult::PropagateToParent,
        }
    }

    fn field_block(
        &self,
        label: &str,
        draft: &str,
        marked: &str,
        placeholder: &str,
        focused: bool,
        on_edit: LoginModalAction,
        click_focus: LoginModalAction,
        tab_focus: LoginModalAction,
    ) -> Box<dyn Element> {
        let mut col = Flex::column().with_cross_axis_alignment(CrossAxisAlignment::Stretch);
        col.add_child(
            ui_text::hud_title(label.to_string(), self.font)
                .with_color(theme::muted())
                .finish(),
        );
        let field = render_field_with_caret(
            draft,
            marked,
            placeholder,
            self.font,
            focused,
            false,
            self.caret_blink.visible,
        );
        let edit_action = on_edit.clone();
        let tab_action = tab_focus.clone();
        let click_action = click_focus.clone();
        col.add_child(
            Container::new(
                wrap_text_field_focus_on_click(
                    TextFieldInput::builder(field, move |ctx, action| {
                        ctx.dispatch_typed_action(match &edit_action {
                            LoginModalAction::EmailEdit(_) => LoginModalAction::EmailEdit(action),
                            LoginModalAction::PasswordEdit(_) => {
                                LoginModalAction::PasswordEdit(action)
                            }
                            _ => LoginModalAction::EmailEdit(action),
                        });
                    })
                    .focused(focused)
                    .on_keydown(move |ctx, keystroke| {
                        Self::field_keydown(ctx, keystroke, tab_action.clone())
                    })
                    .finish(),
                    move |ctx| ctx.dispatch_typed_action(click_action.clone()),
                ),
            )
            .with_uniform_padding(10.0)
            .with_vertical_margin(6.0)
            .with_background(theme::canvas())
            .with_border(Border::all(1.0).with_border_fill(theme::border()))
            .with_corner_radius(CornerRadius::with_all(Radius::Pixels(4.0)))
            .finish(),
        );
        col.finish()
    }

    fn action_button(&self, label: &str, action: LoginModalAction, primary: bool) -> Box<dyn Element> {
        let text_color = if primary {
            theme::accent()
        } else {
            theme::muted()
        };
        let bg = if primary {
            theme::accent_bg(24)
        } else {
            ColorU::transparent_black()
        };
        let border = if primary {
            theme::accent()
        } else {
            theme::border()
        };
        Container::new(
            EventHandler::new(
                ui_text::body(label.to_string(), self.font)
                    .with_color(text_color)
                    .finish(),
            )
            .on_left_mouse_down(move |ctx, _, _| {
                ctx.dispatch_typed_action(action.clone());
                DispatchEventResult::StopPropagation
            })
            .finish(),
        )
        .with_uniform_padding(10.0)
        .with_background(bg)
        .with_border(Border::all(1.0).with_border_fill(border))
        .with_corner_radius(CornerRadius::with_all(Radius::Pixels(4.0)))
        .finish()
    }

    fn dialog(&self) -> Box<dyn Element> {
        let mut col = Flex::column().with_cross_axis_alignment(CrossAxisAlignment::Stretch);
        col.add_child(
            ui_text::title("登录 Wormhole", self.font)
                .with_color(theme::text())
                .finish(),
        );
        col.add_child(
            Container::new(
                ui_text::body(
                    "登录后绑定本机设备身份，可使用 P2P 集群、聊天与同步。",
                    self.font,
                )
                .with_color(theme::muted())
                .finish(),
            )
            .with_vertical_margin(8.0)
            .finish(),
        );
        col.add_child(self.field_block(
            "邮箱",
            &self.email,
            &self.email_field.marked_text,
            "name@example.com",
            self.email_focused,
            LoginModalAction::EmailEdit(TextFieldEditAction::TypedCharacters(String::new())),
            LoginModalAction::FocusEmail,
            LoginModalAction::FocusPassword,
        ));
        col.add_child(self.field_block(
            "密码",
            &self.password,
            &self.password_field.marked_text,
            "••••••••",
            self.password_focused,
            LoginModalAction::PasswordEdit(TextFieldEditAction::TypedCharacters(String::new())),
            LoginModalAction::FocusPassword,
            LoginModalAction::FocusEmail,
        ));

        if !self.status.is_empty() {
            col.add_child(status_line(
                self.status.clone(),
                self.font,
                self.status_tone,
            ));
        }

        let mut actions = Flex::row()
            .with_cross_axis_alignment(CrossAxisAlignment::Center)
            .with_main_axis_alignment(MainAxisAlignment::End)
            .with_main_axis_size(MainAxisSize::Max);
        actions.add_child(self.action_button("取消", LoginModalAction::Close, false));
        actions.add_child(
            Container::new(Flex::column().finish())
                .with_horizontal_margin(8.0)
                .finish(),
        );
        actions.add_child(self.action_button(
            if self.busy { "登录中…" } else { "登录" },
            LoginModalAction::Submit,
            true,
        ));
        col.add_child(
            Container::new(actions.finish())
                .with_vertical_margin(14.0)
                .finish(),
        );

        let panel = EventHandler::new(
            Container::new(
                ConstrainedBox::new(col.finish())
                    .with_width(400.0)
                    .finish(),
            )
            .with_uniform_padding(24.0)
            .with_background(theme::panel())
            .with_border(Border::all(1.0).with_border_fill(theme::border_bright()))
            .with_corner_radius(CornerRadius::with_all(Radius::Pixels(10.0)))
            .finish(),
        )
        .with_always_handle()
        .on_keydown(|_, _, keystroke| {
            if Self::consumes_shell_navigation(keystroke) {
                return DispatchEventResult::StopPropagation;
            }
            DispatchEventResult::PropagateToParent
        })
        .on_left_mouse_down(|_, _, _| DispatchEventResult::StopPropagation)
        .finish();

        Container::new(Align::new(panel).finish())
            .with_uniform_padding(24.0)
            .finish()
    }

    fn submit(&mut self, ctx: &mut ViewContext<Self>) {
        let email = self.email.trim().to_string();
        let password = self.password.clone();
        if email.is_empty() || password.is_empty() {
            self.status = "请输入邮箱和密码。".into();
            self.status_tone = StatusTone::Danger;
            ctx.notify();
            return;
        }
        self.busy = true;
        self.status = "正在登录…".into();
        self.status_tone = StatusTone::Placeholder;
        ctx.notify();
        let core = self.core.clone();
        ctx.spawn(
            async move {
                let state = core.runtime().state.clone();
                supabase_password_login(
                    &state,
                    SupabasePasswordLoginParams { email, password },
                )
                .await?;
                device_bootstrap(&state).await?;
                cloud_auth_status(&state).await
            },
            |view, output, ctx| {
                view.busy = false;
                match output {
                    Ok(status) => {
                        view.open = false;
                        view.status.clear();
                        ctx.emit(LoginModalEvent::AuthChanged {
                            authenticated: status.authenticated,
                            user_id: status.user_id,
                        });
                    }
                    Err(err) => {
                        view.status = err;
                        view.status_tone = StatusTone::Danger;
                    }
                }
                ctx.notify();
            },
        );
    }
}

impl Entity for LoginModalView {
    type Event = LoginModalEvent;
}

impl View for LoginModalView {
    fn ui_name() -> &'static str {
        "LoginModalView"
    }

    fn render(&self, _app: &AppContext) -> Box<dyn Element> {
        if !self.open {
            return Flex::column().finish();
        }
        let scrim = Container::new(self.dialog())
            .with_background(ColorU::new(8, 7, 11, 180))
            .finish();
        Expanded::new(
            1.0,
            EventHandler::new(scrim)
                .on_left_mouse_down(|ctx, _, _| {
                    ctx.dispatch_typed_action(LoginModalAction::Close);
                    DispatchEventResult::StopPropagation
                })
                .finish(),
        )
        .finish()
    }
}

impl TypedActionView for LoginModalView {
    type Action = LoginModalAction;

    fn handle_action(&mut self, action: &LoginModalAction, ctx: &mut ViewContext<Self>) {
        match action {
            LoginModalAction::Open => self.open(ctx),
            LoginModalAction::Close => self.close(ctx),
            LoginModalAction::Submit => self.submit(ctx),
            LoginModalAction::FocusEmail => {
                self.email_focused = true;
                self.password_focused = false;
                self.sync_caret(ctx);
                ctx.notify();
            }
            LoginModalAction::FocusPassword => {
                self.email_focused = false;
                self.password_focused = true;
                self.sync_caret(ctx);
                ctx.notify();
            }
            LoginModalAction::EmailEdit(edit) => {
                self.email_field.apply(&mut self.email, edit);
                self.email_focused = true;
                self.password_focused = false;
                self.sync_caret(ctx);
                ctx.notify();
            }
            LoginModalAction::PasswordEdit(edit) => {
                self.password_field.apply(&mut self.password, edit);
                self.email_focused = false;
                self.password_focused = true;
                self.sync_caret(ctx);
                ctx.notify();
            }
        }
    }
}

impl CaretBlinkHost for LoginModalView {
    fn caret_blink(&mut self) -> &mut CaretBlink {
        &mut self.caret_blink
    }

    fn caret_input_focused(&self) -> bool {
        self.open && self.any_field_focused()
    }
}
