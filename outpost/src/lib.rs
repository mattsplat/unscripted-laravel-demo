//! Outpost: a team shooter on a fixed map, drawn with WebGL 2 (sun shadows, textures made in
//! the shader). The game server (outpost/server) decides everything; this shows it.
//!
//! - Your own moves and shots run here at once, through the same rules as the server's
//!   (sim.rs). When the server's word on where you are comes (30 times a second), it's taken,
//!   and the moves it hasn't run yet are played again on top of it.
//! - Everyone else is drawn 100 ms in the past, between the two snapshots around then, so they
//!   move smoothly. Shots say which moment that was, and the server checks them against it.
//!
//! The page talks to it through window:
//! - in: `start(canvasId, boxes, url)` (the map's boxes, flat x0, y0, z0, x1, y1, z1, material...;
//!   the server's WebSocket), `join(name)`;
//! - out: CustomEvents `outpost-status` ("open" / "closed"), `outpost-welcome` ({id, team}, JSON),
//!   `outpost-roster` ([{id, name, team, kills, deaths, alive}], JSON), `outpost-hp` [hp, alive, seconds left looking around after spawning],
//!   `outpost-kill` ({killer, victim, head, gun}, JSON), `outpost-sound` [kind, volume, gun] and
//!   `outpost-scores` [1/0].
//!
//! Controls: click to play (pointer lock), WASD to move, shift to crouch, space to jump, mouse to
//! shoot, right button to scope (the sniper rifle), 1 / 2 / 3, Q or the wheel to change guns,
//! R to reload, Tab for the scores.

#[allow(dead_code)] // some rules are only the server's (respawning, damage)
mod sim;

use sim::*;
use std::cell::RefCell;
use std::collections::{HashMap, HashSet, VecDeque};
use std::rc::Rc;
use wasm_bindgen::prelude::*;
use wasm_bindgen::JsCast;
use web_sys::{
    HtmlCanvasElement, HtmlElement, KeyboardEvent, MessageEvent, MouseEvent, WebGl2RenderingContext as GL,
    WebGlBuffer, WebGlFramebuffer, WebGlProgram, WebGlShader, WebGlTexture, WebGlUniformLocation,
    WebGlVertexArrayObject, WebSocket,
};

const SHADOW: i32 = 2048;
const FLOATS: usize = 14; // per vertex: position, normal, face uv, face size, material, colour
/// Everyone else is drawn this many ticks in the past: 100 ms, three snapshots.
const INTERP: f32 = 6.0;

// Materials: the map's (0 sand, 1 plaster, 2 stone, 3 crate, 4 concrete, 5 dark wood, 6 brick),
// then a plain colour, and a colour that glows (no light, no shadow).
const FLAT: f32 = 10.0;
const GLOW: f32 = 11.0;
/// A plain colour with a shine (metal, lenses, pads), and skin.
const SHINY: f32 = 12.0;
const SKIN_MAT: f32 = 13.0;

// Sounds, for the page: `outpost-sound` [kind, volume].
const SOUND_SHOT: f64 = 0.0;
const SOUND_FAR_SHOT: f64 = 1.0;
const SOUND_HIT: f64 = 2.0;
const SOUND_RELOAD: f64 = 3.0;
const SOUND_DRAW: f64 = 4.0;
const SOUND_BOLT: f64 = 5.0;
/// The field of view, unscoped.
const FOV: f32 = 74.0 * std::f32::consts::PI / 180.0;

type Mat = [f32; 16];

/// `p` turned by `a` about the x axis through `pivot` (positive tips +z up).
fn rot_x(p: V3, pivot: V3, a: f32) -> V3 {
    let (s, c) = a.sin_cos();
    let (y, z) = (p[1] - pivot[1], p[2] - pivot[2]);
    [p[0], pivot[1] + y * c + z * s, pivot[2] + z * c - y * s]
}

/// `p` turned by `a` about the z axis through the origin.
fn rot_z(p: V3, a: f32) -> V3 {
    let (s, c) = a.sin_cos();
    [p[0] * c - p[1] * s, p[0] * s + p[1] * c, p[2]]
}

fn perspective(fovy: f32, aspect: f32, near: f32, far: f32) -> Mat {
    let f = 1.0 / (fovy / 2.0).tan();
    let mut m = [0.0; 16];
    m[0] = f / aspect;
    m[5] = f;
    m[10] = (far + near) / (near - far);
    m[11] = -1.0;
    m[14] = 2.0 * far * near / (near - far);
    m
}

fn ortho(r: f32, near: f32, far: f32) -> Mat {
    let mut m = [0.0; 16];
    m[0] = 1.0 / r;
    m[5] = 1.0 / r;
    m[10] = -2.0 / (far - near);
    m[14] = -(far + near) / (far - near);
    m[15] = 1.0;
    m
}

fn look(eye: V3, f: V3) -> Mat {
    let s = normalize(cross(f, [0.0, 1.0, 0.0]));
    let u = cross(s, f);
    [
        s[0], u[0], -f[0], 0.0, //
        s[1], u[1], -f[1], 0.0, //
        s[2], u[2], -f[2], 0.0, //
        -dot(s, eye), -dot(u, eye), dot(f, eye), 1.0,
    ]
}

fn multiply(a: &Mat, b: &Mat) -> Mat {
    let mut r = [0.0; 16];
    for c in 0..4 {
        for row in 0..4 {
            r[c * 4 + row] = (0..4).map(|k| a[k * 4 + row] * b[c * 4 + k]).sum();
        }
    }
    r
}

// ---- Geometry -------------------------------------------------------------------------------

/// A box's faces, as corners (0 min, 1 max per axis), counter-clockwise from outside:
/// +x, -x, +y, -y, +z, -z.
const FACES: [[[u8; 3]; 4]; 6] = [
    [[1, 0, 0], [1, 1, 0], [1, 1, 1], [1, 0, 1]],
    [[0, 0, 1], [0, 1, 1], [0, 1, 0], [0, 0, 0]],
    [[0, 1, 0], [0, 1, 1], [1, 1, 1], [1, 1, 0]],
    [[0, 0, 0], [1, 0, 0], [1, 0, 1], [0, 0, 1]],
    [[0, 0, 1], [1, 0, 1], [1, 1, 1], [0, 1, 1]],
    [[1, 0, 0], [0, 0, 0], [0, 1, 0], [1, 1, 0]],
];

fn corner(c: [u8; 3], min: V3, max: V3) -> V3 {
    [
        if c[0] == 1 { max[0] } else { min[0] },
        if c[1] == 1 { max[1] } else { min[1] },
        if c[2] == 1 { max[2] } else { min[2] },
    ]
}

/// A map box, with each face's own coordinates (for crate frames and the like).
fn map_box(v: &mut Vec<f32>, min: V3, max: V3, mat: f32) {
    for (f, corners) in FACES.iter().enumerate() {
        let axis = f / 2;
        let (u, w) = match axis {
            0 => (2, 1),
            1 => (0, 2),
            _ => (0, 1),
        };
        let mut n = [0.0; 3];
        n[axis] = if f % 2 == 0 { 1.0 } else { -1.0 };
        let p = corners.map(|c| corner(c, min, max));
        for i in [0, 1, 2, 0, 2, 3] {
            let q = p[i];
            v.extend_from_slice(&[
                q[0], q[1], q[2], n[0], n[1], n[2],
                q[u] - min[u], q[w] - min[w], max[u] - min[u], max[w] - min[w],
                mat, 1.0, 1.0, 1.0,
            ]);
        }
    }
}

/// A box from `min` to `max` in some body's space, put in the world by `at`. Each face keeps its
/// own coordinates, in the body's metres, so the shader can soften its edges and weave its cloth.
fn part(v: &mut Vec<f32>, min: V3, max: V3, mat: f32, col: V3, at: &dyn Fn(V3) -> V3) {
    for (f, corners) in FACES.iter().enumerate() {
        let (u, w) = match f / 2 {
            0 => (2, 1),
            1 => (0, 2),
            _ => (0, 1),
        };
        let local = corners.map(|c| corner(c, min, max));
        let p = local.map(|q| at(q));
        let n = normalize(cross(sub(p[1], p[0]), sub(p[3], p[0])));
        for i in [0, 1, 2, 0, 2, 3] {
            let (q, l) = (p[i], local[i]);
            v.extend_from_slice(&[
                q[0], q[1], q[2], n[0], n[1], n[2],
                l[u] - min[u], l[w] - min[w], max[u] - min[u], max[w] - min[w],
                mat, col[0], col[1], col[2],
            ]);
        }
    }
}

/// A bar from `a` to `b` (in some body's space), `half` wide across and up, put in the world by
/// `at`: for limbs and anything else at an angle.
fn limb(v: &mut Vec<f32>, a: V3, b: V3, half: [f32; 2], mat: f32, col: V3, at: &dyn Fn(V3) -> V3) {
    let d = sub(b, a);
    let len = dot(d, d).sqrt();
    if len < 1e-4 {
        return;
    }
    let f = mul(d, 1.0 / len);
    let side = normalize(if f[1].abs() > 0.99 { cross(f, [0.0, 0.0, 1.0]) } else { cross(f, [0.0, 1.0, 0.0]) });
    let up = cross(f, side); // so side, up, f turn the right way, and faces point out
    let place = |p: V3| at(add(a, add(add(mul(side, p[0]), mul(up, p[1])), mul(f, p[2]))));
    part(v, [-half[0], -half[1], 0.0], [half[0], half[1], len], mat, col, &place);
}

/// A thin glowing rod from `a` to `b`.
fn rod(v: &mut Vec<f32>, a: V3, b: V3, width: f32, col: V3) {
    limb(v, a, b, [width, width], GLOW, col, &|p| p);
}

// ---- People ---------------------------------------------------------------------------------

struct Kit {
    shirt: V3,
    pants: V3,
    vest: V3,
    pouch: V3,
    hat: V3,
    /// The rifle's furniture: wood for the raiders, black polymer for the guards.
    furniture: V3,
    /// The team's colour (as on the HUD), on armbands: to tell sides apart at a glance.
    accent: V3,
}

const KITS: [Kit; 2] = [
    // Raiders: warm desert tan and brown, a rust-red head wrap and goggles, orange armbands.
    Kit {
        shirt: [0.66, 0.5, 0.3],
        pants: [0.5, 0.38, 0.24],
        vest: [0.36, 0.27, 0.16],
        pouch: [0.44, 0.33, 0.2],
        hat: [0.62, 0.22, 0.13],
        furniture: [0.42, 0.26, 0.14], // wood
        accent: [0.95, 0.55, 0.1],
    },
    // Guards: deep blue, a blue-grey helmet with a headset, bright blue armbands.
    Kit {
        shirt: [0.13, 0.24, 0.5],
        pants: [0.1, 0.16, 0.34],
        vest: [0.07, 0.1, 0.2],
        pouch: [0.1, 0.14, 0.27],
        hat: [0.16, 0.24, 0.38],
        furniture: [0.08, 0.08, 0.085],
        accent: [0.3, 0.62, 1.0],
    },
];
const SKIN: V3 = [0.74, 0.56, 0.43];
const BOOTS: V3 = [0.12, 0.1, 0.08];
const STRAP: V3 = [0.09, 0.09, 0.08];
const METAL: V3 = [0.13, 0.13, 0.14];
const LENS: V3 = [0.05, 0.06, 0.07];
const FLASH: V3 = [1.0, 0.82, 0.45];

const HIP: f32 = 0.92;
const KNEE: f32 = 0.5;
const SHOULDER: V3 = [0.0, 1.36, 0.0];
const NECK: V3 = [0.0, 1.46, 0.0];
/// The sniper rifle's furniture: olive.
const OLIVE: V3 = [0.25, 0.28, 0.19];
/// Each gun's muzzle, in your own view's gun space.
const MUZZLES: [V3; 3] = [[0.0, 0.009, 0.69], [0.0, 0.03, 0.17], [0.0, 0.006, 0.98]];

/// Where someone else's gun's muzzle is, in their own space.
fn remote_muzzle(gun: u8) -> V3 {
    match gun {
        PISTOL => [0.02, 1.345, 0.6],
        SNIPER => [0.075, 1.26, 1.13],
        _ => [0.075, 1.265, 0.88],
    }
}

struct Remote {
    at: V3,
    yaw: f32,
    pitch: f32,
    team: usize,
    alive: bool,
    dead_at: Option<f64>,
    flash_until: f64,
    /// Where in a stride their legs are (radians), and how much they're walking (0 to 1).
    stride: f32,
    moving: f32,
    /// How far down they are (0 to 1), and the gun they have out.
    crouch: f32,
    gun: u8,
}

impl Remote {
    /// A point in this player's own space (x right, y up, z ahead) in the world. `arms` turns it
    /// with their aim; falling over turns everything.
    fn place(&self, p: V3, arms: bool, now: f64) -> V3 {
        let p = if arms { rot_x(p, SHOULDER, self.pitch.clamp(-1.2, 1.2)) } else { p };
        let fall = self.dead_at.map_or(0.0, |t| ((now - t) / 450.0).min(1.0) as f32);
        let p = rot_x(p, [0.0; 3], fall * fall * 1.5);
        let bob = self.stride.sin().abs() * 0.03 * self.moving - self.crouch * CROUCH_DROP;
        let (s, c) = self.yaw.sin_cos();
        [self.at[0] + p[0] * c + p[2] * s, self.at[1] + p[1] + bob, self.at[2] + p[0] * s - p[2] * c]
    }

    fn draw(&self, v: &mut Vec<f32>, now: f64) {
        let kit = &KITS[self.team];
        let body = |p: V3| self.place(p, false, now);
        let head = |p: V3| self.place(rot_x(p, NECK, self.pitch.clamp(-1.2, 1.2) * 0.6), false, now);
        let arms = |p: V3| self.place(p, true, now);

        // Legs, swinging from the hip and bending at the knee as they walk; folded crouching
        // (the whole body's lowered, and the thighs come up and the shins go back to the ground).
        let c = self.crouch;
        for side in [-1.0f32, 1.0] {
            let phase = self.stride + if side < 0.0 { 0.0 } else { std::f32::consts::PI };
            let swing = phase.sin() * 0.55 * self.moving * (1.0 - 0.6 * c) + 1.25 * c;
            let bend = -(phase.cos().max(0.0) * 0.7 + 0.08) * self.moving.max(0.15) * (1.0 - c) - 2.0 * c;
            let x = |a: f32, b: f32| if side < 0.0 { (-b, -a) } else { (a, b) };
            let thigh = |p: V3| body(rot_x(p, [0.0, HIP, 0.0], swing));
            let shin = |p: V3| thigh(rot_x(p, [0.0, KNEE, 0.0], bend));
            let (x0, x1) = x(0.025, 0.175);
            part(v, [x0, KNEE - 0.02, -0.085], [x1, HIP + 0.02, 0.085], FLAT, kit.pants, &thigh);
            let (p0, p1) = x(0.175, 0.2);
            part(v, [p0, 0.6, -0.06], [p1, 0.78, 0.065], FLAT, kit.pouch, &thigh); // cargo pocket
            let (x0, x1) = x(0.035, 0.165);
            part(v, [x0, 0.13, -0.075], [x1, KNEE + 0.02, 0.075], FLAT, kit.pants, &shin);
            let (k0, k1) = x(0.045, 0.155);
            part(v, [k0, 0.42, 0.07], [k1, 0.58, 0.11], SHINY, STRAP, &shin); // knee pad
            let (b0, b1) = x(0.03, 0.17);
            part(v, [b0, 0.0, -0.1], [b1, 0.16, 0.1], FLAT, BOOTS, &shin);
            let (t0, t1) = x(0.04, 0.16);
            part(v, [t0, 0.0, 0.1], [t1, 0.09, 0.17], FLAT, BOOTS, &shin);
        }

        // Hips, belt, torso.
        part(v, [-0.19, 0.84, -0.11], [0.19, 0.98, 0.11], FLAT, kit.pants, &body);
        part(v, [-0.2, 0.92, -0.12], [0.2, 0.97, 0.12], FLAT, STRAP, &body);
        part(v, [-0.03, 0.925, 0.12], [0.03, 0.965, 0.13], SHINY, METAL, &body); // buckle
        part(v, [-0.17, 0.96, -0.1], [0.17, 1.16, 0.1], FLAT, kit.shirt, &body);
        part(v, [-0.2, 1.14, -0.12], [0.2, 1.42, 0.12], FLAT, kit.shirt, &body);
        part(v, [-0.235, 1.33, -0.11], [0.235, 1.43, 0.11], FLAT, kit.shirt, &body);

        // The plate carrier: plates, the band round the sides, straps, pouches, a radio, a pack.
        part(v, [-0.17, 1.02, 0.12], [0.17, 1.38, 0.16], FLAT, kit.vest, &body);
        part(v, [-0.17, 1.02, -0.16], [0.17, 1.4, -0.12], FLAT, kit.vest, &body);
        part(v, [-0.205, 1.02, -0.125], [0.205, 1.2, 0.125], FLAT, kit.vest, &body);
        for x in [-0.15f32, 0.08] {
            part(v, [x, 1.38, -0.135], [x + 0.07, 1.445, 0.135], FLAT, kit.vest, &body);
        }
        for x in [-0.11f32, 0.0, 0.11] {
            part(v, [x - 0.045, 1.04, 0.16], [x + 0.045, 1.19, 0.205], FLAT, kit.pouch, &body);
            part(v, [x - 0.047, 1.17, 0.158], [x + 0.047, 1.205, 0.212], FLAT, kit.vest, &body);
        }
        part(v, [-0.12, 1.25, 0.16], [0.12, 1.33, 0.19], FLAT, kit.pouch, &body); // admin pouch
        part(v, [-0.16, 1.2, -0.21], [-0.07, 1.37, -0.16], FLAT, STRAP, &body); // radio
        part(v, [-0.125, 1.37, -0.19], [-0.115, 1.62, -0.18], FLAT, STRAP, &body); // its aerial
        part(v, [-0.04, 1.06, -0.22], [0.14, 1.36, -0.16], FLAT, kit.pouch, &body); // pack

        // Neck, head, face.
        part(v, [-0.055, 1.41, -0.05], [0.055, 1.5, 0.06], SKIN_MAT, SKIN, &body);
        part(v, [-0.1, 1.48, -0.11], [0.1, 1.74, 0.11], SKIN_MAT, SKIN, &head);
        part(v, [-0.018, 1.56, 0.11], [0.018, 1.62, 0.135], SKIN_MAT, SKIN, &head); // nose
        for s in [-1.0f32, 1.0] {
            let (e0, e1) = if s < 0.0 { (-0.115, -0.1) } else { (0.1, 0.115) };
            part(v, [e0, 1.57, -0.02], [e1, 1.65, 0.03], SKIN_MAT, SKIN, &head); // ear
        }
        if self.team == 0 {
            // A shemagh over the head and face, its tail down the back; goggles on the band.
            part(v, [-0.12, 1.4, -0.125], [0.12, 1.5, 0.125], FLAT, kit.hat, &body);
            part(v, [-0.107, 1.47, -0.117], [0.107, 1.595, 0.122], FLAT, kit.hat, &head);
            part(v, [-0.112, 1.66, -0.12], [0.112, 1.785, 0.12], FLAT, kit.hat, &head);
            part(v, [-0.09, 1.76, -0.1], [0.09, 1.8, 0.1], FLAT, kit.hat, &head);
            part(v, [-0.05, 1.46, -0.15], [0.05, 1.7, -0.115], FLAT, kit.hat, &head);
            part(v, [-0.114, 1.625, -0.122], [0.114, 1.665, 0.12], FLAT, STRAP, &head);
            for x in [-0.085f32, 0.01] {
                part(v, [x, 1.61, 0.112], [x + 0.075, 1.67, 0.138], SHINY, LENS, &head);
            }
        } else {
            // A helmet: shell, rim, rails, a mount for night sights; a headset; eye protection.
            part(v, [-0.13, 1.67, -0.14], [0.13, 1.81, 0.14], FLAT, kit.hat, &head);
            part(v, [-0.1, 1.81, -0.11], [0.1, 1.85, 0.11], FLAT, kit.hat, &head);
            part(v, [-0.135, 1.655, -0.145], [0.135, 1.68, 0.15], FLAT, kit.hat, &head);
            part(v, [-0.13, 1.58, -0.14], [0.13, 1.67, -0.1], FLAT, kit.hat, &head); // back
            for s in [-1.0f32, 1.0] {
                let x = |a: f32, b: f32| if s < 0.0 { (-b, -a) } else { (a, b) };
                let (r0, r1) = x(0.13, 0.145);
                part(v, [r0, 1.69, -0.07], [r1, 1.73, 0.09], SHINY, METAL, &head); // rail
                let (c0, c1) = x(0.1, 0.145);
                part(v, [c0, 1.56, -0.045], [c1, 1.65, 0.05], FLAT, STRAP, &head); // ear cup
                let (s0, s1) = x(0.09, 0.1);
                part(v, [s0, 1.48, 0.0], [s1, 1.66, 0.02], FLAT, STRAP, &head); // chin strap
            }
            part(v, [-0.03, 1.7, 0.14], [0.03, 1.77, 0.165], SHINY, METAL, &head); // mount
            limb(v, [-0.12, 1.58, 0.03], [-0.04, 1.53, 0.13], [0.008, 0.008], FLAT, STRAP, &head); // mic
            part(v, [-0.095, 1.605, 0.108], [0.095, 1.65, 0.128], SHINY, LENS, &head);
        }

        // Arms, holding the gun: a long gun at the shoulder, elbows out, the left hand under
        // it; a pistol out in front in both hands.
        let glove = STRAP;
        let joints: [(V3, V3, V3); 2] = match self.gun {
            PISTOL => [
                ([0.2, 1.38, 0.0], [0.21, 1.2, 0.2], [0.04, 1.29, 0.4]),
                ([-0.2, 1.38, 0.0], [-0.15, 1.18, 0.22], [-0.01, 1.28, 0.41]),
            ],
            SNIPER => [
                ([0.2, 1.38, 0.0], [0.25, 1.16, 0.05], [0.085, 1.17, 0.18]),
                ([-0.2, 1.38, 0.0], [-0.16, 1.12, 0.33], [0.05, 1.2, 0.56]),
            ],
            _ => [
                ([0.2, 1.38, 0.0], [0.25, 1.16, 0.05], [0.085, 1.17, 0.18]),
                ([-0.2, 1.38, 0.0], [-0.17, 1.13, 0.28], [0.04, 1.2, 0.48]),
            ],
        };
        for (shoulder, elbow, hand) in joints {
            limb(v, shoulder, elbow, [0.058, 0.06], FLAT, kit.shirt, &arms);
            limb(v, lerp(shoulder, elbow, 0.3), lerp(shoulder, elbow, 0.55), [0.064, 0.066], FLAT, kit.accent, &arms); // armband
            limb(v, elbow, hand, [0.05, 0.052], FLAT, kit.shirt, &arms);
            let near = lerp(elbow, hand, 0.82);
            limb(v, near, hand, [0.055, 0.057], FLAT, kit.pouch, &arms); // cuff
            part(v, sub(hand, [0.035, 0.04, 0.04]), add(hand, [0.035, 0.035, 0.04]), FLAT, glove, &arms);
        }
        part(v, [0.18, 1.3, -0.07], [0.27, 1.42, 0.07], FLAT, kit.vest, &arms); // shoulder pad

        match self.gun {
            PISTOL => {
                let x = 0.02;
                part(v, [x - 0.02, 1.31, 0.36], [x + 0.02, 1.36, 0.6], SHINY, METAL, &arms); // slide
                part(v, [x - 0.017, 1.29, 0.38], [x + 0.017, 1.31, 0.55], SHINY, METAL, &arms); // frame
                limb(v, [x, 1.3, 0.41], [x, 1.19, 0.38], [0.015, 0.024], FLAT, STRAP, &arms); // grip
            }
            SNIPER => {
                let x = 0.075;
                part(v, [x - 0.022, 1.19, -0.1], [x + 0.022, 1.3, 0.12], FLAT, OLIVE, &arms); // stock
                part(v, [x - 0.024, 1.23, 0.1], [x + 0.024, 1.31, 0.34], SHINY, METAL, &arms); // action
                part(v, [x - 0.028, 1.21, 0.34], [x + 0.028, 1.29, 0.68], FLAT, OLIVE, &arms); // forend
                part(v, [x - 0.01, 1.25, 0.68], [x + 0.01, 1.27, 1.06], SHINY, METAL, &arms); // barrel
                part(v, [x - 0.016, 1.245, 1.04], [x + 0.016, 1.275, 1.12], SHINY, METAL, &arms); // brake
                part(v, [x - 0.02, 1.33, 0.08], [x + 0.02, 1.37, 0.42], SHINY, METAL, &arms); // scope
                part(v, [x - 0.03, 1.32, 0.36], [x + 0.03, 1.38, 0.44], SHINY, METAL, &arms);
                part(v, [x - 0.026, 1.325, 0.04], [x + 0.026, 1.375, 0.1], SHINY, METAL, &arms);
                part(v, [x - 0.012, 1.31, 0.14], [x + 0.012, 1.33, 0.32], SHINY, METAL, &arms); // mount
                limb(v, [x + 0.024, 1.29, 0.2], [x + 0.07, 1.26, 0.18], [0.008, 0.008], SHINY, METAL, &arms); // bolt
                limb(v, [x, 1.23, 0.2], [x, 1.1, 0.16], [0.016, 0.022], FLAT, OLIVE, &arms); // grip
                part(v, [x - 0.018, 1.15, 0.24], [x + 0.018, 1.23, 0.32], SHINY, METAL, &arms); // magazine
            }
            _ => {
                let x = 0.075;
                let furniture = kit.furniture;
                part(v, [x - 0.02, 1.2, -0.08], [x + 0.02, 1.3, 0.12], FLAT, furniture, &arms); // stock
                part(v, [x - 0.025, 1.23, 0.1], [x + 0.025, 1.31, 0.38], SHINY, METAL, &arms); // receiver
                part(v, [x + 0.025, 1.26, 0.18], [x + 0.028, 1.29, 0.28], SHINY, LENS, &arms); // ejection port
                part(v, [x - 0.015, 1.31, 0.12], [x + 0.015, 1.325, 0.4], SHINY, METAL, &arms); // rail
                part(v, [x - 0.012, 1.325, 0.14], [x + 0.012, 1.36, 0.17], SHINY, METAL, &arms); // rear sight
                part(v, [x - 0.027, 1.22, 0.38], [x + 0.027, 1.3, 0.6], FLAT, furniture, &arms); // handguard
                part(v, [x - 0.009, 1.255, 0.6], [x + 0.009, 1.275, 0.82], SHINY, METAL, &arms); // barrel
                part(v, [x - 0.006, 1.275, 0.6], [x + 0.006, 1.335, 0.62], SHINY, METAL, &arms); // front sight
                part(v, [x - 0.013, 1.25, 0.8], [x + 0.013, 1.28, 0.87], SHINY, METAL, &arms); // muzzle
                limb(v, [x, 1.23, 0.2], [x, 1.1, 0.16], [0.016, 0.022], FLAT, furniture, &arms); // grip
                limb(v, [x, 1.235, 0.28], [x, 1.12, 0.3], [0.017, 0.035], SHINY, METAL, &arms); // magazine,
                limb(v, [x, 1.13, 0.298], [x, 1.03, 0.355], [0.017, 0.035], SHINY, METAL, &arms); // curved
            }
        }
        if now < self.flash_until {
            let m = remote_muzzle(self.gun);
            part(v, [m[0] - 0.05, m[1] - 0.05, m[2]], [m[0] + 0.05, m[1] + 0.05, m[2] + 0.12], GLOW, FLASH, &arms);
        }
    }
}

// ---- The game -------------------------------------------------------------------------------

struct Mesh {
    vao: WebGlVertexArrayObject,
    buffer: WebGlBuffer,
    count: i32,
}

struct Particle {
    p: V3,
    v: V3,
    col: V3,
    size: f32,
    until: f64,
}

struct Tracer {
    from: V3,
    to: V3,
    start: f64,
}

struct Game {
    gl: GL,
    canvas: HtmlCanvasElement,
    main: WebGlProgram,
    shadow: WebGlProgram,
    sky: WebGlProgram,
    u: HashMap<&'static str, WebGlUniformLocation>,
    shadow_fb: WebGlFramebuffer,
    shadow_tex: WebGlTexture,
    sky_vao: WebGlVertexArrayObject,
    map_mesh: Mesh,
    things: Mesh,
    gun: Mesh,
    hud_ammo: Option<HtmlElement>,
    hud_cross: Option<HtmlElement>,
    hud_hit: Option<HtmlElement>,
    hud_scope: Option<HtmlElement>,
    hud_text: String,
    hud_gap: f32,
    /// What the crosshair and scope were last set to: (crosshair shown, scope shown).
    hud_shown: (bool, bool),

    map: Map,
    socket: WebSocket,
    open: bool,
    /// The name to join with, once the socket's open.
    joining: Option<String>,
    /// Yours, once you've joined.
    id: Option<u32>,
    me: Player,
    /// Where you were a tick ago: you're drawn between that and now, as frames don't land on ticks.
    before: V3,
    /// Time not played yet, in seconds (under a tick).
    unplayed: f32,
    seq: u32,
    /// Your inputs the server hasn't run yet (played again on each snapshot), and those not sent.
    unacked: VecDeque<Input>,
    unsent: Vec<Input>,
    /// Where you're aiming: the mouse's, ahead of the ticks.
    yaw: f32,
    pitch: f32,
    keys: HashSet<String>,
    trigger: bool,
    scope_button: bool,
    reload_key: bool,
    /// The gun asked for (1 / 2 / 3, Q, the wheel), and the one before it, for Q.
    want_gun: u8,
    last_gun: u8,
    /// The gun last shown out, and when it came out: for its draw.
    shown_gun: u8,
    drawn_at: f64,
    /// Your eye's height over your feet, eased as you crouch and stand.
    eye_height: f32,

    /// Snapshots of everyone, by server tick, to draw them between two.
    snapshots: VecDeque<(u32, Vec<Seen>)>,
    /// The server's tick is now (ms) * 0.06 + this, smoothed over the network's jitter.
    clock: Option<f64>,
    remotes: HashMap<u32, Remote>,
    tracers: Vec<Tracer>,
    holes: Vec<(V3, V3)>,
    particles: Vec<Particle>,

    kick: f32,
    bob: f32,
    /// The view's jolt when you're hit.
    shake: f32,
    last_shot: f64,
    hit_at: f64,
    spawned_at: f64,
    seed: u32,
    /// The health, life and look-around seconds the page was last told.
    told: (i32, bool, u32),

    now: f64,
    /// CustomEvents to dispatch once the game isn't borrowed (a listener may call back in).
    outbox: Vec<(&'static str, JsValue)>,
}

thread_local! {
    static GAME: RefCell<Option<Game>> = const { RefCell::new(None) };
}

/// Runs `f` on the game, if it's started, then sends the events it queued.
fn with_game(f: impl FnOnce(&mut Game)) {
    let events = GAME.with(|cell| match cell.borrow_mut().as_mut() {
        Some(game) => {
            f(game);
            std::mem::take(&mut game.outbox)
        }
        None => Vec::new(),
    });
    let window = web_sys::window().unwrap();
    for (name, detail) in events {
        let init = web_sys::CustomEventInit::new();
        init.set_detail(&detail);
        if let Ok(event) = web_sys::CustomEvent::new_with_event_init_dict(name, &init) {
            let _ = window.dispatch_event(&event);
        }
    }
}

fn numbers(values: &[f64]) -> JsValue {
    values.iter().map(|&n| JsValue::from_f64(n)).collect::<js_sys::Array>().into()
}

/// The way from angle `a` to `b`, the short way round.
fn turn(a: f32, b: f32) -> f32 {
    (b - a + std::f32::consts::PI).rem_euclid(std::f32::consts::TAU) - std::f32::consts::PI
}

impl Game {
    fn random(&mut self) -> f32 {
        self.seed ^= self.seed << 13;
        self.seed ^= self.seed >> 17;
        self.seed ^= self.seed << 5;
        (self.seed as f64 / u32::MAX as f64) as f32
    }

    fn sound(&mut self, kind: f64, volume: f64) {
        self.gun_sound(kind, volume, RIFLE);
    }

    fn gun_sound(&mut self, kind: f64, volume: f64, gun: u8) {
        self.outbox.push(("outpost-sound", numbers(&[kind, volume, gun as f64])));
    }

    /// Asks for gun `gun` (it's out after the server's draw time).
    fn choose(&mut self, gun: u8) {
        if gun != self.want_gun && (gun as usize) < GUNS.len() {
            self.last_gun = self.want_gun;
            self.want_gun = gun;
        }
    }

    fn tell(&mut self, event: &'static str, json: String) {
        self.outbox.push((event, JsValue::from_str(&json)));
    }

    fn send(&self, message: &ToServer) {
        if self.open {
            if let Ok(text) = serde_json::to_string(message) {
                let _ = self.socket.send_with_str(&text);
            }
        }
    }

    /// The server's tick now, as near as we can tell, and the one everyone else is drawn at.
    fn server_tick(&self) -> f32 {
        self.clock.map_or(0.0, |c| (self.now * 0.06 + c) as f32)
    }

    fn view_tick(&self) -> f32 {
        self.server_tick() - INTERP
    }

    fn eye(&self) -> V3 {
        let p = lerp(self.before, self.me.pos, (self.unplayed / TICK).min(1.0));
        [p[0], p[1] + self.eye_height, p[2]]
    }

    /// The camera: where it is and which way it looks (recoil shows, a little).
    fn camera(&self) -> (V3, f32, f32) {
        if self.id.is_none() {
            // Before joining: a slow look round the map from above.
            let t = (self.now / 1000.0 * 0.06) as f32;
            let eye = [t.sin() * 26.0, 15.0, t.cos() * 18.0];
            let to = sub([0.0, 0.0, 0.0], eye);
            return (eye, to[0].atan2(-to[2]), (to[1] / dot(to, to).sqrt()).asin());
        }
        (self.eye(), self.yaw + self.me.punch[1] * 0.6, self.pitch + self.me.punch[0] * 0.6 + self.shake)
    }

    /// The field of view now: narrow through a scope.
    fn fov(&self) -> f32 {
        if self.id.is_some() && self.me.alive && self.me.scoped { self.me.gun().zoom } else { FOV }
    }

    /// Everyone else alive, where they're drawn: for bumping into.
    fn others(&self) -> Vec<V3> {
        self.remotes.values().filter(|r| r.alive).map(|r| r.at).collect()
    }

    /// One tick of yours: the keys held now, played at once and saved to send.
    fn play_tick(&mut self) {
        let mut held = 0;
        for (code, bit) in [
            ("KeyW", keys::FORWARD),
            ("KeyS", keys::BACK),
            ("KeyA", keys::LEFT),
            ("KeyD", keys::RIGHT),
            ("Space", keys::JUMP),
            ("ShiftLeft", keys::CROUCH),
            ("ShiftRight", keys::CROUCH),
        ] {
            if self.keys.contains(code) {
                held |= bit;
            }
        }
        if self.trigger {
            held |= keys::FIRE;
        }
        if self.scope_button {
            held |= keys::SCOPE;
        }
        if std::mem::take(&mut self.reload_key) {
            held |= keys::RELOAD;
        }
        self.seq += 1;
        let input = Input { seq: self.seq, keys: held, yaw: self.yaw, pitch: self.pitch, gun: self.want_gun };
        let others = self.others();
        self.before = self.me.pos;
        let reloading = self.me.reload > 0;
        if let Some(shot) = self.me.step(&input, &self.map, &others) {
            self.fired(shot);
        }
        if !reloading && self.me.reload > 0 {
            self.sound(SOUND_RELOAD, 1.0);
        }
        self.unacked.push_back(input);
        self.unsent.push(input);
    }

    /// Your shot, as your screen shows it: what it hits here is what the server will check.
    fn fired(&mut self, shot: Shot) {
        self.kick = 1.0;
        self.last_shot = self.now;
        let targets: Vec<(V3, f32)> =
            self.remotes.values().filter(|r| r.alive && r.team != self.me.team as usize).map(|r| (r.at, r.crouch)).collect();
        let (t, normal, hit) = trace(&self.map, shot.origin, shot.dir, &targets);
        let end = add(shot.origin, mul(shot.dir, t));
        let muzzle = self.gun_to_world(MUZZLES[shot.gun as usize], self.gun_pose());
        self.tracers.push(Tracer { from: muzzle, to: end, start: self.now });
        if let Some(n) = normal {
            self.holes.push((add(end, mul(n, 0.003)), n));
            if self.holes.len() > 60 {
                self.holes.remove(0);
            }
            self.burst_of(end, n, [0.72, 0.65, 0.52], 5);
        }
        if hit.is_some() {
            self.hit_at = self.now;
            self.burst_of(end, mul(shot.dir, -1.0), [0.55, 0.06, 0.05], 7);
            self.sound(SOUND_HIT, 1.0);
        }
        self.gun_sound(SOUND_SHOT, 1.0, shot.gun);
        if shot.gun == SNIPER {
            self.sound(SOUND_BOLT, 1.0);
        }
    }

    /// Bits flying off a hit: dust off walls, blood off people.
    fn burst_of(&mut self, at: V3, n: V3, col: V3, count: usize) {
        for _ in 0..count {
            let jitter = [self.random() - 0.5, self.random() - 0.5, self.random() - 0.5];
            let v = add(mul(n, 1.2 + self.random()), mul(jitter, 1.6));
            let size = 0.015 + self.random() * 0.025;
            let until = self.now + 300.0 + self.random() as f64 * 250.0;
            self.particles.push(Particle { p: at, v, col, size, until });
        }
    }

    fn receive(&mut self, text: &str) {
        let Ok(message) = serde_json::from_str::<ToClient>(text) else { return };
        match message {
            ToClient::Welcome { id, team } => {
                self.id = Some(id);
                self.tell("outpost-welcome", format!(r#"{{"id":{id},"team":{team}}}"#));
            }
            ToClient::Roster { players } => {
                let json = serde_json::to_string(&players).unwrap_or_default();
                self.tell("outpost-roster", json);
            }
            ToClient::Snapshot { tick, ack, you, players, events } => self.snapshot(tick, ack, you, players, events),
        }
    }

    fn snapshot(&mut self, tick: u32, ack: u32, you: Option<Player>, players: Vec<Seen>, events: Vec<Event>) {
        let offset = tick as f64 - self.now * 0.06;
        self.clock = Some(match self.clock {
            Some(c) if (offset - c).abs() < 30.0 => c + (offset - c) * 0.05,
            _ => offset,
        });
        self.snapshots.push_back((tick, players));
        while self.snapshots.len() > 40 {
            self.snapshots.pop_front();
        }

        // The server's word on you, with the moves it hasn't run yet played again on top.
        if let (Some(_), Some(mut state)) = (self.id, you) {
            self.unacked.retain(|i| i.seq > ack);
            let others = self.others();
            for input in &self.unacked {
                state.step(input, &self.map, &others);
            }
            let was_alive = self.me.alive;
            let moved = sub(state.pos, self.me.pos);
            self.before = if dot(moved, moved) > 4.0 { state.pos } else { add(self.before, moved) };
            self.me = state;
            if self.me.alive && !was_alive {
                self.spawned_at = self.now;
                self.yaw = self.me.yaw;
                self.pitch = 0.0;
            }
            let shield = self.me.shield.div_ceil(60);
            if (self.me.hp, self.me.alive, shield) != self.told {
                self.told = (self.me.hp, self.me.alive, shield);
                self.outbox.push(("outpost-hp", numbers(&[self.me.hp as f64, self.me.alive as u8 as f64, shield as f64])));
            }
        }

        for event in events {
            match event {
                Event::Shot { id, from, to, gun } if Some(id) != self.id => {
                    let now = self.now;
                    let from = match self.remotes.get_mut(&id) {
                        Some(r) => {
                            r.flash_until = now + 50.0;
                            r.place(remote_muzzle(gun), true, now)
                        }
                        None => from,
                    };
                    self.tracers.push(Tracer { from, to, start: now });
                    let d = sub(from, self.eye());
                    self.gun_sound(SOUND_FAR_SHOT, 1.0 / (1.0 + dot(d, d).sqrt() as f64 / 8.0), gun);
                }
                Event::Hit { target, .. } if Some(target) == self.id => self.shake = 0.03,
                Event::Kill { killer, victim, head, gun } => {
                    if Some(victim) == self.id {
                        self.shake = 0.05;
                    } else if let Some(r) = self.remotes.get_mut(&victim) {
                        if r.alive {
                            r.alive = false;
                            r.dead_at = Some(self.now);
                        }
                    }
                    self.tell("outpost-kill", format!(r#"{{"killer":{killer},"victim":{victim},"head":{head},"gun":{gun}}}"#));
                }
                _ => {}
            }
        }
    }

    /// Everyone else, where they were at the view tick: between the snapshots either side.
    fn place_others(&mut self) {
        if self.snapshots.is_empty() {
            return;
        }
        let at = self.view_tick();
        let last = self.snapshots.len() - 1;
        let (a, b, k) = match self.snapshots.iter().position(|(t, _)| *t as f32 > at) {
            Some(0) => (0, 0, 0.0),
            Some(i) => {
                let (t0, t1) = (self.snapshots[i - 1].0 as f32, self.snapshots[i].0 as f32);
                (i - 1, i, (at - t0) / (t1 - t0))
            }
            None => (last, last, 0.0),
        };
        let (older, newer) = (self.snapshots[a].1.clone(), self.snapshots[b].1.clone());
        let now = self.now;
        for s in &newer {
            if Some(s.id) == self.id {
                continue;
            }
            let p = older.iter().find(|p| p.id == s.id).unwrap_or(s);
            let pos = lerp(p.pos, s.pos, k);
            let yaw = p.yaw + turn(p.yaw, s.yaw) * k;
            let pitch = p.pitch + (s.pitch - p.pitch) * k;
            let crouch = p.crouch + (s.crouch - p.crouch) * k;
            let r = self.remotes.entry(s.id).or_insert(Remote {
                at: pos,
                yaw,
                pitch,
                team: 0,
                alive: s.alive,
                dead_at: None,
                flash_until: 0.0,
                stride: 0.0,
                moving: 0.0,
                crouch,
                gun: s.gun,
            });
            // Their legs go round with the ground they cover, and settle when they stop.
            let moved = ((pos[0] - r.at[0]).powi(2) + (pos[2] - r.at[2]).powi(2)).sqrt().min(0.5);
            r.stride += moved * 3.6;
            let walking = if moved * 60.0 > 0.6 && s.alive { 1.0 } else { 0.0 };
            r.moving += (walking - r.moving) * 0.15;
            r.at = pos;
            r.yaw = yaw;
            r.pitch = pitch;
            r.team = s.team as usize;
            r.crouch = crouch;
            r.gun = s.gun;
            if r.alive && !s.alive {
                r.dead_at = Some(now);
            }
            if s.alive {
                r.dead_at = None;
            }
            r.alive = s.alive;
        }
        self.remotes.retain(|id, _| newer.iter().any(|s| s.id == *id));
    }

    /// How far through a reload you are, 0 to 1 (or -1, not reloading).
    fn reloaded(&self) -> f32 {
        if self.me.reload == 0 {
            return -1.0;
        }
        1.0 - self.me.reload as f32 / self.me.gun().reload_ticks as f32
    }

    /// Where the gun sits, from the camera: offset, pitch and roll. Bobs as you walk, kicks
    /// as it fires, dips to reload, comes up when you spawn.
    fn gun_pose(&self) -> (V3, f32, f32) {
        let speed = self.me.speed() / RUN;
        let (base, kick) = match self.me.gun {
            PISTOL => ([0.12, -0.15, 0.36], 0.12),
            SNIPER => ([0.15, -0.18, 0.28], 0.14),
            _ => ([0.15, -0.17, 0.3], 0.05),
        };
        let mut offset = [
            base[0] + self.bob.sin() * 0.008 * speed,
            base[1] - self.bob.cos().abs() * 0.01 * speed,
            base[2] - self.kick * 0.035,
        ];
        let mut pitch = self.kick * kick;
        let mut roll = 0.0;
        if self.me.reload > 0 {
            let dip = (self.reloaded() * std::f32::consts::PI).sin();
            offset[1] -= dip * 0.08;
            pitch -= dip * 0.35;
            roll += dip * 0.5;
        }
        let up = ((self.now - self.spawned_at.max(self.drawn_at)) / 400.0).clamp(0.0, 1.0) as f32;
        offset[1] -= (1.0 - up) * 0.25;
        pitch -= (1.0 - up) * 0.6;
        (offset, pitch, roll)
    }

    /// A point in the gun's space (z along the barrel) in the world.
    fn gun_to_world(&self, p: V3, pose: (V3, f32, f32)) -> V3 {
        let (eye, yaw, pitch) = self.camera();
        let (offset, gp, roll) = pose;
        let q = add(rot_z(rot_x(p, [0.0; 3], gp), roll), offset);
        let f = forward(yaw, pitch);
        let (s, c) = yaw.sin_cos();
        let right = [c, 0.0, s];
        let up = cross(right, f);
        add(eye, add(add(mul(right, q[0]), mul(up, q[1])), mul(f, q[2])))
    }

    fn frame(&mut self, now: f64) {
        let dt = (((now - self.now) / 1000.0) as f32).clamp(0.0, 0.25);
        self.now = now;
        if !self.locked() {
            self.keys.clear();
            self.trigger = false;
        }
        // Ticks only while alive: dead (or not yet spawned), there's nothing to send, and the
        // server's spawn says which way you face.
        if self.id.is_some() && self.me.alive {
            self.unplayed += dt;
            while self.unplayed >= TICK {
                self.unplayed -= TICK;
                self.play_tick();
            }
            if self.me.grounded {
                self.bob += self.me.speed() * dt * 1.7;
            }
        }
        if !self.unsent.is_empty() {
            let inputs = std::mem::take(&mut self.unsent);
            self.send(&ToServer::Input { view: self.view_tick(), inputs });
        }
        self.place_others();
        if self.me.gun != self.shown_gun {
            self.shown_gun = self.me.gun;
            self.drawn_at = now;
            self.sound(SOUND_DRAW, 1.0);
        }
        let eye = self.me.eye()[1] - self.me.pos[1];
        self.eye_height += (eye - self.eye_height) * (dt * 18.0).min(1.0);

        self.kick = (self.kick - dt * 14.0).max(0.0);
        self.shake *= 1.0 - (dt * 10.0).min(1.0);
        for p in &mut self.particles {
            p.v[1] -= 9.0 * dt;
            p.p = add(p.p, mul(p.v, dt));
        }
        self.particles.retain(|p| p.until > now);
        self.tracers.retain(|t| now - t.start < 200.0);

        self.hud();
        self.draw();
    }

    /// The parts of the HUD that change every frame: ammo, the crosshair's gap, the hit marker.
    fn hud(&mut self) {
        let (g, n) = (self.me.gun(), self.me.gun as usize);
        let text = match (self.id, self.me.alive) {
            (Some(_), true) if self.me.reload > 0 => format!("<i>{}</i><b>…</b> / {}", g.name, self.me.reserve[n]),
            (Some(_), true) => format!("<i>{}</i><b>{}</b> / {}", g.name, self.me.ammo[n], self.me.reserve[n]),
            _ => String::new(),
        };
        if text != self.hud_text {
            if let Some(e) = &self.hud_ammo {
                e.set_inner_html(&text);
            }
            self.hud_text = text;
        }
        let gap = 3.0 + self.me.spread() * 260.0 + self.kick * 4.0;
        if (gap - self.hud_gap).abs() > 0.4 {
            self.hud_gap = gap;
            if let Some(e) = &self.hud_cross {
                let _ = e.style().set_property("--gap", &format!("{gap:.1}px"));
            }
        }
        if let Some(e) = &self.hud_hit {
            let shown = self.now - self.hit_at < 180.0;
            let _ = e.style().set_property("opacity", if shown { "1" } else { "0" });
        }
        // No crosshair on a sniper rifle (it's no help unscoped), and the scope's own lines
        // through it.
        let playing = self.id.is_some() && self.me.alive;
        let shown = (!(playing && self.me.gun == SNIPER), playing && self.me.scoped);
        if shown != self.hud_shown {
            self.hud_shown = shown;
            if let Some(e) = &self.hud_cross {
                let _ = e.style().set_property("display", if shown.0 { "block" } else { "none" });
            }
            if let Some(e) = &self.hud_scope {
                let _ = e.style().set_property("display", if shown.1 { "block" } else { "none" });
            }
        }
    }

    fn locked(&self) -> bool {
        let document = web_sys::window().unwrap().document().unwrap();
        document.pointer_lock_element().is_some_and(|e| JsValue::from(e) == JsValue::from(self.canvas.clone()))
    }

    fn draw(&mut self) {
        let now = self.now;
        let gl = &self.gl;

        // What moves: people, tracers, bullet holes, bits.
        let mut v = Vec::new();
        for r in self.remotes.values() {
            if r.alive || r.dead_at.is_some_and(|t| now - t < 4000.0) {
                r.draw(&mut v, now);
            }
        }
        for t in &self.tracers {
            let d = sub(t.to, t.from);
            let len = dot(d, d).sqrt();
            let head = ((now - t.start) as f32 / 1000.0 * 450.0).min(len);
            let tail = (head - 3.0).max(0.0);
            if tail < len {
                let dir = mul(d, 1.0 / len.max(1e-3));
                rod(&mut v, add(t.from, mul(dir, tail)), add(t.from, mul(dir, head)), 0.006, [1.0, 0.86, 0.55]);
            }
        }
        for &(at, n) in &self.holes {
            let size = mul(sub([1.0; 3], [n[0].abs(), n[1].abs(), n[2].abs()]), 0.028);
            let depth = mul([n[0].abs(), n[1].abs(), n[2].abs()], 0.002);
            let half = add(size, depth);
            part(&mut v, sub(at, half), add(at, half), FLAT, [0.07, 0.06, 0.05], &|p| p);
        }
        for p in &self.particles {
            let h = [p.size; 3];
            part(&mut v, sub(p.p, h), add(p.p, h), FLAT, p.col, &|q| q);
        }
        upload(gl, &mut self.things, &v);

        // Your rifle and hands.
        let mut g = Vec::new();
        if self.id.is_some() && self.me.alive && !self.me.scoped {
            self.viewmodel(&mut g);
        }
        upload(gl, &mut self.gun, &g);

        // Shadows: the map and the people, from the sun.
        let sun = normalize([-0.45, 0.82, 0.35]);
        let light = multiply(&ortho(110.0, 1.0, 320.0), &look(mul(sun, 150.0), mul(sun, -1.0)));
        gl.bind_framebuffer(GL::FRAMEBUFFER, Some(&self.shadow_fb));
        gl.viewport(0, 0, SHADOW, SHADOW);
        gl.clear(GL::DEPTH_BUFFER_BIT);
        gl.use_program(Some(&self.shadow));
        gl.uniform_matrix4fv_with_f32_array(self.u.get("s_light"), false, &light);
        gl.enable(GL::POLYGON_OFFSET_FILL);
        gl.polygon_offset(2.0, 4.0);
        for mesh in [&self.map_mesh, &self.things] {
            draw_mesh(gl, mesh);
        }
        gl.disable(GL::POLYGON_OFFSET_FILL);

        // The view.
        // Sharp on high-density screens, but no more than about 1920 x 1200 pixels: every one
        // runs the texture shader.
        let (cw, ch) = (self.canvas.client_width().max(1) as f64, self.canvas.client_height().max(1) as f64);
        let scale = web_sys::window().unwrap().device_pixel_ratio().min((2_300_000.0 / (cw * ch)).sqrt());
        let (w, h) = ((cw * scale) as u32, (ch * scale) as u32);
        if self.canvas.width() != w || self.canvas.height() != h {
            self.canvas.set_width(w);
            self.canvas.set_height(h);
        }
        gl.bind_framebuffer(GL::FRAMEBUFFER, None);
        gl.viewport(0, 0, w as i32, h as i32);
        gl.clear(GL::COLOR_BUFFER_BIT | GL::DEPTH_BUFFER_BIT);

        let fov = self.fov();
        let (eye, yaw, pitch) = self.camera();
        let projection = perspective(fov, w as f32 / h.max(1) as f32, 0.04, 400.0);
        let mvp = multiply(&projection, &look(eye, forward(yaw, pitch)));

        gl.use_program(Some(&self.sky));
        gl.uniform1f(self.u.get("k_pitch"), pitch);
        gl.uniform1f(self.u.get("k_tan"), (fov / 2.0).tan());
        gl.disable(GL::DEPTH_TEST);
        gl.bind_vertex_array(Some(&self.sky_vao));
        gl.draw_arrays(GL::TRIANGLES, 0, 3);
        gl.enable(GL::DEPTH_TEST);

        gl.use_program(Some(&self.main));
        gl.uniform_matrix4fv_with_f32_array(self.u.get("m_mvp"), false, &mvp);
        gl.uniform_matrix4fv_with_f32_array(self.u.get("m_light"), false, &light);
        gl.uniform3f(self.u.get("m_eye"), eye[0], eye[1], eye[2]);
        gl.uniform3f(self.u.get("m_sun"), sun[0], sun[1], sun[2]);
        gl.active_texture(GL::TEXTURE0);
        gl.bind_texture(GL::TEXTURE_2D, Some(&self.shadow_tex));
        gl.uniform1i(self.u.get("m_shadow"), 0);
        draw_mesh(gl, &self.map_mesh);
        draw_mesh(gl, &self.things);
        // The rifle over everything, so it never pokes into a wall.
        gl.clear(GL::DEPTH_BUFFER_BIT);
        draw_mesh(gl, &self.gun);
    }

    fn viewmodel(&self, v: &mut Vec<f32>) {
        let pose = self.gun_pose();
        let kit = &KITS[self.me.team as usize];
        let at = |p: V3| self.gun_to_world(p, pose);
        // The magazine's out for the middle of a reload.
        let mag_out = (0.25..0.6).contains(&self.reloaded());
        let glove = [0.13, 0.12, 0.1];
        match self.me.gun {
            PISTOL => {
                let shiny: [(V3, V3); 6] = [
                    ([-0.021, 0.0, -0.085], [0.021, 0.045, 0.165]),   // slide
                    ([-0.006, 0.045, -0.07], [0.006, 0.052, 0.155]),  // rib
                    ([-0.004, 0.045, 0.135], [0.004, 0.062, 0.155]),  // front sight
                    ([-0.013, 0.045, -0.085], [0.013, 0.063, -0.068]), // rear sight
                    ([-0.017, -0.022, -0.06], [0.017, 0.0, 0.125]),   // frame
                    ([-0.004, -0.058, 0.0], [0.004, -0.05, 0.075]),   // trigger guard
                ];
                for (min, max) in shiny {
                    part(v, min, max, SHINY, METAL, &at);
                }
                part(v, [-0.004, -0.058, 0.068], [0.004, -0.022, 0.076], SHINY, METAL, &at);
                part(v, [-0.012, 0.012, 0.165], [0.012, 0.035, 0.168], SHINY, LENS, &at); // the bore
                limb(v, [0.0, -0.012, -0.03], [0.0, -0.135, -0.072], [0.017, 0.027], FLAT, STRAP, &at); // grip
                if !mag_out {
                    part(v, [-0.016, -0.15, -0.09], [0.016, -0.135, -0.048], SHINY, METAL, &at);
                }
                // Both hands round the grip, the left cupping the right; arms reaching out.
                part(v, [-0.034, -0.125, -0.1], [0.034, -0.025, -0.01], FLAT, glove, &at);
                part(v, [-0.05, -0.13, -0.085], [-0.012, -0.04, 0.02], FLAT, glove, &at);
                limb(v, [0.026, -0.02, -0.02], [0.005, -0.04, 0.03], [0.011, 0.011], FLAT, glove, &at); // finger
                limb(v, [0.07, -0.28, -0.42], [0.0, -0.08, -0.07], [0.042, 0.045], FLAT, kit.shirt, &at);
                limb(v, [0.04, -0.17, -0.22], [0.0, -0.08, -0.07], [0.047, 0.05], FLAT, kit.pouch, &at);
                limb(v, [-0.17, -0.28, -0.38], [-0.04, -0.09, -0.04], [0.042, 0.045], FLAT, kit.shirt, &at);
                limb(v, [-0.1, -0.18, -0.2], [-0.04, -0.09, -0.04], [0.047, 0.05], FLAT, kit.pouch, &at);
            }
            SNIPER => {
                let shiny: [(V3, V3, V3); 13] = [
                    ([-0.026, -0.03, -0.06], [0.026, 0.035, 0.2], METAL),     // action
                    ([-0.011, -0.004, 0.2], [0.011, 0.018, 0.93], METAL),     // barrel
                    ([-0.019, -0.01, 0.9], [0.019, 0.025, 0.99], METAL),      // brake
                    ([-0.021, 0.06, -0.08], [0.021, 0.1, 0.26], METAL),       // scope tube
                    ([-0.032, 0.048, 0.2], [0.032, 0.112, 0.29], METAL),      // its front bell
                    ([-0.028, 0.052, -0.13], [0.028, 0.108, -0.06], METAL),   // and back
                    ([-0.026, 0.05, -0.136], [0.026, 0.11, -0.13], LENS),     // eyepiece
                    ([-0.011, 0.1, 0.05], [0.011, 0.124, 0.09], METAL),       // turret
                    ([0.021, 0.07, 0.05], [0.036, 0.09, 0.09], METAL),        // side turret
                    ([-0.024, 0.033, -0.02], [0.024, 0.062, 0.01], METAL),    // rings
                    ([-0.024, 0.033, 0.15], [0.024, 0.062, 0.18], METAL),
                    ([0.068, -0.025, -0.01], [0.09, -0.003, 0.012], METAL),   // bolt knob
                    ([0.008, -0.05, 0.0], [0.012, -0.03, 0.06], METAL),       // trigger guard
                ];
                for (min, max, col) in shiny {
                    part(v, min, max, SHINY, col, &at);
                }
                limb(v, [0.026, 0.012, 0.01], [0.07, -0.014, 0.0], [0.008, 0.008], SHINY, METAL, &at); // bolt
                let flat: [(V3, V3); 3] = [
                    ([-0.024, -0.065, -0.4], [0.024, 0.022, -0.06]),  // stock
                    ([-0.02, 0.022, -0.32], [0.02, 0.048, -0.13]),    // cheek rest
                    ([-0.031, -0.045, 0.2], [0.031, 0.022, 0.58]),    // forend
                ];
                for (min, max) in flat {
                    part(v, min, max, FLAT, OLIVE, &at);
                }
                part(v, [-0.026, -0.07, -0.42], [0.026, 0.03, -0.395], FLAT, STRAP, &at); // butt pad
                limb(v, [0.0, -0.03, 0.02], [0.0, -0.13, -0.02], [0.016, 0.022], FLAT, OLIVE, &at); // grip
                if !mag_out {
                    part(v, [-0.019, -0.095, 0.06], [0.019, -0.03, 0.15], SHINY, METAL, &at);
                }
                for x in [-0.015f32, 0.015] {
                    limb(v, [x, -0.045, 0.52], [x, -0.05, 0.3], [0.006, 0.006], SHINY, METAL, &at); // bipod, folded
                }
                // Right hand on the grip, the left under the forend.
                part(v, [-0.035, -0.105, -0.06], [0.035, -0.025, 0.04], FLAT, glove, &at);
                limb(v, [0.03, -0.04, 0.03], [-0.015, -0.035, 0.05], [0.012, 0.012], FLAT, glove, &at);
                limb(v, [0.06, -0.15, -0.3], [0.02, -0.08, -0.04], [0.042, 0.045], FLAT, kit.shirt, &at);
                limb(v, [0.04, -0.12, -0.13], [0.02, -0.08, -0.04], [0.047, 0.05], FLAT, kit.pouch, &at);
                part(v, [-0.045, -0.095, 0.33], [0.04, -0.04, 0.44], FLAT, glove, &at);
                limb(v, [-0.15, -0.23, 0.1], [-0.02, -0.08, 0.38], [0.045, 0.048], FLAT, kit.shirt, &at);
                limb(v, [-0.075, -0.15, 0.26], [-0.02, -0.08, 0.38], [0.05, 0.053], FLAT, kit.pouch, &at);
            }
            _ => {
                let shiny: [(V3, V3, V3); 9] = [
                    ([-0.025, -0.03, -0.05], [0.025, 0.035, 0.25], METAL),     // receiver
                    ([0.025, 0.0, 0.04], [0.028, 0.025, 0.14], LENS),          // ejection port
                    ([-0.015, 0.035, -0.04], [0.015, 0.05, 0.3], METAL),       // rail
                    ([-0.012, 0.05, -0.03], [0.012, 0.075, 0.0], METAL),       // rear sight
                    ([-0.009, 0.0, 0.45], [0.009, 0.018, 0.64], METAL),        // barrel
                    ([-0.006, 0.018, 0.57], [0.006, 0.06, 0.59], METAL),       // front sight
                    ([-0.014, -0.006, 0.62], [0.014, 0.024, 0.69], METAL),     // muzzle
                    ([0.008, -0.045, 0.0], [0.012, -0.03, 0.06], METAL),       // trigger guard
                    ([-0.03, 0.005, 0.06], [-0.025, 0.02, 0.1], METAL),        // charging handle
                ];
                for (min, max, col) in shiny {
                    part(v, min, max, SHINY, col, &at);
                }
                let flat: [(V3, V3, V3); 3] = [
                    ([-0.028, -0.035, 0.25], [0.028, 0.03, 0.45], kit.furniture), // handguard
                    ([-0.022, -0.05, -0.32], [0.022, 0.02, -0.05], kit.furniture), // stock
                    ([-0.024, -0.06, -0.34], [0.024, 0.03, -0.31], STRAP),         // butt pad
                ];
                for (min, max, col) in flat {
                    part(v, min, max, FLAT, col, &at);
                }
                limb(v, [0.0, -0.03, 0.02], [0.0, -0.12, -0.01], [0.016, 0.022], FLAT, kit.furniture, &at); // grip
                if !mag_out {
                    limb(v, [0.0, -0.025, 0.16], [0.0, -0.12, 0.175], [0.018, 0.035], SHINY, METAL, &at);
                    limb(v, [0.0, -0.115, 0.173], [0.0, -0.19, 0.215], [0.018, 0.035], SHINY, METAL, &at);
                }

                // Hands: gloved, the right on the grip, the left round the handguard; sleeves and cuffs.
                part(v, [-0.035, -0.105, -0.06], [0.035, -0.025, 0.04], FLAT, glove, &at);
                limb(v, [0.03, -0.04, 0.03], [-0.015, -0.035, 0.05], [0.012, 0.012], FLAT, glove, &at); // trigger finger
                limb(v, [0.06, -0.15, -0.3], [0.02, -0.08, -0.04], [0.042, 0.045], FLAT, kit.shirt, &at);
                limb(v, [0.04, -0.12, -0.13], [0.02, -0.08, -0.04], [0.047, 0.05], FLAT, kit.pouch, &at);
                part(v, [-0.042, -0.07, 0.3], [0.04, -0.02, 0.4], FLAT, glove, &at);
                for i in 0..3 {
                    let z = 0.31 + i as f32 * 0.03;
                    part(v, [0.028, -0.045, z], [0.045, 0.02, z + 0.024], FLAT, glove, &at); // fingers
                }
                limb(v, [-0.13, -0.2, 0.06], [-0.02, -0.06, 0.33], [0.045, 0.048], FLAT, kit.shirt, &at);
                limb(v, [-0.06, -0.12, 0.23], [-0.02, -0.06, 0.33], [0.05, 0.053], FLAT, kit.pouch, &at);
            }
        }
        if self.now - self.last_shot < 45.0 {
            let m = MUZZLES[self.me.gun as usize];
            part(v, [m[0] - 0.035, m[1] - 0.035, m[2]], [m[0] + 0.035, m[1] + 0.035, m[2] + 0.08], GLOW, FLASH, &at);
            part(v, [m[0] - 0.012, m[1] - 0.06, m[2]], [m[0] + 0.012, m[1] + 0.06, m[2] + 0.04], GLOW, FLASH, &at);
        }
    }
}

// ---- WebGL ----------------------------------------------------------------------------------

const MAIN_VERTEX: &str = r#"#version 300 es
layout(location = 0) in vec3 a_pos;
layout(location = 1) in vec3 a_normal;
layout(location = 2) in vec2 a_uv;
layout(location = 3) in vec2 a_size;
layout(location = 4) in float a_mat;
layout(location = 5) in vec3 a_col;
uniform mat4 m_mvp;
uniform mat4 m_light;
out vec3 v_pos;
out vec3 v_normal;
out vec2 v_uv;
out vec2 v_size;
out float v_mat;
out vec3 v_col;
out vec4 v_light;
void main() {
    gl_Position = m_mvp * vec4(a_pos, 1.0);
    v_pos = a_pos;
    v_normal = a_normal;
    v_uv = a_uv;
    v_size = a_size;
    v_mat = a_mat;
    v_col = a_col;
    v_light = m_light * vec4(a_pos + a_normal * 0.02, 1.0);
}"#;

// The map's textures are made here, from where on a face a pixel is: no image files.
const MAIN_FRAGMENT: &str = r#"#version 300 es
precision highp float;
precision highp sampler2DShadow;
in vec3 v_pos;
in vec3 v_normal;
in vec2 v_uv;
in vec2 v_size;
in float v_mat;
in vec3 v_col;
in vec4 v_light;
uniform vec3 m_eye;
uniform vec3 m_sun;
uniform sampler2DShadow m_shadow;
out vec4 o;

float hash(vec2 p) {
    p = fract(p * vec2(123.34, 456.21));
    p += dot(p, p + 45.32);
    return fract(p.x * p.y);
}
float noise(vec2 p) {
    vec2 i = floor(p), f = fract(p);
    f = f * f * (3.0 - 2.0 * f);
    return mix(mix(hash(i), hash(i + vec2(1, 0)), f.x), mix(hash(i + vec2(0, 1)), hash(i + vec2(1, 1)), f.x), f.y);
}
float fbm(vec2 p) {
    float v = 0.0, a = 0.5;
    for (int i = 0; i < 3; i++) { v += a * noise(p); p *= 2.03; a *= 0.5; }
    return v;
}
// Courses of bricks `s` big: x is 1 on a brick and 0 in the mortar, y is the brick's own shade.
vec2 bricks(vec2 p, vec2 s, float mortar) {
    p /= s;
    p.x += mod(floor(p.y), 2.0) * 0.5;
    vec2 f = fract(p);
    vec2 e = min(f, 1.0 - f) * s;
    return vec2(smoothstep(0.0, mortar, min(e.x, e.y)), hash(floor(p)));
}

vec3 albedo(int m, vec2 w) {
    if (m == 0) { // sand, packed down, patchy
        float g = fbm(w * 0.35);
        vec3 c = mix(vec3(0.8, 0.69, 0.5), vec3(0.68, 0.57, 0.4), g);
        return c * (0.94 + 0.08 * hash(floor(w * 28.0)));
    }
    if (m == 1) { // plaster, stained, worn through to brick in places
        vec3 c = mix(vec3(0.9, 0.82, 0.66), vec3(0.78, 0.68, 0.52), smoothstep(0.35, 0.75, fbm(w * 0.6 + 3.0)));
        vec2 b = bricks(w, vec2(0.24, 0.08), 0.012);
        vec3 brick = mix(vec3(0.45, 0.4, 0.34), vec3(0.62, 0.4, 0.3) * (0.85 + 0.3 * b.y), b.x);
        c = mix(c, brick, smoothstep(0.69, 0.71, fbm(w * 0.45 + 11.0)));
        return c * (0.97 + 0.06 * hash(floor(w * 40.0)));
    }
    if (m == 2) { // dressed stone
        vec2 b = bricks(w, vec2(0.8, 0.4), 0.02);
        vec3 c = vec3(0.7, 0.65, 0.56) * (0.85 + 0.25 * b.y) * (0.93 + 0.1 * fbm(w * 3.0));
        return mix(vec3(0.45, 0.42, 0.38), c, b.x);
    }
    if (m == 3) { // a wooden crate: planks, a frame, a brace
        vec2 e = min(v_uv, v_size - v_uv);
        float frame = step(min(e.x, e.y), 0.1);
        float plank = fract(v_uv.y / 0.183);
        float gap = smoothstep(0.0, 0.05, plank) * smoothstep(1.0, 0.95, plank);
        vec3 wood = vec3(0.6, 0.45, 0.27) * (0.8 + 0.3 * noise(vec2(v_uv.x * 3.0, v_uv.y * 40.0)));
        vec3 c = mix(wood * mix(0.5, 1.0, gap), wood * 1.1, frame);
        float diag = abs(v_uv.x / v_size.x - v_uv.y / v_size.y) * min(v_size.x, v_size.y);
        if (diag < 0.07 && frame < 0.5) c = wood * 1.05;
        return c;
    }
    if (m == 4) { // concrete, cast in panels
        vec2 f = fract(w / 2.0);
        float seam = smoothstep(0.0, 0.01, min(min(f.x, 1.0 - f.x), min(f.y, 1.0 - f.y)));
        return vec3(0.62, 0.62, 0.6) * (0.85 + 0.2 * fbm(w * 2.0)) * mix(0.7, 1.0, seam);
    }
    if (m == 5) { // dark wood
        return vec3(0.34, 0.24, 0.15) * (0.8 + 0.3 * noise(vec2(w.x * 30.0, w.y * 2.0)));
    }
    if (m == 6) { // brick
        vec2 b = bricks(w, vec2(0.25, 0.075), 0.01);
        return mix(vec3(0.55, 0.52, 0.47), vec3(0.62, 0.36, 0.27) * (0.8 + 0.35 * b.y), b.x);
    }
    return v_col;
}

void main() {
    int m = int(v_mat + 0.5);
    if (m == 11) { o = vec4(v_col, 1.0); return; }
    vec3 n = normalize(v_normal);
    vec2 w = abs(n.x) > 0.5 ? v_pos.zy : abs(n.y) > 0.5 ? v_pos.xz : v_pos.xy;
    vec3 a = albedo(m, w);
    // People and things: edges a little darker, as if rounded; cloth woven and worn.
    if (m >= 10 && v_size.x > 0.0) {
        vec2 e = min(v_uv, v_size - v_uv);
        float bevel = min(0.018, 0.3 * min(v_size.x, v_size.y));
        a *= mix(0.68, 1.0, smoothstep(0.0, bevel, min(e.x, e.y)));
        if (m == 10) a *= (0.93 + 0.07 * noise(v_uv * 260.0)) * (0.9 + 0.14 * fbm(v_uv * 9.0 + v_col.xy * 40.0));
        if (m == 13) a *= 0.95 + 0.07 * fbm(v_uv * 30.0);
    }

    vec3 p = v_light.xyz / v_light.w * 0.5 + 0.5;
    float lit = 1.0;
    if (all(greaterThan(p, vec3(0.0))) && all(lessThan(p, vec3(1.0)))) {
        // Four lookups, each already blended over 2 x 2 texels by the hardware.
        vec2 t = vec2(0.75 / 2048.0);
        lit = 0.25 * (texture(m_shadow, vec3(p.xy + vec2(-t.x, -t.y), p.z - 0.0005))
            + texture(m_shadow, vec3(p.xy + vec2(t.x, -t.y), p.z - 0.0005))
            + texture(m_shadow, vec3(p.xy + vec2(-t.x, t.y), p.z - 0.0005))
            + texture(m_shadow, vec3(p.xy + vec2(t.x, t.y), p.z - 0.0005)));
    }
    float sun = max(dot(n, m_sun), 0.0) * lit;
    vec3 sky = mix(vec3(0.5, 0.43, 0.34), vec3(0.58, 0.67, 0.82), n.y * 0.5 + 0.5);
    // Walls darken toward the ground, where less light gets in.
    float ao = m < 10 && n.y < 0.5 ? mix(0.6, 1.0, clamp(v_pos.y / 1.4, 0.0, 1.0)) : 1.0;
    vec3 c = a * (sky * 0.55 * ao + vec3(1.0, 0.92, 0.78) * 1.1 * sun);
    vec3 view = normalize(m_eye - v_pos);
    if (m == 12 || m == 13) { // a shine where the sun glints off
        float gloss = m == 12 ? 60.0 : 14.0;
        float spec = pow(max(dot(reflect(-m_sun, n), view), 0.0), gloss) * lit;
        c += vec3(1.0, 0.94, 0.85) * spec * (m == 12 ? 0.9 : 0.08);
        c += sky * 0.08 * (m == 12 ? 1.0 : 0.0);
    }
    if (m >= 10) c += sky * 0.1 * pow(1.0 - max(dot(n, view), 0.0), 3.0); // a rim of sky light

    float haze = 1.0 - exp(-max(distance(v_pos, m_eye) - 20.0, 0.0) * 0.012);
    c = mix(c, vec3(0.83, 0.83, 0.8), haze * 0.55);
    o = vec4(pow(c, vec3(0.95)), 1.0);
}"#;

const SHADOW_VERTEX: &str = r#"#version 300 es
layout(location = 0) in vec3 a_pos;
uniform mat4 s_light;
void main() { gl_Position = s_light * vec4(a_pos, 1.0); }"#;

const SHADOW_FRAGMENT: &str = r#"#version 300 es
precision mediump float;
out vec4 o;
void main() { o = vec4(1.0); }"#;

// The sky: one triangle over the screen, from haze at the horizon to blue overhead.
const SKY_VERTEX: &str = r#"#version 300 es
out vec2 v;
void main() {
    vec2 p = vec2(float((gl_VertexID << 1) & 2), float(gl_VertexID & 2)) * 2.0 - 1.0;
    v = p;
    gl_Position = vec4(p, 0.0, 1.0);
}"#;

const SKY_FRAGMENT: &str = r#"#version 300 es
precision mediump float;
in vec2 v;
uniform float k_pitch;
uniform float k_tan;
out vec4 o;
void main() {
    float a = k_pitch + atan(v.y * k_tan);
    vec3 c = mix(vec3(0.87, 0.85, 0.8), vec3(0.4, 0.6, 0.87), smoothstep(-0.05, 0.8, a));
    o = vec4(c, 1.0);
}"#;

fn program(gl: &GL, vertex: &str, fragment: &str) -> Result<WebGlProgram, String> {
    let compile = |kind: u32, source: &str| -> Result<WebGlShader, String> {
        let shader = gl.create_shader(kind).ok_or("no shader")?;
        gl.shader_source(&shader, source);
        gl.compile_shader(&shader);
        if gl.get_shader_parameter(&shader, GL::COMPILE_STATUS).as_bool() == Some(true) {
            Ok(shader)
        } else {
            Err(gl.get_shader_info_log(&shader).unwrap_or_default())
        }
    };
    let program = gl.create_program().ok_or("no program")?;
    gl.attach_shader(&program, &compile(GL::VERTEX_SHADER, vertex)?);
    gl.attach_shader(&program, &compile(GL::FRAGMENT_SHADER, fragment)?);
    gl.link_program(&program);
    if gl.get_program_parameter(&program, GL::LINK_STATUS).as_bool() != Some(true) {
        return Err(gl.get_program_info_log(&program).unwrap_or_default());
    }
    Ok(program)
}

fn mesh(gl: &GL) -> Mesh {
    let vao = gl.create_vertex_array().unwrap();
    let buffer = gl.create_buffer().unwrap();
    gl.bind_vertex_array(Some(&vao));
    gl.bind_buffer(GL::ARRAY_BUFFER, Some(&buffer));
    let stride = (FLOATS * 4) as i32;
    for (location, size, offset) in [(0, 3, 0), (1, 3, 3), (2, 2, 6), (3, 2, 8), (4, 1, 10), (5, 3, 11)] {
        gl.enable_vertex_attrib_array(location);
        gl.vertex_attrib_pointer_with_i32(location, size, GL::FLOAT, false, stride, offset * 4);
    }
    Mesh { vao, buffer, count: 0 }
}

fn upload(gl: &GL, mesh: &mut Mesh, v: &[f32]) {
    gl.bind_buffer(GL::ARRAY_BUFFER, Some(&mesh.buffer));
    gl.buffer_data_with_array_buffer_view(GL::ARRAY_BUFFER, &js_sys::Float32Array::from(v), GL::DYNAMIC_DRAW);
    mesh.count = (v.len() / FLOATS) as i32;
}

fn draw_mesh(gl: &GL, mesh: &Mesh) {
    if mesh.count > 0 {
        gl.bind_vertex_array(Some(&mesh.vao));
        gl.draw_arrays(GL::TRIANGLES, 0, mesh.count);
    }
}

// ---- The page's way in ----------------------------------------------------------------------

/// Starts the game on canvas `#canvas_id` with the map's `boxes` (x0, y0, z0, x1, y1, z1,
/// material, ...), watching the game server at `url` (ws://...) until you join.
#[wasm_bindgen]
pub fn start(canvas_id: &str, boxes: Vec<f32>, url: &str) -> Result<(), JsValue> {
    let window = web_sys::window().unwrap();
    let document = window.document().unwrap();
    let canvas: HtmlCanvasElement = document.get_element_by_id(canvas_id).ok_or("no canvas")?.dyn_into()?;
    let gl: GL = canvas.get_context("webgl2")?.ok_or("no WebGL 2")?.dyn_into()?;
    let main = program(&gl, MAIN_VERTEX, MAIN_FRAGMENT)?;
    let shadow = program(&gl, SHADOW_VERTEX, SHADOW_FRAGMENT)?;
    let sky = program(&gl, SKY_VERTEX, SKY_FRAGMENT)?;
    gl.enable(GL::DEPTH_TEST);

    let mut u = HashMap::new();
    for (prog, names) in [
        (&main, &["m_mvp", "m_light", "m_eye", "m_sun", "m_shadow"][..]),
        (&shadow, &["s_light"][..]),
        (&sky, &["k_pitch", "k_tan"][..]),
    ] {
        for &name in names {
            if let Some(loc) = gl.get_uniform_location(prog, name) {
                u.insert(name, loc);
            }
        }
    }

    // The sun's depth map, for shadows: compared as it's read (sampler2DShadow), smoothed.
    let shadow_tex = gl.create_texture().ok_or("no texture")?;
    gl.bind_texture(GL::TEXTURE_2D, Some(&shadow_tex));
    gl.tex_storage_2d(GL::TEXTURE_2D, 1, GL::DEPTH_COMPONENT24, SHADOW, SHADOW);
    for (name, value) in [
        (GL::TEXTURE_MIN_FILTER, GL::LINEAR),
        (GL::TEXTURE_MAG_FILTER, GL::LINEAR),
        (GL::TEXTURE_WRAP_S, GL::CLAMP_TO_EDGE),
        (GL::TEXTURE_WRAP_T, GL::CLAMP_TO_EDGE),
        (GL::TEXTURE_COMPARE_MODE, GL::COMPARE_REF_TO_TEXTURE),
        (GL::TEXTURE_COMPARE_FUNC, GL::LEQUAL),
    ] {
        gl.tex_parameteri(GL::TEXTURE_2D, name, value as i32);
    }
    let shadow_fb = gl.create_framebuffer().ok_or("no framebuffer")?;
    gl.bind_framebuffer(GL::FRAMEBUFFER, Some(&shadow_fb));
    gl.framebuffer_texture_2d(GL::FRAMEBUFFER, GL::DEPTH_ATTACHMENT, GL::TEXTURE_2D, Some(&shadow_tex), 0);
    gl.draw_buffers(&js_sys::Array::of1(&JsValue::from(GL::NONE)));
    gl.read_buffer(GL::NONE);
    gl.bind_framebuffer(GL::FRAMEBUFFER, None);

    let map = Map::new(&boxes);
    let mut v = Vec::new();
    for b in &map.blocks {
        map_box(&mut v, b.min, b.max, b.mat);
    }
    let mut map_mesh = mesh(&gl);
    upload(&gl, &mut map_mesh, &v);

    let socket = WebSocket::new(url)?;
    let element = |id: &str| document.get_element_by_id(id).and_then(|e| e.dyn_into::<HtmlElement>().ok());
    let now = window.performance().unwrap().now();
    let game = Game {
        u,
        shadow_fb,
        shadow_tex,
        sky_vao: gl.create_vertex_array().ok_or("no vao")?,
        map_mesh,
        things: mesh(&gl),
        gun: mesh(&gl),
        main,
        shadow,
        sky,
        gl,
        canvas: canvas.clone(),
        hud_ammo: element("ammo"),
        hud_cross: element("crosshair"),
        hud_hit: element("hitmarker"),
        hud_scope: element("scope"),
        hud_text: String::from("-"),
        hud_gap: 0.0,
        hud_shown: (false, true),
        map,
        socket: socket.clone(),
        open: false,
        joining: None,
        id: None,
        me: Player::new(0, 0),
        before: [0.0; 3],
        unplayed: 0.0,
        seq: 0,
        unacked: VecDeque::new(),
        unsent: Vec::new(),
        yaw: 0.0,
        pitch: 0.0,
        keys: HashSet::new(),
        trigger: false,
        scope_button: false,
        reload_key: false,
        want_gun: RIFLE,
        last_gun: PISTOL,
        shown_gun: RIFLE,
        drawn_at: 0.0,
        eye_height: EYE,
        snapshots: VecDeque::new(),
        clock: None,
        remotes: HashMap::new(),
        tracers: Vec::new(),
        holes: Vec::new(),
        particles: Vec::new(),
        kick: 0.0,
        bob: 0.0,
        shake: 0.0,
        last_shot: f64::MIN,
        hit_at: f64::MIN,
        spawned_at: 0.0,
        seed: (now as u32) | 1,
        told: (0, false, 0),
        now,
        outbox: Vec::new(),
    };
    GAME.with(|cell| *cell.borrow_mut() = Some(game));

    // The game server: snapshots in, inputs out.
    let on_open = Closure::<dyn FnMut()>::new(|| {
        with_game(|g| {
            g.open = true;
            if let Some(name) = g.joining.take() {
                g.send(&ToServer::Join { name });
            }
            g.tell("outpost-status", "\"open\"".into());
        });
    });
    socket.set_onopen(Some(on_open.as_ref().unchecked_ref()));
    on_open.forget();
    let on_message = Closure::<dyn FnMut(MessageEvent)>::new(|e: MessageEvent| {
        if let Some(text) = e.data().as_string() {
            with_game(|g| g.receive(&text));
        }
    });
    socket.set_onmessage(Some(on_message.as_ref().unchecked_ref()));
    on_message.forget();
    let on_close = Closure::<dyn FnMut()>::new(|| {
        with_game(|g| {
            g.open = false;
            g.tell("outpost-status", "\"closed\"".into());
        });
    });
    socket.set_onclose(Some(on_close.as_ref().unchecked_ref()));
    on_close.forget();

    let on_mouse_down = Closure::<dyn FnMut(MouseEvent)>::new(move |e: MouseEvent| {
        with_game(|g| {
            if !g.locked() {
                g.canvas.request_pointer_lock();
            } else if e.button() == 0 {
                g.trigger = true;
            } else if e.button() == 2 {
                g.scope_button = true;
            }
        });
    });
    canvas.add_event_listener_with_callback("mousedown", on_mouse_down.as_ref().unchecked_ref())?;
    on_mouse_down.forget();

    let on_mouse_up = Closure::<dyn FnMut(MouseEvent)>::new(|e: MouseEvent| {
        with_game(|g| {
            if e.button() == 0 {
                g.trigger = false;
            }
            if e.button() == 2 {
                g.scope_button = false;
            }
        });
    });
    document.add_event_listener_with_callback("mouseup", on_mouse_up.as_ref().unchecked_ref())?;
    on_mouse_up.forget();

    let no_menu = Closure::<dyn FnMut(MouseEvent)>::new(|e: MouseEvent| e.prevent_default());
    canvas.add_event_listener_with_callback("contextmenu", no_menu.as_ref().unchecked_ref())?;
    no_menu.forget();

    let on_mouse_move = Closure::<dyn FnMut(MouseEvent)>::new(|e: MouseEvent| {
        with_game(|g| {
            if g.locked() {
                // Slower through a scope, so the same hand movement covers the same on screen.
                let speed = 0.0022 * (g.fov() / 2.0).tan() / (FOV / 2.0).tan();
                g.yaw += e.movement_x() as f32 * speed;
                g.pitch = (g.pitch - e.movement_y() as f32 * speed).clamp(-1.5, 1.5);
            }
        });
    });
    document.add_event_listener_with_callback("mousemove", on_mouse_move.as_ref().unchecked_ref())?;
    on_mouse_move.forget();

    let on_key_down = Closure::<dyn FnMut(KeyboardEvent)>::new(|e: KeyboardEvent| {
        with_game(|g| {
            if !g.locked() {
                return; // typing in the page, not playing
            }
            e.prevent_default();
            let code = e.code();
            if code == "KeyR" {
                g.reload_key = true;
            }
            match code.as_str() {
                "Digit1" => g.choose(RIFLE),
                "Digit2" => g.choose(PISTOL),
                "Digit3" => g.choose(SNIPER),
                "KeyQ" if !e.repeat() => g.choose(g.last_gun),
                _ => {}
            }
            if code == "Tab" && !e.repeat() {
                g.outbox.push(("outpost-scores", numbers(&[1.0])));
            }
            g.keys.insert(code);
        });
    });
    document.add_event_listener_with_callback("keydown", on_key_down.as_ref().unchecked_ref())?;
    on_key_down.forget();

    let on_key_up = Closure::<dyn FnMut(KeyboardEvent)>::new(|e: KeyboardEvent| {
        with_game(|g| {
            if e.code() == "Tab" {
                g.outbox.push(("outpost-scores", numbers(&[0.0])));
            }
            g.keys.remove(&e.code());
        });
    });
    document.add_event_listener_with_callback("keyup", on_key_up.as_ref().unchecked_ref())?;
    on_key_up.forget();

    // The wheel goes round the guns.
    let on_wheel = Closure::<dyn FnMut(web_sys::WheelEvent)>::new(|e: web_sys::WheelEvent| {
        with_game(|g| {
            if g.locked() && e.delta_y() != 0.0 {
                let n = GUNS.len() as u8;
                let next = if e.delta_y() > 0.0 { (g.want_gun + 1) % n } else { (g.want_gun + n - 1) % n };
                g.choose(next);
            }
        });
    });
    canvas.add_event_listener_with_callback("wheel", on_wheel.as_ref().unchecked_ref())?;
    on_wheel.forget();

    // The loop: one frame per animation frame, for as long as the page is open.
    let next: Rc<RefCell<Option<Closure<dyn FnMut(f64)>>>> = Rc::new(RefCell::new(None));
    let first = next.clone();
    *first.borrow_mut() = Some(Closure::new(move |now: f64| {
        with_game(|g| g.frame(now));
        let window = web_sys::window().unwrap();
        let _ = window.request_animation_frame(next.borrow().as_ref().unwrap().as_ref().unchecked_ref());
    }));
    window.request_animation_frame(first.borrow().as_ref().unwrap().as_ref().unchecked_ref())?;
    Ok(())
}

/// Joins the game as `name`: now if the socket's open, else once it is.
#[wasm_bindgen]
pub fn join(name: &str) {
    with_game(|g| {
        if g.id.is_some() {
            return;
        }
        if g.open {
            g.send(&ToServer::Join { name: name.to_string() });
        } else {
            g.joining = Some(name.to_string());
        }
    });
}
