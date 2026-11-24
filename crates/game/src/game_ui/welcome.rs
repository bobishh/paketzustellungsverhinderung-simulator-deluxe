use super::{Layer, PAPER, button_with_label, clicked};
use bevy::{
    prelude::*,
    text::{FontWeight, LetterSpacing, LineHeight},
    ui::widget::NodeImageMode,
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
pub(super) struct CopyPane;

#[derive(Component)]
pub(super) enum Typography {
    Title,
    Subtitle,
}

const COVER_INK: Color = Color::srgb(0.12, 0.09, 0.06);
const COVER_PAPER: Color = Color::srgb(0.96, 0.92, 0.83);

pub(super) fn spawn(parent: &mut ChildSpawnerCommands, assets: &AssetServer) {
    parent
        .spawn((
            Welcome,
            Layer::Welcome,
            ZIndex(25),
            Node {
                position_type: PositionType::Absolute,
                width: percent(100),
                height: percent(100),
                ..default()
            },
            BackgroundColor(COVER_PAPER),
        ))
        .with_children(|screen| {
            screen.spawn((
                Artwork,
                ImageNode::new(assets.load("landing/chamissoplatz.png"))
                    .with_mode(NodeImageMode::Stretch),
                Node {
                    position_type: PositionType::Absolute,
                    ..default()
                },
                BackgroundColor(Color::srgb(0.30, 0.43, 0.44)),
            ));
            screen
                .spawn((
                    CopyPane,
                    Node {
                        position_type: PositionType::Absolute,
                        align_items: AlignItems::Start,
                        justify_content: JustifyContent::Center,
                        flex_direction: FlexDirection::Column,
                        ..default()
                    },
                ))
                .with_children(|copy| {
                    copy.spawn((
                        Typography::Title,
                        Text::new("Paketzustellungs-\nverhinderung"),
                        TextFont::from_font_size(FontSize::Px(60.))
                            .with_font_weight(FontWeight::BLACK),
                        TextColor(COVER_INK),
                        TextLayout::no_wrap(),
                        LineHeight::RelativeToFont(1.0),
                        LetterSpacing::Px(-0.8),
                    ));
                    copy.spawn((
                        Typography::Subtitle,
                        Text::new("Simulator Deluxe"),
                        TextFont::from_font_size(FontSize::Px(26.))
                            .with_font_weight(FontWeight::MEDIUM),
                        TextColor(Color::srgb(0.42, 0.24, 0.12)),
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
                        width: px(180),
                        height: px(50),
                        margin: UiRect::top(px(8)),
                        align_items: AlignItems::Center,
                        justify_content: JustifyContent::Center,
                        ..default()
                    })
                    .observe(clicked);
                    copy.spawn(Node {
                        column_gap: px(8),
                        margin: UiRect::top(px(3)),
                        ..default()
                    })
                    .with_children(|links| {
                        for (panel, label) in [("help", "How to play"), ("settings", "Settings")] {
                            links
                                .spawn((
                                    Utility,
                                    button_with_label(
                                        json!({"action":"ui-panel","panel":panel}),
                                        label.into(),
                                        (
                                            Text::new(label),
                                            TextFont::from_font_size(FontSize::Px(14.)),
                                            TextColor(PAPER),
                                            TextLayout::no_wrap(),
                                        ),
                                    ),
                                ))
                                .insert(Node {
                                    height: px(36),
                                    padding: UiRect::horizontal(px(11)),
                                    align_items: AlignItems::Center,
                                    justify_content: JustifyContent::Center,
                                    ..default()
                                })
                                .observe(clicked);
                        }
                    });
                });
        });
}

pub(super) fn responsive(
    windows: Query<&Window, With<PrimaryWindow>>,
    images: Res<Assets<Image>>,
    mut artwork: Query<(&mut ImageNode, &mut Node), With<Artwork>>,
    mut layout: Query<&mut Node, (With<CopyPane>, Without<Artwork>)>,
    mut text: Query<(&Typography, &mut TextFont)>,
) {
    let Ok(window) = windows.single() else { return };
    let viewport = Vec2::new(window.width(), window.height());
    if viewport.min_element() <= 0. {
        return;
    }
    let portrait = viewport.x < 620. || viewport.x < viewport.y * 0.95;
    let pane = if portrait {
        Vec2::new(
            viewport.x,
            (viewport.y * 0.5).max(244.).min(viewport.y * 0.68),
        )
    } else {
        Vec2::new(viewport.x * 0.43, viewport.y)
    };
    let art_origin = if portrait {
        Vec2::ZERO
    } else {
        Vec2::new(pane.x, 0.)
    };
    let art_size = if portrait {
        Vec2::new(viewport.x, viewport.y - pane.y)
    } else {
        Vec2::new(viewport.x - pane.x, viewport.y)
    };
    let inset = if portrait {
        (viewport.x * 0.055).clamp(15., 35.)
    } else {
        (viewport.x * 0.033).clamp(22., 70.)
    };
    for (mut image, mut node) in &mut artwork {
        let mut next = node.clone();
        next.left = px(art_origin.x);
        next.top = px(art_origin.y);
        next.width = px(art_size.x);
        next.height = px(art_size.y);
        node.set_if_neq(next);
        if let Some(texture) = images.get(&image.image) {
            let source = texture.size().as_vec2();
            let scale = (art_size / source).max_element();
            let size = (art_size / scale).min(source);
            let focal = Vec2::new(0.54, 0.59) * source;
            let min = (focal - size * 0.5).clamp(Vec2::ZERO, source - size);
            let rect = Some(Rect::from_corners(min, min + size));
            if image.rect != rect {
                image.rect = rect;
            }
        }
    }
    for mut node in &mut layout {
        let mut next = node.clone();
        next.left = px(0);
        next.bottom = px(0);
        next.width = px(pane.x);
        next.height = px(pane.y);
        next.padding = UiRect::horizontal(px(inset));
        next.row_gap = px(if viewport.y < 500. { 4. } else { 8. });
        node.set_if_neq(next);
    }
    let title = ((pane.x - 2. * inset) / 9.9)
        .min(pane.y * 0.16)
        .clamp(15., 76.);
    for (role, mut font) in &mut text {
        let size = match role {
            Typography::Title => title,
            Typography::Subtitle => (title * 0.52).clamp(16., 32.),
        };
        font.set_if_neq(TextFont {
            font_size: FontSize::Px(size),
            ..font.clone()
        });
    }
}
