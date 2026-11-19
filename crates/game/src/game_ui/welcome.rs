use super::{Layer, PAPER, button, button_with_label, clicked};
use bevy::{
    prelude::*,
    text::{FontWeight, LetterSpacing, LineHeight},
    ui::{BackgroundGradient, ColorStop, LinearGradient, widget::NodeImageMode},
    window::PrimaryWindow,
};
use serde_json::json;

#[derive(Component)]
pub(super) struct Welcome;

#[derive(Component)]
pub(super) struct Utility;

#[derive(Component)]
pub(super) struct Artwork;

#[derive(Component)]
pub(super) struct Scrim;

#[derive(Component)]
pub(super) enum Layout {
    Toolbar,
    Copy,
}

#[derive(Component)]
pub(super) enum Typography {
    Title,
    Subtitle,
}

const COVER_INK: Color = Color::srgb(0.09, 0.07, 0.055);

pub(super) fn spawn(parent: &mut ChildSpawnerCommands, assets: &AssetServer) {
    parent
        .spawn((
            Welcome,
            Layer::Welcome,
            ZIndex(25),
            fill(),
            BackgroundColor(COVER_INK),
        ))
        .with_children(|screen| {
            screen.spawn((
                Artwork,
                ImageNode::new(assets.load("landing/delivery-day.png"))
                    .with_mode(NodeImageMode::Stretch),
                fill(),
            ));
            screen.spawn((Scrim, fill(), scrim(false)));
            screen
                .spawn((
                    Layout::Toolbar,
                    Node {
                        position_type: PositionType::Absolute,
                        top: px(28),
                        left: px(48),
                        right: px(48),
                        column_gap: px(6),
                        justify_content: JustifyContent::End,
                        align_items: AlignItems::Center,
                        ..default()
                    },
                ))
                .with_children(|links| {
                    for (panel, label) in [("help", "How to play"), ("settings", "Settings")] {
                        links
                            .spawn((
                                Utility,
                                button(json!({"action":"ui-panel","panel":panel}), label),
                            ))
                            .insert(Node {
                                min_height: px(40),
                                padding: UiRect::horizontal(px(10)),
                                align_items: AlignItems::Center,
                                justify_content: JustifyContent::Center,
                                ..default()
                            })
                            .observe(clicked);
                    }
                });

            screen
                .spawn((
                    Layout::Copy,
                    Node {
                        position_type: PositionType::Absolute,
                        left: px(48),
                        bottom: percent(12),
                        align_items: AlignItems::Start,
                        flex_direction: FlexDirection::Column,
                        row_gap: px(10),
                        ..default()
                    },
                ))
                .with_children(|copy| {
                    copy.spawn((
                        Typography::Title,
                        Text::new("Paketzustellungs-\nverhinderung"),
                        TextFont::from_font_size(FontSize::Px(64.))
                            .with_font_weight(FontWeight::BLACK),
                        TextColor(PAPER),
                        TextLayout::no_wrap(),
                        LineHeight::RelativeToFont(1.02),
                        LetterSpacing::Px(-1.3),
                    ));
                    copy.spawn((
                        Typography::Subtitle,
                        Text::new("Simulator Deluxe"),
                        TextFont::from_font_size(FontSize::Px(29.))
                            .with_font_weight(FontWeight::MEDIUM),
                        TextColor(Color::srgb(1., 0.73, 0.47)),
                        TextLayout::no_wrap(),
                    ));
                    copy.spawn(button_with_label(
                        json!({"action":"welcome-play"}),
                        "Play".into(),
                        (
                            Text::new("Play"),
                            TextFont::from_font_size(FontSize::Px(20.))
                                .with_font_weight(FontWeight::BOLD),
                            TextColor(COVER_INK),
                        ),
                    ))
                    .insert(Node {
                        width: px(192),
                        height: px(56),
                        margin: UiRect::top(px(15)),
                        align_items: AlignItems::Center,
                        justify_content: JustifyContent::Center,
                        ..default()
                    })
                    .observe(clicked);
                });
        });
}

fn fill() -> Node {
    Node {
        position_type: PositionType::Absolute,
        width: percent(100),
        height: percent(100),
        ..default()
    }
}

fn scrim(portrait: bool) -> BackgroundGradient {
    let (angle, stops) = if portrait {
        (
            LinearGradient::TO_TOP,
            [(0., 0.98), (37., 0.9), (55., 0.18), (77., 0.)],
        )
    } else {
        (
            LinearGradient::TO_RIGHT,
            [(0., 0.98), (32., 0.87), (53., 0.25), (70., 0.)],
        )
    };
    let shade = |alpha| Color::srgba(0.09, 0.07, 0.055, alpha);
    BackgroundGradient(vec![
        LinearGradient::new(
            LinearGradient::TO_BOTTOM,
            vec![
                ColorStop::new(shade(0.62), percent(0)),
                ColorStop::new(shade(0.), percent(23)),
            ],
        )
        .into(),
        LinearGradient::new(
            angle,
            stops
                .into_iter()
                .map(|(at, alpha)| ColorStop::new(shade(alpha), percent(at)))
                .collect(),
        )
        .into(),
    ])
}

pub(super) fn responsive(
    windows: Query<&Window, With<PrimaryWindow>>,
    images: Res<Assets<Image>>,
    mut artwork: Query<&mut ImageNode, With<Artwork>>,
    mut shade: Query<&mut BackgroundGradient, With<Scrim>>,
    mut layout: Query<(&Layout, &mut Node)>,
    mut text: Query<(&Typography, &mut TextFont)>,
) {
    let Ok(window) = windows.single() else { return };
    let viewport = Vec2::new(window.width(), window.height());
    if viewport.min_element() <= 0. {
        return;
    }
    let portrait = viewport.x < 620. || viewport.x < viewport.y * 0.95;
    let short = viewport.y < 500.;
    let inset = if portrait {
        20.
    } else {
        (viewport.x * 0.05).clamp(32., 88.)
    };

    for mut image in &mut artwork {
        if let Some(texture) = images.get(&image.image) {
            let source = texture.size().as_vec2();
            let scale = (viewport / source).max_element();
            let size = (viewport / scale).min(source);
            // Keep the courier and dog in view when the window crops the original frame.
            let focal = Vec2::new(if portrait { 0.63 } else { 0.57 }, 0.51) * source;
            let min = (focal - size * 0.5).clamp(Vec2::ZERO, source - size);
            let rect = Some(Rect::from_corners(min, min + size));
            if image.rect != rect {
                image.rect = rect;
            }
        }
    }
    let gradient = scrim(portrait);
    for mut shade in &mut shade {
        shade.set_if_neq(gradient.clone());
    }
    for (part, mut node) in &mut layout {
        let mut next = node.clone();
        match part {
            Layout::Toolbar => {
                next.left = px(inset);
                next.right = px(inset);
                next.top = px(if short || portrait { 16. } else { 30. });
            }
            Layout::Copy => {
                next.left = px(inset);
                next.bottom = if portrait || short {
                    px(if short { 28. } else { 40. })
                } else {
                    percent(13)
                };
            }
        }
        node.set_if_neq(next);
    }
    let title = if portrait {
        (viewport.x * 0.096).clamp(22., 52.)
    } else {
        (viewport.x * 0.047).clamp(32., if short { 49. } else { 86. })
    };
    for (role, mut font) in &mut text {
        let size = match role {
            Typography::Title => title,
            Typography::Subtitle => (title * 0.49).clamp(17., 36.),
        };
        if font.font_size != FontSize::Px(size) {
            font.font_size = FontSize::Px(size);
        }
    }
}
