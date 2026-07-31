//! AI first-run recommendations / preference onboarding (`.ai-onboarding`).
//!
//! Catalog and copy match `desktop-current.html` `#ai-onboarding`.

use pathfinder_color::ColorU;
use warpui::elements::{
    Align, Border, ClippedScrollStateHandle, ClippedScrollable, ConstrainedBox, Container,
    CornerRadius, CrossAxisAlignment, DispatchEventResult, EventHandler, Expanded, Fill, Flex,
    Hoverable, MainAxisAlignment, MainAxisSize, MouseState, MouseStateHandle, ParentElement,
    Radius, ScrollbarWidth, Text, Wrap,
};
use warpui::fonts::FamilyId;
use warpui::Element;
use wormhole_desktop_core::ai_onboarding_prefs::AiOnboardingSelections;

use super::AgentPanelAction;
use crate::ui::theme;
use crate::ui_text;

use std::sync::{Arc, Mutex};

const INNER_MAX_WIDTH: f32 = 880.0;
const GROUP_HEAD_WIDTH: f32 = 148.0;
const OPTION_MIN_HEIGHT: f32 = 44.0;
const ACTION_BTN_HEIGHT: f32 = 44.0;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum AiOnboardingGroup {
    Career,
    Interests,
    Daily,
}

impl AiOnboardingGroup {
    pub fn title(self) -> &'static str {
        match self {
            Self::Career => "职业",
            Self::Interests => "兴趣爱好",
            Self::Daily => "日常活动",
        }
    }

    pub fn subtitle(self) -> &'static str {
        match self {
            Self::Career => "你主要投入的领域",
            Self::Interests => "你愿意持续探索的主题",
            Self::Daily => "你经常需要完成的事情",
        }
    }

    pub fn all() -> [Self; 3] {
        [Self::Career, Self::Interests, Self::Daily]
    }
}

#[derive(Debug, Clone, Copy)]
pub struct AiOnboardingOption {
    pub id: &'static str,
    pub label: &'static str,
}

pub const CAREER_OPTIONS: &[AiOnboardingOption] = &[
    AiOnboardingOption {
        id: "software-development",
        label: "软件与开发",
    },
    AiOnboardingOption {
        id: "design-creative",
        label: "设计与创意",
    },
    AiOnboardingOption {
        id: "product-operations",
        label: "产品与运营",
    },
    AiOnboardingOption {
        id: "data-research",
        label: "数据与研究",
    },
    AiOnboardingOption {
        id: "marketing-sales",
        label: "市场与销售",
    },
    AiOnboardingOption {
        id: "education",
        label: "教育",
    },
    AiOnboardingOption {
        id: "healthcare",
        label: "医疗与健康",
    },
    AiOnboardingOption {
        id: "finance-legal",
        label: "金融与法律",
    },
    AiOnboardingOption {
        id: "engineering-manufacturing",
        label: "工程与制造",
    },
    AiOnboardingOption {
        id: "construction-real-estate",
        label: "建筑与地产",
    },
    AiOnboardingOption {
        id: "media-culture",
        label: "媒体与文化",
    },
    AiOnboardingOption {
        id: "public-service",
        label: "公共服务",
    },
    AiOnboardingOption {
        id: "administration-hr",
        label: "行政与人力",
    },
    AiOnboardingOption {
        id: "service-retail",
        label: "服务与零售",
    },
    AiOnboardingOption {
        id: "agriculture-environment",
        label: "农业与环境",
    },
    AiOnboardingOption {
        id: "entrepreneurship-management",
        label: "创业与管理",
    },
    AiOnboardingOption {
        id: "freelance",
        label: "自由职业",
    },
    AiOnboardingOption {
        id: "student",
        label: "学生",
    },
];

pub const INTEREST_OPTIONS: &[AiOnboardingOption] = &[
    AiOnboardingOption {
        id: "technology",
        label: "科技数码",
    },
    AiOnboardingOption {
        id: "reading-writing",
        label: "阅读写作",
    },
    AiOnboardingOption {
        id: "film-music",
        label: "影视音乐",
    },
    AiOnboardingOption {
        id: "gaming",
        label: "游戏",
    },
    AiOnboardingOption {
        id: "photography",
        label: "摄影",
    },
    AiOnboardingOption {
        id: "fitness",
        label: "运动健康",
    },
    AiOnboardingOption {
        id: "travel",
        label: "旅行",
    },
    AiOnboardingOption {
        id: "making",
        label: "手工创作",
    },
];

pub const DAILY_OPTIONS: &[AiOnboardingOption] = &[
    AiOnboardingOption {
        id: "coding-debugging",
        label: "编程与调试",
    },
    AiOnboardingOption {
        id: "learning",
        label: "学习新知识",
    },
    AiOnboardingOption {
        id: "content-creation",
        label: "内容创作",
    },
    AiOnboardingOption {
        id: "project-management",
        label: "项目管理",
    },
    AiOnboardingOption {
        id: "file-organization",
        label: "文件整理",
    },
    AiOnboardingOption {
        id: "collaboration",
        label: "团队协作",
    },
    AiOnboardingOption {
        id: "life-planning",
        label: "生活规划",
    },
    AiOnboardingOption {
        id: "information-search",
        label: "信息检索",
    },
    AiOnboardingOption {
        id: "cooking",
        label: "做饭与备餐",
    },
    AiOnboardingOption {
        id: "household-chores",
        label: "家务与整理",
    },
    AiOnboardingOption {
        id: "family-care",
        label: "育儿与家庭照护",
    },
    AiOnboardingOption {
        id: "shopping-errands",
        label: "购物与日常办事",
    },
    AiOnboardingOption {
        id: "exercise-health",
        label: "锻炼与健康管理",
    },
    AiOnboardingOption {
        id: "finance-budgeting",
        label: "记账与理财",
    },
    AiOnboardingOption {
        id: "social-communication",
        label: "社交与沟通",
    },
    AiOnboardingOption {
        id: "travel-planning",
        label: "通勤与出行规划",
    },
    AiOnboardingOption {
        id: "leisure-entertainment",
        label: "娱乐与休闲",
    },
    AiOnboardingOption {
        id: "appointments-reminders",
        label: "预约与提醒",
    },
];

pub fn options_for(group: AiOnboardingGroup) -> &'static [AiOnboardingOption] {
    match group {
        AiOnboardingGroup::Career => CAREER_OPTIONS,
        AiOnboardingGroup::Interests => INTEREST_OPTIONS,
        AiOnboardingGroup::Daily => DAILY_OPTIONS,
    }
}

pub fn status_text(selections: &AiOnboardingSelections) -> String {
    let complete = selections.complete_group_count();
    if selections.all_groups_complete() {
        format!("已选择 {} 项，可以保存", selections.selected_count())
    } else if complete == 0 {
        "请在每一类中至少选择一项".into()
    } else {
        format!("已完成 {complete} / 3 类，请在每一类中至少选择一项")
    }
}

fn group_selected<'a>(
    selections: &'a AiOnboardingSelections,
    group: AiOnboardingGroup,
) -> &'a [String] {
    match group {
        AiOnboardingGroup::Career => &selections.career,
        AiOnboardingGroup::Interests => &selections.interests,
        AiOnboardingGroup::Daily => &selections.daily,
    }
}

fn option_chip(
    font: FamilyId,
    group: AiOnboardingGroup,
    option: &AiOnboardingOption,
    selected: bool,
) -> Box<dyn Element> {
    let mouse: MouseStateHandle = Arc::new(Mutex::new(MouseState::default()));
    let id = option.id.to_string();
    let label = option.label.to_string();
    HoverableChip {
        mouse,
        font,
        group,
        id,
        label,
        selected,
    }
    .build()
}

/// Small helper so chip hover color rebuilds with stable selected state from parent.
struct HoverableChip {
    mouse: MouseStateHandle,
    font: FamilyId,
    group: AiOnboardingGroup,
    id: String,
    label: String,
    selected: bool,
}

impl HoverableChip {
    fn build(self) -> Box<dyn Element> {
        let font = self.font;
        let group = self.group;
        let id = self.id.clone();
        let label = self.label.clone();
        let selected = self.selected;
        let hoverable = Hoverable::new(self.mouse, move |state| {
            let hovered = state.is_hovered();
            let (border, bg, fg, box_border, box_bg, box_fg) = if selected {
                (
                    theme::accent_cool(),
                    theme::accent_bg_default(),
                    theme::text(),
                    theme::accent_cool(),
                    theme::accent_cool(),
                    theme::canvas(),
                )
            } else if hovered {
                (
                    ColorU::new(
                        ((theme::border_bright().r as u16 * 72 + theme::text().r as u16 * 28) / 100)
                            as u8,
                        ((theme::border_bright().g as u16 * 72 + theme::text().g as u16 * 28) / 100)
                            as u8,
                        ((theme::border_bright().b as u16 * 72 + theme::text().b as u16 * 28) / 100)
                            as u8,
                        255,
                    ),
                    ColorU::new(
                        ((theme::panel().r as u16 * 72 + theme::canvas().r as u16 * 28) / 100) as u8,
                        ((theme::panel().g as u16 * 72 + theme::canvas().g as u16 * 28) / 100) as u8,
                        ((theme::panel().b as u16 * 72 + theme::canvas().b as u16 * 28) / 100) as u8,
                        255,
                    ),
                    theme::text(),
                    theme::border_bright(),
                    ColorU::new(0, 0, 0, 0),
                    ColorU::new(0, 0, 0, 0),
                )
            } else {
                (
                    theme::border_bright(),
                    ColorU::new(
                        ((theme::panel().r as u16 * 72 + theme::canvas().r as u16 * 28) / 100) as u8,
                        ((theme::panel().g as u16 * 72 + theme::canvas().g as u16 * 28) / 100) as u8,
                        ((theme::panel().b as u16 * 72 + theme::canvas().b as u16 * 28) / 100) as u8,
                        255,
                    ),
                    theme::muted(),
                    theme::border_bright(),
                    ColorU::new(0, 0, 0, 0),
                    ColorU::new(0, 0, 0, 0),
                )
            };
            let check = Text::new(
                if selected {
                    "✓".to_string()
                } else {
                    " ".to_string()
                },
                font,
                11.0,
            )
            .with_color(box_fg)
            .finish();
            let check_box = ConstrainedBox::new(
                Container::new(Align::new(check).finish())
                    .with_background(box_bg)
                    .with_border(Border::all(1.0).with_border_fill(box_border))
                    .with_corner_radius(CornerRadius::with_all(Radius::Pixels(3.0)))
                    .finish(),
            )
            .with_width(15.0)
            .with_height(15.0)
            .finish();
            let row = Flex::row()
                .with_cross_axis_alignment(CrossAxisAlignment::Center)
                .with_main_axis_size(MainAxisSize::Min)
                .with_child(check_box)
                .with_child(
                    ConstrainedBox::new(warpui::elements::Empty::new().finish())
                        .with_width(8.0)
                        .with_height(1.0)
                        .finish(),
                )
                .with_child(
                    Text::new(label.clone(), font, 13.0)
                        .with_color(fg)
                        .finish(),
                )
                .finish();
            ConstrainedBox::new(
                Container::new(row)
                    .with_padding_left(13.0)
                    .with_padding_right(13.0)
                    .with_background(bg)
                    .with_border(Border::all(1.0).with_border_fill(border))
                    .with_corner_radius(CornerRadius::with_all(Radius::Pixels(6.0)))
                    .finish(),
            )
            .with_min_height(OPTION_MIN_HEIGHT)
            .finish()
        })
        .finish();

        EventHandler::new(hoverable)
            .on_left_mouse_down(move |ctx, _, _| {
                ctx.dispatch_typed_action(AgentPanelAction::ToggleAiOnboardingOption {
                    group,
                    id: id.clone(),
                });
                DispatchEventResult::StopPropagation
            })
            .finish()
    }
}

fn render_group(
    font: FamilyId,
    group: AiOnboardingGroup,
    selections: &AiOnboardingSelections,
) -> Box<dyn Element> {
    let selected = group_selected(selections, group);
    let chips: Vec<Box<dyn Element>> = options_for(group)
        .iter()
        .map(|opt| {
            let is_on = selected.iter().any(|s| s == opt.id);
            option_chip(font, group, opt, is_on)
        })
        .collect();
    let mut options = Wrap::row().with_spacing(8.0).with_run_spacing(8.0);
    options.extend(chips);

    let head = ConstrainedBox::new(
        Flex::column()
            .with_cross_axis_alignment(CrossAxisAlignment::Start)
            .with_main_axis_size(MainAxisSize::Min)
            .with_child(
                ui_text::body(group.title().to_string(), font)
                    .with_color(theme::text())
                    .finish(),
            )
            .with_child(
                Container::new(
                    Text::new(group.subtitle().to_string(), font, 12.0)
                        .with_color(theme::placeholder())
                        .finish(),
                )
                .with_padding_top(4.0)
                .finish(),
            )
            .finish(),
    )
    .with_width(GROUP_HEAD_WIDTH)
    .finish();

    Container::new(
        Flex::row()
            .with_cross_axis_alignment(CrossAxisAlignment::Start)
            .with_main_axis_size(MainAxisSize::Max)
            .with_child(head)
            .with_child(
                ConstrainedBox::new(warpui::elements::Empty::new().finish())
                    .with_width(24.0)
                    .with_height(1.0)
                    .finish(),
            )
            .with_child(Expanded::new(1.0, options.finish()).finish())
            .finish(),
    )
    .with_padding_top(20.0)
    .with_padding_bottom(20.0)
    .with_border(Border::top(1.0).with_border_fill(theme::border()))
    .finish()
}

fn action_button(
    font: FamilyId,
    label: &str,
    primary: bool,
    enabled: bool,
    action: AgentPanelAction,
) -> Box<dyn Element> {
    let (bg, fg, border) = if primary && enabled {
        (theme::accent_cool(), theme::canvas(), theme::accent_cool())
    } else if primary {
        (
            theme::panel(),
            theme::placeholder(),
            theme::border(),
        )
    } else {
        (
            ColorU::new(0, 0, 0, 0),
            theme::muted(),
            ColorU::new(0, 0, 0, 0),
        )
    };
    let label = label.to_string();
    let inner = ConstrainedBox::new(
        Container::new(
            Align::new(
                ui_text::body(label, font).with_color(fg).finish(),
            )
            .finish(),
        )
        .with_padding_left(16.0)
        .with_padding_right(16.0)
        .with_background(bg)
        .with_border(Border::all(1.0).with_border_fill(border))
        .with_corner_radius(CornerRadius::with_all(Radius::Pixels(6.0)))
        .finish(),
    )
    .with_height(ACTION_BTN_HEIGHT)
    .finish();

    if !enabled {
        return inner;
    }
    EventHandler::new(inner)
        .on_left_mouse_down(move |ctx, _, _| {
            ctx.dispatch_typed_action(action.clone());
            DispatchEventResult::StopPropagation
        })
        .finish()
}

/// Full-bleed AI onboarding overlay (covers agent shell including sidebar).
pub fn render(
    font: FamilyId,
    mono: FamilyId,
    selections: &AiOnboardingSelections,
    scroll: ClippedScrollStateHandle,
) -> Box<dyn Element> {
    let submit_enabled = selections.all_groups_complete();
    let status = status_text(selections);

    let mut col = Flex::column()
        .with_cross_axis_alignment(CrossAxisAlignment::Stretch)
        .with_main_axis_size(MainAxisSize::Min);

    col.add_child(
        Text::new("AI · 首次设置".to_string(), mono, 11.0)
            .with_color(theme::accent_cool())
            .finish(),
    );
    col.add_child(
        Container::new(
            Text::new("让 AI 更懂你的日常".to_string(), font, 34.0)
                .with_color(theme::text())
                .finish(),
        )
        .with_padding_top(12.0)
        .finish(),
    );
    col.add_child(
        Container::new(
            Text::new(
                "选择你愿意分享的内容，可多选。AI 会据此调整任务推荐与回答侧重点，偏好会同步保存到当前账号。"
                    .to_string(),
                font,
                14.0,
            )
            .with_color(theme::muted())
            .finish(),
        )
        .with_padding_top(14.0)
        .with_padding_bottom(26.0)
        .finish(),
    );

    for group in AiOnboardingGroup::all() {
        col.add_child(render_group(font, group, selections));
    }

    let actions = Container::new(
        Flex::row()
            .with_cross_axis_alignment(CrossAxisAlignment::Center)
            .with_main_axis_alignment(MainAxisAlignment::SpaceBetween)
            .with_main_axis_size(MainAxisSize::Max)
            .with_child(
                Expanded::new(
                    1.0,
                    Text::new(status, font, 12.0)
                        .with_color(theme::placeholder())
                        .finish(),
                )
                .finish(),
            )
            .with_child(
                Flex::row()
                    .with_cross_axis_alignment(CrossAxisAlignment::Center)
                    .with_main_axis_size(MainAxisSize::Min)
                    .with_child(action_button(
                        font,
                        "暂时跳过",
                        false,
                        true,
                        AgentPanelAction::SkipAiOnboarding,
                    ))
                    .with_child(
                        ConstrainedBox::new(warpui::elements::Empty::new().finish())
                            .with_width(8.0)
                            .with_height(1.0)
                            .finish(),
                    )
                    .with_child(action_button(
                        font,
                        "保存并进入 AI",
                        true,
                        submit_enabled,
                        AgentPanelAction::SubmitAiOnboarding,
                    ))
                    .finish(),
            )
            .finish(),
    )
    .with_padding_top(20.0)
    .with_border(Border::top(1.0).with_border_fill(theme::border()))
    .finish();
    col.add_child(actions);

    let inner = ConstrainedBox::new(col.finish())
        .with_max_width(INNER_MAX_WIDTH)
        .finish();

    let padded = Container::new(
        Align::new(inner).top_center().finish(),
    )
    .with_padding_top(42.0)
    .with_padding_bottom(42.0)
    .with_padding_left(48.0)
    .with_padding_right(48.0)
    .with_background(theme::canvas())
    .finish();

    let scrollable = ClippedScrollable::vertical(
        scroll,
        padded,
        ScrollbarWidth::Auto,
        Fill::None,
        Fill::None,
        Fill::None,
    )
    .finish();

    // Stop clicks from falling through to sidebar / composer underneath.
    EventHandler::new(
        Container::new(scrollable)
            .with_background(theme::canvas())
            .finish(),
    )
    .on_left_mouse_down(|_, _, _| DispatchEventResult::StopPropagation)
    .finish()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn catalog_matches_design_counts() {
        assert_eq!(CAREER_OPTIONS.len(), 18);
        assert_eq!(INTEREST_OPTIONS.len(), 8);
        assert_eq!(DAILY_OPTIONS.len(), 18);
    }

    #[test]
    fn status_text_progresses() {
        let empty = AiOnboardingSelections::empty();
        assert!(status_text(&empty).contains("每一类"));
        let mut s = empty;
        s.career.push("student".into());
        assert!(status_text(&s).contains("1 / 3"));
        s.interests.push("gaming".into());
        s.daily.push("learning".into());
        assert!(status_text(&s).contains("可以保存"));
    }
}
