//! The game's rules: moving, crouching, the guns, what a shot hits. The server runs them to decide what
//! happens; the browser runs the same code to show your own moves and shots at once, then
//! takes the server's word when it comes (float rounding can differ a little between the two).
//!
//! One step is one tick, 1/60 s, with one `Input`: the keys held and where you're aiming.

use serde::{Deserialize, Serialize};

pub type V3 = [f32; 3];

pub fn add(a: V3, b: V3) -> V3 {
    [a[0] + b[0], a[1] + b[1], a[2] + b[2]]
}
pub fn sub(a: V3, b: V3) -> V3 {
    [a[0] - b[0], a[1] - b[1], a[2] - b[2]]
}
pub fn mul(a: V3, s: f32) -> V3 {
    [a[0] * s, a[1] * s, a[2] * s]
}
pub fn dot(a: V3, b: V3) -> f32 {
    a[0] * b[0] + a[1] * b[1] + a[2] * b[2]
}
pub fn cross(a: V3, b: V3) -> V3 {
    [a[1] * b[2] - a[2] * b[1], a[2] * b[0] - a[0] * b[2], a[0] * b[1] - a[1] * b[0]]
}
pub fn normalize(a: V3) -> V3 {
    mul(a, 1.0 / dot(a, a).sqrt().max(1e-6))
}
pub fn lerp(a: V3, b: V3, t: f32) -> V3 {
    add(a, mul(sub(b, a), t))
}

/// Where you look, from yaw (0 is -z, turning right) and pitch (up).
pub fn forward(yaw: f32, pitch: f32) -> V3 {
    let (s, c) = yaw.sin_cos();
    let (ps, pc) = pitch.sin_cos();
    [s * pc, ps, -c * pc]
}

/// Where a ray from `o` along `d` enters the box, and the normal of the face it enters by.
/// From inside, at 0.
pub fn ray_box(o: V3, d: V3, min: V3, max: V3) -> Option<(f32, V3)> {
    let (mut t0, mut t1) = (f32::NEG_INFINITY, f32::INFINITY);
    let mut n = [0.0; 3];
    for i in 0..3 {
        if d[i].abs() < 1e-9 {
            if o[i] < min[i] || o[i] > max[i] {
                return None;
            }
            continue;
        }
        let (mut a, mut b) = ((min[i] - o[i]) / d[i], (max[i] - o[i]) / d[i]);
        let mut sign = -1.0;
        if a > b {
            std::mem::swap(&mut a, &mut b);
            sign = 1.0;
        }
        if a > t0 {
            t0 = a;
            n = [0.0; 3];
            n[i] = sign;
        }
        t1 = t1.min(b);
        if t0 > t1 {
            return None;
        }
    }
    (t1 >= 0.0).then(|| (t0.max(0.0), n))
}

// ---- The rules' numbers -----------------------------------------------------------------------

pub const TICK_RATE: u32 = 60;
pub const TICK: f32 = 1.0 / TICK_RATE as f32;

// Moving, in metres and seconds: about a tactical shooter's.
const GRAVITY: f32 = 15.0;
const JUMP: f32 = 6.2;
pub const RUN: f32 = 5.2;
/// Crouched, you go at this much of a run, and your head's this much lower.
const CROUCH_SPEED: f32 = 0.34;
pub const CROUCH_DROP: f32 = 0.45;
const FRICTION: f32 = 5.5;
const ACCEL: f32 = 6.5;
const AIR_ACCEL: f32 = 12.0;
const AIR_CAP: f32 = 0.8;
const STEP: f32 = 0.45;
pub const HALF: f32 = 0.3;
pub const HEIGHT: f32 = 1.8;
pub const EYE: f32 = 1.62;

pub const RANGE: f32 = 150.0;
pub const RESPAWN_TICKS: u32 = 3 * TICK_RATE;

/// A gun. Times are in ticks; spreads are how far off the aim a shot may go.
pub struct Gun {
    pub name: &'static str,
    pub fire_ticks: u32,
    pub reload_ticks: u32,
    /// To get it out.
    pub draw_ticks: u32,
    pub mag: i32,
    pub reserve: i32,
    /// By part hit: 1 head, 2 body, 3 legs.
    pub damage: [i32; 4],
    /// Fires for as long as the trigger's held; otherwise once a click.
    pub auto: bool,
    /// Standing still, more at a run, and more for each shot in a quick string. (Not for the
    /// rifle's or magnum's first shot after a pause: that goes where it's aimed.)
    pub spread: f32,
    pub moving: f32,
    pub per_shot: f32,
    /// How far each shot kicks the aim up.
    pub kick: f32,
    /// How fast you run with it out (1 is a run).
    pub speed: f32,
    /// The field of view through its scope (radians), or 0: no scope.
    pub zoom: f32,
}

pub const RIFLE: u8 = 0;
pub const PISTOL: u8 = 1;
pub const SNIPER: u8 = 2;

pub const GUNS: [Gun; 3] = [
    // An assault rifle: 600 a minute, three to the body or one to the head.
    Gun {
        name: "Rifle",
        fire_ticks: 6,
        reload_ticks: 144,
        draw_ticks: 30,
        mag: 30,
        reserve: 90,
        damage: [0, 112, 34, 26],
        auto: true,
        spread: 0.0025,
        moving: 0.06,
        per_shot: 0.003,
        kick: 0.0155,
        speed: 1.0,
        zoom: 0.0,
    },
    // A heavy pistol: seven big rounds, a head shot kills, wild if you hurry it.
    Gun {
        name: "Magnum",
        fire_ticks: 14,
        reload_ticks: 132,
        draw_ticks: 24,
        mag: 7,
        reserve: 35,
        damage: [0, 180, 54, 40],
        auto: false,
        spread: 0.004,
        moving: 0.05,
        per_shot: 0.014,
        kick: 0.05,
        speed: 1.04,
        zoom: 0.0,
    },
    // A bolt-action sniper rifle: kills with any shot above the legs, a shot every 1.5 s,
    // only accurate scoped and still.
    Gun {
        name: "Sniper",
        fire_ticks: 88,
        reload_ticks: 220,
        draw_ticks: 60,
        mag: 10,
        reserve: 30,
        damage: [0, 400, 115, 85],
        auto: false,
        spread: 0.09,
        moving: 0.2,
        per_shot: 0.0,
        kick: 0.06,
        speed: 0.82,
        zoom: 0.31, // about 18 degrees
    },
];

/// An `Input`'s keys.
pub mod keys {
    pub const FORWARD: u16 = 1;
    pub const BACK: u16 = 2;
    pub const LEFT: u16 = 4;
    pub const RIGHT: u16 = 8;
    pub const JUMP: u16 = 16;
    pub const CROUCH: u16 = 32;
    pub const FIRE: u16 = 64;
    pub const RELOAD: u16 = 128;
    pub const SCOPE: u16 = 256;
}

// ---- The map ------------------------------------------------------------------------------------

pub struct Block {
    pub min: V3,
    pub max: V3,
    pub mat: f32,
}

pub struct Map {
    pub blocks: Vec<Block>,
}

impl Map {
    /// From boxes as x0, y0, z0, x1, y1, z1, material, ...
    pub fn new(flat: &[f32]) -> Map {
        let blocks = flat
            .chunks_exact(7)
            .map(|b| Block { min: [b[0], b[1], b[2]], max: [b[3], b[4], b[5]], mat: b[6] })
            .collect();
        Map { blocks }
    }

    /// Someone `height` tall with their feet at `p` would be inside a wall.
    pub fn walls(&self, p: V3, height: f32) -> bool {
        let (a, b) = ([p[0] - HALF, p[1], p[2] - HALF], [p[0] + HALF, p[1] + height, p[2] + HALF]);
        self.blocks.iter().any(|k| {
            a[0] < k.max[0] && b[0] > k.min[0] && a[1] < k.max[1] && b[1] > k.min[1] && a[2] < k.max[2] && b[2] > k.min[2]
        })
    }

    /// The highest top of a box under feet at `p`, at or below them.
    fn floor_below(&self, p: V3) -> f32 {
        self.blocks
            .iter()
            .filter(|k| {
                p[0] - HALF < k.max[0] && p[0] + HALF > k.min[0] && p[2] - HALF < k.max[2] && p[2] + HALF > k.min[2] && k.max[1] <= p[1] + 1e-3
            })
            .map(|k| k.max[1])
            .fold(f32::MIN, f32::max)
    }

    /// How far a ray goes before it hits the map, and the face's normal.
    pub fn ray(&self, o: V3, d: V3) -> (f32, Option<V3>) {
        let mut best = (RANGE, None);
        for b in &self.blocks {
            if let Some((t, n)) = ray_box(o, d, b.min, b.max) {
                if t < best.0 {
                    best = (t, Some(n));
                }
            }
        }
        best
    }
}

/// Where shots hit someone at `at` (feet), `crouch` of the way down (0 to 1): head, body, legs.
/// Upright, whichever way they face; crouching lowers the head and body and shortens the legs.
pub fn hitboxes(at: V3, crouch: f32) -> [(u8, V3, V3); 3] {
    let [x, y, z] = at;
    let d = crouch * CROUCH_DROP;
    [
        (1, [x - 0.13, y + 1.47 - d, z - 0.13], [x + 0.13, y + 1.78 - d, z + 0.13]),
        (2, [x - 0.24, y + 0.9 - d, z - 0.17], [x + 0.24, y + 1.47 - d, z + 0.17]),
        (3, [x - 0.2, y, z - 0.14], [x + 0.2, y + 0.9 - d, z + 0.14]),
    ]
}

/// What a shot from `o` along `d` hits first: how far, the wall's normal if a wall, and if one
/// of `targets` (people's feet, and how crouched), which and where (1 head, 2 body, 3 legs).
pub fn trace(map: &Map, o: V3, d: V3, targets: &[(V3, f32)]) -> (f32, Option<V3>, Option<(usize, u8)>) {
    let (mut best, mut normal) = map.ray(o, d);
    let mut hit = None;
    for (i, &(at, crouch)) in targets.iter().enumerate() {
        for (part, min, max) in hitboxes(at, crouch) {
            if let Some((t, _)) = ray_box(o, d, min, max) {
                if t < best {
                    best = t;
                    normal = None;
                    hit = Some((i, part));
                }
            }
        }
    }
    (best, normal, hit)
}

/// A number from 0 to 1 that's the same for the same (seed, n, k) everywhere: the spread of
/// shot `n` comes out the same in the browser and on the server.
fn noise(seed: u32, n: u32, k: u32) -> f32 {
    let mut x = seed.wrapping_mul(0x9E37_79B1) ^ n.wrapping_mul(0x85EB_CA77) ^ k.wrapping_mul(0xC2B2_AE3D);
    x ^= x >> 16;
    x = x.wrapping_mul(0x7FEB_352D);
    x ^= x >> 15;
    x = x.wrapping_mul(0x846C_A68B);
    x ^= x >> 16;
    (x as f64 / u32::MAX as f64) as f32
}

// ---- A player -----------------------------------------------------------------------------------

/// One tick's keys, aim and the gun wanted out, numbered so the server can say which it's
/// got to.
#[derive(Clone, Copy, Default, Debug, Serialize, Deserialize)]
pub struct Input {
    pub seq: u32,
    pub keys: u16,
    pub yaw: f32,
    pub pitch: f32,
    #[serde(default)]
    pub gun: u8,
}

/// A shot fired this tick, and with which gun.
pub struct Shot {
    pub origin: V3,
    pub dir: V3,
    pub gun: u8,
}

/// Everything about a player the rules need. The server sends you yours every snapshot.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Player {
    /// Feet, at the middle of the player.
    pub pos: V3,
    pub vel: V3,
    pub grounded: bool,
    pub yaw: f32,
    pub pitch: f32,
    /// Recoil: how far the shots have kicked the aim up and sideways.
    pub punch: [f32; 2],
    /// The gun out, and each gun's rounds: in it, and spare.
    pub gun: u8,
    pub ammo: [i32; 3],
    pub reserve: [i32; 3],
    /// Looking through the scope; to look through it again once the bolt's worked.
    pub scoped: bool,
    pub rescope: bool,
    /// How far down you are, 0 (standing) to 1 (crouched).
    pub crouch: f32,
    /// Whether the trigger and the scope button were held last tick (a click is a press).
    pub trigger: bool,
    pub scope_held: bool,
    /// Ticks left reloading (0: not).
    pub reload: u32,
    /// Ticks until it can fire again (or, just switched, until it's out).
    pub cooldown: u32,
    /// Shots in this burst.
    pub burst: u32,
    /// Ticks since the last shot.
    pub idle: u32,
    /// Shots ever: with `seed`, each one's spread.
    pub shots: u32,
    pub seed: u32,
    pub alive: bool,
    pub hp: i32,
    pub team: u8,
    /// Ticks left of the look-around just after spawning: you can look, but not move or shoot,
    /// and no one can see or hit you (the server's part), so a camper can't catch you there.
    #[serde(default)]
    pub shield: u32,
}

/// How long the look-around after spawning lasts: 3 s.
pub const SHIELD_TICKS: u32 = 180;

impl Player {
    pub fn new(seed: u32, team: u8) -> Player {
        Player {
            pos: [0.0; 3],
            vel: [0.0; 3],
            grounded: false,
            yaw: 0.0,
            pitch: 0.0,
            punch: [0.0; 2],
            gun: RIFLE,
            ammo: GUNS.map(|g| g.mag),
            reserve: GUNS.map(|g| g.reserve),
            scoped: false,
            rescope: false,
            crouch: 0.0,
            trigger: false,
            scope_held: false,
            reload: 0,
            cooldown: 0,
            burst: 0,
            idle: 100,
            shots: 0,
            seed,
            alive: false,
            hp: 0,
            team,
            shield: 0,
        }
    }

    /// Alive and out in the open: seen, solid, and fair game (not still looking around after
    /// spawning).
    pub fn exposed(&self) -> bool {
        self.alive && self.shield == 0
    }

    /// In at `at`, facing `yaw`: alive, full health, full ammo, the same gun out, and a few
    /// seconds to look around first (`shield`).
    pub fn spawn(&mut self, at: V3, yaw: f32) {
        let gun = self.gun;
        *self = Player { pos: at, yaw, alive: true, hp: 100, shots: self.shots, gun, shield: SHIELD_TICKS, ..Player::new(self.seed, self.team) };
    }

    pub fn gun(&self) -> &'static Gun {
        &GUNS[self.gun as usize]
    }

    pub fn height(&self) -> f32 {
        HEIGHT - self.crouch * CROUCH_DROP
    }

    pub fn eye(&self) -> V3 {
        [self.pos[0], self.pos[1] + EYE - self.crouch * CROUCH_DROP, self.pos[2]]
    }

    pub fn speed(&self) -> f32 {
        (self.vel[0] * self.vel[0] + self.vel[2] * self.vel[2]).sqrt()
    }

    /// How far off the aim a shot may go now: more when moving, in the air, deep in a burst;
    /// less crouched. A sniper rifle's only accurate through its scope.
    pub fn spread(&self) -> f32 {
        let g = self.gun();
        let still = if self.scoped { 0.0006 } else { g.spread };
        let mut spread = still + g.moving * (self.speed() / RUN).min(1.0) + g.per_shot * (self.burst.saturating_sub(1).min(8)) as f32;
        if !self.grounded {
            spread += 0.15;
        }
        if self.grounded && self.crouch > 0.5 {
            spread *= 0.7;
        }
        spread
    }

    /// One tick: moves by `input` through `map`, not into `others` (their feet), and fires if
    /// it can. The shot, if one.
    pub fn step(&mut self, input: &Input, map: &Map, others: &[V3]) -> Option<Shot> {
        if !self.alive {
            return None;
        }
        self.yaw = input.yaw;
        self.pitch = input.pitch.clamp(-1.5, 1.5);

        // Just spawned: looking around only. (A trigger held as it ends isn't a click.)
        if self.shield > 0 {
            self.shield -= 1;
            self.trigger = input.keys & keys::FIRE != 0;
            self.walk(0, map, others);
            return None;
        }

        // Down when crouch is held; up when it isn't, if there's room overhead.
        let down = input.keys & keys::CROUCH != 0 || (self.crouch > 0.0 && map.walls(self.pos, HEIGHT));
        let target = if down { 1.0 } else { 0.0 };
        self.crouch += (target - self.crouch).clamp(-TICK * 7.0, TICK * 7.0);
        self.walk(input.keys, map, others);

        // Another gun: out after a moment; a reload or the scope's lost.
        if input.gun != self.gun && (input.gun as usize) < GUNS.len() {
            self.gun = input.gun;
            self.reload = 0;
            self.scoped = false;
            self.rescope = false;
            self.burst = 0;
            self.cooldown = self.gun().draw_ticks;
        }
        let g = self.gun();
        let n = self.gun as usize;

        self.cooldown = self.cooldown.saturating_sub(1);
        if self.rescope && self.cooldown == 0 {
            self.scoped = true;
            self.rescope = false;
        }
        if self.reload > 0 {
            self.reload -= 1;
            if self.reload == 0 {
                let take = (g.mag - self.ammo[n]).min(self.reserve[n]);
                self.ammo[n] += take;
                self.reserve[n] -= take;
            }
        }
        if input.keys & keys::RELOAD != 0 {
            self.start_reload();
        }
        let scope = input.keys & keys::SCOPE != 0;
        if scope && !self.scope_held && g.zoom > 0.0 && self.reload == 0 {
            self.scoped = !self.scoped && !self.rescope;
            self.rescope = false;
        }
        self.scope_held = scope;

        let pulled = input.keys & keys::FIRE != 0;
        let fire = pulled && (g.auto || !self.trigger);
        self.trigger = pulled;
        let mut shot = None;
        if fire && self.reload == 0 && self.cooldown == 0 {
            if self.ammo[n] == 0 {
                self.cooldown = 15;
                self.start_reload();
            } else {
                shot = Some(self.fire());
            }
        } else {
            self.idle = self.idle.saturating_add(1);
        }
        // Recoil wears off once you stop shooting.
        if self.idle > 6 {
            let k = (TICK * 7.0).min(1.0);
            self.punch = [self.punch[0] * (1.0 - k), self.punch[1] * (1.0 - k)];
        }
        if self.idle > 18 {
            self.burst = 0;
        }
        shot
    }

    fn start_reload(&mut self) {
        let (g, n) = (self.gun(), self.gun as usize);
        if self.reload == 0 && self.ammo[n] < g.mag && self.reserve[n] > 0 {
            self.reload = g.reload_ticks;
            self.scoped = false;
            self.rescope = false;
        }
    }

    fn fire(&mut self) -> Shot {
        let g = self.gun();
        self.ammo[self.gun as usize] -= 1;
        self.burst += 1;
        self.shots += 1;
        self.cooldown = g.fire_ticks;
        self.idle = 0;

        // Where it goes: the aim, kicked up by the burst so far, then off by the spread.
        let aim = forward(self.yaw + self.punch[1], self.pitch + self.punch[0]);
        let (s, c) = (self.yaw + self.punch[1]).sin_cos();
        let right = [c, 0.0, s];
        let up = cross(right, aim);
        let a = noise(self.seed, self.shots, 0) * std::f32::consts::TAU;
        // The rifle's and magnum's first shot (after a pause) goes where it's aimed.
        let first = self.burst == 1 && self.gun != SNIPER;
        let r = if first { 0.0 } else { noise(self.seed, self.shots, 1).sqrt() * self.spread() };
        let dir = normalize(add(aim, add(mul(right, a.cos() * r), mul(up, a.sin() * r))));

        // Recoil. The rifle's climbs for the first shots, then wanders side to side; the others
        // just kick.
        if self.gun == RIFLE {
            self.punch[0] = (self.punch[0] + if self.burst <= 9 { g.kick } else { 0.002 }).min(0.2);
            if self.burst > 4 {
                self.punch[1] += (self.burst as f32 * 0.55).sin() * 0.011;
            }
        } else {
            self.punch[0] = (self.punch[0] + g.kick).min(0.25);
        }
        // The sniper rifle's bolt is worked off the scope, then it's back to it.
        if self.scoped {
            self.scoped = false;
            self.rescope = true;
        }
        Shot { origin: self.eye(), dir, gun: self.gun }
    }

    fn walk(&mut self, held: u16, map: &Map, others: &[V3]) {
        let (s, c) = self.yaw.sin_cos();
        let mut wish = [0.0f32, 0.0];
        for (key, f, r) in [(keys::FORWARD, 1.0, 0.0), (keys::BACK, -1.0, 0.0), (keys::RIGHT, 0.0, 1.0), (keys::LEFT, 0.0, -1.0)] {
            if held & key != 0 {
                wish[0] += s * f + c * r;
                wish[1] += -c * f + s * r;
            }
        }
        let len = (wish[0] * wish[0] + wish[1] * wish[1]).sqrt();
        let mut top = RUN * self.gun().speed * if self.scoped { 0.55 } else { 1.0 };
        if self.crouch > 0.5 {
            top = top.min(RUN * CROUCH_SPEED);
        }
        let (dir, speed) = if len > 0.0 { ([wish[0] / len, wish[1] / len], top) } else { ([0.0, 0.0], 0.0) };

        let accelerate = |vel: &mut V3, speed: f32, rate: f32| {
            let add = speed - (vel[0] * dir[0] + vel[2] * dir[1]);
            if add > 0.0 {
                let a = (rate * top * TICK).min(add);
                vel[0] += dir[0] * a;
                vel[2] += dir[1] * a;
            }
        };
        let mut vel = self.vel;
        if self.grounded {
            let sp = (vel[0] * vel[0] + vel[2] * vel[2]).sqrt();
            if sp > 0.0 {
                let k = (sp - sp.max(1.5) * FRICTION * TICK).max(0.0) / sp;
                vel[0] *= k;
                vel[2] *= k;
            }
            accelerate(&mut vel, speed, ACCEL);
            if held & keys::JUMP != 0 {
                vel[1] = JUMP;
                self.grounded = false;
            }
        } else {
            accelerate(&mut vel, speed.min(AIR_CAP), AIR_ACCEL);
        }
        vel[1] = (vel[1] - GRAVITY * TICK).max(-25.0);
        self.vel = vel;

        if map.walls(self.pos, self.height()) {
            self.pos[1] += 0.05; // stuck (spawned into something): out, a little at a time
        }
        if !self.slide(0, vel[0] * TICK, map, others) {
            self.vel[0] = 0.0;
        }
        if !self.slide(2, vel[2] * TICK, map, others) {
            self.vel[2] = 0.0;
        }
        self.fall(self.vel[1] * TICK, map, others);
        if self.pos[1] < -10.0 {
            self.pos[1] = 2.0; // out of the map: back on top
        }
    }

    /// Whether you can move from `from` to `p`. Players are solid, but two already overlapping
    /// (both just spawned, say) can walk apart.
    fn free(&self, p: V3, from: V3, map: &Map, others: &[V3]) -> bool {
        let bumps = |p: V3| {
            others.iter().any(|o| (p[0] - o[0]).abs() < 2.0 * HALF && (p[2] - o[2]).abs() < 2.0 * HALF && (p[1] - o[1]).abs() < HEIGHT)
        };
        !map.walls(p, self.height()) && !(bumps(p) && !bumps(from))
    }

    /// Moves `by` along x or z, stepping up anything up to a stair high. Whether it could.
    fn slide(&mut self, axis: usize, by: f32, map: &Map, others: &[V3]) -> bool {
        let mut p = self.pos;
        p[axis] += by;
        if self.free(p, self.pos, map, others) {
            self.pos = p;
            return true;
        }
        if self.grounded {
            let mut lifted = self.pos;
            lifted[1] += STEP;
            let mut up = p;
            up[1] += STEP;
            if self.free(lifted, self.pos, map, others) && self.free(up, lifted, map, others) {
                up[1] = map.floor_below(up).max(self.pos[1]);
                self.pos = up;
                return true;
            }
        }
        false
    }

    fn fall(&mut self, by: f32, map: &Map, others: &[V3]) {
        let mut p = self.pos;
        p[1] += by;
        if self.free(p, self.pos, map, others) {
            self.pos = p;
            self.grounded = false;
            return;
        }
        if by < 0.0 {
            self.grounded = true;
            let floor = map.floor_below(self.pos);
            if floor >= p[1] - 1e-3 {
                self.pos[1] = floor; // land on it, not just above
            }
        }
        self.vel[1] = 0.0;
    }
}

// ---- What goes over the wire (JSON) -----------------------------------------------------------

#[derive(Debug, Serialize, Deserialize)]
#[serde(tag = "t", rename_all = "snake_case")]
pub enum ToServer {
    Join { name: String },
    /// The ticks played since the last message, and the server tick the page was showing
    /// everyone else at (for lag compensation).
    Input { view: f32, inputs: Vec<Input> },
    /// The bots' level for the whole match: 0 easy, 1 normal, 2 hard.
    Level { level: u8 },
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(tag = "t", rename_all = "snake_case")]
pub enum ToClient {
    Welcome { id: u32, team: u8 },
    /// Where everyone is at `tick`. `you` is your own state after your input `ack`.
    Snapshot { tick: u32, ack: u32, you: Option<Player>, players: Vec<Seen>, events: Vec<Event> },
    Roster { players: Vec<Entry> },
    /// The bots' level for the whole match (0 easy, 1 normal, 2 hard): on connecting, and
    /// whenever anyone changes it.
    Level { level: u8 },
}

/// Someone else, as everyone sees them.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Seen {
    pub id: u32,
    pub pos: V3,
    pub yaw: f32,
    pub pitch: f32,
    pub team: u8,
    pub alive: bool,
    pub crouch: f32,
    pub gun: u8,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(tag = "t", rename_all = "snake_case")]
pub enum Event {
    Shot { id: u32, from: V3, to: V3, gun: u8 },
    Hit { target: u32, by: u32, hp: i32 },
    Kill { killer: u32, victim: u32, head: bool, gun: u8 },
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Entry {
    pub id: u32,
    pub name: String,
    pub team: u8,
    pub kills: u32,
    pub deaths: u32,
    pub alive: bool,
}
