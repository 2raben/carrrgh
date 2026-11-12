//! Distance, three hearts, held shield, boost gauge, and restart instructions.
use crate::run::Run;
use crate::visuals::palette;
use bevy::prelude::*;

#[derive(Component)]
struct Progress;
#[derive(Component)]
struct FinishCue;
#[derive(Component)]
struct BoostBar;
#[derive(Component)]
struct Heart(u8);
#[derive(Component)]
struct ShieldIcon;

pub struct HudPlugin;
impl Plugin for HudPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Startup, spawn_hud)
            .add_systems(Update, show_run);
    }
}

fn text(content: impl Into<String>, size: f32) -> impl Bundle {
    (
        Text::new(content),
        TextFont {
            font_size: FontSize::Px(size),
            ..default()
        },
        TextColor(palette::TEXT),
    )
}

fn panel(node: Node) -> impl Bundle {
    (
        Node {
            position_type: PositionType::Absolute,
            padding: UiRect::all(Val::Px(18.0)),
            ..node
        },
        BackgroundColor(palette::PANEL),
    )
}

// Geometry avoids depending on optional heart glyphs in the bundled font.
fn heart(index: u8) -> impl Bundle {
    let part = |left, top, round| {
        (
            Heart(index),
            Node {
                position_type: PositionType::Absolute,
                left: Val::Px(left),
                top: Val::Px(top),
                width: Val::Px(18.0),
                height: Val::Px(18.0),
                border_radius: if round {
                    BorderRadius::MAX
                } else {
                    BorderRadius::ZERO
                },
                ..default()
            },
            BackgroundColor(palette::HEART),
        )
    };
    (
        Node {
            width: Val::Px(32.0),
            height: Val::Px(32.0),
            ..default()
        },
        children![
            (
                part(7.0, 10.0, false),
                UiTransform::from_rotation(Rot2::radians(std::f32::consts::FRAC_PI_4))
            ),
            part(0.6, 3.6, true),
            part(13.4, 3.6, true),
        ],
    )
}

fn shield_icon() -> impl Bundle {
    let part = |left, top, width, height| {
        (
            ShieldIcon,
            Node {
                position_type: PositionType::Absolute,
                left: Val::Px(left),
                top: Val::Px(top),
                width: Val::Px(width),
                height: Val::Px(height),
                ..default()
            },
            BackgroundColor(palette::DIM),
        )
    };
    (
        Node {
            width: Val::Px(32.0),
            height: Val::Px(32.0),
            margin: UiRect::left(Val::Px(8.0)),
            ..default()
        },
        children![
            part(5.0, 3.0, 22.0, 14.0),
            (
                part(8.0, 9.0, 16.0, 16.0),
                UiTransform::from_rotation(Rot2::radians(std::f32::consts::FRAC_PI_4))
            ),
        ],
    )
}

fn spawn_hud(mut commands: Commands) {
    commands.spawn((
        panel(Node {
            top: Val::Px(20.0),
            left: Val::Px(20.0),
            width: Val::Px(230.0),
            flex_direction: FlexDirection::Column,
            row_gap: Val::Px(15.0),
            ..default()
        }),
        children![
            text("CAR DRIVE", 28.0),
            text("FIND YOUR WAY", 13.0),
            (
                Node {
                    column_gap: Val::Px(12.0),
                    ..default()
                },
                children![heart(0), heart(1), heart(2), shield_icon()]
            ),
            (Progress, text("", 20.0)),
            text("Watch the turn signals.\nTrucks from 4000 m.", 14.0),
            text("BOOST / SPACE", 15.0),
            (
                Node {
                    width: Val::Percent(100.0),
                    height: Val::Px(12.0),
                    ..default()
                },
                BackgroundColor(Color::srgb(0.2, 0.26, 0.3)),
                children![(
                    BoostBar,
                    Node {
                        width: Val::Percent(100.0),
                        height: Val::Percent(100.0),
                        ..default()
                    },
                    BackgroundColor(palette::BLUE)
                ),]
            ),
            text("Release to recharge.\nBoost adds speed, not armor.", 14.0),
            text(
                "Pick up hearts to heal.\nA shield blocks the next hit.",
                14.0
            ),
        ],
    ));
    commands.spawn((panel(Node { bottom: Val::Px(20.0), left: Val::Px(20.0), ..default() }), text("A / D or Left / Right   Steer\nSPACE   Boost     R   Restart     ESC   Quit\nController: Stick / D-pad / South / Select", 14.0)));
    commands.spawn((
        FinishCue,
        panel(Node {
            top: Val::Percent(38.0),
            width: Val::Percent(100.0),
            justify_content: JustifyContent::Center,
            ..default()
        }),
        text("", 30.0),
        TextLayout::justify(Justify::Center),
        Visibility::Hidden,
    ));
}

fn show_run(
    run: Res<Run>,
    mut progress: Query<&mut Text, (With<Progress>, Without<FinishCue>)>,
    mut finish: Query<(&mut Text, &mut Visibility), With<FinishCue>>,
    mut bars: Query<&mut Node, With<BoostBar>>,
    mut hearts: Query<(&Heart, &mut BackgroundColor), Without<ShieldIcon>>,
    mut shield: Query<&mut BackgroundColor, With<ShieldIcon>>,
) {
    for mut text in &mut progress {
        text.0 = format!(
            "{:06.0} m\nLEVEL {} / 10",
            run.distance / 10.0,
            run.difficulty_step() + 1
        );
    }
    for mut node in &mut bars {
        node.width = Val::Percent(run.energy * 100.0);
    }
    for (heart, mut color) in &mut hearts {
        color.0 = if heart.0 < run.lives {
            palette::HEART
        } else {
            palette::DIM
        };
    }
    for mut color in &mut shield {
        color.0 = if run.shield {
            palette::SHIELD
        } else {
            palette::DIM
        };
    }
    for (mut text, mut visibility) in &mut finish {
        if run.finished {
            text.0 = format!(
                "END OF THE ROAD\n{:.0} metres\nR / Select to drive again",
                run.distance / 10.0
            );
            *visibility = Visibility::Inherited;
        } else {
            *visibility = Visibility::Hidden;
        }
    }
}
