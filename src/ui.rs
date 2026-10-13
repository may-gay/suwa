use iced::widget::{
    self, button as b, checkbox as c, container, pick_list as p, slider as s, text, text_input as t,
};
use iced::{Background, Border, Color, Element, Font, Shadow, Theme, Vector, gradient};
use std::borrow::Borrow;

pub const BODY: Font = Font::with_name("Maple Mono");
pub const DISPLAY: Font = Font {
    weight: iced::font::Weight::Bold,
    ..BODY
};
pub const LAVENDER: Color = Color::from_rgb8(200, 164, 232);
pub const BLUSH: Color = Color::from_rgb8(240, 168, 200);
pub const MINT: Color = Color::from_rgb8(160, 224, 200);
pub fn reader_chrome_height(expanded: bool) -> f32 {
    if expanded { 240.0 } else { 130.0 }
}

pub struct Colors {
    pub base: Color,
    pub panel: Color,
    pub surface: Color,
    pub border: Color,
    pub text: Color,
    pub secondary: Color,
    pub muted: Color,
    pub accent: Color,
    pub blush: Color,
    pub mint: Color,
}
pub fn colors(theme: &Theme) -> Colors {
    let p = theme.palette();
    if theme.to_string().starts_with("Pastel goth") && theme.extended_palette().is_dark {
        Colors {
            base: p.background,
            panel: Color::from_rgb8(26, 21, 32),
            surface: Color::from_rgb8(36, 29, 46),
            border: Color::from_rgb8(46, 36, 56),
            text: p.text,
            secondary: Color::from_rgb8(184, 164, 208),
            muted: Color::from_rgb8(144, 120, 152),
            accent: p.primary,
            blush: BLUSH,
            mint: MINT,
        }
    } else {
        let e = theme.extended_palette();
        Colors {
            base: p.background,
            panel: e.background.weak.color,
            surface: e.background.strong.color,
            border: mix(p.background, p.text, 0.20),
            text: p.text,
            secondary: mix(p.background, p.text, 0.78),
            muted: mix(p.background, p.text, 0.64),
            accent: p.primary,
            blush: p.danger,
            mint: p.success,
        }
    }
}
fn mix(a: Color, b: Color, t: f32) -> Color {
    Color::from_rgb(
        a.r + (b.r - a.r) * t,
        a.g + (b.g - a.g) * t,
        a.b + (b.b - a.b) * t,
    )
}
fn alpha(color: Color, a: f32) -> Color {
    Color { a, ..color }
}
fn border(color: Color, radius: f32) -> Border {
    Border {
        color,
        width: 1.0,
        radius: radius.into(),
    }
}
fn gradient(a: Color, b: Color) -> Background {
    gradient::Linear::new(iced::Radians(2.35))
        .add_stop(0.0, a)
        .add_stop(1.0, b)
        .into()
}

pub fn heading(value: &str) -> widget::Text<'_> {
    text(value)
        .font(DISPLAY)
        .size(28)
        .style(|theme: &Theme| text::Style {
            color: Some(colors(theme).accent),
        })
}
pub fn caption<'a>(value: impl iced::advanced::text::IntoFragment<'a>) -> widget::Text<'a> {
    text(value).size(13).style(|theme: &Theme| text::Style {
        color: Some(colors(theme).secondary),
    })
}
pub fn button<'a, M: 'a>(content: impl Into<Element<'a, M>>) -> widget::Button<'a, M> {
    widget::button(content).padding([9, 18]).style(primary)
}
pub fn primary(theme: &Theme, status: b::Status) -> b::Style {
    filled(theme, status, false)
}
pub fn danger(theme: &Theme, status: b::Status) -> b::Style {
    filled(theme, status, true)
}
fn filled(theme: &Theme, status: b::Status, danger: bool) -> b::Style {
    let p = colors(theme);
    if status == b::Status::Disabled {
        return disabled(theme);
    }
    let accent = if danger { p.blush } else { p.accent };
    let end = if danger {
        mix(accent, Color::from_rgb8(232, 138, 160), 0.8)
    } else {
        mix(accent, p.blush, 0.28)
    };
    let hovered = status == b::Status::Hovered;
    b::Style {
        background: Some(gradient(accent, end)),
        text_color: iced::theme::palette::readable(accent, Color::from_rgb8(18, 15, 24)),
        border: border(alpha(accent, 0.6), 999.0),
        shadow: Shadow {
            color: alpha(accent, if hovered { 0.28 } else { 0.10 }),
            offset: Vector::new(0.0, if hovered { 3.0 } else { 1.0 }),
            blur_radius: if hovered { 15.0 } else { 6.0 },
        },
        ..Default::default()
    }
}
fn disabled(theme: &Theme) -> b::Style {
    let p = colors(theme);
    b::Style {
        background: Some(p.surface.into()),
        text_color: p.muted,
        border: border(p.border, 999.0),
        ..Default::default()
    }
}
pub fn secondary(theme: &Theme, status: b::Status) -> b::Style {
    if status == b::Status::Disabled {
        return disabled(theme);
    }
    let p = colors(theme);
    b::Style {
        background: Some(
            if status == b::Status::Hovered {
                mix(p.surface, p.accent, 0.10)
            } else {
                p.surface
            }
            .into(),
        ),
        text_color: p.accent,
        border: border(
            alpha(
                p.accent,
                if status == b::Status::Hovered {
                    0.9
                } else {
                    0.45
                },
            ),
            999.0,
        ),
        ..Default::default()
    }
}
pub fn ghost(theme: &Theme, status: b::Status) -> b::Style {
    let p = colors(theme);
    b::Style {
        background: (matches!(status, b::Status::Hovered | b::Status::Pressed))
            .then(|| mix(p.panel, p.accent, 0.10).into()),
        text_color: match status {
            b::Status::Hovered | b::Status::Pressed => p.accent,
            b::Status::Disabled => p.muted,
            _ => p.secondary,
        },
        border: Border {
            radius: 12.0.into(),
            ..Default::default()
        },
        ..Default::default()
    }
}
pub fn navigation(theme: &Theme, status: b::Status) -> b::Style {
    let p = colors(theme);
    b::Style {
        background: Some(
            mix(
                p.panel,
                p.accent,
                if status == b::Status::Hovered {
                    0.18
                } else {
                    0.10
                },
            )
            .into(),
        ),
        text_color: p.accent,
        border: border(alpha(p.accent, 0.38), 12.0),
        ..Default::default()
    }
}
pub fn cover_button(theme: &Theme, status: b::Status) -> b::Style {
    let p = colors(theme);
    b::Style {
        background: (status == b::Status::Hovered).then(|| mix(p.base, p.accent, 0.04).into()),
        text_color: p.text,
        border: Border {
            radius: 8.0.into(),
            ..Default::default()
        },
        shadow: Shadow {
            color: alpha(
                p.accent,
                if status == b::Status::Hovered {
                    0.12
                } else {
                    0.0
                },
            ),
            blur_radius: 16.0,
            ..Default::default()
        },
        ..Default::default()
    }
}
pub fn panel(theme: &Theme) -> container::Style {
    let p = colors(theme);
    container::Style {
        background: Some(p.panel.into()),
        text_color: Some(p.text),
        border: border(p.border, 20.0),
        ..Default::default()
    }
}
pub fn inset(theme: &Theme) -> container::Style {
    let p = colors(theme);
    container::Style {
        background: Some(p.surface.into()),
        text_color: Some(p.text),
        border: border(p.border, 12.0),
        ..Default::default()
    }
}
pub fn badge(theme: &Theme) -> container::Style {
    let p = colors(theme);
    container::Style {
        background: Some(mix(p.panel, p.mint, 0.10).into()),
        text_color: Some(p.mint),
        border: border(alpha(p.mint, 0.22), 999.0),
        ..Default::default()
    }
}
pub fn reader_bar(theme: &Theme) -> container::Style {
    let p = colors(theme);
    container::Style {
        background: Some(p.panel.into()),
        text_color: Some(p.text),
        ..Default::default()
    }
}
pub fn text_input<'a, M: Clone + 'a>(placeholder: &str, value: &str) -> widget::TextInput<'a, M> {
    widget::text_input(placeholder, value)
        .padding([11, 14])
        .size(15)
        .style(|theme, status| {
            let p = colors(theme);
            t::Style {
                background: p.surface.into(),
                border: border(
                    match status {
                        t::Status::Focused { .. } => p.accent,
                        t::Status::Hovered => alpha(p.accent, 0.55),
                        _ => p.border,
                    },
                    12.0,
                ),
                icon: p.secondary,
                placeholder: p.muted,
                value: if status == t::Status::Disabled {
                    p.muted
                } else {
                    p.text
                },
                selection: mix(p.surface, p.accent, 0.35),
            }
        })
}
pub fn checkbox<'a, M: 'a>(checked: bool) -> widget::Checkbox<'a, M> {
    widget::checkbox(checked)
        .size(18)
        .spacing(10)
        .text_size(14)
        .style(|theme, status| {
            let p = colors(theme);
            let checked = match status {
                c::Status::Active { is_checked }
                | c::Status::Hovered { is_checked }
                | c::Status::Disabled { is_checked } => is_checked,
            };
            c::Style {
                background: if checked { p.accent } else { p.surface }.into(),
                icon_color: iced::theme::palette::readable(p.accent, p.base),
                border: border(
                    if checked || matches!(status, c::Status::Hovered { .. }) {
                        p.accent
                    } else {
                        mix(p.border, p.secondary, 0.25)
                    },
                    6.0,
                ),
                text_color: Some(if matches!(status, c::Status::Disabled { .. }) {
                    p.muted
                } else {
                    p.text
                }),
            }
        })
}
pub fn pick_list<'a, T, L, V, M>(
    options: L,
    selected: Option<V>,
    on_selected: impl Fn(T) -> M + 'a,
) -> widget::PickList<'a, T, L, V, M>
where
    T: ToString + PartialEq + Clone + 'a,
    L: Borrow<[T]> + 'a,
    V: Borrow<T> + 'a,
    M: Clone + 'a,
{
    widget::pick_list(options, selected, on_selected)
        .padding([9, 14])
        .text_size(14)
        .style(|theme, status| {
            let p = colors(theme);
            p::Style {
                text_color: p.text,
                placeholder_color: p.muted,
                handle_color: p.accent,
                background: p.surface.into(),
                border: border(
                    if matches!(status, p::Status::Hovered | p::Status::Opened { .. }) {
                        p.accent
                    } else {
                        p.border
                    },
                    12.0,
                ),
            }
        })
        .menu_style(|theme| {
            let p = colors(theme);
            widget::overlay::menu::Style {
                text_color: p.text,
                background: p.surface.into(),
                border: border(alpha(p.accent, 0.4), 12.0),
                selected_text_color: p.accent,
                selected_background: mix(p.surface, p.accent, 0.15).into(),
                shadow: Shadow {
                    color: alpha(p.base, 0.5),
                    offset: Vector::new(0.0, 6.0),
                    blur_radius: 20.0,
                },
            }
        })
}
pub fn slider<'a, M: Clone + 'a>(
    range: std::ops::RangeInclusive<f32>,
    value: f32,
    on_change: impl Fn(f32) -> M + 'a,
) -> widget::Slider<'a, f32, M> {
    widget::slider(range, value, on_change)
        .height(22)
        .style(|theme, status| {
            let p = colors(theme);
            s::Style {
                rail: s::Rail {
                    backgrounds: (gradient(p.accent, p.blush), p.surface.into()),
                    width: 5.0,
                    border: border(p.border, 999.0),
                },
                handle: s::Handle {
                    shape: s::HandleShape::Circle {
                        radius: if matches!(status, s::Status::Hovered | s::Status::Dragged) {
                            8.0
                        } else {
                            7.0
                        },
                    },
                    background: p.accent.into(),
                    border_width: 2.0,
                    border_color: p.panel,
                },
            }
        })
}

pub fn theme(settings: &crate::domain::Settings) -> Theme {
    if !settings.oled
        && settings.theme != "Pastel goth"
        && let Some(theme) = Theme::ALL.iter().find(|t| t.to_string() == settings.theme)
    {
        return theme.clone();
    }
    let hex = settings.accent.trim_start_matches('#');
    let accent = u32::from_str_radix(hex, 16)
        .ok()
        .filter(|_| hex.len() == 6)
        .map(|v| Color::from_rgb8((v >> 16) as u8, (v >> 8) as u8, v as u8))
        .unwrap_or(LAVENDER);
    if !settings.dark && !settings.oled {
        return Theme::custom(
            "Pastel goth light",
            iced::theme::Palette {
                background: Color::from_rgb8(246, 240, 252),
                text: Color::from_rgb8(46, 36, 56),
                primary: Color::from_rgb8(115, 73, 149),
                success: Color::from_rgb8(40, 115, 92),
                warning: Color::from_rgb8(140, 96, 30),
                danger: Color::from_rgb8(170, 60, 110),
            },
        );
    }
    Theme::custom(
        "Pastel goth",
        iced::theme::Palette {
            background: if settings.oled {
                Color::BLACK
            } else {
                Color::from_rgb8(18, 15, 24)
            },
            text: Color::from_rgb8(232, 216, 248),
            primary: accent,
            success: MINT,
            warning: Color::from_rgb8(232, 200, 164),
            danger: BLUSH,
        },
    )
}

pub fn section<'a, M: 'a>(
    title: &'a str,
    description: &'a str,
    body: impl Into<Element<'a, M>>,
) -> Element<'a, M> {
    container(
        widget::column![heading(title).size(18), caption(description), body.into()]
            .spacing(14)
            .width(iced::Length::Fill),
    )
    .padding(24)
    .width(iced::Length::Fill)
    .style(panel)
    .into()
}
