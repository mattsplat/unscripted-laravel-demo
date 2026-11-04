//! Rally drawn with three-d: the same game as `rally/` (its rules and the ball's flight are
//! `rally/src/sim.rs`, shared, not copied), with three-d's lights, shadows, materials and meshes
//! in place of hand-written WebGL. Built to compare the two: looks, size and speed.
//!
//! The page talks to it exactly as to `rally/`:
//! - in: `start(canvasId)`, `play(level)`, `logo(canvas)`, and `stats()` (frames, average and
//!   worst milliseconds of work per frame since the last call);
//! - out: CustomEvents `rally-state`, `rally-end` and `rally-sound`.
//!
//! Until the first `play`, the computer plays itself (silently, as "idle" to the page).
//!
//! What's built once: the room, the table (a texture made here, the logo painted in when the page
//! hands it over) and the net. What moves each frame: the ball, paddles, your arm and the other
//! player, drawn as instances of four shapes (sphere, cylinder, box, disc face) whose transforms
//! are set each frame; nothing is rebuilt.

#[path = "../../rally/src/sim.rs"]
pub mod sim;

use sim::*;
use std::cell::RefCell;
use std::rc::Rc;
use std::sync::Arc;
use three_d::{
    radians, vec3, vec4, AmbientLight, Attenuation, Camera, ClearState, ColorMaterial, Context, CpuMaterial,
    CpuMesh, CpuTexture, Cull, DirectionalLight, Geometry, Gm, InstancedMesh, Instances, Mat4, Material, Mesh,
    Object, PhysicalMaterial, RenderTarget, SpotLight, Srgba, Texture2DRef, TextureData, Vec3, Viewport,
};
use wasm_bindgen::prelude::*;
use wasm_bindgen::JsCast;
use web_sys::{CanvasRenderingContext2d, HtmlCanvasElement, MouseEvent};

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
const SHADOW: u32 = 2048;

const SKIN: V3 = [0.78, 0.6, 0.47];
const WOOD: V3 = [0.66, 0.48, 0.3];
const RED_RUBBER: V3 = [0.72, 0.08, 0.07];
const BLACK_RUBBER: V3 = [0.05, 0.05, 0.055];
const SHIRT: V3 = [0.86, 0.34, 0.1];
const SHORTS: V3 = [0.1, 0.11, 0.14];
const HAIR: V3 = [0.16, 0.11, 0.08];
const BALL: V3 = [0.98, 0.95, 0.88];
const STEEL: V3 = [0.2, 0.21, 0.23];
const SLEEVE: V3 = [0.14, 0.32, 0.6];

fn v3(a: V3) -> Vec3 {
    vec3(a[0], a[1], a[2])
}

fn srgba(c: V3) -> Srgba {
    let b = |x: f32| (x.clamp(0.0, 1.0) * 255.0).round() as u8;
    Srgba::new(b(c[0]), b(c[1]), b(c[2]), 255)
}

/// A transform taking the unit shape's x, y and z axes to `x`, `y` and `z`, and its origin to `at`.
fn basis(x: V3, y: V3, z: V3, at: V3) -> Mat4 {
    Mat4::from_cols(vec4(x[0], x[1], x[2], 0.0), vec4(y[0], y[1], y[2], 0.0), vec4(z[0], z[1], z[2], 0.0), vec4(at[0], at[1], at[2], 1.0))
}

/// Two perpendiculars to `f`, so that (side, up, f) turn the right way.
fn across(f: V3) -> (V3, V3) {
    let side = normalize(if f[1].abs() > 0.99 { cross(f, [0.0, 0.0, 1.0]) } else { cross(f, [0.0, 1.0, 0.0]) });
    (side, cross(f, side))
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

// ---- Shapes ---------------------------------------------------------------------------------

/// What a shape's made of: matte (cloth, skin, paint), shiny (steel, rubber, the ball), or glowing
/// (lights, the ball's streak: no shading).
#[derive(Clone, Copy)]
enum Finish {
    Matte = 0,
    Shiny = 1,
    Glow = 2,
}

const SPHERE: usize = 0;
const CYLINDER: usize = 1;
const BOX: usize = 2;
const DISC: usize = 3;

/// This frame's instances of each shape, by finish: transforms and colours.
#[derive(Default)]
struct Batch {
    shapes: [[Vec<(Mat4, Srgba)>; 4]; 3],
}

impl Batch {
    fn clear(&mut self) {
        self.shapes.iter_mut().flatten().for_each(Vec::clear);
    }

    fn push(&mut self, finish: Finish, shape: usize, m: Mat4, col: V3) {
        self.shapes[finish as usize][shape].push((m, srgba(col)));
    }

    /// An ellipsoid, `r` across, up and along: heads, hands, shoes, a torso.
    fn blob(&mut self, finish: Finish, c: V3, r: V3, col: V3) {
        self.push(finish, SPHERE, basis([r[0], 0.0, 0.0], [0.0, r[1], 0.0], [0.0, 0.0, r[2]], c), col);
    }

    fn sphere(&mut self, finish: Finish, c: V3, r: f32, col: V3) {
        self.blob(finish, c, [r; 3], col);
    }

    /// A round limb from `a` to `b`, `ra` thick at `a` and `rb` at `b`, its ends rounded off. (The
    /// cylinder's straight, at the average thickness: a transform can't taper it.)
    fn tube(&mut self, finish: Finish, a: V3, b: V3, ra: f32, rb: f32, col: V3) {
        let d = sub(b, a);
        let l = len(d);
        if l < 1e-4 {
            return;
        }
        let f = mul(d, 1.0 / l);
        let (s, u) = across(f);
        let r = (ra + rb) / 2.0;
        self.push(finish, CYLINDER, basis(d, mul(s, r), mul(u, r), a), col);
        self.sphere(finish, a, ra, col);
        self.sphere(finish, b, rb, col);
    }

    /// A bar from `a` to `b`, `half` wide across and up: a handle.
    fn bar(&mut self, finish: Finish, a: V3, b: V3, half: [f32; 2], col: V3) {
        let d = sub(b, a);
        let l = len(d);
        if l < 1e-4 {
            return;
        }
        let (s, u) = across(mul(d, 1.0 / l));
        self.push(finish, BOX, basis(mul(s, half[0]), mul(u, half[1]), mul(d, 0.5), lerp(a, b, 0.5)), col);
    }

    /// An upright box from `min` to `max`.
    fn cube(&mut self, finish: Finish, min: V3, max: V3, col: V3) {
        let h = mul(sub(max, min), 0.5);
        self.push(finish, BOX, basis([h[0], 0.0, 0.0], [0.0, h[1], 0.0], [0.0, 0.0, h[2]], add(min, h)), col);
    }

    /// A flat cylinder: a disc of radius `r` facing `n` around `c`, from `c + n * a` to `c + n * b`.
    fn disc(&mut self, finish: Finish, c: V3, n: V3, a: f32, b: f32, r: f32, col: V3) {
        let (s, u) = across(n);
        self.push(finish, CYLINDER, basis(mul(n, b - a), mul(s, r), mul(u, r), add(c, mul(n, a))), col);
        self.push(finish, DISC, basis(mul(s, r), mul(u, r), n, add(c, mul(n, b))), col);
        self.push(finish, DISC, basis(mul(u, r), mul(s, r), mul(n, -1.0), add(c, mul(n, a))), col);
    }
}

/// One finish's four shapes on the GPU, each an instanced mesh, and how many of each to draw.
struct Layer<M: Material> {
    shapes: Vec<Gm<InstancedMesh, M>>,
    counts: [usize; 4],
}

impl<M: Material> Layer<M> {
    fn new(context: &Context, material: impl Fn() -> M) -> Self {
        let none = Instances { transformations: vec![Mat4::from_scale(0.0)], texture_transformations: None, colors: None };
        let meshes = [CpuMesh::sphere(16), CpuMesh::cylinder(16), CpuMesh::cube(), CpuMesh::circle(24)];
        let shapes = meshes.iter().map(|m| Gm::new(InstancedMesh::new(context, &none, m), material())).collect();
        Layer { shapes, counts: [0; 4] }
    }

    fn set(&mut self, shapes: &[Vec<(Mat4, Srgba)>; 4]) {
        for (i, list) in shapes.iter().enumerate() {
            self.counts[i] = list.len();
            if !list.is_empty() {
                self.shapes[i].geometry.set_instances(&Instances {
                    transformations: list.iter().map(|(m, _)| *m).collect(),
                    texture_transformations: None,
                    colors: Some(list.iter().map(|(_, c)| *c).collect()),
                });
            }
        }
    }

    /// The shapes with anything to draw.
    fn drawn(&self) -> impl Iterator<Item = &Gm<InstancedMesh, M>> {
        self.shapes.iter().zip(self.counts).filter(|(_, n)| *n > 0).map(|(s, _)| s)
    }
}

/// The three finishes' layers.
struct Layers {
    matte: Layer<PhysicalMaterial>,
    shiny: Layer<PhysicalMaterial>,
    glow: Layer<ColorMaterial>,
}

impl Layers {
    fn new(context: &Context) -> Self {
        let lit = |roughness: f32| {
            let context = context.clone();
            move || {
                PhysicalMaterial::new_opaque(&context, &CpuMaterial { albedo: Srgba::WHITE, roughness, metallic: 0.0, ..Default::default() })
            }
        };
        Layers {
            matte: Layer::new(context, lit(0.85)),
            shiny: Layer::new(context, lit(0.32)),
            glow: Layer::new(context, || ColorMaterial { color: Srgba::WHITE, ..Default::default() }),
        }
    }

    fn set(&mut self, batch: &Batch) {
        self.matte.set(&batch.shapes[0]);
        self.shiny.set(&batch.shapes[1]);
        self.glow.set(&batch.shapes[2]);
    }
}

// ---- Textures, made here ----------------------------------------------------------------------

fn hash(x: f32, y: f32) -> f32 {
    let (mut px, mut py) = ((x * 123.34).fract(), (y * 456.21).fract());
    let d = px * (px + 45.32) + py * (py + 45.32);
    px += d;
    py += d;
    (px * py).fract().abs()
}

fn noise(x: f32, y: f32) -> f32 {
    let (ix, iy) = (x.floor(), y.floor());
    let (fx, fy) = (x - ix, y - iy);
    let (ux, uy) = (fx * fx * (3.0 - 2.0 * fx), fy * fy * (3.0 - 2.0 * fy));
    let mix = |a: f32, b: f32, t: f32| a + (b - a) * t;
    mix(mix(hash(ix, iy), hash(ix + 1.0, iy), ux), mix(hash(ix, iy + 1.0), hash(ix + 1.0, iy + 1.0), ux), uy)
}

fn fbm(mut x: f32, mut y: f32) -> f32 {
    let (mut v, mut a) = (0.0, 0.5);
    for _ in 0..3 {
        v += a * noise(x, y);
        x *= 2.03;
        y *= 2.03;
        a *= 0.5;
    }
    v
}

/// 1 within `w` of a line (`d` from it), softly over `soft`.
fn band(d: f32, w: f32, soft: f32) -> f32 {
    1.0 - ((d - (w - soft)) / (2.0 * soft)).clamp(0.0, 1.0)
}

fn mix3(a: V3, b: V3, t: f32) -> V3 {
    lerp(a, b, t.clamp(0.0, 1.0))
}

/// A texture of `w` by `h`, each pixel's colour from where it is (u, v in 0..1, v down the rows).
/// On a `panel`, the first row is the end its `y` points to.
fn texture(w: u32, h: u32, pixel: impl Fn(f32, f32) -> [f32; 4]) -> CpuTexture {
    let mut data = Vec::with_capacity((w * h) as usize);
    for j in 0..h {
        for i in 0..w {
            let c = pixel((i as f32 + 0.5) / w as f32, (j as f32 + 0.5) / h as f32);
            data.push(c.map(|x| (x.clamp(0.0, 1.0) * 255.0).round() as u8));
        }
    }
    CpuTexture { data: TextureData::RgbaU8(data), width: w, height: h, ..Default::default() }
}

fn opaque(c: V3) -> [f32; 4] {
    [c[0], c[1], c[2], 1.0]
}

/// The floor: red-brown rubber, the court marked out in white. It spans x -9..9, z -10..8.
fn floor_texture() -> CpuTexture {
    texture(1024, 1024, |u, v| {
        let (x, z) = (-9.0 + 18.0 * u, -10.0 + 18.0 * v);
        let c = mul([0.42, 0.15, 0.11], (0.9 + 0.12 * fbm(x * 1.5, z * 1.5)) * (0.96 + 0.05 * hash((x * 90.0).floor(), (z * 90.0).floor())));
        let (dx, dz) = ((x).abs() - 4.0, (z + 0.75).abs() - 5.15);
        let line = band(dx.max(dz).abs(), 0.03, 0.01);
        opaque(mix3(c, [0.85, 0.83, 0.8], line * 0.9))
    })
}

/// A wall: dark panels, 18 m along and 7 m up.
fn wall_texture() -> CpuTexture {
    texture(1024, 400, |u, v| {
        let (a, y) = (18.0 * u, 7.0 * (1.0 - v));
        let (fx, fy) = ((a / 2.4).fract(), (y / 1.2).fract());
        let edge = fx.min(1.0 - fx).min(fy).min(1.0 - fy);
        let seam = (edge / 0.01).clamp(0.0, 1.0);
        opaque(mul([0.07, 0.08, 0.11], (0.85 + 0.2 * fbm(a * 1.5, y * 1.5)) * (0.6 + 0.4 * seam)))
    })
}

/// The table's top: blue, white lines round the edge and down the middle, and the logo in each
/// half, upright from your end. `logo` is RGBA rows, top first, `size` square.
fn table_texture(logo: Option<&(u32, Vec<u8>)>) -> CpuTexture {
    texture(1024, 1840, |u, v| {
        let (x, z) = ((2.0 * u - 1.0) * HALF_W, (2.0 * v - 1.0) * HALF_L);
        let mut c = mul([0.06, 0.2, 0.42], 0.95 + 0.06 * noise(x * 60.0, z * 60.0));
        if let Some((size, pixels)) = logo {
            let (qx, qy) = (x / 0.62 + 0.5, (z - z.signum() * 0.685) / 0.62 + 0.5);
            if (0.0..1.0).contains(&qx) && (0.0..1.0).contains(&qy) {
                // Between the four nearest of the logo's pixels (bilinear), its colour weighted by
                // how solid each is, so its edges don't darken.
                let n = *size as usize;
                let (fx, fy) = (qx * n as f32 - 0.5, qy * n as f32 - 0.5);
                let (x0, y0) = (fx.floor().max(0.0) as usize, fy.floor().max(0.0) as usize);
                let (tx, ty) = ((fx - x0 as f32).clamp(0.0, 1.0), (fy - y0 as f32).clamp(0.0, 1.0));
                let (mut rgb, mut alpha) = ([0.0f32; 3], 0.0f32);
                for (dx, dy, w) in [(0, 0, (1.0 - tx) * (1.0 - ty)), (1, 0, tx * (1.0 - ty)), (0, 1, (1.0 - tx) * ty), (1, 1, tx * ty)] {
                    let k = ((y0 + dy).min(n - 1) * n + (x0 + dx).min(n - 1)) * 4;
                    let a = pixels[k + 3] as f32 / 255.0 * w;
                    for i in 0..3 {
                        rgb[i] += pixels[k + i] as f32 / 255.0 * a;
                    }
                    alpha += a;
                }
                if alpha > 0.0 {
                    c = mix3(c, mul(rgb, 1.0 / alpha), alpha);
                }
            }
        }
        let edge = band(HALF_W - x.abs(), 0.02, 0.0015).max(band(HALF_L - z.abs(), 0.02, 0.0015));
        let middle = band(x.abs(), 0.0015, 0.0015);
        opaque(mix3(c, [0.93, 0.93, 0.9], edge.max(middle)))
    })
}

/// The net: dark mesh with holes (clear), a white tape along the top. 1.83 m by 15 cm.
fn net_texture() -> CpuTexture {
    let height = NET_TOP - TABLE_Y;
    texture(1024, 96, |u, v| {
        let (x, y) = (u * 2.0 * NET_HALF, (1.0 - v) * height);
        if y > height - 0.016 {
            return [0.9, 0.9, 0.88, 1.0];
        }
        let (gx, gy) = ((x / 0.014).fract(), (y / 0.014).fract());
        if gx > 0.18 && gy > 0.18 {
            [0.05, 0.07, 0.12, 0.0]
        } else {
            [0.05, 0.07, 0.12, 1.0]
        }
    })
}

/// A flat square mesh, its x and y spanning `x` and `y` (from the middle), facing `x × y`.
fn panel(context: &Context, at: V3, x: V3, y: V3, material: PhysicalMaterial) -> Gm<Mesh, PhysicalMaterial> {
    let n = normalize(cross(x, y));
    let mut mesh = Mesh::new(context, &CpuMesh::square());
    mesh.set_transformation(basis(x, y, n, at));
    Gm::new(mesh, material)
}

fn textured(context: &Context, texture: CpuTexture, roughness: f32) -> PhysicalMaterial {
    PhysicalMaterial::new_opaque(context, &CpuMaterial { albedo: Srgba::WHITE, albedo_texture: Some(texture), roughness, metallic: 0.0, ..Default::default() })
}

/// The room's fixed things that aren't textured: boards, lights, the table's frame and legs, the
/// net's posts.
fn room(b: &mut Batch) {
    for x in [-6.0, -2.0, 2.0, 6.0] {
        b.cube(Finish::Glow, [x - 1.4, 4.2, -9.99], [x + 1.4, 4.32, -9.95], [1.0, 0.97, 0.9]);
    }
    let board = [0.09, 0.17, 0.36];
    let white = [0.9, 0.9, 0.9];
    b.cube(Finish::Matte, [-4.2, 0.0, -6.0], [4.2, 0.72, -5.94], board);
    b.cube(Finish::Matte, [-4.2, 0.72, -6.0], [4.2, 0.75, -5.94], white);
    for x in [-4.2, 4.14] {
        b.cube(Finish::Matte, [x, 0.0, -6.0], [x + 0.06, 0.72, 4.5], board);
        b.cube(Finish::Matte, [x, 0.72, -6.0], [x + 0.06, 0.75, 4.5], white);
    }
    // The table's edge (its top's a textured panel), a frame under it, legs on feet.
    b.cube(Finish::Matte, [-HALF_W, TABLE_Y - 0.025, -HALF_L], [HALF_W, TABLE_Y - 0.0005, HALF_L], [0.04, 0.12, 0.25]);
    b.cube(Finish::Matte, [-HALF_W + 0.05, TABLE_Y - 0.09, -HALF_L + 0.06], [HALF_W - 0.05, TABLE_Y - 0.025, HALF_L - 0.06], STEEL);
    for z in [-1.05f32, 1.05] {
        for x in [-0.62f32, 0.62] {
            b.cube(Finish::Shiny, [x - 0.03, 0.06, z - 0.03], [x + 0.03, TABLE_Y - 0.09, z + 0.03], STEEL);
            b.cube(Finish::Matte, [x - 0.035, 0.0, z - 0.035], [x + 0.035, 0.06, z + 0.035], [0.06, 0.06, 0.06]);
        }
        b.cube(Finish::Shiny, [-0.62, 0.3, z - 0.02], [0.62, 0.34, z + 0.02], STEEL);
    }
    b.cube(Finish::Shiny, [-0.02, 0.3, -1.05], [0.02, 0.34, 1.05], STEEL);
    for s in [-1.0f32, 1.0] {
        let (a, c) = if s > 0.0 { (HALF_W - 0.04, NET_HALF + 0.015) } else { (-NET_HALF - 0.015, -HALF_W + 0.04) };
        b.cube(Finish::Shiny, [a, TABLE_Y - 0.035, -0.02], [c, TABLE_Y + 0.005, 0.02], STEEL);
        let x = s * (NET_HALF + 0.005);
        b.cube(Finish::Shiny, [x - 0.01, TABLE_Y, -0.012], [x + 0.01, NET_TOP + 0.005, 0.012], STEEL);
    }
}

/// `side`'s paddle at `p`, moving `vel`: red rubber toward the other end, black behind, on a
/// wooden handle. Yours has a hand and forearm. Where the hand holds it.
fn paddle(b: &mut Batch, side: usize, p: V3, vel: V3, chop: bool) -> V3 {
    let s = sign(side);
    let forward = (-vel[2] * s).clamp(0.0, 4.0);
    let tilt = if chop { -0.55 } else { 0.12 + 0.07 * forward };
    let n = normalize([vel[0] * 0.04, -tilt, -s]);
    let h = normalize([0.5 * s, -1.0, 0.0]);
    let down = normalize(sub(h, mul(n, dot(h, n))));
    b.disc(Finish::Shiny, p, n, 0.0015, 0.0055, PADDLE_R * 0.94, RED_RUBBER);
    b.disc(Finish::Matte, p, n, -0.0055, -0.0015, PADDLE_R * 0.94, BLACK_RUBBER);
    b.disc(Finish::Matte, p, n, -0.0015, 0.0015, PADDLE_R * 0.95, WOOD);
    let grip = add(p, mul(down, PADDLE_R * 0.9));
    let end = add(p, mul(down, PADDLE_R + 0.1));
    b.bar(Finish::Matte, grip, end, [0.013, 0.011], WOOD);
    let hand = add(p, mul(down, PADDLE_R + 0.05));
    if side == YOU {
        b.blob(Finish::Matte, hand, [0.032, 0.042, 0.036], SKIN);
        let wrist = add(hand, [0.05, -0.05, 0.09]);
        b.tube(Finish::Matte, hand, add(wrist, [0.06, -0.06, 0.12]), 0.027, 0.034, SKIN);
        b.tube(Finish::Matte, add(wrist, [0.04, -0.04, 0.08]), add(wrist, [0.14, -0.13, 0.3]), 0.042, 0.048, SLEEVE);
    }
    hand
}

/// The computer's body. Its feet step to keep up as it moves; it swings, and pumps a fist when
/// it wins a point.
struct Figure {
    feet: [V3; 2],
    steps: [Option<(V3, V3, f64)>; 2],
    swung: f64,
    point: (f64, bool),
}

const TURN: f32 = std::f32::consts::PI;

impl Figure {
    fn new() -> Figure {
        Figure { feet: [[0.0, 0.0, -2.2], [0.4, 0.0, -2.2]], steps: [None, None], swung: f64::MIN, point: (f64::MIN, false) }
    }

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
                let to = add(home[i], mul(normalize(sub(home[i], self.feet[i])), 0.05));
                self.steps[i] = Some((self.feet[i], [to[0], 0.0, to[2]], now));
            }
        }
    }

    fn draw(&self, b: &mut Batch, body: V3, grip: V3, now: f64) {
        let m = Finish::Matte;
        let t = (now / 1000.0) as f32;
        let mid = [(self.feet[0][0] + self.feet[1][0]) / 2.0, 0.0, (self.feet[0][2] + self.feet[1][2]) / 2.0];
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

        for (i, out) in [(0, -1.0f32), (1, 1.0)] {
            let hip = add(hips, [out * 0.09, -0.04, 0.0]);
            let ankle = add(self.feet[i], [0.0, 0.08, 0.0]);
            let knee = bend(hip, ankle, 0.43, 0.42, [out * 0.3, 0.0, 1.0]);
            b.tube(m, hip, lerp(hip, knee, 0.55), 0.078, 0.066, SHORTS);
            b.tube(m, lerp(hip, knee, 0.5), knee, 0.06, 0.05, SKIN);
            b.tube(m, knee, ankle, 0.05, 0.034, SKIN);
            b.tube(m, lerp(knee, ankle, 0.8), ankle, 0.037, 0.035, [0.95, 0.95, 0.93]);
            b.blob(m, add(self.feet[i], [0.0, 0.04, 0.05]), [0.048, 0.04, 0.12], [0.93, 0.93, 0.9]);
            b.blob(m, add(self.feet[i], [0.0, 0.012, 0.05]), [0.05, 0.014, 0.125], [0.2, 0.22, 0.26]);
        }

        b.blob(m, add(hips, [0.0, 0.02, 0.0]), [0.17, 0.11, 0.12], SHORTS);
        b.blob(m, lerp(hips, chest, 0.62), [0.175, 0.27, 0.115], SHIRT);
        b.blob(m, lerp(hips, chest, 0.25), [0.152, 0.14, 0.105], SHIRT);
        let neck = add(chest, [0.0, 0.13, 0.0]);
        b.tube(m, neck, add(neck, [0.0, 0.08, 0.01]), 0.048, 0.044, SKIN);

        let head = add(neck, [0.0, 0.17 - glum * 0.03, 0.02 + glum * 0.04]);
        b.blob(m, head, [0.088, 0.112, 0.1], SKIN);
        b.blob(m, add(head, [0.0, 0.035, -0.028]), [0.096, 0.098, 0.1], HAIR);
        for out in [-1.0f32, 1.0] {
            b.blob(m, add(head, [out * 0.033, 0.012, 0.086]), [0.013, 0.011, 0.01], [0.08, 0.06, 0.05]);
            b.blob(m, add(head, [out * 0.034, 0.04, 0.088]), [0.022, 0.006, 0.008], HAIR);
            b.blob(m, add(head, [out * 0.088, 0.0, 0.0]), [0.014, 0.03, 0.02], SKIN);
        }
        b.blob(m, add(head, [0.0, -0.012, 0.098]), [0.014, 0.024, 0.018], SKIN);
        b.blob(m, add(head, [0.0, -0.055, 0.086]), [0.024, 0.006, 0.006], [0.45, 0.2, 0.18]);

        let shoulder = add(chest, [-0.165, 0.06, 0.0]);
        let elbow = bend(shoulder, grip, 0.29, 0.27, [-0.6, -0.6, -0.3]);
        Self::arm(b, shoulder, elbow, grip);
        b.blob(m, grip, [0.034, 0.042, 0.036], SKIN);
        let shoulder = add(chest, [0.165, 0.06, 0.0]);
        let ready = add(chest, [0.24, -0.2, 0.3]);
        let fist = add(shoulder, [0.1, 0.42, 0.1]);
        let hand = lerp(ready, fist, cheer);
        let elbow = bend(shoulder, hand, 0.29, 0.27, lerp([0.6, -0.8, -0.2], [1.0, -0.2, 0.4], cheer));
        Self::arm(b, shoulder, elbow, hand);
        b.blob(m, hand, [0.036, 0.036, 0.036], SKIN);
    }

    fn arm(b: &mut Batch, shoulder: V3, elbow: V3, hand: V3) {
        b.tube(Finish::Matte, shoulder, lerp(shoulder, elbow, 0.5), 0.054, 0.05, SHIRT);
        b.tube(Finish::Matte, lerp(shoulder, elbow, 0.45), elbow, 0.046, 0.04, SKIN);
        b.tube(Finish::Matte, elbow, hand, 0.04, 0.028, SKIN);
    }
}

// ---- The game -------------------------------------------------------------------------------

struct Game {
    context: Context,
    canvas: HtmlCanvasElement,
    floor: Gm<Mesh, PhysicalMaterial>,
    walls: Vec<Gm<Mesh, PhysicalMaterial>>,
    table: Gm<Mesh, PhysicalMaterial>,
    net: Gm<Mesh, PhysicalMaterial>,
    fixed: Layers,
    moving: Layers,
    batch: Batch,
    ambient: AmbientLight,
    lamp: SpotLight,
    fill: DirectionalLight,
    /// The logo for the table, once the page has handed it over: its size and RGBA rows.
    logo: Option<(u32, Vec<u8>)>,

    rally: Rally,
    figure: Figure,
    demo: bool,
    want: [f32; 2],
    chop: bool,
    toss: bool,
    unplayed: f32,
    before: (V3, [V3; 2]),
    points: [u32; 2],
    last: i32,
    told: String,
    now: f64,
    outbox: Vec<(&'static str, JsValue)>,
    /// Frames since the page last asked, and their total and worst milliseconds of work.
    timing: (u32, f64, f64),
}

thread_local! {
    static GAME: RefCell<Option<Game>> = const { RefCell::new(None) };
}

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

fn clock() -> f64 {
    web_sys::window().unwrap().performance().unwrap().now()
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
        let serving = matches!(self.rally.phase, Phase::Ready { .. } | Phase::Tossed) && self.rally.server() == YOU;
        if serving {
            self.want[1] = self.want[1].max(HALF_L + 0.1);
        }
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

        // Where everything that moves is, this frame.
        let b = &mut self.batch;
        b.clear();
        if self.rally.phase != Phase::Idle {
            b.sphere(Finish::Shiny, ball, BALL_R, BALL);
            let speed = len(self.rally.ball.v);
            if speed > 5.0 && self.rally.phase == Phase::Rally {
                let back = mul(self.rally.ball.v, -0.012);
                for i in 1..4 {
                    let f = i as f32 / 4.0;
                    let col = lerp(BALL, [0.2, 0.25, 0.35], 0.25 * i as f32);
                    b.sphere(Finish::Glow, add(ball, mul(back, f)), BALL_R * (1.0 - 0.22 * i as f32), col);
                }
            }
        }
        paddle(b, YOU, paddles[YOU], self.rally.paddles[YOU].v, self.chop);
        let coming = self.rally.phase == Phase::Rally && self.rally.hitter == YOU;
        let theirs = self.figure.swing(paddles[CPU], &Ball { p: ball, ..self.rally.ball }, coming, self.now);
        let grip = paddle(b, CPU, theirs, self.rally.paddles[CPU].v, false);
        let body = [(paddles[CPU][0] + 0.3).clamp(-1.8, 1.8), 0.0, (paddles[CPU][2] - 0.5).min(-1.9)];
        self.figure.walk(body, self.now);
        self.figure.draw(b, body, grip, self.now);
        self.moving.set(&self.batch);

        // Sharp on high-density screens, but no more than about 1920 x 1200 pixels.
        let (cw, ch) = (self.canvas.client_width().max(1) as f64, self.canvas.client_height().max(1) as f64);
        let scale = web_sys::window().unwrap().device_pixel_ratio().min((2_300_000.0 / (cw * ch)).sqrt());
        let (w, h) = ((cw * scale) as u32, (ch * scale) as u32);
        if self.canvas.width() != w || self.canvas.height() != h {
            self.canvas.set_width(w);
            self.canvas.set_height(h);
        }

        // Behind your end, drifting a little with your paddle.
        let x = paddles[YOU][0];
        let aspect = w as f32 / h.max(1) as f32;
        let fov = if aspect < 1.25 { 2.0 * ((FOV / 2.0).tan() * 1.25 / aspect).atan() } else { FOV };
        let camera = Camera::new_perspective(
            Viewport::new_at_origo(w, h),
            v3([x * 0.22, 1.72, 3.9]),
            v3([x * 0.06, 0.74, -0.7]),
            vec3(0.0, 1.0, 0.0),
            radians(fov),
            0.05,
            60.0,
        );

        // Shadows from the lamp over the table: everything that casts one, the net aside.
        let casters: Vec<&dyn Geometry> = std::iter::empty()
            .chain(self.fixed.matte.drawn().map(|g| g as &dyn Geometry))
            .chain(self.fixed.shiny.drawn().map(|g| g as &dyn Geometry))
            .chain(self.moving.matte.drawn().map(|g| g as &dyn Geometry))
            .chain(self.moving.shiny.drawn().map(|g| g as &dyn Geometry))
            .chain([&self.table as &dyn Geometry])
            .collect();
        let _ = self.lamp.generate_shadow_map(SHADOW, casters.iter().copied());

        let mut objects: Vec<&dyn Object> = vec![&self.floor, &self.table];
        objects.extend(self.walls.iter().map(|w| w as &dyn Object));
        for layers in [&self.fixed, &self.moving] {
            objects.extend(layers.matte.drawn().map(|g| g as &dyn Object));
            objects.extend(layers.shiny.drawn().map(|g| g as &dyn Object));
            objects.extend(layers.glow.drawn().map(|g| g as &dyn Object));
        }
        objects.push(&self.net);
        RenderTarget::screen(&self.context, w, h)
            .clear(ClearState::color_and_depth(0.04, 0.05, 0.07, 1.0, 1.0))
            .render(&camera, objects.iter().copied(), &[&self.ambient, &self.lamp, &self.fill]);
    }
}

// ---- The page's way in ----------------------------------------------------------------------

/// Starts the game on canvas `#canvas_id`: the table, waiting for `play`.
#[wasm_bindgen]
pub fn start(canvas_id: &str) -> Result<(), JsValue> {
    let window = web_sys::window().unwrap();
    let document = window.document().unwrap();
    let canvas: HtmlCanvasElement = document.get_element_by_id(canvas_id).ok_or("no canvas")?.dyn_into()?;
    let webgl: web_sys::WebGl2RenderingContext = canvas.get_context("webgl2")?.ok_or("no WebGL 2")?.dyn_into()?;
    let gl = three_d::context::Context::from_webgl2_context(webgl);
    let context = Context::from_gl_context(Arc::new(gl)).map_err(|e| JsValue::from_str(&e.to_string()))?;

    let floor = panel(&context, [0.0, 0.0, -1.0], [9.0, 0.0, 0.0], [0.0, 0.0, -9.0], textured(&context, floor_texture(), 0.9));
    let wall = textured(&context, wall_texture(), 0.95);
    let walls = vec![
        panel(&context, [0.0, 3.5, -10.0], [9.0, 0.0, 0.0], [0.0, 3.5, 0.0], wall.clone()),
        panel(&context, [-9.0, 3.5, -1.0], [0.0, 0.0, -9.0], [0.0, 3.5, 0.0], wall.clone()),
        panel(&context, [9.0, 3.5, -1.0], [0.0, 0.0, 9.0], [0.0, 3.5, 0.0], wall),
    ];
    let table = panel(&context, [0.0, TABLE_Y, 0.0], [HALF_W, 0.0, 0.0], [0.0, 0.0, -HALF_L], textured(&context, table_texture(None), 0.45));
    let mut net_material = PhysicalMaterial::new_transparent(
        &context,
        &CpuMaterial { albedo: Srgba::WHITE, albedo_texture: Some(net_texture()), roughness: 0.9, ..Default::default() },
    );
    net_material.render_states.cull = Cull::None;
    let net_height = (NET_TOP - TABLE_Y) / 2.0;
    let net = panel(&context, [0.0, TABLE_Y + net_height, 0.0], [NET_HALF, 0.0, 0.0], [0.0, net_height, 0.0], net_material);

    let mut fixed = Layers::new(&context);
    let mut b = Batch::default();
    room(&mut b);
    fixed.set(&b);

    // A lamp over the table, a little toward you; a dim fill from behind you; the room's light.
    let lamp = SpotLight::new(
        &context,
        2.6,
        Srgba::new(255, 246, 230, 255),
        vec3(0.7, 6.0, 1.0),
        vec3(-0.12, -1.0, -0.18),
        radians(0.62),
        Attenuation { constant: 1.0, linear: 0.0, quadratic: 0.0 },
    );
    let fill = DirectionalLight::new(&context, 0.45, Srgba::new(255, 240, 225, 255), vec3(-0.3, -0.5, -1.0));
    let ambient = AmbientLight::new(&context, 1.0, Srgba::new(205, 215, 255, 255));

    let now = clock();
    let mut rally = Rally::new((now as u32).wrapping_mul(2_654_435_761) | 1);
    rally.level[YOU] = Some(LEVELS[1]);
    rally.start(1);
    let before = (rally.ball.p, [rally.paddles[0].p, rally.paddles[1].p]);
    let want = [rally.paddles[YOU].p[0], rally.paddles[YOU].p[2]];
    let game = Game {
        moving: Layers::new(&context),
        context,
        canvas: canvas.clone(),
        floor,
        walls,
        table,
        net,
        fixed,
        batch: Batch::default(),
        ambient,
        lamp,
        fill,
        logo: None,
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
        timing: (0, 0.0, 0.0),
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

    // The loop: one frame per animation frame, timed for `stats()`.
    let next: Rc<RefCell<Option<Closure<dyn FnMut(f64)>>>> = Rc::new(RefCell::new(None));
    let first = next.clone();
    *first.borrow_mut() = Some(Closure::new(move |time: f64| {
        with_game(|g| {
            let started = clock();
            g.frame(time);
            let took = clock() - started;
            g.timing = (g.timing.0 + 1, g.timing.1 + took, g.timing.2.max(took));
        });
        let window = web_sys::window().unwrap();
        let _ = window.request_animation_frame(next.borrow().as_ref().unwrap().as_ref().unchecked_ref());
    }));
    window.request_animation_frame(first.borrow().as_ref().unwrap().as_ref().unchecked_ref())?;
    Ok(())
}

/// The logo to print on the table: a square canvas with it drawn on (the page draws its SVG).
#[wasm_bindgen]
pub fn logo(image: &HtmlCanvasElement) -> Result<(), JsValue> {
    let size = image.width().min(image.height());
    let context: CanvasRenderingContext2d = image.get_context("2d")?.ok_or("no 2d context")?.dyn_into()?;
    let pixels = context.get_image_data(0.0, 0.0, size as f64, size as f64)?.data().0;
    with_game(|g| {
        g.logo = Some((size, pixels));
        // As a material made from it would: its colours are sRGB, and the shader wants them linear.
        let mut texture = table_texture(g.logo.as_ref());
        texture.data.to_linear_srgb();
        g.table.material.albedo_texture = Some(Texture2DRef::from_cpu_texture(&g.context, &texture));
    });
    Ok(())
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

/// Frames drawn since the last call, and their average and worst milliseconds of work (the
/// game's ticks and drawing, on the CPU: the GPU's share isn't counted).
#[wasm_bindgen]
pub fn stats() -> Vec<f64> {
    let mut out = vec![0.0; 3];
    with_game(|g| {
        let (n, total, worst) = std::mem::take(&mut g.timing);
        out = vec![n as f64, if n > 0 { total / n as f64 } else { 0.0 }, worst];
    });
    out
}
