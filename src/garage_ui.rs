//! Keyboard, controller and pointer share the same garage intents.
use crate::bindings::{ACTIONS, BindingIntent, Bindings};
use crate::garage::{Garage, Intent, Tab};
use crate::profile::{Account, ProfilePhase};
use crate::vehicles::*;
use crate::visuals::palette;
use bevy::prelude::*;
use bevy::ui_widgets::{Activate, Button};

#[derive(Component)]
struct GarageRoot;
#[derive(Component)]
struct MenuButton(usize);
#[derive(Resource, Default)]
pub struct MenuActions(pub Vec<Intent>);

pub struct GarageUiPlugin;
impl Plugin for GarageUiPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<MenuActions>().add_systems(
            Update,
            (draw, highlight)
                .chain()
                .after(crate::garage::update_garage),
        );
    }
}
fn text(label: impl Into<String>, size: f32) -> impl Bundle {
    crate::hud::colored_text(label, size, palette::TEXT)
}
fn button(
    parent: &mut ChildSpawnerCommands,
    actions: &mut MenuActions,
    label: String,
    intent: Intent,
) {
    let index = actions.0.len();
    actions.0.push(intent.clone());
    parent
        .spawn((
            Button,
            MenuButton(index),
            Node {
                width: Val::Percent(100.),
                min_height: Val::Px(30.),
                padding: UiRect::axes(Val::Px(8.), Val::Px(5.)),
                border_radius: BorderRadius::all(Val::Px(5.)),
                ..default()
            },
            BackgroundColor(palette::DIM),
            children![(text(label, 14.),)],
        ))
        .observe(move |_: On<Activate>, mut garage: ResMut<Garage>| {
            garage.enqueue(intent.clone());
        });
}
fn column(width: f32) -> impl Bundle {
    (
        Node {
            width: Val::Percent(width),
            flex_direction: FlexDirection::Column,
            padding: UiRect::all(Val::Px(12.)),
            row_gap: Val::Px(7.),
            ..default()
        },
        BackgroundColor(Color::srgb(0.06, 0.09, 0.13)),
    )
}

fn draw(
    mut commands: Commands,
    mut garage: ResMut<Garage>,
    profile: (Res<Account>, Res<State<ProfilePhase>>),
    roots: Query<Entity, With<GarageRoot>>,
    mut actions: ResMut<MenuActions>,
    mut last: Local<Option<(u64, u64, bool, bool)>>,
    bindings: Res<Bindings>,
) {
    let (account, phase) = profile;
    let visible = garage.open && *phase.get() == ProfilePhase::Playing;
    let key = (
        garage.revision,
        bindings.revision,
        visible,
        account.saving(),
    );
    if *last == Some(key) {
        return;
    }
    *last = Some(key);
    for root in &roots {
        commands.entity(root).despawn();
    }
    actions.0.clear();
    if !visible {
        return;
    }
    if bindings.open {
        draw_bindings(&mut commands, &mut actions, &bindings);
        return;
    }
    let selected = garage.selected_car();
    let balance = account.profile.as_ref().map_or(0, |p| p.coins);
    commands.spawn((GarageRoot, GlobalZIndex(20), Node {
        position_type: PositionType::Absolute, width: Val::Percent(100.), height: Val::Percent(100.),
        flex_direction: FlexDirection::Column, padding: UiRect::all(Val::Px(16.)), row_gap: Val::Px(10.), ..default()
    }, BackgroundColor(Color::srgb(0.025, 0.04, 0.065))))
    .with_children(|root| {
        root.spawn(text(format!("{}    /    {balance} COINS", if account.online { "PRIVATE MARKET" } else { "GARAGE" }), 27.));
        root.spawn((Node { column_gap: Val::Px(10.), ..default() },)).with_children(|tabs| {
            for (tab, label) in [(Tab::Owned, "My vehicles"), (Tab::Catalog, "Showroom"), (Tab::Market, "Marketplace")] {
                tabs.spawn(Node { width: Val::Percent(25.), ..default() }).with_children(|p| {
                    button(p, &mut actions, format!("{} {label}", if garage.tab == tab { ">" } else { "" }), Intent::Tab(tab));
                });
            }
            tabs.spawn(Node { width: Val::Percent(25.), ..default() }).with_children(|p| {
                button(p, &mut actions, "Drive / Start".into(), Intent::Start);
            });
        });
        button(root, &mut actions, "Key bindings".into(), Intent::Bindings(BindingIntent::Open));
        root.spawn(Node { flex_grow: 1., min_height: Val::Px(0.), column_gap: Val::Px(10.), ..default() }).with_children(|panels| {
            panels.spawn(column(29.)).with_children(|list| {
                list.spawn(text(match garage.tab { Tab::Owned => "YOUR COLLECTION", Tab::Catalog => "CHOOSE YOUR NEXT CAR", Tab::Market => "GUARANTEED BUYBACK" }, 15.));
                let cars: Vec<_> = match garage.tab {
                    Tab::Catalog => MODELS.iter().map(|m| Vehicle::new("preview".into(), *m)).collect(),
                    Tab::Owned => garage.data.as_ref().map_or_else(Vec::new, |d| d.vehicles.clone()),
                    Tab::Market => garage.data.as_ref().map_or_else(Vec::new, |d| d.listings.clone()),
                };
                let first = garage.selected / 8 * 8;
                for (i, car) in cars.iter().enumerate().skip(first).take(8) {
                    let suffix = match garage.tab {
                        Tab::Catalog => format!("{} coins", car.model.price()),
                        Tab::Market => format!("{} coins", car.market_price()),
                        Tab::Owned => if garage.data.as_ref().is_some_and(|d| d.active == car.id) { "Active".into() } else { car.paint.name().into() },
                    };
                    button(list, &mut actions, format!("{} {}\n{suffix}", if i == garage.selected { ">" } else { "" }, car.model.name()), Intent::Item(i));
                }
                if first > 0 { button(list, &mut actions, "Previous vehicles".into(), Intent::Item(first - 8)); }
                if first + 8 < cars.len() { button(list, &mut actions, "More vehicles".into(), Intent::Item(first + 8)); }
                if garage.tab == Tab::Market {
                    let page = garage.data.as_ref().map_or(0, |d| d.page);
                    button(list, &mut actions, "Refresh market".into(), Intent::Refresh(page));
                    if page > 0 { button(list, &mut actions, "Previous market page".into(), Intent::Refresh(page - 1)); }
                    if garage.data.as_ref().is_some_and(|d| d.more) { button(list, &mut actions, "Next market page".into(), Intent::Refresh(page + 1)); }
                    if cars.is_empty() { list.spawn(text("No vehicles listed yet.\nSell a spare car for instant coins.", 14.)); }
                }
            });
            panels.spawn(column(36.)).with_children(|detail| {
                if let Some(car) = &selected {
                    let mut preview = car.clone();
                    if let Some((GarageRequest { action: GarageAction::Configure { change, .. }, .. }, _)) = &garage.confirmation {
                        let _ = preview.configure(change);
                    }
                    detail.spawn(text(car.model.name(), 23.));
                    detail.spawn(Node { width: Val::Percent(100.), height: Val::Px(175.), ..default() }).with_children(|drawing| {
                        let scale = 1.4;
                        for part in crate::vehicle_art::parts(&preview) {
                            drawing.spawn((Node { position_type: PositionType::Absolute,
                                left: Val::Px(125. + (part.at.x - part.size.x * 0.5) * scale),
                                top: Val::Px(85. - (part.at.y + part.size.y * 0.5) * scale),
                                width: Val::Px(part.size.x * scale), height: Val::Px(part.size.y * scale), ..default()
                            }, BackgroundColor(part.color)));
                        }
                    });
                    let t = preview.tuning();
                    let current = garage.data.as_ref().and_then(|d| d.vehicles.iter().find(|c| c.id == d.active)).map(Vehicle::tuning).unwrap_or_default();
                    detail.spawn(text(format!("{}  /  {} tires\nMotor {}  /  Carbon {}\n\nCruise  {:.0}  ({:+.0})\nBoost  {:.0}  ({:+.0})\nSteering  {:.0}  ({:+.0})\nBoost duration  {:.1}s\nRecharge  {:.1}s\nSize  {:.0} x {:.0}\n\nCompared with your active car.\nThree hearts with every vehicle.", preview.paint.name(), preview.tires.name(), preview.motor,
                        if preview.carbon { "on" } else { "off" }, t.cruise_speed, t.cruise_speed-current.cruise_speed,
                        t.boost_speed, t.boost_speed-current.boost_speed, t.steer_speed, t.steer_speed-current.steer_speed,
                        1./t.boost_drain, 1./t.boost_recharge, t.size.x, t.size.y), 15.));
                } else { detail.spawn(text("Your garage is loading...", 18.)); }
            });
            panels.spawn(column(35.)).with_children(|workshop| {
                workshop.spawn(text(if garage.tab == Tab::Owned { "WORKSHOP" } else { "PURCHASE" }, 17.));
                if let Some(car) = &selected {
                    match garage.tab {
                        Tab::Catalog => button(workshop, &mut actions, format!("Buy for {} coins", car.model.price()), Intent::Action(GarageAction::Buy(car.model))),
                        Tab::Market => button(workshop, &mut actions, format!("Buy for {} coins", car.market_price()), Intent::Action(GarageAction::BuyListing(car.id.clone()))),
                        Tab::Owned => {
                            button(workshop, &mut actions, "Set as active vehicle".into(), Intent::Action(GarageAction::Select(car.id.clone())));
                            let paint = PAINTS[(PAINTS.iter().position(|p| *p == car.paint).unwrap_or(0) + 1) % PAINTS.len()];
                            let configure = |change| Intent::Action(GarageAction::Configure { vehicle: car.id.clone(), change });
                            button(workshop, &mut actions, format!("Color: {} > {} (free)", car.paint.name(), paint.name()), configure(Change::Paint(paint)));
                            button(workshop, &mut actions, "Use standard tires (free)".into(), configure(Change::Tires(Tires::Standard)));
                            for upgrade in UPGRADES {
                                let cost = if car.owns(upgrade) { "equip free".into() } else { format!("{} coins", upgrade.price(car.model)) };
                                button(workshop, &mut actions, format!("{} / {cost}", upgrade.name()), configure(Change::Buy(upgrade)));
                            }
                            button(workshop, &mut actions, "Use standard motor (free)".into(), configure(Change::Motor(0)));
                            if car.owns(Upgrade::Carbon) { button(workshop, &mut actions, "Remove body kit (keep owned)".into(), configure(Change::Carbon(false))); }
                            button(workshop, &mut actions, format!("Sell now / receive {} coins", car.sale_price()), Intent::Action(GarageAction::Sell(car.id.clone())));
                        }
                    }
                }
            });
        });
        let status = if garage.busy() { "Saving / loading..." } else if account.saving() { "Waiting for drive coins..." }
            else { garage.problem.as_deref().or(account.problem.as_deref()).unwrap_or(&garage.notice) };
        root.spawn(text(status, 14.));
        if let Some((_, quote)) = &garage.confirmation {
            root.spawn(text(quote.clone(), 16.));
            root.spawn(Node { column_gap: Val::Px(10.), ..default() }).with_children(|row| {
                button(row, &mut actions, "Confirm".into(), Intent::Confirm);
                button(row, &mut actions, "Cancel".into(), Intent::Cancel);
            });
        } else if garage.problem.is_some() || account.problem.is_some() {
            button(root, &mut actions, "Retry / refresh safely".into(), Intent::Retry);
        }
        root.spawn(text("Up / Down: focus    Enter / South: choose    Tab / Shoulder: section\nEsc / East: cancel    G / Start: garage after a drive", 13.));
    });
    if garage.focus == usize::MAX {
        garage.focus = actions.0.len().saturating_sub(2);
    }
}

fn highlight(
    garage: Res<Garage>,
    actions: Res<MenuActions>,
    mut buttons: Query<(&MenuButton, &mut BackgroundColor)>,
) {
    let focus = garage.focus.min(actions.0.len().saturating_sub(1));
    for (button, mut color) in &mut buttons {
        color.0 = if button.0 == focus {
            Color::srgb(0.12, 0.42, 0.55)
        } else {
            Color::srgb(0.13, 0.18, 0.23)
        };
    }
}

fn draw_bindings(commands: &mut Commands, actions: &mut MenuActions, bindings: &Bindings) {
    commands
        .spawn((
            GarageRoot,
            GlobalZIndex(21),
            Node {
                position_type: PositionType::Absolute,
                width: Val::Percent(100.),
                height: Val::Percent(100.),
                flex_direction: FlexDirection::Column,
                padding: UiRect::all(Val::Px(24.)),
                row_gap: Val::Px(10.),
                ..default()
            },
            BackgroundColor(Color::srgb(0.025, 0.04, 0.065)),
        ))
        .with_children(|root| {
            root.spawn(text("KEY BINDINGS", 27.));
            root.spawn(text(
                "Physical key positions (letters use US labels). G and Esc are reserved.
Up / Down: focus   Enter: choose   Esc: back",
                14.,
            ));
            let mut add = |parent: &mut ChildSpawnerCommands, label: String, intent| {
                button(parent, actions, label, Intent::Bindings(intent));
            };
            if bindings.capture.is_some() {
                root.spawn(text(
                    format!("{}: press a new key", bindings.selected.name()),
                    23.,
                ));
                root.spawn(text(&bindings.notice, 16.));
                add(root, "Cancel capture".into(), BindingIntent::Cancel);
                return;
            }
            root.spawn(Node {
                column_gap: Val::Px(20.),
                ..default()
            })
            .with_children(|panels| {
                panels.spawn(column(50.)).with_children(|list| {
                    for action in ACTIONS {
                        add(
                            list,
                            format!(
                                "{}{}: {}",
                                if bindings.selected == action {
                                    "> "
                                } else {
                                    ""
                                },
                                action.name(),
                                bindings.map.labels(action)
                            ),
                            BindingIntent::Select(action),
                        );
                    }
                });
                panels.spawn(column(50.)).with_children(|detail| {
                    detail.spawn(text(bindings.selected.name(), 20.));
                    for (i, key) in bindings.map.keys(bindings.selected).iter().enumerate() {
                        add(
                            detail,
                            format!("Replace {}", crate::bindings::label(key)),
                            BindingIntent::Capture(Some(i)),
                        );
                        if bindings.map.keys(bindings.selected).len() > 1 {
                            add(
                                detail,
                                format!("Remove {}", crate::bindings::label(key)),
                                BindingIntent::Remove(i),
                            );
                        }
                    }
                    if bindings.map.keys(bindings.selected).len() < 4 {
                        add(
                            detail,
                            "Add alternative key".into(),
                            BindingIntent::Capture(None),
                        );
                    }
                });
            });
            root.spawn(text(&bindings.notice, 15.));
            if bindings.save_error {
                add(root, "Retry saving".into(), BindingIntent::Retry);
            }
            add(root, "Restore defaults".into(), BindingIntent::Reset);
            add(root, "Back to garage".into(), BindingIntent::Close);
        });
}
