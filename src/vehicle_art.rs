//! Shared procedural shapes for the road car and the garage preview.
use crate::vehicles::{Model, Tires, Vehicle};
use bevy::prelude::*;

pub struct Part {
    pub color: Color,
    pub size: Vec2,
    pub at: Vec2,
}
pub fn parts(car: &Vehicle) -> Vec<Part> {
    let size = car.model.size();
    let w = size.x;
    let h = size.y;
    let [r, g, b] = car.paint.rgb();
    let paint = Color::srgb(r, g, b);
    let glass = Color::srgb(0.07, 0.19, 0.25);
    let dark = Color::srgb(0.055, 0.07, 0.09);
    let metal = Color::srgb(0.63, 0.69, 0.73);
    let mut parts = Vec::new();
    let mut rect = |color, width, height, x, y| {
        parts.push(Part {
            color,
            size: Vec2::new(width, height),
            at: Vec2::new(x, y),
        })
    };
    for side in [-1., 1.] {
        for end in [-1., 1.] {
            let tractor = car.model == Model::Fendt1050;
            let tw = if tractor { 17. } else { 8. };
            let th = if tractor && end < 0. { 35. } else { h * 0.24 };
            rect(dark, tw, th, side * (w - tw) * 0.5, end * h * 0.29);
            let stripe = match car.tires {
                Tires::Standard => metal,
                Tires::Sport => Color::srgb(0.96, 0.34, 0.2),
                Tires::Efficiency => Color::srgb(0.3, 0.87, 0.7),
            };
            rect(stripe, 2., th - 4., side * (w - tw) * 0.5, end * h * 0.29);
            if tractor || car.tires == Tires::Sport {
                for offset in [-6., 0., 6.] {
                    rect(
                        metal.with_alpha(0.45),
                        tw - 2.,
                        2.,
                        side * (w - tw) * 0.5,
                        end * h * 0.29 + offset,
                    );
                }
            }
        }
    }
    let body_w = if car.model == Model::Fendt1050 {
        w * 0.5
    } else {
        w - 8.
    };
    match car.model {
        Model::BobbyCar => {
            rect(paint, body_w, h * 0.8, 0., 0.);
            rect(paint, w * 0.8, h * 0.34, 0., h * 0.28);
            rect(dark, w * 0.42, h * 0.22, 0., -h * 0.17);
            rect(Color::WHITE, w * 0.45, 3., 0., h * 0.13);
            rect(Color::WHITE, 3., 9., 0., h * 0.13);
        }
        Model::Fendt1050 => {
            rect(paint, body_w * 0.8, h * 0.9, 0., 0.);
            rect(dark, body_w, h * 0.43, 0., -h * 0.12);
            rect(glass, body_w - 5., h * 0.35, 0., -h * 0.12);
            rect(Color::srgb(0.85, 0.87, 0.8), body_w + 4., 8., 0., -h * 0.24);
            for y in [0.25, 0.3, 0.35] {
                rect(dark, body_w * 0.6, 3., 0., h * y);
            }
        }
        Model::Jesko | Model::M1 => {
            rect(paint, body_w, h * 0.64, 0., -h * 0.1);
            rect(paint, body_w * 0.85, h * 0.28, 0., h * 0.32);
            rect(paint, body_w * 0.65, 7., 0., h * 0.47);
            rect(glass, body_w * 0.68, h * 0.25, 0., h * 0.04);
            for i in 0..4 {
                rect(dark, body_w * 0.6, 3., 0., -h * (0.2 + i as f32 * 0.055));
            }
            if car.model == Model::Jesko {
                rect(dark, w, 7., 0., -h * 0.42);
            }
        }
        _ => {
            rect(paint, body_w, h * 0.86, 0., 0.);
            rect(paint, body_w - 6., h, 0., 0.);
            rect(glass, body_w - 6., h * 0.17, 0., h * 0.19);
            if car.model == Model::F150 {
                rect(dark, body_w - 6., h * 0.44, 0., -h * 0.24);
                for y in [-0.36, -0.24, -0.12] {
                    rect(metal, body_w - 12., 2., 0., h * y);
                }
                rect(metal, w - 8., 4., 0., -h * 0.47);
            } else {
                let rear = if matches!(car.model, Model::Rs6 | Model::Mazda5) {
                    0.25
                } else {
                    0.14
                };
                rect(glass, body_w - 7., h * rear, 0., -h * 0.27);
                if matches!(car.model, Model::Rs6 | Model::Ateca | Model::Xc60) {
                    for side in [-1., 1.] {
                        rect(metal, 2., h * 0.56, side * body_w * 0.38, -h * 0.04);
                    }
                }
                if car.model == Model::Mazda5 {
                    for side in [-1., 1.] {
                        rect(dark, 2., h * 0.32, side * body_w * 0.44, -h * 0.03);
                        rect(metal, 4., 3., side * body_w * 0.4, h * 0.02);
                    }
                }
            }
        }
    }
    if car.model != Model::BobbyCar {
        for side in [-1., 1.] {
            rect(
                Color::srgb(1., 0.96, 0.76),
                7.,
                4.,
                side * body_w * 0.32,
                h * 0.43,
            );
            rect(
                Color::srgb(0.95, 0.14, 0.13),
                6.,
                4.,
                side * body_w * 0.32,
                -h * 0.43,
            );
            if car.model == Model::Xc60 {
                rect(Color::WHITE, 2., 9., side * body_w * 0.32, h * 0.41);
            }
        }
    }
    if car.carbon {
        rect(dark, body_w * 0.62, h * 0.17, 0., h * 0.34);
        rect(dark, w - 3., 5., 0., -h * 0.47);
        for i in 0..4 {
            rect(
                metal.with_alpha(0.5),
                2.,
                h * 0.14,
                (i as f32 - 1.5) * 5.,
                h * 0.34,
            );
        }
    }
    for i in 0..car.motor {
        rect(
            Color::srgb(1., 0.7, 0.2),
            3.,
            7.,
            (f32::from(i) - 0.5) * 5.,
            h * 0.32,
        );
    }
    parts
}
