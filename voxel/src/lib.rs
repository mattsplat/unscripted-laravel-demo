//! A shared block world, drawn with WebGL 2. The terrain is made from a fixed seed, so it's the
//! same on every page; the server keeps only the blocks people changed.
//!
//! The page talks to it both ways through window:
//! - in: `start(canvasId)`, `load(edits)` (flat x, y, z, kind...), `set_block`, `set_player`,
//!   `remove_player`;
//! - out: CustomEvents `voxel-edit` [x, y, z, kind, was], `voxel-move` [x, y, z, yaw] (at most
//!   10 a second, and every 2 seconds standing still) and `voxel-select` [kind].
//!
//! Controls: click to look around (pointer lock), WASD to move, space to jump, left click to
//! break, right click to place, 1-7 to pick a block.

use std::cell::RefCell;
use std::collections::{HashMap, HashSet};
use std::rc::Rc;
use wasm_bindgen::prelude::*;
use wasm_bindgen::JsCast;
use web_sys::{
    HtmlCanvasElement, KeyboardEvent, MouseEvent, WebGl2RenderingContext as GL, WebGlBuffer,
    WebGlProgram, WebGlShader, WebGlUniformLocation, WebGlVertexArrayObject,
};

const W: i32 = 64;
const H: i32 = 32;
const D: i32 = 64;
const CHUNK: i32 = 16;

const AIR: u8 = 0;
const GRASS: u8 = 1;
const DIRT: u8 = 2;
const STONE: u8 = 3;
const WOOD: u8 = 4;
const LEAVES: u8 = 5;
const SAND: u8 = 6;
const PLANKS: u8 = 7;
const BEDROCK: u8 = 8;

const SKY: [f32; 3] = [0.62, 0.78, 0.95];
const REACH: f32 = 6.0;

fn color(kind: u8, top: bool) -> [f32; 3] {
    match kind {
        GRASS if top => [0.36, 0.66, 0.25],
        GRASS | DIRT => [0.52, 0.37, 0.24],
        STONE => [0.5, 0.5, 0.53],
        WOOD if top => [0.66, 0.52, 0.32],
        WOOD => [0.4, 0.28, 0.16],
        LEAVES => [0.2, 0.48, 0.18],
        SAND => [0.86, 0.8, 0.56],
        PLANKS => [0.74, 0.58, 0.36],
        _ => [0.18, 0.18, 0.2],
    }
}

/// Each face of a unit cube: its normal, its corners (in order round the face) and its shade.
const FACES: [([i32; 3], [[i32; 3]; 4], f32); 6] = [
    ([1, 0, 0], [[1, 0, 0], [1, 1, 0], [1, 1, 1], [1, 0, 1]], 0.8),
    ([-1, 0, 0], [[0, 0, 1], [0, 1, 1], [0, 1, 0], [0, 0, 0]], 0.8),
    ([0, 1, 0], [[0, 1, 0], [0, 1, 1], [1, 1, 1], [1, 1, 0]], 1.0),
    ([0, -1, 0], [[0, 0, 0], [1, 0, 0], [1, 0, 1], [0, 0, 1]], 0.55),
    ([0, 0, 1], [[0, 0, 1], [1, 0, 1], [1, 1, 1], [0, 1, 1]], 0.68),
    ([0, 0, -1], [[1, 0, 0], [0, 0, 0], [0, 1, 0], [1, 1, 0]], 0.68),
];

// ---- The world ------------------------------------------------------------------------------

fn hash(x: i32, y: i32, z: i32) -> f32 {
    let mut h = (x as u32).wrapping_mul(374_761_393)
        ^ (y as u32).wrapping_mul(668_265_263)
        ^ (z as u32).wrapping_mul(2_246_822_519);
    h = (h ^ (h >> 13)).wrapping_mul(1_274_126_177);
    h ^= h >> 16;
    (h & 0xffff) as f32 / 65535.0
}

fn noise(x: i32, z: i32, scale: f32, seed: i32) -> f32 {
    let (fx, fz) = (x as f32 / scale, z as f32 / scale);
    let (x0, z0) = (fx.floor() as i32, fz.floor() as i32);
    let smooth = |t: f32| t * t * (3.0 - 2.0 * t);
    let (tx, tz) = (smooth(fx - x0 as f32), smooth(fz - z0 as f32));
    let lerp = |a: f32, b: f32, t: f32| a + (b - a) * t;
    let top = lerp(hash(x0, seed, z0), hash(x0 + 1, seed, z0), tx);
    let bottom = lerp(hash(x0, seed, z0 + 1), hash(x0 + 1, seed, z0 + 1), tx);
    lerp(top, bottom, tz)
}

fn height(x: i32, z: i32) -> i32 {
    (6.0 + noise(x, z, 24.0, 1) * 14.0 + noise(x, z, 8.0, 2) * 4.0) as i32
}

struct World {
    blocks: Vec<u8>,
}

impl World {
    fn generate() -> World {
        let mut world = World { blocks: vec![AIR; (W * H * D) as usize] };
        for x in 0..W {
            for z in 0..D {
                let h = height(x, z);
                let beach = h <= 9;
                for y in 0..=h {
                    let kind = match y {
                        0 => BEDROCK,
                        _ if beach && y > h - 3 => SAND,
                        _ if y == h => GRASS,
                        _ if y > h - 4 => DIRT,
                        _ => STONE,
                    };
                    world.set(x, y, z, kind);
                }
            }
        }
        for x in 3..W - 3 {
            for z in 3..D - 3 {
                let h = height(x, z);
                if h <= 10 || h >= H - 8 || hash(x, 7, z) > 0.012 {
                    continue;
                }
                for y in h + 1..=h + 4 {
                    world.set(x, y, z, WOOD);
                }
                for dy in 3..=6 {
                    let r: i32 = if dy >= 5 { 1 } else { 2 };
                    for dx in -r..=r {
                        for dz in -r..=r {
                            if dx.abs() == 2 && dz.abs() == 2 {
                                continue;
                            }
                            if world.get(x + dx, h + dy, z + dz) == AIR {
                                world.set(x + dx, h + dy, z + dz, LEAVES);
                            }
                        }
                    }
                }
            }
        }
        world
    }

    fn inside(x: i32, y: i32, z: i32) -> bool {
        (0..W).contains(&x) && (0..H).contains(&y) && (0..D).contains(&z)
    }

    /// The block at x, y, z: air outside the world.
    fn get(&self, x: i32, y: i32, z: i32) -> u8 {
        if World::inside(x, y, z) {
            self.blocks[((y * D + z) * W + x) as usize]
        } else {
            AIR
        }
    }

    fn set(&mut self, x: i32, y: i32, z: i32, kind: u8) {
        if World::inside(x, y, z) {
            self.blocks[((y * D + z) * W + x) as usize] = kind;
        }
    }

    /// Whether a player can't be at x, y, z: a block, or the world's sides and floor.
    fn solid(&self, x: i32, y: i32, z: i32) -> bool {
        if x < 0 || x >= W || z < 0 || z >= D || y < 0 {
            return true;
        }
        self.get(x, y, z) != AIR
    }

    /// Chunk (cx, cy, cz)'s triangles: x, y, z, r, g, b each corner. Only faces next to air,
    /// darkened in corners (ambient occlusion).
    fn mesh(&self, cx: i32, cy: i32, cz: i32) -> Vec<f32> {
        let mut v = Vec::new();
        let filled = |p: [i32; 3]| self.get(p[0], p[1], p[2]) != AIR;
        for x in cx * CHUNK..(cx + 1) * CHUNK {
            for y in cy * CHUNK..(cy + 1) * CHUNK {
                for z in cz * CHUNK..(cz + 1) * CHUNK {
                    let kind = self.get(x, y, z);
                    if kind == AIR {
                        continue;
                    }
                    let tint = 0.9 + 0.1 * hash(x, y, z);
                    for (n, corners, shade) in FACES {
                        let out = [x + n[0], y + n[1], z + n[2]];
                        if filled(out) {
                            continue;
                        }
                        let base = color(kind, n[1] == 1);
                        let mut quad = [[0.0f32; 6]; 4];
                        let mut ao = [0u8; 4];
                        for (i, c) in corners.iter().enumerate() {
                            // The two neighbours beside this corner, and the one diagonal to it.
                            let axes: Vec<usize> = (0..3).filter(|&a| n[a] == 0).collect();
                            let mut side1 = out;
                            let mut side2 = out;
                            side1[axes[0]] += if c[axes[0]] == 1 { 1 } else { -1 };
                            side2[axes[1]] += if c[axes[1]] == 1 { 1 } else { -1 };
                            let mut corner = side1;
                            corner[axes[1]] = side2[axes[1]];
                            let (s1, s2, cn) = (filled(side1), filled(side2), filled(corner));
                            ao[i] = if s1 && s2 { 0 } else { 3 - (s1 as u8 + s2 as u8 + cn as u8) };
                            let light = shade * tint * (0.55 + 0.15 * ao[i] as f32);
                            quad[i] = [
                                (x + c[0]) as f32,
                                (y + c[1]) as f32,
                                (z + c[2]) as f32,
                                base[0] * light,
                                base[1] * light,
                                base[2] * light,
                            ];
                        }
                        // Split the quad along the diagonal that keeps the shading even.
                        let order = if ao[0] + ao[2] >= ao[1] + ao[3] { [0, 1, 2, 0, 2, 3] } else { [1, 2, 3, 1, 3, 0] };
                        for i in order {
                            v.extend_from_slice(&quad[i]);
                        }
                    }
                }
            }
        }
        v
    }
}

// ---- Matrices (column-major, as WebGL wants) --------------------------------------------------

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

fn look(eye: [f32; 3], f: [f32; 3]) -> Mat {
    let norm = |v: [f32; 3]| {
        let l = (v[0] * v[0] + v[1] * v[1] + v[2] * v[2]).sqrt();
        [v[0] / l, v[1] / l, v[2] / l]
    };
    let cross = |a: [f32; 3], b: [f32; 3]| [a[1] * b[2] - a[2] * b[1], a[2] * b[0] - a[0] * b[2], a[0] * b[1] - a[1] * b[0]];
    let dot = |a: [f32; 3], b: [f32; 3]| a[0] * b[0] + a[1] * b[1] + a[2] * b[2];
    let s = norm(cross(f, [0.0, 1.0, 0.0]));
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

/// A box from `min` to `max` in a body's own space (x to its right, y up, z ahead), turned to
/// face `yaw` and stood at `at`.
fn push_part(v: &mut Vec<f32>, at: [f32; 3], yaw: f32, min: [f32; 3], max: [f32; 3], col: [f32; 3]) {
    let (s, c) = yaw.sin_cos();
    for (_, corners, shade) in FACES {
        for i in [0, 1, 2, 0, 2, 3] {
            let k = corners[i];
            let (x, y, z) = (
                if k[0] == 1 { max[0] } else { min[0] },
                if k[1] == 1 { max[1] } else { min[1] },
                if k[2] == 1 { max[2] } else { min[2] },
            );
            // right = (cos, sin), ahead = (sin, -cos)
            let q = [at[0] + x * c + z * s, at[1] + y, at[2] + x * s - z * c];
            v.extend_from_slice(&[q[0], q[1], q[2], col[0] * shade, col[1] * shade, col[2] * shade]);
        }
    }
}

/// A villager with its feet at `at`, facing `yaw`, its robe in `robe`.
fn villager(v: &mut Vec<f32>, at: [f32; 3], yaw: f32, robe: [f32; 3]) {
    let skin = [0.74, 0.55, 0.42];
    let dark = [robe[0] * 0.7, robe[1] * 0.7, robe[2] * 0.7];
    let mut part = |min: [f32; 3], max: [f32; 3], col: [f32; 3]| push_part(v, at, yaw, min, max, col);
    // Legs, mostly hidden by the robe.
    part([-0.2, 0.0, -0.1], [-0.01, 0.6, 0.1], [0.3, 0.24, 0.2]);
    part([0.01, 0.0, -0.1], [0.2, 0.6, 0.1], [0.3, 0.24, 0.2]);
    // The robe, down past the knees.
    part([-0.25, 0.35, -0.16], [0.25, 1.3, 0.16], robe);
    // Arms folded across the chest, hands in the middle.
    part([-0.32, 0.92, 0.16], [0.32, 1.12, 0.32], dark);
    part([-0.32, 0.92, -0.06], [-0.25, 1.12, 0.16], dark);
    part([0.25, 0.92, -0.06], [0.32, 1.12, 0.16], dark);
    part([-0.1, 0.94, 0.32], [0.1, 1.1, 0.33], skin);
    // The head: tall, with a long nose, eyes and one eyebrow.
    part([-0.21, 1.3, -0.21], [0.21, 1.86, 0.21], skin);
    part([-0.05, 1.4, 0.21], [0.05, 1.6, 0.31], [0.66, 0.48, 0.36]);
    part([-0.14, 1.6, 0.21], [-0.06, 1.66, 0.22], [0.95, 0.95, 0.95]);
    part([0.06, 1.6, 0.21], [0.14, 1.66, 0.22], [0.95, 0.95, 0.95]);
    part([-0.1, 1.6, 0.215], [-0.06, 1.66, 0.225], [0.15, 0.45, 0.2]);
    part([0.06, 1.6, 0.215], [0.1, 1.66, 0.225], [0.15, 0.45, 0.2]);
    part([-0.16, 1.68, 0.21], [0.16, 1.72, 0.225], [0.3, 0.2, 0.12]);
}

// ---- The game -------------------------------------------------------------------------------

struct Mesh {
    vao: WebGlVertexArrayObject,
    buffer: WebGlBuffer,
    count: i32,
}

struct Remote {
    at: [f32; 3],
    target: [f32; 3],
    yaw: f32,
    color: [f32; 3],
}

struct Game {
    gl: GL,
    canvas: HtmlCanvasElement,
    u_mvp: WebGlUniformLocation,
    u_eye: WebGlUniformLocation,
    u_fog: WebGlUniformLocation,
    world: World,
    chunks: Vec<Mesh>,
    dirty: HashSet<usize>,
    others: Mesh,
    outline: Mesh,

    /// Feet, at the middle of the player.
    pos: [f32; 3],
    fall: f32,
    grounded: bool,
    yaw: f32,
    pitch: f32,
    keys: HashSet<String>,
    selected: u8,
    remotes: HashMap<i32, Remote>,

    last_frame: f64,
    last_sent: f64,
    sent: [f32; 4],
    /// CustomEvents to dispatch once the game isn't borrowed (a listener may call back in).
    outbox: Vec<(&'static str, js_sys::Array)>,
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

fn numbers(values: &[f64]) -> js_sys::Array {
    values.iter().map(|&n| JsValue::from_f64(n)).collect()
}

fn chunk_index(cx: i32, cy: i32, cz: i32) -> usize {
    ((cy * (D / CHUNK) + cz) * (W / CHUNK) + cx) as usize
}

impl Game {
    fn eye(&self) -> [f32; 3] {
        [self.pos[0], self.pos[1] + 1.62, self.pos[2]]
    }

    fn forward(&self) -> [f32; 3] {
        let (s, c) = self.yaw.sin_cos();
        let (ps, pc) = self.pitch.sin_cos();
        [s * pc, ps, -c * pc]
    }

    fn collides(&self, p: [f32; 3]) -> bool {
        let min = [p[0] - 0.3, p[1], p[2] - 0.3];
        let max = [p[0] + 0.3, p[1] + 1.8, p[2] + 0.3];
        for x in min[0].floor() as i32..=(max[0] - 1e-4).floor() as i32 {
            for y in min[1].floor() as i32..=(max[1] - 1e-4).floor() as i32 {
                for z in min[2].floor() as i32..=(max[2] - 1e-4).floor() as i32 {
                    if self.world.solid(x, y, z) {
                        return true;
                    }
                }
            }
        }
        false
    }

    /// Whether standing at `p` would overlap another player.
    fn bumps(&self, p: [f32; 3]) -> bool {
        self.remotes.values().any(|r| {
            (p[0] - r.at[0]).abs() < 0.6 && (p[2] - r.at[2]).abs() < 0.6 && (p[1] - r.at[1]).abs() < 1.8
        })
    }

    fn step(&mut self, axis: usize, by: f32) {
        let mut p = self.pos;
        p[axis] += by;
        // Players are solid too. Two already overlapping (both just spawned, say) can still
        // walk apart: only moving into someone new is stopped.
        let blocked = self.collides(p) || (self.bumps(p) && !self.bumps(self.pos));
        if !blocked {
            self.pos = p;
        } else if axis == 1 {
            self.grounded = by < 0.0;
            self.fall = 0.0;
        }
    }

    fn spawn(&mut self) {
        let (x, z) = (W / 2, D / 2);
        let top = (0..H).rev().find(|&y| self.world.get(x, y, z) != AIR).unwrap_or(0);
        self.pos = [x as f32 + 0.5, top as f32 + 1.0, z as f32 + 0.5];
    }

    /// The block looked at, within reach, and the face's normal.
    fn target(&self) -> Option<([i32; 3], [i32; 3])> {
        let o = self.eye();
        let d = self.forward();
        let mut cell = [o[0].floor() as i32, o[1].floor() as i32, o[2].floor() as i32];
        let mut step = [0; 3];
        let mut next = [f32::INFINITY; 3];
        let mut delta = [f32::INFINITY; 3];
        for i in 0..3 {
            if d[i] > 0.0 {
                step[i] = 1;
                next[i] = (cell[i] as f32 + 1.0 - o[i]) / d[i];
                delta[i] = 1.0 / d[i];
            } else if d[i] < 0.0 {
                step[i] = -1;
                next[i] = (o[i] - cell[i] as f32) / -d[i];
                delta[i] = -1.0 / d[i];
            }
        }
        let mut normal = [0; 3];
        loop {
            if self.world.get(cell[0], cell[1], cell[2]) != AIR {
                return Some((cell, normal));
            }
            let i = if next[0] < next[1] && next[0] < next[2] { 0 } else if next[1] < next[2] { 1 } else { 2 };
            if next[i] > REACH {
                return None;
            }
            cell[i] += step[i];
            next[i] += delta[i];
            normal = [0; 3];
            normal[i] = -step[i];
        }
    }

    /// Changes a block and marks the chunks that show it (shading reaches one block over).
    fn put(&mut self, x: i32, y: i32, z: i32, kind: u8) {
        if !World::inside(x, y, z) {
            return;
        }
        self.world.set(x, y, z, kind);
        for dx in -1..=1 {
            for dy in -1..=1 {
                for dz in -1..=1 {
                    let (bx, by, bz) = (x + dx, y + dy, z + dz);
                    if World::inside(bx, by, bz) {
                        self.dirty.insert(chunk_index(bx / CHUNK, by / CHUNK, bz / CHUNK));
                    }
                }
            }
        }
    }

    fn edit(&mut self, x: i32, y: i32, z: i32, kind: u8) {
        let was = self.world.get(x, y, z);
        self.put(x, y, z, kind);
        self.outbox.push(("voxel-edit", numbers(&[x as f64, y as f64, z as f64, kind as f64, was as f64])));
    }

    fn click(&mut self, button: i16) {
        let Some((cell, normal)) = self.target() else { return };
        if button == 0 {
            if self.world.get(cell[0], cell[1], cell[2]) != BEDROCK {
                self.edit(cell[0], cell[1], cell[2], AIR);
            }
        } else if button == 2 {
            let [x, y, z] = [cell[0] + normal[0], cell[1] + normal[1], cell[2] + normal[2]];
            if !World::inside(x, y, z) || self.world.get(x, y, z) != AIR {
                return;
            }
            self.world.set(x, y, z, self.selected);
            let inside_me = self.collides(self.pos);
            self.world.set(x, y, z, AIR);
            if !inside_me {
                self.edit(x, y, z, self.selected);
            }
        }
    }

    fn locked(&self) -> bool {
        let document = web_sys::window().unwrap().document().unwrap();
        document.pointer_lock_element().is_some_and(|e| JsValue::from(e) == JsValue::from(self.canvas.clone()))
    }

    fn frame(&mut self, now: f64) {
        let dt = (((now - self.last_frame) / 1000.0) as f32).clamp(0.0, 0.05);
        self.last_frame = now;
        if !self.locked() {
            self.keys.clear();
        }

        // Moving: WASD along where you face, gravity, a jump from the ground.
        let (s, c) = self.yaw.sin_cos();
        let mut dir = [0.0f32, 0.0];
        for (key, f, r) in [("KeyW", 1.0, 0.0), ("KeyS", -1.0, 0.0), ("KeyD", 0.0, 1.0), ("KeyA", 0.0, -1.0)] {
            if self.keys.contains(key) {
                dir[0] += s * f + c * r;
                dir[1] += -c * f + s * r;
            }
        }
        let len = (dir[0] * dir[0] + dir[1] * dir[1]).sqrt();
        if len > 0.0 {
            dir = [dir[0] / len * 4.5, dir[1] / len * 4.5];
        }
        if self.keys.contains("Space") && self.grounded {
            self.fall = 8.0;
        }
        self.fall = (self.fall - 24.0 * dt).max(-30.0);
        if self.collides(self.pos) {
            self.pos[1] += 1.0; // a block was put where you stand: out on top of it
        }
        self.step(0, dir[0] * dt);
        self.step(2, dir[1] * dt);
        self.grounded = false;
        self.step(1, self.fall * dt);

        for remote in self.remotes.values_mut() {
            let t = (dt * 12.0).min(1.0);
            for i in 0..3 {
                remote.at[i] += (remote.target[i] - remote.at[i]) * t;
            }
        }

        // Telling the page where you are: when you've moved, at most 10 a second.
        let at = [self.pos[0], self.pos[1], self.pos[2], self.yaw];
        let moved = (0..4).any(|i| (at[i] - self.sent[i]).abs() > 0.01);
        if now - self.last_sent >= 100.0 && (moved || now - self.last_sent >= 2000.0) {
            self.last_sent = now;
            self.sent = at;
            let round = |n: f32| (n as f64 * 100.0).round() / 100.0;
            self.outbox.push(("voxel-move", numbers(&[round(at[0]), round(at[1]), round(at[2]), round(at[3])])));
        }

        self.draw();
    }

    fn draw(&mut self) {
        let gl = &self.gl;
        let dpr = web_sys::window().unwrap().device_pixel_ratio();
        let (w, h) = ((self.canvas.client_width() as f64 * dpr) as u32, (self.canvas.client_height() as f64 * dpr) as u32);
        if self.canvas.width() != w || self.canvas.height() != h {
            self.canvas.set_width(w);
            self.canvas.set_height(h);
        }
        gl.viewport(0, 0, w as i32, h as i32);
        gl.clear_color(SKY[0], SKY[1], SKY[2], 1.0);
        gl.clear(GL::COLOR_BUFFER_BIT | GL::DEPTH_BUFFER_BIT);

        for i in std::mem::take(&mut self.dirty) {
            let cx = i as i32 % (W / CHUNK);
            let cz = (i as i32 / (W / CHUNK)) % (D / CHUNK);
            let cy = i as i32 / (W / CHUNK * (D / CHUNK));
            let v = self.world.mesh(cx, cy, cz);
            upload(gl, &mut self.chunks[i], &v, GL::STATIC_DRAW);
        }

        let eye = self.eye();
        let projection = perspective(70f32.to_radians(), w as f32 / h.max(1) as f32, 0.05, 200.0);
        let mvp = multiply(&projection, &look(eye, self.forward()));
        gl.uniform_matrix4fv_with_f32_array(Some(&self.u_mvp), false, &mvp);
        gl.uniform3f(Some(&self.u_eye), eye[0], eye[1], eye[2]);
        gl.uniform3f(Some(&self.u_fog), SKY[0], SKY[1], SKY[2]);

        for chunk in &self.chunks {
            if chunk.count > 0 {
                gl.bind_vertex_array(Some(&chunk.vao));
                gl.draw_arrays(GL::TRIANGLES, 0, chunk.count);
            }
        }

        // Other players, as villagers: a robe in their colour, folded arms, a big head and nose.
        let mut v = Vec::new();
        for r in self.remotes.values() {
            villager(&mut v, r.at, r.yaw, r.color);
        }
        upload(gl, &mut self.others, &v, GL::DYNAMIC_DRAW);
        if self.others.count > 0 {
            gl.bind_vertex_array(Some(&self.others.vao));
            gl.draw_arrays(GL::TRIANGLES, 0, self.others.count);
        }

        // The block you're looking at, outlined.
        let mut v = Vec::new();
        if let Some((c, _)) = self.target() {
            let (a, b) = (-0.003, 1.003);
            let p = |i: usize| {
                [
                    c[0] as f32 + if i & 1 == 1 { b } else { a },
                    c[1] as f32 + if i & 2 == 2 { b } else { a },
                    c[2] as f32 + if i & 4 == 4 { b } else { a },
                ]
            };
            for (i, j) in [(0, 1), (2, 3), (4, 5), (6, 7), (0, 2), (1, 3), (4, 6), (5, 7), (0, 4), (1, 5), (2, 6), (3, 7)] {
                for q in [p(i), p(j)] {
                    v.extend_from_slice(&[q[0], q[1], q[2], 0.1, 0.1, 0.1]);
                }
            }
        }
        upload(gl, &mut self.outline, &v, GL::DYNAMIC_DRAW);
        if self.outline.count > 0 {
            gl.bind_vertex_array(Some(&self.outline.vao));
            gl.draw_arrays(GL::LINES, 0, self.outline.count);
        }
    }
}

// ---- WebGL ----------------------------------------------------------------------------------

const VERTEX: &str = r#"#version 300 es
layout(location = 0) in vec3 a_pos;
layout(location = 1) in vec3 a_col;
uniform mat4 u_mvp;
uniform vec3 u_eye;
out vec3 v_col;
out float v_dist;
void main() {
    gl_Position = u_mvp * vec4(a_pos, 1.0);
    v_col = a_col;
    v_dist = distance(a_pos, u_eye);
}"#;

const FRAGMENT: &str = r#"#version 300 es
precision mediump float;
in vec3 v_col;
in float v_dist;
uniform vec3 u_fog;
out vec4 color;
void main() {
    float fog = clamp((v_dist - 40.0) / 45.0, 0.0, 1.0);
    color = vec4(mix(v_col, u_fog, fog), 1.0);
}"#;

fn program(gl: &GL) -> Result<WebGlProgram, String> {
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
    gl.attach_shader(&program, &compile(GL::VERTEX_SHADER, VERTEX)?);
    gl.attach_shader(&program, &compile(GL::FRAGMENT_SHADER, FRAGMENT)?);
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
    gl.enable_vertex_attrib_array(0);
    gl.vertex_attrib_pointer_with_i32(0, 3, GL::FLOAT, false, 24, 0);
    gl.enable_vertex_attrib_array(1);
    gl.vertex_attrib_pointer_with_i32(1, 3, GL::FLOAT, false, 24, 12);
    Mesh { vao, buffer, count: 0 }
}

fn upload(gl: &GL, mesh: &mut Mesh, v: &[f32], usage: u32) {
    gl.bind_buffer(GL::ARRAY_BUFFER, Some(&mesh.buffer));
    gl.buffer_data_with_array_buffer_view(GL::ARRAY_BUFFER, &js_sys::Float32Array::from(v), usage);
    mesh.count = (v.len() / 6) as i32;
}

// ---- The page's way in ----------------------------------------------------------------------

/// Starts the game on canvas `#canvas_id`: the generated world, you in the middle.
#[wasm_bindgen]
pub fn start(canvas_id: &str) -> Result<(), JsValue> {
    let window = web_sys::window().unwrap();
    let document = window.document().unwrap();
    let canvas: HtmlCanvasElement = document
        .get_element_by_id(canvas_id)
        .ok_or("no canvas")?
        .dyn_into()?;
    let gl: GL = canvas.get_context("webgl2")?.ok_or("no WebGL 2")?.dyn_into()?;
    let program = program(&gl)?;
    gl.use_program(Some(&program));
    gl.enable(GL::DEPTH_TEST);

    let chunks = (0..(W / CHUNK) * (H / CHUNK) * (D / CHUNK)).map(|_| mesh(&gl)).collect::<Vec<_>>();
    let mut game = Game {
        u_mvp: gl.get_uniform_location(&program, "u_mvp").unwrap(),
        u_eye: gl.get_uniform_location(&program, "u_eye").unwrap(),
        u_fog: gl.get_uniform_location(&program, "u_fog").unwrap(),
        dirty: (0..chunks.len()).collect(),
        chunks,
        others: mesh(&gl),
        outline: mesh(&gl),
        gl,
        canvas: canvas.clone(),
        world: World::generate(),
        pos: [0.0; 3],
        fall: 0.0,
        grounded: false,
        yaw: 0.0,
        pitch: -0.2,
        keys: HashSet::new(),
        selected: GRASS,
        remotes: HashMap::new(),
        last_frame: window.performance().unwrap().now(),
        last_sent: 0.0,
        sent: [f32::NAN; 4],
        outbox: Vec::new(),
    };
    game.spawn();
    GAME.with(|cell| *cell.borrow_mut() = Some(game));

    let on_mouse_down = Closure::<dyn FnMut(MouseEvent)>::new(move |e: MouseEvent| {
        with_game(|g| {
            if g.locked() {
                g.click(e.button());
            } else {
                g.canvas.request_pointer_lock();
            }
        });
    });
    canvas.add_event_listener_with_callback("mousedown", on_mouse_down.as_ref().unchecked_ref())?;
    on_mouse_down.forget();

    let no_menu = Closure::<dyn FnMut(MouseEvent)>::new(|e: MouseEvent| e.prevent_default());
    canvas.add_event_listener_with_callback("contextmenu", no_menu.as_ref().unchecked_ref())?;
    no_menu.forget();

    let on_mouse_move = Closure::<dyn FnMut(MouseEvent)>::new(|e: MouseEvent| {
        with_game(|g| {
            if g.locked() {
                g.yaw += e.movement_x() as f32 * 0.0025;
                g.pitch = (g.pitch - e.movement_y() as f32 * 0.0025).clamp(-1.55, 1.55);
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
            if let Some(n) = code.strip_prefix("Digit").and_then(|n| n.parse::<u8>().ok()) {
                if (1..=7).contains(&n) && n != g.selected {
                    g.selected = n;
                    g.outbox.push(("voxel-select", numbers(&[n as f64])));
                }
            }
            g.keys.insert(code);
        });
    });
    document.add_event_listener_with_callback("keydown", on_key_down.as_ref().unchecked_ref())?;
    on_key_down.forget();

    let on_key_up = Closure::<dyn FnMut(KeyboardEvent)>::new(|e: KeyboardEvent| {
        with_game(|g| {
            g.keys.remove(&e.code());
        });
    });
    document.add_event_listener_with_callback("keyup", on_key_up.as_ref().unchecked_ref())?;
    on_key_up.forget();

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

/// The world as generated, then `edits` (x, y, z, kind, ...) on top: the server's changes.
#[wasm_bindgen]
pub fn load(edits: Vec<i32>) {
    with_game(|g| {
        g.world = World::generate();
        for e in edits.chunks_exact(4) {
            g.world.set(e[0], e[1], e[2], e[3].clamp(0, PLANKS as i32) as u8);
        }
        g.dirty = (0..g.chunks.len()).collect();
    });
}

#[wasm_bindgen]
pub fn set_block(x: i32, y: i32, z: i32, kind: i32) {
    with_game(|g| g.put(x, y, z, kind.clamp(0, BEDROCK as i32) as u8));
}

/// Shows player `id` at x, y, z (their feet) facing `yaw`, in `color` (#rrggbb).
#[wasm_bindgen]
pub fn set_player(id: i32, x: f32, y: f32, z: f32, yaw: f32, color: &str) {
    let hex = |i: usize| u8::from_str_radix(color.get(i..i + 2).unwrap_or("88"), 16).unwrap_or(136) as f32 / 255.0;
    let color = [hex(1), hex(3), hex(5)];
    with_game(|g| {
        let remote = g.remotes.entry(id).or_insert(Remote { at: [x, y, z], target: [x, y, z], yaw, color });
        remote.target = [x, y, z];
        remote.yaw = yaw;
        remote.color = color;
    });
}

#[wasm_bindgen]
pub fn remove_player(id: i32) {
    with_game(|g| {
        g.remotes.remove(&id);
    });
}
