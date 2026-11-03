//! Rally: table tennis against the computer, drawn with WebGL 2 (an overhead light's shadows, the
//! table, net and floor textured in the shader). The rules and the ball's flight are sim.rs; this
//! is the room, the mouse, and telling the page what happened.
//!
//! The page talks to it through window:
//! - in: `start(canvasId)`, `play(level)` (0 easy, 1 normal, 2 hard: a new match), `logo(canvas)`
//!   (an image for the table);
//! - out: CustomEvents `rally-state` ({phase, score, games, server, why, last, paused}, JSON) when any of
//!   it changes, `rally-end` ({games, points}, JSON) when a match is over, and `rally-sound`
//!   [kind, volume].
//!
//! Until the first `play`, the computer plays itself (silently, as "idle" to the page).
//!
//! Controls: click to play (pointer lock); the mouse moves your paddle (up is toward the net);
//! swing forward through the ball to hit it, harder for more pace, sideways to place it; hold the
//! right button for backspin; click to toss when it's your serve. Esc pauses.

pub mod sim;

use sim::*;
use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;
use wasm_bindgen::prelude::*;
use wasm_bindgen::JsCast;
use web_sys::{
    HtmlCanvasElement, MouseEvent, WebGl2RenderingContext as GL, WebGlBuffer, WebGlFramebuffer, WebGlProgram,
    WebGlShader, WebGlTexture, WebGlUniformLocation, WebGlVertexArrayObject,
};

const SHADOW: i32 = 2048;
const FLOATS: usize = 14; // per vertex: position, normal, face uv, face size, material, colour

// Materials: the room's (made in the shader), then a plain colour, one that glows (no light),
// one with a shine, and skin.
const TABLE: f32 = 0.0;
const FLOOR: f32 = 1.0;
const WALL: f32 = 2.0;
const NET: f32 = 3.0;
const FLAT: f32 = 10.0;
const GLOW: f32 = 11.0;
const SHINY: f32 = 12.0;
const SKIN_MAT: f32 = 13.0;

// Sounds, for the page: `rally-sound` [kind, volume].
const SOUND_BOUNCE: f64 = 0.0;
const SOUND_HIT: f64 = 1.0;
const SOUND_NET: f64 = 2.0;
const SOUND_FLOOR: f64 = 3.0;
const SOUND_WON: f64 = 4.0;
const SOUND_LOST: f64 = 5.0;
const SOUND_LET: f64 = 6.0;

const FOV: f32 = 50.0 * std::f32::consts::PI / 180.0;
/// How far the paddle goes for a pixel of mouse.
const MOUSE: f32 = 0.0024;

const SKIN: V3 = [0.78, 0.6, 0.47];
const WOOD: V3 = [0.66, 0.48, 0.3];
const RED_RUBBER: V3 = [0.72, 0.08, 0.07];
const BLACK_RUBBER: V3 = [0.05, 0.05, 0.055];
const SHIRT: V3 = [0.86, 0.34, 0.1];
const SHORTS: V3 = [0.1, 0.11, 0.14];
const HAIR: V3 = [0.16, 0.11, 0.08];
const BALL: V3 = [0.98, 0.95, 0.88];
const STEEL: V3 = [0.2, 0.21, 0.23];

type Mat = [f32; 16];

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

fn look(eye: V3, f: V3, up: V3) -> Mat {
    let s = normalize(cross(f, up));
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

fn vertex(v: &mut Vec<f32>, p: V3, n: V3, uv: [f32; 2], size: [f32; 2], mat: f32, col: V3) {
    v.extend_from_slice(&[p[0], p[1], p[2], n[0], n[1], n[2], uv[0], uv[1], size[0], size[1], mat, col[0], col[1], col[2]]);
}

/// A box from `min` to `max` in some body's space, put in the world by `at`. Each face keeps its
/// own coordinates, so the shader can soften its edges.
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
            let l = local[i];
            vertex(v, p[i], n, [l[u] - min[u], l[w] - min[w]], [max[u] - min[u], max[w] - min[w]], mat, col);
        }
    }
}

fn cube(v: &mut Vec<f32>, min: V3, max: V3, mat: f32, col: V3) {
    part(v, min, max, mat, col, &|p| p);
}

/// Two perpendiculars to `f`, turning the right way (so faces built on them point out).
fn across(f: V3) -> (V3, V3) {
    let side = normalize(if f[1].abs() > 0.99 { cross(f, [0.0, 0.0, 1.0]) } else { cross(f, [0.0, 1.0, 0.0]) });
    (side, cross(f, side))
}

/// A bar from `a` to `b`, `half` wide across and up: limbs, handles, legs.
fn limb(v: &mut Vec<f32>, a: V3, b: V3, half: [f32; 2], mat: f32, col: V3) {
    let d = sub(b, a);
    let l = len(d);
    if l < 1e-4 {
        return;
    }
    let f = mul(d, 1.0 / l);
    let (side, up) = across(f);
    let place = |p: V3| add(a, add(add(mul(side, p[0]), mul(up, p[1])), mul(f, p[2])));
    part(v, [-half[0], -half[1], 0.0], [half[0], half[1], l], mat, col, &place);
}

/// A ball: smooth, so no face coordinates (the shader leaves its edges alone).
fn sphere(v: &mut Vec<f32>, c: V3, r: f32, mat: f32, col: V3) {
    blob(v, c, [r; 3], mat, col);
}

/// An ellipsoid, `r` across, up and along: heads, hands, shoes, a torso.
fn blob(v: &mut Vec<f32>, c: V3, r: V3, mat: f32, col: V3) {
    let (rings, segments) = (8, 14);
    let at = |i: usize, j: usize| {
        let (t, p) = (std::f32::consts::PI * i as f32 / rings as f32, std::f32::consts::TAU * j as f32 / segments as f32);
        [t.sin() * p.cos(), t.cos(), t.sin() * p.sin()]
    };
    for i in 0..rings {
        for j in 0..segments {
            let q = [at(i, j), at(i + 1, j), at(i + 1, j + 1), at(i, j + 1)];
            for k in [0, 2, 1, 0, 3, 2] {
                let u = q[k];
                let n = normalize([u[0] / r[0], u[1] / r[1], u[2] / r[2]]);
                vertex(v, add(c, [u[0] * r[0], u[1] * r[1], u[2] * r[2]]), n, [0.0; 2], [0.0; 2], mat, col);
            }
        }
    }
}

/// A round limb from `a` to `b`, `ra` thick at `a` and `rb` at `b`, its ends rounded off.
fn tube(v: &mut Vec<f32>, a: V3, b: V3, ra: f32, rb: f32, mat: f32, col: V3) {
    let d = sub(b, a);
    let l = len(d);
    if l < 1e-4 {
        return;
    }
    let f = mul(d, 1.0 / l);
    let (s, u) = across(f);
    let segments = 10;
    let slope = (ra - rb) / l;
    let round = |j: usize| {
        let t = std::f32::consts::TAU * j as f32 / segments as f32;
        add(mul(s, t.cos()), mul(u, t.sin()))
    };
    for j in 0..segments {
        let (e0, e1) = (round(j), round(j + 1));
        let (n0, n1) = (normalize(add(e0, mul(f, slope))), normalize(add(e1, mul(f, slope))));
        let corners = [
            (add(a, mul(e0, ra)), n0),
            (add(a, mul(e1, ra)), n1),
            (add(b, mul(e1, rb)), n1),
            (add(b, mul(e0, rb)), n0),
        ];
        for k in [0, 1, 2, 0, 2, 3] {
            vertex(v, corners[k].0, corners[k].1, [0.0; 2], [0.0; 2], mat, col);
        }
    }
    sphere(v, a, ra, mat, col);
    sphere(v, b, rb, mat, col);
}

/// Where the middle joint (elbow, knee) goes on a limb from `a` to `b` of two parts `l1` and `l2`
/// long: bent toward `pole`.
fn bend(a: V3, b: V3, l1: f32, l2: f32, pole: V3) -> V3 {
    let d = sub(b, a);
    let f = normalize(d);
    let reach = len(d).clamp(0.05, l1 + l2 - 0.002);
    let x = (l1 * l1 - l2 * l2 + reach * reach) / (2.0 * reach);
    let h = (l1 * l1 - x * x).max(0.0).sqrt();
    let p = normalize(sub(pole, mul(f, dot(pole, f))));
    add(a, add(mul(f, x), mul(p, h)))
}

/// A flat cylinder: a disc of radius `r` facing `n` around `c`, from `c + n * a` to `c + n * b`.
fn disc(v: &mut Vec<f32>, c: V3, n: V3, a: f32, b: f32, r: f32, mat: f32, col: V3) {
    let (s, u) = across(n);
    let segments = 22;
    let rim = |j: usize| {
        let t = std::f32::consts::TAU * j as f32 / segments as f32;
        add(mul(s, t.cos()), mul(u, t.sin()))
    };
    let (front, back) = (add(c, mul(n, b)), add(c, mul(n, a)));
    let away = mul(n, -1.0);
    for j in 0..segments {
        let (e0, e1) = (rim(j), rim(j + 1));
        let f = [add(front, mul(e0, r)), add(front, mul(e1, r))];
        let k = [add(back, mul(e0, r)), add(back, mul(e1, r))];
        for p in [front, f[0], f[1]] {
            vertex(v, p, n, [0.0; 2], [0.0; 2], mat, col);
        }
        for p in [back, k[1], k[0]] {
            vertex(v, p, away, [0.0; 2], [0.0; 2], mat, col);
        }
        for (p, nn) in [(k[0], e0), (k[1], e1), (f[1], e1), (k[0], e0), (f[1], e1), (f[0], e0)] {
            vertex(v, p, nn, [0.0; 2], [0.0; 2], mat, col);
        }
    }
}

/// The room, table and all, but the net.
fn room(v: &mut Vec<f32>) {
    cube(v, [-9.0, -0.1, -10.0], [9.0, 0.0, 8.0], FLOOR, [0.0; 3]);
    cube(v, [-9.0, 0.0, -10.2], [9.0, 7.0, -10.0], WALL, [0.0; 3]);
    cube(v, [-9.2, 0.0, -10.0], [-9.0, 7.0, 8.0], WALL, [0.0; 3]);
    cube(v, [9.0, 0.0, -10.0], [9.2, 7.0, 8.0], WALL, [0.0; 3]);
    // Light strips along the far wall, and the boards round the court, white along the top.
    for x in [-6.0, -2.0, 2.0, 6.0] {
        cube(v, [x - 1.4, 4.2, -9.99], [x + 1.4, 4.32, -9.95], GLOW, [1.0, 0.97, 0.9]);
    }
    let board = [0.09, 0.17, 0.36];
    let white = [0.9, 0.9, 0.9];
    cube(v, [-4.2, 0.0, -6.0], [4.2, 0.72, -5.94], FLAT, board);
    cube(v, [-4.2, 0.72, -6.0], [4.2, 0.75, -5.94], FLAT, white);
    for x in [-4.2, 4.14] {
        cube(v, [x, 0.0, -6.0], [x + 0.06, 0.72, 4.5], FLAT, board);
        cube(v, [x, 0.72, -6.0], [x + 0.06, 0.75, 4.5], FLAT, white);
    }

    // The table: its top, a frame under it, legs on feet.
    cube(v, [-HALF_W, TABLE_Y - 0.025, -HALF_L], [HALF_W, TABLE_Y, HALF_L], TABLE, [0.0; 3]);
    cube(v, [-HALF_W + 0.05, TABLE_Y - 0.09, -HALF_L + 0.06], [HALF_W - 0.05, TABLE_Y - 0.025, HALF_L - 0.06], FLAT, STEEL);
    for z in [-1.05f32, 1.05] {
        for x in [-0.62f32, 0.62] {
            cube(v, [x - 0.03, 0.06, z - 0.03], [x + 0.03, TABLE_Y - 0.09, z + 0.03], SHINY, STEEL);
            cube(v, [x - 0.035, 0.0, z - 0.035], [x + 0.035, 0.06, z + 0.035], FLAT, [0.06, 0.06, 0.06]);
        }
        cube(v, [-0.62, 0.3, z - 0.02], [0.62, 0.34, z + 0.02], SHINY, STEEL);
    }
    cube(v, [-0.02, 0.3, -1.05], [0.02, 0.34, 1.05], SHINY, STEEL);
    // The net's posts, clamped to the table's sides.
    for s in [-1.0f32, 1.0] {
        let (a, b) = if s > 0.0 { (HALF_W - 0.04, NET_HALF + 0.015) } else { (-NET_HALF - 0.015, -HALF_W + 0.04) };
        cube(v, [a, TABLE_Y - 0.035, -0.02], [b, TABLE_Y + 0.005, 0.02], SHINY, STEEL);
        let x = s * (NET_HALF + 0.005);
        cube(v, [x - 0.01, TABLE_Y, -0.012], [x + 0.01, NET_TOP + 0.005, 0.012], SHINY, STEEL);
    }
}

/// `side`'s paddle at `p`, moving `vel`: red rubber toward the other end, black behind, on a
/// wooden handle. Yours has a hand and forearm. Where the hand holds it.
fn paddle(v: &mut Vec<f32>, side: usize, p: V3, vel: V3, chop: bool) -> V3 {
    let s = sign(side);
    // Closed (tipped over the ball) as you swing forward; open for backspin.
    let forward = (-vel[2] * s).clamp(0.0, 4.0);
    let tilt = if chop { -0.55 } else { 0.12 + 0.07 * forward };
    let n = normalize([vel[0] * 0.04, -tilt, -s]);
    // The handle points down and out to the right hand's side (+x for you, -x for the computer).
    let h = normalize([0.5 * s, -1.0, 0.0]);
    let down = normalize(sub(h, mul(n, dot(h, n))));
    disc(v, p, n, 0.0015, 0.0055, PADDLE_R * 0.94, SHINY, RED_RUBBER);
    disc(v, p, n, -0.0055, -0.0015, PADDLE_R * 0.94, FLAT, BLACK_RUBBER);
    disc(v, p, n, -0.0015, 0.0015, PADDLE_R * 0.95, FLAT, WOOD);
    let grip = add(p, mul(down, PADDLE_R * 0.9));
    let end = add(p, mul(down, PADDLE_R + 0.1));
    limb(v, grip, end, [0.013, 0.011], FLAT, WOOD);
    let hand = add(p, mul(down, PADDLE_R + 0.05));
    if side == YOU {
        blob(v, hand, [0.032, 0.042, 0.036], SKIN_MAT, SKIN);
        let wrist = add(hand, [0.05, -0.05, 0.09]);
        tube(v, hand, add(wrist, [0.06, -0.06, 0.12]), 0.027, 0.034, SKIN_MAT, SKIN);
        tube(v, add(wrist, [0.04, -0.04, 0.08]), add(wrist, [0.14, -0.13, 0.3]), 0.042, 0.048, FLAT, [0.14, 0.32, 0.6]);
    }
    hand
}

/// The computer's body. Its feet step to keep up as it moves; it swings, and pumps a fist when
/// it wins a point.
struct Figure {
    feet: [V3; 2],
    /// A foot on its way: from, to, since (ms).
    steps: [Option<(V3, V3, f64)>; 2],
    /// When it last hit the ball, and when it last won or lost a point (and which).
    swung: f64,
    point: (f64, bool),
}

const TURN: f32 = std::f32::consts::PI;

impl Figure {
    fn new() -> Figure {
        Figure { feet: [[0.0, 0.0, -2.2], [0.4, 0.0, -2.2]], steps: [None, None], swung: f64::MIN, point: (f64::MIN, false) }
    }

    /// Where the paddle's drawn: where it is, drawn back as the ball comes, and carried through
    /// after a hit (forward, up and across).
    fn swing(&self, paddle: V3, ball: &Ball, coming: bool, now: f64) -> V3 {
        let mut p = paddle;
        if coming && ball.v[2] < -0.5 {
            let t = (ball.p[2] - paddle[2]) / -ball.v[2];
            if t > 0.0 && t < 0.32 {
                let back = (TURN * (0.32 - t) / 0.32).sin();
                p = add(p, mul([-0.08, -0.06, -0.16], back));
            }
        }
        let k = ((now - self.swung) / 300.0) as f32;
        if (0.0..1.0).contains(&k) {
            p = add(p, mul([0.2, 0.15, 0.2], (k * TURN).sin()));
        }
        p
    }

    /// The feet: one at a time, each stepping when the body's moved off it.
    fn walk(&mut self, body: V3, now: f64) {
        let home = [[body[0] - 0.2, 0.0, body[2] - 0.04], [body[0] + 0.2, 0.0, body[2] + 0.06]];
        for i in 0..2 {
            if let Some((from, to, since)) = self.steps[i] {
                let k = ((now - since) / 150.0) as f32;
                if k >= 1.0 {
                    self.feet[i] = to;
                    self.steps[i] = None;
                } else {
                    self.feet[i] = lerp(from, to, k);
                    self.feet[i][1] = (k * TURN).sin() * 0.07;
                }
            }
        }
        if self.steps.iter().all(|s| s.is_none()) {
            let off = |i: usize| ((self.feet[i][0] - home[i][0]).powi(2) + (self.feet[i][2] - home[i][2]).powi(2)).sqrt();
            let i = if off(0) > off(1) { 0 } else { 1 };
            if off(i) > 0.13 {
                // A little past where it's needed, the way it's going.
                let to = add(home[i], mul(normalize(sub(home[i], self.feet[i])), 0.05));
                self.steps[i] = Some((self.feet[i], [to[0], 0.0, to[2]], now));
            }
        }
    }

    /// Drawn holding its paddle at `grip`, its body at `body` (hips, over the floor).
    fn draw(&self, v: &mut Vec<f32>, body: V3, grip: V3, now: f64) {
        let t = (now / 1000.0) as f32;
        let mid = [(self.feet[0][0] + self.feet[1][0]) / 2.0, 0.0, (self.feet[0][2] + self.feet[1][2]) / 2.0];
        // Hips between the feet and where it's heading, low in the ready stance, dipping as it
        // steps; the chest over the table, leaning toward the paddle.
        let stepping = self.feet[0][1].max(self.feet[1][1]);
        let hips = [(mid[0] + body[0]) / 2.0, 0.83 + (t * 3.0).sin() * 0.006 - stepping * 0.3, (mid[2] + body[2]) / 2.0];
        let reach = ((grip[0] - hips[0] + 0.3) * 0.12).clamp(-0.1, 0.1);
        let chest = add(hips, [reach, 0.45, 0.13]);
        let since = ((now - self.point.0) / 900.0) as f32;
        let (cheer, glum) = match (0.0..1.0).contains(&since) {
            true if self.point.1 => ((since * TURN).sin().min(0.7) / 0.7, 0.0),
            true => (0.0, (since * TURN).sin()),
            false => (0.0, 0.0),
        };

        // Legs: knees forward and a little out, socks and shoes.
        for (i, out) in [(0, -1.0f32), (1, 1.0)] {
            let hip = add(hips, [out * 0.09, -0.04, 0.0]);
            let ankle = add(self.feet[i], [0.0, 0.08, 0.0]);
            let knee = bend(hip, ankle, 0.43, 0.42, [out * 0.3, 0.0, 1.0]);
            tube(v, hip, lerp(hip, knee, 0.55), 0.078, 0.066, FLAT, SHORTS);
            tube(v, lerp(hip, knee, 0.5), knee, 0.06, 0.05, SKIN_MAT, SKIN);
            tube(v, knee, ankle, 0.05, 0.034, SKIN_MAT, SKIN);
            tube(v, lerp(knee, ankle, 0.8), ankle, 0.037, 0.035, FLAT, [0.95, 0.95, 0.93]);
            blob(v, add(self.feet[i], [0.0, 0.04, 0.05]), [0.048, 0.04, 0.12], FLAT, [0.93, 0.93, 0.9]);
            blob(v, add(self.feet[i], [0.0, 0.012, 0.05]), [0.05, 0.014, 0.125], FLAT, [0.2, 0.22, 0.26]);
        }

        // Shorts, shirt, neck.
        blob(v, add(hips, [0.0, 0.02, 0.0]), [0.17, 0.11, 0.12], FLAT, SHORTS);
        // One torso, broad at the chest and narrowing to the waist.
        blob(v, lerp(hips, chest, 0.62), [0.175, 0.27, 0.115], FLAT, SHIRT);
        blob(v, lerp(hips, chest, 0.25), [0.152, 0.14, 0.105], FLAT, SHIRT);
        let neck = add(chest, [0.0, 0.13, 0.0]);
        tube(v, neck, add(neck, [0.0, 0.08, 0.01]), 0.048, 0.044, SKIN_MAT, SKIN);

        // The head, a little down after losing a point; a face toward you.
        let head = add(neck, [0.0, 0.17 - glum * 0.03, 0.02 + glum * 0.04]);
        blob(v, head, [0.088, 0.112, 0.1], SKIN_MAT, SKIN);
        blob(v, add(head, [0.0, 0.035, -0.028]), [0.096, 0.098, 0.1], FLAT, HAIR);
        for out in [-1.0f32, 1.0] {
            blob(v, add(head, [out * 0.033, 0.012, 0.086]), [0.013, 0.011, 0.01], FLAT, [0.08, 0.06, 0.05]);
            blob(v, add(head, [out * 0.034, 0.04, 0.088]), [0.022, 0.006, 0.008], FLAT, HAIR);
            blob(v, add(head, [out * 0.088, 0.0, 0.0]), [0.014, 0.03, 0.02], SKIN_MAT, SKIN);
        }
        blob(v, add(head, [0.0, -0.012, 0.098]), [0.014, 0.024, 0.018], SKIN_MAT, SKIN);
        blob(v, add(head, [0.0, -0.055, 0.086]), [0.024, 0.006, 0.006], FLAT, [0.45, 0.2, 0.18]);

        // The paddle arm (its right: -x, facing you) bent to the grip; the other out in front for
        // balance, or up in a fist.
        let shoulder = add(chest, [-0.165, 0.06, 0.0]);
        let elbow = bend(shoulder, grip, 0.29, 0.27, [-0.6, -0.6, -0.3]);
        self.arm(v, shoulder, elbow, grip);
        blob(v, grip, [0.034, 0.042, 0.036], SKIN_MAT, SKIN);
        let shoulder = add(chest, [0.165, 0.06, 0.0]);
        let ready = add(chest, [0.24, -0.2, 0.3]);
        let fist = add(shoulder, [0.1, 0.42, 0.1]);
        let hand = lerp(ready, fist, cheer);
        let elbow = bend(shoulder, hand, 0.29, 0.27, lerp([0.6, -0.8, -0.2], [1.0, -0.2, 0.4], cheer));
        self.arm(v, shoulder, elbow, hand);
        blob(v, hand, [0.036, 0.036, 0.036], SKIN_MAT, SKIN);
    }

    fn arm(&self, v: &mut Vec<f32>, shoulder: V3, elbow: V3, hand: V3) {
        tube(v, shoulder, lerp(shoulder, elbow, 0.5), 0.054, 0.05, FLAT, SHIRT);
        tube(v, lerp(shoulder, elbow, 0.45), elbow, 0.046, 0.04, SKIN_MAT, SKIN);
        tube(v, elbow, hand, 0.04, 0.028, SKIN_MAT, SKIN);
    }
}

// ---- The game -------------------------------------------------------------------------------

struct Mesh {
    vao: WebGlVertexArrayObject,
    buffer: WebGlBuffer,
    count: i32,
}

struct Game {
    gl: GL,
    canvas: HtmlCanvasElement,
    main: WebGlProgram,
    shadow: WebGlProgram,
    u: HashMap<&'static str, WebGlUniformLocation>,
    shadow_fb: WebGlFramebuffer,
    /// The logo on the table, once the page has handed it over (`logo`).
    logo: Option<WebGlTexture>,
    shadow_tex: WebGlTexture,
    room: Mesh,
    net: Mesh,
    things: Mesh,

    rally: Rally,
    figure: Figure,
    /// Before your first match the computer plays itself, quietly.
    demo: bool,
    /// Where you want your paddle (x, z): the mouse's, ahead of the ticks.
    want: [f32; 2],
    chop: bool,
    /// A click to toss, kept until a tick takes it.
    toss: bool,
    /// Time not played yet, in seconds (under a tick); the ball and paddles a tick ago, as frames
    /// are drawn between the last two ticks.
    unplayed: f32,
    before: (V3, [V3; 2]),
    /// Points won this match, for the page's results, and who won the last (-1 none yet).
    points: [u32; 2],
    last: i32,
    /// What the page was last told.
    told: String,
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

impl Game {
    fn sound(&mut self, kind: f64, volume: f32) {
        self.outbox.push(("rally-sound", numbers(&[kind, volume.clamp(0.0, 1.0) as f64])));
    }

    fn locked(&self) -> bool {
        let document = web_sys::window().unwrap().document().unwrap();
        document.pointer_lock_element().is_some_and(|e| JsValue::from(e) == JsValue::from(self.canvas.clone()))
    }

    fn frame(&mut self, now: f64) {
        let dt = (((now - self.now) / 1000.0) as f32).clamp(0.0, 0.1);
        self.now = now;
        let playing = self.locked();
        if !playing {
            self.chop = false;
        }
        if self.demo && self.rally.phase == Phase::Over {
            self.rally.start(1);
        }
        // Your serve: your paddle starts behind the table's end, so a push forward is a swing.
        let serving = matches!(self.rally.phase, Phase::Ready { .. } | Phase::Tossed) && self.rally.server() == YOU;
        if serving {
            self.want[1] = self.want[1].max(HALF_L + 0.1);
        }
        // Paused while you're not playing (Esc), so the computer doesn't win points meanwhile.
        if playing || self.demo {
            self.unplayed += dt;
            while self.unplayed >= TICK {
                self.unplayed -= TICK;
                self.before = (self.rally.ball.p, [self.rally.paddles[0].p, self.rally.paddles[1].p]);
                let input = Input { paddle: self.want, chop: self.chop, toss: self.toss };
                self.toss = false;
                self.rally.tick(&input);
                self.heard();
            }
        }
        self.tell(!playing);
        self.draw();
    }

    /// Sounds for what the ball touched, and the score.
    fn heard(&mut self) {
        let events = std::mem::take(&mut self.rally.events);
        for e in &events {
            match *e {
                Event::Hit { who: CPU, .. } => self.figure.swung = self.now,
                Event::Point { winner } => self.figure.point = (self.now, winner == CPU),
                _ => {}
            }
        }
        if self.demo {
            return;
        }
        for e in events {
            match e {
                Event::Bounce { speed } => self.sound(SOUND_BOUNCE, speed / 9.0),
                Event::Hit { who, power } => self.sound(SOUND_HIT, (0.45 + 0.55 * power) * if who == YOU { 1.0 } else { 0.7 }),
                Event::Net { speed } => self.sound(SOUND_NET, speed / 6.0),
                Event::Floor { speed } => self.sound(SOUND_FLOOR, speed / 6.0),
                Event::Point { winner } => {
                    self.points[winner] += 1;
                    self.last = winner as i32;
                    self.sound(if winner == YOU { SOUND_WON } else { SOUND_LOST }, 1.0);
                    if self.rally.phase == Phase::Over {
                        let (g, p) = (self.rally.games, self.points);
                        let json = format!("{{\"games\":[{},{}],\"points\":[{},{}]}}", g[0], g[1], p[0], p[1]);
                        self.outbox.push(("rally-end", JsValue::from_str(&json)));
                    }
                }
                Event::Let => self.sound(SOUND_LET, 1.0),
            }
        }
    }

    /// Tells the page the phase and score, if they've changed.
    fn tell(&mut self, paused: bool) {
        let r = &self.rally;
        let phase = match r.phase {
            _ if self.demo => "idle",
            Phase::Idle => "idle",
            Phase::Ready { .. } => "ready",
            Phase::Tossed => "tossed",
            Phase::Rally => "rally",
            Phase::Point { .. } => "point",
            Phase::Game { .. } => "game",
            Phase::Over => "over",
        };
        let json = format!(
            "{{\"phase\":\"{phase}\",\"score\":[{},{}],\"games\":[{},{}],\"server\":{},\"why\":\"{}\",\"last\":{},\"paused\":{paused}}}",
            r.score[0], r.score[1], r.games[0], r.games[1], r.server(), r.why, self.last
        );
        if json != self.told {
            self.outbox.push(("rally-state", JsValue::from_str(&json)));
            self.told = json;
        }
    }

    fn draw(&mut self) {
        let t = self.unplayed / TICK;
        let ball = lerp(self.before.0, self.rally.ball.p, t);
        let paddles = [lerp(self.before.1[0], self.rally.paddles[0].p, t), lerp(self.before.1[1], self.rally.paddles[1].p, t)];
        let gl = &self.gl;

        let mut v = Vec::new();
        if self.rally.phase != Phase::Idle {
            sphere(&mut v, ball, BALL_R, SHINY, BALL);
            // A faint streak behind a fast ball, so you can read where it's going.
            let speed = len(self.rally.ball.v);
            if speed > 5.0 && self.rally.phase == Phase::Rally {
                let back = mul(self.rally.ball.v, -0.012);
                for i in 1..4 {
                    let f = i as f32 / 4.0;
                    let col = lerp(BALL, [0.2, 0.25, 0.35], 0.25 * i as f32);
                    sphere(&mut v, add(ball, mul(back, f)), BALL_R * (1.0 - 0.22 * i as f32), GLOW, col);
                }
            }
        }
        paddle(&mut v, YOU, paddles[YOU], self.rally.paddles[YOU].v, self.chop);
        let coming = self.rally.phase == Phase::Rally && self.rally.hitter == YOU;
        let theirs = self.figure.swing(paddles[CPU], &Ball { p: ball, ..self.rally.ball }, coming, self.now);
        let grip = paddle(&mut v, CPU, theirs, self.rally.paddles[CPU].v, false);
        let body = [(paddles[CPU][0] + 0.3).clamp(-1.8, 1.8), 0.0, (paddles[CPU][2] - 0.5).min(-1.9)];
        self.figure.walk(body, self.now);
        self.figure.draw(&mut v, body, grip, self.now);
        upload(gl, &mut self.things, &v);

        // Shadows, from the light over the table.
        let light_dir = normalize([0.12, 1.0, 0.18]);
        let light = multiply(&ortho(6.5, 1.0, 30.0), &look(mul(light_dir, 15.0), mul(light_dir, -1.0), [0.0, 0.0, -1.0]));
        gl.bind_framebuffer(GL::FRAMEBUFFER, Some(&self.shadow_fb));
        gl.viewport(0, 0, SHADOW, SHADOW);
        gl.clear(GL::DEPTH_BUFFER_BIT);
        gl.use_program(Some(&self.shadow));
        gl.uniform_matrix4fv_with_f32_array(self.u.get("s_light"), false, &light);
        gl.enable(GL::POLYGON_OFFSET_FILL);
        gl.polygon_offset(2.0, 4.0);
        draw_mesh(gl, &self.room);
        draw_mesh(gl, &self.things);
        gl.disable(GL::POLYGON_OFFSET_FILL);

        // Sharp on high-density screens, but no more than about 1920 x 1200 pixels.
        let (cw, ch) = (self.canvas.client_width().max(1) as f64, self.canvas.client_height().max(1) as f64);
        let scale = web_sys::window().unwrap().device_pixel_ratio().min((2_300_000.0 / (cw * ch)).sqrt());
        let (w, h) = ((cw * scale) as u32, (ch * scale) as u32);
        if self.canvas.width() != w || self.canvas.height() != h {
            self.canvas.set_width(w);
            self.canvas.set_height(h);
        }
        gl.bind_framebuffer(GL::FRAMEBUFFER, None);
        gl.viewport(0, 0, w as i32, h as i32);
        gl.clear_color(0.04, 0.05, 0.07, 1.0);
        gl.clear(GL::COLOR_BUFFER_BIT | GL::DEPTH_BUFFER_BIT);

        // Behind your end, drifting a little with your paddle.
        let x = paddles[YOU][0];
        let eye = [x * 0.22, 1.72, 3.9];
        let target = [x * 0.06, 0.74, -0.7];
        let aspect = w as f32 / h.max(1) as f32;
        // Narrow windows see the table's width all the same.
        let fov = if aspect < 1.25 { 2.0 * ((FOV / 2.0).tan() * 1.25 / aspect).atan() } else { FOV };
        let mvp = multiply(&perspective(fov, aspect, 0.05, 60.0), &look(eye, normalize(sub(target, eye)), [0.0, 1.0, 0.0]));

        gl.use_program(Some(&self.main));
        gl.uniform_matrix4fv_with_f32_array(self.u.get("m_mvp"), false, &mvp);
        gl.uniform_matrix4fv_with_f32_array(self.u.get("m_light"), false, &light);
        gl.uniform3f(self.u.get("m_eye"), eye[0], eye[1], eye[2]);
        gl.uniform3f(self.u.get("m_sun"), light_dir[0], light_dir[1], light_dir[2]);
        gl.active_texture(GL::TEXTURE0);
        gl.bind_texture(GL::TEXTURE_2D, Some(&self.shadow_tex));
        gl.uniform1i(self.u.get("m_shadow"), 0);
        gl.active_texture(GL::TEXTURE1);
        gl.bind_texture(GL::TEXTURE_2D, self.logo.as_ref());
        gl.uniform1i(self.u.get("m_logo"), 1);
        gl.uniform1f(self.u.get("m_has_logo"), if self.logo.is_some() { 1.0 } else { 0.0 });
        draw_mesh(gl, &self.room);
        draw_mesh(gl, &self.net);
        draw_mesh(gl, &self.things);
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
    v_light = m_light * vec4(a_pos + a_normal * 0.004, 1.0);
}"#;

// The room's surfaces are made here, from where on them a pixel is: no image files but the logo.
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
uniform sampler2D m_logo;
uniform float m_has_logo;
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
// 1 within `w` of a line (`d` from it), softly.
float band(float d, float w) { return 1.0 - smoothstep(w - 0.0015, w + 0.0015, d); }

vec3 albedo(int m, vec3 n) {
    if (m == 0) { // the table: blue, white lines on top (round the edge, and down the middle)
        vec3 blue = vec3(0.06, 0.2, 0.42) * (0.95 + 0.06 * noise(v_pos.xz * 60.0));
        if (n.y < 0.5) return blue * 0.6;
        // The logo, in the middle of each half, upright as you look down the table.
        vec2 q = vec2(v_pos.x, v_pos.z - sign(v_pos.z) * 0.685) / 0.62 + 0.5;
        if (m_has_logo > 0.5 && all(greaterThan(q, vec2(0.0))) && all(lessThan(q, vec2(1.0)))) {
            vec4 l = texture(m_logo, q); // premultiplied
            blue = blue * (1.0 - l.a) + l.rgb;
        }
        float edge = max(band(0.7625 - abs(v_pos.x), 0.02), band(1.37 - abs(v_pos.z), 0.02));
        float middle = band(abs(v_pos.x), 0.0015);
        return mix(blue, vec3(0.93, 0.93, 0.9), max(edge, middle));
    }
    if (m == 1) { // a rubber sports floor, red-brown, the court marked out in white
        vec3 c = vec3(0.42, 0.15, 0.11) * (0.9 + 0.12 * fbm(v_pos.xz * 1.5)) * (0.96 + 0.05 * hash(floor(v_pos.xz * 90.0)));
        vec2 d = abs(v_pos.xz - vec2(0.0, -0.75)) - vec2(4.0, 5.15);
        float line = band(abs(max(d.x, d.y)), 0.03);
        return mix(c, vec3(0.85, 0.83, 0.8), line * 0.9);
    }
    if (m == 2) { // dark walls, in panels
        vec2 w = abs(n.x) > 0.5 ? v_pos.zy : v_pos.xy;
        vec2 f = fract(w / vec2(2.4, 1.2));
        float seam = smoothstep(0.0, 0.01, min(min(f.x, 1.0 - f.x), min(f.y, 1.0 - f.y)));
        return vec3(0.07, 0.08, 0.11) * (0.85 + 0.2 * fbm(w * 1.5)) * mix(0.6, 1.0, seam);
    }
    return v_col;
}

void main() {
    int m = int(v_mat + 0.5);
    if (m == 3) { // the net: dark mesh, a white tape along the top
        vec2 g = fract(v_pos.xy / 0.014);
        bool tape = v_pos.y > 0.9125 - 0.016;
        if (!tape && g.x > 0.18 && g.y > 0.18) discard;
        o = vec4(tape ? vec3(0.9, 0.9, 0.88) : vec3(0.05, 0.07, 0.12), 1.0);
        return;
    }
    if (m == 11) { o = vec4(v_col, 1.0); return; }
    vec3 n = normalize(v_normal);
    vec3 a = albedo(m, n);
    if (m >= 10 && v_size.x > 0.0) { // things' edges a little darker, as if rounded
        vec2 e = min(v_uv, v_size - v_uv);
        float bevel = min(0.012, 0.3 * min(v_size.x, v_size.y));
        a *= mix(0.72, 1.0, smoothstep(0.0, bevel, min(e.x, e.y)));
        if (m == 10) a *= 0.94 + 0.07 * noise(v_uv * 200.0);
    }

    vec3 p = v_light.xyz / v_light.w * 0.5 + 0.5;
    float lit = 1.0;
    if (all(greaterThan(p, vec3(0.0))) && all(lessThan(p, vec3(1.0)))) {
        vec2 t = vec2(0.75 / 2048.0);
        lit = 0.25 * (texture(m_shadow, vec3(p.xy + vec2(-t.x, -t.y), p.z - 0.0004))
            + texture(m_shadow, vec3(p.xy + vec2(t.x, -t.y), p.z - 0.0004))
            + texture(m_shadow, vec3(p.xy + vec2(-t.x, t.y), p.z - 0.0004))
            + texture(m_shadow, vec3(p.xy + vec2(t.x, t.y), p.z - 0.0004)));
    }
    // The lights are over the table: brightest there, dimmer out to the walls.
    vec2 off = v_pos.xz - vec2(0.0, -0.3);
    float pool = mix(0.35, 1.0, exp(-dot(off, off) / 22.0));
    float key = max(dot(n, m_sun), 0.0) * lit * pool;
    vec3 ambient = vec3(0.32, 0.34, 0.4) * (0.6 + 0.4 * (n.y * 0.5 + 0.5)) * mix(0.5, 1.0, pool);
    vec3 c = a * (ambient + vec3(1.0, 0.96, 0.9) * 1.05 * key);
    vec3 view = normalize(m_eye - v_pos);
    if (m == 12 || m == 13 || m == 0) { // a shine where the light glints off
        float gloss = m == 12 ? 50.0 : m == 0 ? 30.0 : 12.0;
        float spec = pow(max(dot(reflect(-m_sun, n), view), 0.0), gloss) * lit;
        c += vec3(1.0, 0.96, 0.9) * spec * (m == 12 ? 0.7 : m == 0 ? 0.25 : 0.06) * pool;
    }
    if (m >= 10) {
        c += a * 0.45 * max(dot(n, normalize(vec3(0.3, 0.5, 1.0))), 0.0) * pool; // fill light, from behind you
        c += vec3(0.4, 0.45, 0.55) * 0.12 * pow(1.0 - max(dot(n, view), 0.0), 3.0); // a rim of light
    }
    o = vec4(pow(c, vec3(0.92)), 1.0);
}"#;

const SHADOW_VERTEX: &str = r#"#version 300 es
layout(location = 0) in vec3 a_pos;
uniform mat4 s_light;
void main() { gl_Position = s_light * vec4(a_pos, 1.0); }"#;

const SHADOW_FRAGMENT: &str = r#"#version 300 es
precision mediump float;
out vec4 o;
void main() { o = vec4(1.0); }"#;

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

/// Starts the game on canvas `#canvas_id`: the table, waiting for `play`.
#[wasm_bindgen]
pub fn start(canvas_id: &str) -> Result<(), JsValue> {
    let window = web_sys::window().unwrap();
    let document = window.document().unwrap();
    let canvas: HtmlCanvasElement = document.get_element_by_id(canvas_id).ok_or("no canvas")?.dyn_into()?;
    let gl: GL = canvas.get_context("webgl2")?.ok_or("no WebGL 2")?.dyn_into()?;
    let main = program(&gl, MAIN_VERTEX, MAIN_FRAGMENT)?;
    let shadow = program(&gl, SHADOW_VERTEX, SHADOW_FRAGMENT)?;
    gl.enable(GL::DEPTH_TEST);

    let mut u = HashMap::new();
    for (prog, names) in [(&main, &["m_mvp", "m_light", "m_eye", "m_sun", "m_shadow", "m_logo", "m_has_logo"][..]), (&shadow, &["s_light"][..])] {
        for &name in names {
            if let Some(loc) = gl.get_uniform_location(prog, name) {
                u.insert(name, loc);
            }
        }
    }

    // The light's depth map, for shadows: compared as it's read (sampler2DShadow), smoothed.
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

    let mut v = Vec::new();
    room(&mut v);
    let mut room_mesh = mesh(&gl);
    upload(&gl, &mut room_mesh, &v);
    // The net on its own: drawn, but casting no shadow (its holes are cut in the shader).
    let mut v = Vec::new();
    cube(&mut v, [-NET_HALF, TABLE_Y, -0.002], [NET_HALF, NET_TOP, 0.002], NET, [0.0; 3]);
    let mut net = mesh(&gl);
    upload(&gl, &mut net, &v);

    let now = window.performance().unwrap().now();
    let mut rally = Rally::new((now as u32).wrapping_mul(2_654_435_761) | 1);
    rally.level[YOU] = Some(LEVELS[1]);
    rally.start(1);
    let before = (rally.ball.p, [rally.paddles[0].p, rally.paddles[1].p]);
    let want = [rally.paddles[YOU].p[0], rally.paddles[YOU].p[2]];
    let game = Game {
        u,
        shadow_fb,
        logo: None,
        shadow_tex,
        room: room_mesh,
        net,
        things: mesh(&gl),
        main,
        shadow,
        gl,
        canvas: canvas.clone(),
        rally,
        figure: Figure::new(),
        demo: true,
        want,
        chop: false,
        toss: false,
        unplayed: 0.0,
        before,
        points: [0, 0],
        last: -1,
        told: String::new(),
        now,
        outbox: Vec::new(),
    };
    GAME.with(|cell| *cell.borrow_mut() = Some(game));

    let on_mouse_down = Closure::<dyn FnMut(MouseEvent)>::new(|e: MouseEvent| {
        with_game(|g| {
            if !g.locked() {
                g.canvas.request_pointer_lock();
            } else if e.button() == 0 {
                g.toss = true;
            } else if e.button() == 2 {
                g.chop = true;
            }
        });
    });
    canvas.add_event_listener_with_callback("mousedown", on_mouse_down.as_ref().unchecked_ref())?;
    on_mouse_down.forget();

    let on_mouse_up = Closure::<dyn FnMut(MouseEvent)>::new(|e: MouseEvent| {
        with_game(|g| {
            if e.button() == 2 {
                g.chop = false;
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
                g.want[0] = (g.want[0] + e.movement_x() as f32 * MOUSE).clamp(-YOUR_X, YOUR_X);
                g.want[1] = (g.want[1] + e.movement_y() as f32 * MOUSE).clamp(YOUR_Z[0], YOUR_Z[1]);
            }
        });
    });
    document.add_event_listener_with_callback("mousemove", on_mouse_move.as_ref().unchecked_ref())?;
    on_mouse_move.forget();

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

/// The logo to print on the table: a square canvas with it drawn on (the page draws its SVG).
#[wasm_bindgen]
pub fn logo(image: &HtmlCanvasElement) {
    with_game(|g| {
        let gl = &g.gl;
        let tex = gl.create_texture();
        gl.active_texture(GL::TEXTURE1);
        gl.bind_texture(GL::TEXTURE_2D, tex.as_ref());
        gl.pixel_storei(GL::UNPACK_PREMULTIPLY_ALPHA_WEBGL, 1);
        let ok = gl
            .tex_image_2d_with_u32_and_u32_and_html_canvas_element(GL::TEXTURE_2D, 0, GL::RGBA as i32, GL::RGBA, GL::UNSIGNED_BYTE, image)
            .is_ok();
        gl.pixel_storei(GL::UNPACK_PREMULTIPLY_ALPHA_WEBGL, 0);
        if ok {
            gl.generate_mipmap(GL::TEXTURE_2D);
            gl.tex_parameteri(GL::TEXTURE_2D, GL::TEXTURE_MIN_FILTER, GL::LINEAR_MIPMAP_LINEAR as i32);
            gl.tex_parameteri(GL::TEXTURE_2D, GL::TEXTURE_WRAP_S, GL::CLAMP_TO_EDGE as i32);
            gl.tex_parameteri(GL::TEXTURE_2D, GL::TEXTURE_WRAP_T, GL::CLAMP_TO_EDGE as i32);
            g.logo = tex;
        }
    });
}

/// A new match against the computer at `level`: 0 easy, 1 normal, 2 hard.
#[wasm_bindgen]
pub fn play(level: u32) {
    with_game(|g| {
        g.demo = false;
        g.rally.level[YOU] = None;
        g.rally.start(level as usize);
        g.want = [g.rally.paddles[YOU].p[0], g.rally.paddles[YOU].p[2]];
        g.before = (g.rally.ball.p, [g.rally.paddles[0].p, g.rally.paddles[1].p]);
        g.points = [0, 0];
        g.last = -1;
    });
}
