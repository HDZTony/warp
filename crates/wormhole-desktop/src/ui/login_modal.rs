use pathfinder_color::ColorU;
use warpui::elements::{
    Align, Border, ConstrainedBox, Container, CornerRadius, CrossAxisAlignment,
    DispatchEventResult, EventHandler, Expanded, Flex, MainAxisAlignment, MainAxisSize,
    ParentElement, Radius,
};
use warpui::fonts::FamilyId;
use warpui::keymap::Keystroke;
use warpui::{AppContext, Element, Entity, TypedActionView, View, ViewContext};

use crate::ui::core_handle::CoreHandle;
use crate::ui::desktop_prefs;
use crate::ui::panel_primitives::{status_line, StatusTone};
use crate::ui::text_field_input::{
    render_field_with_caret, sync_caret_blink, wrap_text_field_focus_on_click, CaretBlink,
    CaretBlinkHost, TextFieldEditAction, TextFieldInput, TextFieldState,
};
use crate::ui::theme;
use crate::ui_text;
use wormhole_desktop_core::device_identity::device_bootstrap;
use wormhole_desktop_core::{
    cloud_auth_status, supabase_password_login, supabase_resend_signup, supabase_signup,
    CloudAuthStatusDto, SupabasePasswordLoginParams, SupabaseResendSignupParams,
    SupabaseSignupParams,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum AuthMode {
    Login,
    Register,
}

#[derive(Debug, Clone)]
pub enum LoginModalAction {
    Open,
    Close,
    Submit,
    ResendConfirmation,
    ToggleMode,
    ToggleRemember,
    ToggleAutoLogin,
    EmailEdit(TextFieldEditAction),
    PasswordEdit(TextFieldEditAction),
    ConfirmPasswordEdit(TextFieldEditAction),
    FocusEmail,
    FocusPassword,
    FocusConfirmPassword,
}

#[derive(Debug, Clone)]
pub enum LoginModalEvent {
    AuthChanged {
        authenticated: bool,
        device_id: Option<String>,
    },
    OpenChanged {
        open: bool,
    },
}

pub struct LoginModalView {
    core: CoreHandle,
    font: FamilyId,
    open: bool,
    mode: AuthMode,
    email: String,
    password: String,
    confirm_password: String,
    email_field: TextFieldState,
    password_field: TextFieldState,
    confirm_password_field: TextFieldState,
    email_focused: bool,
    password_focused: bool,
    confirm_password_focused: bool,
    caret_blink: CaretBlink,
    busy: bool,
    status: String,
    status_tone: StatusTone,
    pending_confirmation: bool,
    remember: bool,
    auto_login: bool,
    /// Guards against repeated silent auto-login attempts in one process.
    auto_login_attempted: bool,
}

impl LoginModalView {
    pub fn new(ctx: &mut ViewContext<Self>, core: CoreHandle) -> Self {
        Self {
            core,
            font: crate::ui::fonts::load_ui_font(ctx),
            open: false,
            mode: AuthMode::Login,
            email: String::new(),
            password: String::new(),
            confirm_password: String::new(),
            email_field: TextFieldState::new(),
            password_field: TextFieldState::new(),
            confirm_password_field: TextFieldState::new(),
            email_focused: true,
            password_focused: false,
            confirm_password_focused: false,
            caret_blink: CaretBlink::new(),
            busy: false,
            status: String::new(),
            status_tone: StatusTone::Placeholder,
            pending_confirmation: false,
            remember: false,
            auto_login: false,
            auto_login_attempted: false,
        }
    }

    pub fn is_open(&self) -> bool {
        self.open
    }

    pub fn open(&mut self, ctx: &mut ViewContext<Self>) {
        self.open = true;
        self.mode = AuthMode::Login;
        self.status.clear();
        self.pending_confirmation = false;
        self.apply_remembered_credentials();
        self.focus_email_only();
        sync_caret_blink(self, ctx);
        ctx.emit(LoginModalEvent::OpenChanged { open: true });
        ctx.notify();
    }

    /// Silent login when prefs request auto-login and no session exists (HTML `tryAutoLogin`).
    /// Returns `true` if a login attempt was started.
    pub fn try_auto_login(&mut self, ctx: &mut ViewContext<Self>) -> bool {
        if self.busy || self.auto_login_attempted {
            return false;
        }
        let prefs = desktop_prefs::load_login_prefs(&self.core.data_dir());
        if !prefs.auto_login || !prefs.remember {
            return false;
        }
        let email = prefs
            .email
            .map(|value| value.trim().to_string())
            .filter(|value| !value.is_empty());
        let password = prefs
            .saved_password
            .filter(|value| !value.is_empty());
        let (Some(email), Some(password)) = (email, password) else {
            return false;
        };
        self.auto_login_attempted = true;
        self.remember = true;
        self.auto_login = true;
        self.email = email.clone();
        self.password = password.clone();
        self.submit_login(ctx, email, password);
        true
    }

    fn apply_remembered_credentials(&mut self) {
        let prefs = desktop_prefs::load_login_prefs(&self.core.data_dir());
        self.auto_login = prefs.auto_login;
        self.remember = prefs.remember || prefs.auto_login;
        if !prefs.remember {
            return;
        }
        if let Some(email) = prefs.email {
            self.email = email;
        }
        if let Some(password) = prefs.saved_password {
            self.password = password;
        }
    }

    fn persist_current_login_prefs(&self) {
        let _ = desktop_prefs::persist_login_prefs(
            &self.core.data_dir(),
            &self.email,
            &self.password,
            self.remember,
            self.auto_login,
        );
    }

    pub fn close(&mut self, ctx: &mut ViewContext<Self>) {
        self.open = false;
        self.busy = false;
        self.email_focused = false;
        self.password_focused = false;
        self.confirm_password_focused = false;
        sync_caret_blink(self, ctx);
        ctx.emit(LoginModalEvent::OpenChanged { open: false });
        ctx.notify();
    }

    fn focus_email_only(&mut self) {
        self.email_focused = true;
        self.password_focused = false;
        self.confirm_password_focused = false;
    }

    fn focus_password_only(&mut self) {
        self.email_focused = false;
        self.password_focused = true;
        self.confirm_password_focused = false;
    }

    fn focus_confirm_only(&mut self) {
        self.email_focused = false;
        self.password_focused = false;
        self.confirm_password_focused = true;
    }

    fn any_field_focused(&self) -> bool {
        self.email_focused || self.password_focused || self.confirm_password_focused
    }

    fn sync_caret(&mut self, ctx: &mut ViewContext<Self>) {
        sync_caret_blink(self, ctx);
    }

    fn toggle_mode(&mut self, ctx: &mut ViewContext<Self>) {
        self.mode = match self.mode {
            AuthMode::Login => AuthMode::Register,
            AuthMode::Register => AuthMode::Login,
        };
        self.status.clear();
        self.pending_confirmation = false;
        self.confirm_password.clear();
        self.confirm_password_field = TextFieldState::new();
        self.focus_email_only();
        self.sync_caret(ctx);
        ctx.notify();
    }

    fn consumes_shell_navigation(keystroke: &Keystroke) -> bool {
        if keystroke.alt || keystroke.meta {
            return false;
        }
        if keystroke.ctrl {
            return matches!(keystroke.key.as_str(), "1" | "2" | "3" | "4" | "5");
        }
        matches!(keystroke.key.as_str(), "left" | "right" | "home" | "end")
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

    fn map_edit_action(
        edit_action: &LoginModalAction,
        action: TextFieldEditAction,
    ) -> LoginModalAction {
        match edit_action {
            LoginModalAction::EmailEdit(_) => LoginModalAction::EmailEdit(action),
            LoginModalAction::PasswordEdit(_) => LoginModalAction::PasswordEdit(action),
            LoginModalAction::ConfirmPasswordEdit(_) => {
                LoginModalAction::ConfirmPasswordEdit(action)
            }
            _ => LoginModalAction::EmailEdit(action),
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
            Container::new(wrap_text_field_focus_on_click(
                TextFieldInput::builder(field, move |ctx, action| {
                    ctx.dispatch_typed_action(Self::map_edit_action(&edit_action, action));
                })
                .focused(focused)
                .ime_preedit(!marked.is_empty())
                .on_keydown(move |ctx, keystroke| {
                    Self::field_keydown(ctx, keystroke, tab_action.clone())
                })
                .finish(),
                move |ctx| ctx.dispatch_typed_action(click_action.clone()),
            ))
            .with_uniform_padding(10.0)
            .with_vertical_margin(6.0)
            .with_background(theme::canvas())
            .with_border(Border::all(1.0).with_border_fill(theme::border()))
            .with_corner_radius(CornerRadius::with_all(Radius::Pixels(4.0)))
            .finish(),
        );
        col.finish()
    }

    fn action_button(
        &self,
        label: &str,
        action: LoginModalAction,
        primary: bool,
    ) -> Box<dyn Element> {
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

    fn link_button(&self, label: &str, action: LoginModalAction) -> Box<dyn Element> {
        EventHandler::new(
            ui_text::body(label.to_string(), self.font)
                .with_color(theme::accent())
                .finish(),
        )
        .on_left_mouse_down(move |ctx, _, _| {
            ctx.dispatch_typed_action(action.clone());
            DispatchEventResult::StopPropagation
        })
        .finish()
    }

    fn checkbox_row(&self, label: &str, checked: bool, action: LoginModalAction) -> Box<dyn Element> {
        let mark = if checked { "✓" } else { "" };
        let box_el = Container::new(
            ConstrainedBox::new(
                Align::new(
                    ui_text::mono(mark.to_string(), self.font)
                        .with_color(theme::accent())
                        .finish(),
                )
                .finish(),
            )
            .with_width(15.0)
            .with_height(15.0)
            .finish(),
        )
        .with_background(theme::canvas())
        .with_border(Border::all(1.0).with_border_fill(theme::border_bright()))
        .with_corner_radius(CornerRadius::with_all(Radius::Pixels(2.0)))
        .finish();

        let mut row = Flex::row()
            .with_cross_axis_alignment(CrossAxisAlignment::Center)
            .with_main_axis_size(MainAxisSize::Min);
        row.add_child(box_el);
        row.add_child(
            Container::new(
                ui_text::body(label.to_string(), self.font)
                    .with_color(theme::muted())
                    .finish(),
            )
            .with_horizontal_margin(8.0)
            .finish(),
        );

        EventHandler::new(row.finish())
            .on_left_mouse_down(move |ctx, _, _| {
                ctx.dispatch_typed_action(action.clone());
                DispatchEventResult::StopPropagation
            })
            .finish()
    }

    fn login_options_block(&self) -> Box<dyn Element> {
        let mut col = Flex::column().with_cross_axis_alignment(CrossAxisAlignment::Start);
        col.add_child(self.checkbox_row(
            "记住账号密码",
            self.remember,
            LoginModalAction::ToggleRemember,
        ));
        col.add_child(
            Container::new(self.checkbox_row(
                "自动登录",
                self.auto_login,
                LoginModalAction::ToggleAutoLogin,
            ))
            .with_vertical_margin(5.0)
            .finish(),
        );
        Container::new(col.finish())
            .with_vertical_margin(4.0)
            .finish()
    }

    fn dialog(&self) -> Box<dyn Element> {
        let is_register = self.mode == AuthMode::Register;
        let title = if is_register {
            "注册 Wormhole"
        } else {
            "登录 Wormhole"
        };
        let description = if is_register {
            "创建账号后绑定本机设备身份，即可使用 P2P 集群、聊天与同步。"
        } else {
            "登录后绑定本机设备身份，可使用 P2P 集群、聊天与同步。"
        };
        let submit_label = if self.busy {
            if is_register {
                "注册中…"
            } else {
                "登录中…"
            }
        } else if is_register {
            "注册"
        } else {
            "登录"
        };
        let switch_prefix = if is_register {
            "已有账号？"
        } else {
            "没有账号？"
        };
        let switch_label = if is_register { "登录" } else { "注册" };

        let mut col = Flex::column().with_cross_axis_alignment(CrossAxisAlignment::Stretch);
        col.add_child(
            ui_text::title(title, self.font)
                .with_color(theme::text())
                .finish(),
        );
        col.add_child(
            Container::new(
                ui_text::body(description, self.font)
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
        let password_tab = if is_register {
            LoginModalAction::FocusConfirmPassword
        } else {
            LoginModalAction::FocusEmail
        };
        col.add_child(self.field_block(
            "密码",
            &self.password,
            &self.password_field.marked_text,
            "••••••••",
            self.password_focused,
            LoginModalAction::PasswordEdit(TextFieldEditAction::TypedCharacters(String::new())),
            LoginModalAction::FocusPassword,
            password_tab,
        ));
        if is_register {
            col.add_child(self.field_block(
                "确认密码",
                &self.confirm_password,
                &self.confirm_password_field.marked_text,
                "••••••••",
                self.confirm_password_focused,
                LoginModalAction::ConfirmPasswordEdit(TextFieldEditAction::TypedCharacters(
                    String::new(),
                )),
                LoginModalAction::FocusConfirmPassword,
                LoginModalAction::FocusEmail,
            ));
        } else {
            col.add_child(self.login_options_block());
        }

        if !self.status.is_empty() {
            col.add_child(status_line(
                self.status.clone(),
                self.font,
                self.status_tone,
            ));
        }

        if self.pending_confirmation {
            col.add_child(
                Container::new(self.link_button(
                    if self.busy {
                        "发送中…"
                    } else {
                        "重新发送确认邮件"
                    },
                    LoginModalAction::ResendConfirmation,
                ))
                .with_vertical_margin(8.0)
                .finish(),
            );
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
        actions.add_child(self.action_button(submit_label, LoginModalAction::Submit, true));
        col.add_child(
            Container::new(actions.finish())
                .with_vertical_margin(14.0)
                .finish(),
        );

        let mut switch_row = Flex::row()
            .with_cross_axis_alignment(CrossAxisAlignment::Center)
            .with_main_axis_alignment(MainAxisAlignment::Center);
        switch_row.add_child(
            ui_text::body(switch_prefix, self.font)
                .with_color(theme::muted())
                .finish(),
        );
        switch_row.add_child(
            Container::new(self.link_button(switch_label, LoginModalAction::ToggleMode))
                .with_horizontal_margin(4.0)
                .finish(),
        );
        col.add_child(switch_row.finish());

        let panel = EventHandler::new(
            Container::new(ConstrainedBox::new(col.finish()).with_width(400.0).finish())
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

    fn finish_authenticated(
        view: &mut Self,
        status: CloudAuthStatusDto,
        ctx: &mut ViewContext<Self>,
    ) {
        view.persist_current_login_prefs();
        view.status.clear();
        view.pending_confirmation = false;
        view.close(ctx);
        ctx.emit(LoginModalEvent::AuthChanged {
            authenticated: status.authenticated,
            device_id: status.device_id,
        });
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
        if self.mode == AuthMode::Register {
            if password.len() < 6 {
                self.status = "密码至少 6 位。".into();
                self.status_tone = StatusTone::Danger;
                ctx.notify();
                return;
            }
            if password != self.confirm_password {
                self.status = "两次输入的密码不一致。".into();
                self.status_tone = StatusTone::Danger;
                ctx.notify();
                return;
            }
            self.submit_register(ctx, email, password);
            return;
        }
        self.submit_login(ctx, email, password);
    }

    fn submit_login(&mut self, ctx: &mut ViewContext<Self>, email: String, password: String) {
        self.busy = true;
        self.pending_confirmation = false;
        self.status = "正在登录…".into();
        self.status_tone = StatusTone::Placeholder;
        ctx.notify();
        let core = self.core.clone();
        ctx.spawn(
            async move {
                let state = core.runtime().state.clone();
                supabase_password_login(&state, SupabasePasswordLoginParams { email, password })
                    .await?;
                let bootstrap = device_bootstrap(&state)
                    .await
                    .map_err(|err| err.to_string())?;
                if !bootstrap.ready {
                    return Err(bootstrap.error.unwrap_or_else(|| "设备身份恢复失败".into()));
                }
                cloud_auth_status(&state).await
            },
            |view, output, ctx| {
                view.busy = false;
                match output {
                    Ok(status) => Self::finish_authenticated(view, status, ctx),
                    Err(err) => {
                        view.status = err.clone();
                        view.status_tone = StatusTone::Danger;
                        if err.contains("邮箱尚未验证") {
                            view.pending_confirmation = true;
                        }
                    }
                }
                ctx.notify();
            },
        );
    }

    fn submit_register(&mut self, ctx: &mut ViewContext<Self>, email: String, password: String) {
        self.busy = true;
        self.pending_confirmation = false;
        self.status = "正在注册…".into();
        self.status_tone = StatusTone::Placeholder;
        ctx.notify();
        let core = self.core.clone();
        ctx.spawn(
            async move {
                let state = core.runtime().state.clone();
                let result = supabase_signup(
                    &state,
                    SupabaseSignupParams {
                        email,
                        password,
                        email_redirect_to: None,
                    },
                )
                .await?;
                if result.pending_confirmation {
                    return Ok(Err(result));
                }
                let bootstrap = device_bootstrap(&state)
                    .await
                    .map_err(|err| err.to_string())?;
                if !bootstrap.ready {
                    return Err(bootstrap.error.unwrap_or_else(|| "设备身份恢复失败".into()));
                }
                let status = result
                    .auth
                    .ok_or_else(|| "注册成功但未返回会话，请尝试登录。".to_string())?;
                Ok(Ok(status))
            },
            |view, output, ctx| {
                view.busy = false;
                match output {
                    Ok(Ok(status)) => Self::finish_authenticated(view, status, ctx),
                    Ok(Err(pending)) => {
                        view.pending_confirmation = true;
                        view.status = "注册成功，请查收验证邮件。".into();
                        view.status_tone = StatusTone::Success;
                        if let Some(email) = pending.email {
                            view.email = email;
                        }
                        view.persist_current_login_prefs();
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

    fn resend_confirmation(&mut self, ctx: &mut ViewContext<Self>) {
        let email = self.email.trim().to_string();
        if email.is_empty() {
            self.status = "请输入邮箱。".into();
            self.status_tone = StatusTone::Danger;
            ctx.notify();
            return;
        }
        if self.busy {
            return;
        }
        self.busy = true;
        self.status = "正在发送确认邮件…".into();
        self.status_tone = StatusTone::Placeholder;
        ctx.notify();
        ctx.spawn(
            async move {
                supabase_resend_signup(SupabaseResendSignupParams {
                    email,
                    email_redirect_to: None,
                })
                .await
            },
            |view, output, ctx| {
                view.busy = false;
                match output {
                    Ok(_) => {
                        view.pending_confirmation = true;
                        view.status = "确认邮件已发送，请查收。".into();
                        view.status_tone = StatusTone::Success;
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
            LoginModalAction::ResendConfirmation => self.resend_confirmation(ctx),
            LoginModalAction::ToggleMode => self.toggle_mode(ctx),
            LoginModalAction::ToggleRemember => {
                self.remember = !self.remember;
                if !self.remember {
                    self.auto_login = false;
                }
                ctx.notify();
            }
            LoginModalAction::ToggleAutoLogin => {
                self.auto_login = !self.auto_login;
                if self.auto_login {
                    self.remember = true;
                }
                ctx.notify();
            }
            LoginModalAction::FocusEmail => {
                self.focus_email_only();
                self.sync_caret(ctx);
                ctx.notify();
            }
            LoginModalAction::FocusPassword => {
                self.focus_password_only();
                self.sync_caret(ctx);
                ctx.notify();
            }
            LoginModalAction::FocusConfirmPassword => {
                self.focus_confirm_only();
                self.sync_caret(ctx);
                ctx.notify();
            }
            LoginModalAction::EmailEdit(edit) => {
                self.email_field.apply(&mut self.email, edit);
                self.focus_email_only();
                self.sync_caret(ctx);
                ctx.notify();
            }
            LoginModalAction::PasswordEdit(edit) => {
                self.password_field.apply(&mut self.password, edit);
                self.focus_password_only();
                self.sync_caret(ctx);
                ctx.notify();
            }
            LoginModalAction::ConfirmPasswordEdit(edit) => {
                self.confirm_password_field
                    .apply(&mut self.confirm_password, edit);
                self.focus_confirm_only();
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
