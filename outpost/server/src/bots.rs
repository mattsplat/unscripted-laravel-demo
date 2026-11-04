//! Computer players. Each one is a `Brain` that looks at the game and makes up a tick's
//! `Input`, as a page would; the server runs it through the same rules as everyone else's, so
//! bots can't do anything a person couldn't.
//!
//! - Getting about: the map's ground is cut into half-metre cells, free where someone fits;
//!   a bot finds the shortest way over them to wherever it's going (somewhere random, leaning
//!   toward the other side; where it last saw someone; whoever just shot it).
//! - Fighting: it only knows about people it could see (ahead of it, nothing in between). It
//!   takes a moment to react, its aim starts off and settles, and it fires as a person would:
//!   the rifle in bursts standing still, the magnum a click at a time, the sniper rifle scoped.

use crate::sim::*;
use std::cmp::Reverse;
use std::collections::BinaryHeap;

/// Ticks from first seeing someone to first shooting at them (0.6 s).
const REACT: u32 = 36;
/// How far to either side a bot notices people (from straight ahead), unless it's already on
/// to them.
const FOV_COS: f32 = 0.57; // about 55 degrees
/// The most a bot turns in a tick (radians): about 4.8 a second.
const TURN: f32 = 0.08;

/// Where people fit on the ground: cells of `CELL` metres.
pub struct Nav {
    min: [f32; 2],
    w: usize,
    h: usize,
    /// Each cell: someone fits there, and it can be walked to from the spawns.
    free: Vec<bool>,
}

const CELL: f32 = 0.5;

impl Nav {
    /// The grid over `map`, keeping only cells that can be got to from `start`.
    pub fn new(map: &Map, start: V3) -> Nav {
        let lo = map.blocks.iter().fold([f32::MAX; 2], |a, b| [a[0].min(b.min[0]), a[1].min(b.min[2])]);
        let hi = map.blocks.iter().fold([f32::MIN; 2], |a, b| [a[0].max(b.max[0]), a[1].max(b.max[2])]);
        let (w, h) = (((hi[0] - lo[0]) / CELL) as usize, ((hi[1] - lo[1]) / CELL) as usize);
        let mut nav = Nav { min: lo, w, h, free: vec![false; w * h] };
        let fits: Vec<bool> = (0..w * h).map(|i| !map.walls(nav.center(i), HEIGHT)).collect();
        // Only what's reachable: a flood from the start.
        let Some(first) = nav.cell(start).filter(|&i| fits[i]) else { return nav };
        let mut todo = vec![first];
        nav.free[first] = true;
        while let Some(i) = todo.pop() {
            for (j, _) in nav.neighbours(i, &fits) {
                if !nav.free[j] {
                    nav.free[j] = true;
                    todo.push(j);
                }
            }
        }
        nav
    }

    fn cell(&self, p: V3) -> Option<usize> {
        let (x, z) = ((p[0] - self.min[0]) / CELL, (p[2] - self.min[1]) / CELL);
        (x >= 0.0 && z >= 0.0 && (x as usize) < self.w && (z as usize) < self.h).then(|| z as usize * self.w + x as usize)
    }

    fn center(&self, i: usize) -> V3 {
        [self.min[0] + ((i % self.w) as f32 + 0.5) * CELL, 0.02, self.min[1] + ((i / self.w) as f32 + 0.5) * CELL]
    }

    /// The cells next to `i` that are `ok`, and the cost of the step: eight ways, but not
    /// cutting a corner.
    fn neighbours(&self, i: usize, ok: &[bool]) -> Vec<(usize, u32)> {
        let (x, z) = ((i % self.w) as i32, (i / self.w) as i32);
        let at = |dx: i32, dz: i32| {
            let (nx, nz) = (x + dx, z + dz);
            (nx >= 0 && nz >= 0 && (nx as usize) < self.w && (nz as usize) < self.h)
                .then(|| nz as usize * self.w + nx as usize)
                .filter(|&j| ok[j])
        };
        let mut out = Vec::with_capacity(8);
        for (dx, dz) in [(1, 0), (-1, 0), (0, 1), (0, -1)] {
            if let Some(j) = at(dx, dz) {
                out.push((j, 10));
            }
        }
        for (dx, dz) in [(1, 1), (1, -1), (-1, 1), (-1, -1)] {
            if let (Some(j), Some(_), Some(_)) = (at(dx, dz), at(dx, 0), at(0, dz)) {
                out.push((j, 14));
            }
        }
        out
    }

    /// The free cell nearest `p` (it may be standing just off the grid, against a wall).
    fn nearest(&self, p: V3) -> Option<usize> {
        let here = self.cell(p)?;
        if self.free[here] {
            return Some(here);
        }
        (0..self.free.len()).filter(|&i| self.free[i]).min_by(|&a, &b| {
            let (da, db) = (sub(self.center(a), p), sub(self.center(b), p));
            dot(da, da).total_cmp(&dot(db, db))
        })
    }

    /// The way from `from` to `to`, as points to walk through (the corners only), or empty
    /// if there isn't one.
    pub fn path(&self, from: V3, to: V3) -> Vec<V3> {
        let (Some(start), Some(goal)) = (self.nearest(from), self.nearest(to)) else { return Vec::new() };
        let guess = |i: usize| {
            let d = sub(self.center(i), self.center(goal));
            ((d[0].abs() + d[2].abs()) / CELL * 10.0) as u32
        };
        let mut cost = vec![u32::MAX; self.free.len()];
        let mut came = vec![usize::MAX; self.free.len()];
        let mut open = BinaryHeap::new();
        cost[start] = 0;
        open.push(Reverse((guess(start), start)));
        while let Some(Reverse((_, i))) = open.pop() {
            if i == goal {
                break;
            }
            for (j, step) in self.neighbours(i, &self.free) {
                let c = cost[i] + step;
                if c < cost[j] {
                    cost[j] = c;
                    came[j] = i;
                    open.push(Reverse((c + guess(j), j)));
                }
            }
        }
        if cost[goal] == u32::MAX {
            return Vec::new();
        }
        let mut cells = vec![goal];
        while *cells.last().unwrap() != start {
            cells.push(came[*cells.last().unwrap()]);
        }
        cells.reverse();
        // Straight on wherever the way's clear, so it walks lines, not stairs.
        let mut points = Vec::new();
        let mut at = self.center(start);
        let mut k = 1;
        while k < cells.len() {
            let mut far = k;
            while far + 1 < cells.len() && self.clear(at, self.center(cells[far + 1])) {
                far += 1;
            }
            at = self.center(cells[far]);
            points.push(at);
            k = far + 1;
        }
        points
    }

    /// Every cell on the line from `a` to `b` is free.
    fn clear(&self, a: V3, b: V3) -> bool {
        let d = sub(b, a);
        let steps = ((d[0].abs() + d[2].abs()) / (CELL * 0.5)).ceil() as usize;
        (0..=steps).all(|s| self.cell(lerp(a, b, s as f32 / steps.max(1) as f32)).is_some_and(|i| self.free[i]))
    }

    /// A free cell at random, on the side of the map where x has `side`'s sign (or anywhere,
    /// for 0).
    fn somewhere(&self, rng: &mut u32, side: f32) -> Option<V3> {
        for _ in 0..200 {
            let i = (next(rng) * self.free.len() as f32) as usize % self.free.len();
            if self.free[i] && self.center(i)[0] * side >= 0.0 {
                return Some(self.center(i));
            }
        }
        None
    }
}

/// Someone else, as a bot sees them.
pub struct Other {
    pub id: u32,
    pub pos: V3,
    pub crouch: f32,
    pub team: u8,
    pub alive: bool,
}

pub struct Brain {
    rng: u32,
    /// Where it's going, and the way there.
    goal: Option<V3>,
    path: Vec<V3>,
    /// Who it's fighting; since when; whether it's going for their head; how far off its aim
    /// still is (yaw, pitch).
    target: Option<u32>,
    seen_at: u32,
    head: bool,
    error: [f32; 2],
    /// Where it last saw someone, and where it was last shot from.
    last_seen: Option<V3>,
    pub shot_from: Option<V3>,
    /// Where it's looking.
    yaw: f32,
    pitch: f32,
    /// Ticks of trigger left in this burst, and of rest before the next.
    burst: u32,
    rest: u32,
    /// Side-stepping between shots: which way, for how long more.
    strafe: f32,
    strafe_for: u32,
    crouching: bool,
    /// For noticing it's stuck: where it was, and how many ticks it's been about there.
    was_at: V3,
    still: u32,
    gun: u8,
    alive: bool,
}

/// 0 to 1, from the brain's own sequence.
fn next(rng: &mut u32) -> f32 {
    *rng ^= *rng << 13;
    *rng ^= *rng >> 17;
    *rng ^= *rng << 5;
    (*rng as f64 / u32::MAX as f64) as f32
}

/// The way from angle `a` to `b`, the short way round.
fn turn(a: f32, b: f32) -> f32 {
    (b - a + std::f32::consts::PI).rem_euclid(std::f32::consts::TAU) - std::f32::consts::PI
}

/// The yaw and pitch that look from `from` at `to`.
fn angles(from: V3, to: V3) -> (f32, f32) {
    let d = sub(to, from);
    (d[0].atan2(-d[2]), d[1].atan2((d[0] * d[0] + d[2] * d[2]).sqrt()))
}

/// The keys that move along `dir` (on the ground) while looking along `yaw`.
fn move_keys(yaw: f32, dir: V3) -> u16 {
    if dir[0] * dir[0] + dir[2] * dir[2] < 1e-6 {
        return 0;
    }
    let a = turn(yaw, dir[0].atan2(-dir[2]));
    let mut keys = 0;
    if a.cos() > 0.38 {
        keys |= keys::FORWARD;
    }
    if a.cos() < -0.38 {
        keys |= keys::BACK;
    }
    if a.sin() > 0.38 {
        keys |= keys::RIGHT;
    }
    if a.sin() < -0.38 {
        keys |= keys::LEFT;
    }
    keys
}

impl Brain {
    pub fn new(seed: u32) -> Brain {
        Brain {
            rng: seed.wrapping_mul(2_654_435_761) | 1,
            goal: None,
            path: Vec::new(),
            target: None,
            seen_at: 0,
            head: false,
            error: [0.0; 2],
            last_seen: None,
            shot_from: None,
            yaw: 0.0,
            pitch: 0.0,
            burst: 0,
            rest: 0,
            strafe: 1.0,
            strafe_for: 0,
            crouching: false,
            was_at: [0.0; 3],
            still: 0,
            gun: RIFLE,
            alive: false,
        }
    }

    fn random(&mut self) -> f32 {
        next(&mut self.rng)
    }

    /// This tick's input (all but its number) for player `id`, `me`, among everyone.
    pub fn think(&mut self, id: u32, me: &Player, everyone: &[Other], map: &Map, nav: &Nav, tick: u32) -> Input {
        let others = everyone.iter().filter(|o| o.id != id);
        if !me.alive {
            self.alive = false;
            return Input { gun: self.gun, ..Input::default() };
        }
        if !self.alive {
            // Just in: forget the last life, and pick a gun (mostly the rifle).
            self.alive = true;
            self.yaw = me.yaw;
            self.pitch = 0.0;
            self.goal = None;
            self.path.clear();
            self.target = None;
            self.last_seen = None;
            self.shot_from = None;
            let r = self.random();
            self.gun = if r < 0.65 { RIFLE } else if r < 0.82 { SNIPER } else { PISTOL };
        }
        let eye = me.eye();
        let mut keys = 0;

        // Who it can see: ahead of it (or the one it's fighting), nothing in the way. The
        // nearest.
        let ahead = forward(self.yaw, 0.0);
        let seen = others
            .filter(|o| o.alive && o.team != me.team)
            .filter_map(|o| {
                let chest = add(o.pos, [0.0, 1.2 - o.crouch * CROUCH_DROP, 0.0]);
                let d = sub(chest, eye);
                let dist = dot(d, d).sqrt();
                let flat = normalize([d[0], 0.0, d[2]]);
                let noticed = Some(o.id) == self.target || self.shot_from.is_some() || dot(flat, ahead) > FOV_COS;
                (noticed && dist < RANGE && map.ray(eye, mul(d, 1.0 / dist)).0 >= dist - 0.05).then_some((o, dist))
            })
            .min_by(|a, b| a.1.total_cmp(&b.1));

        if let Some((o, dist)) = seen {
            if self.target != Some(o.id) {
                // Someone new: a moment to react, and the aim starts off.
                self.target = Some(o.id);
                self.seen_at = tick;
                self.head = self.random() < 0.1;
                self.error = [(self.random() - 0.5) * 0.6, (self.random() - 0.5) * 0.3];
                self.crouching = self.gun != PISTOL && dist > 12.0 && self.random() < 0.4;
            }
            self.last_seen = Some(o.pos);
            self.shot_from = None;
            self.goal = None;
            self.path.clear();

            let height = if self.head { 1.62 } else { 1.2 } - o.crouch * CROUCH_DROP;
            let (want_yaw, want_pitch) = angles(eye, add(o.pos, [0.0, height, 0.0]));
            self.error = [self.error[0] * 0.96, self.error[1] * 0.96];
            self.look(want_yaw + self.error[0], want_pitch + self.error[1]);
            // How far off the aim is, against how big the target looks from here.
            let off = turn(self.yaw, want_yaw).abs() + (want_pitch - self.pitch).abs();
            let size = (if self.head { 0.11 } else { 0.2 }) / dist;
            let ready = tick - self.seen_at >= REACT && off < size;

            let mut firing = false;
            match self.gun {
                SNIPER => {
                    if !me.scoped && !me.rescope && !me.scope_held && me.reload == 0 && me.cooldown == 0 {
                        keys |= keys::SCOPE;
                    }
                    firing = ready && me.scoped && me.speed() < 0.4;
                }
                PISTOL => firing = ready && !me.trigger && me.cooldown == 0,
                _ => {
                    if self.burst > 0 {
                        self.burst -= 1;
                        firing = true;
                    } else if self.rest > 0 {
                        self.rest -= 1;
                    } else if ready {
                        // Shorter bursts further off.
                        let shots = if dist > 20.0 { 3 } else { 4 + (self.random() * 5.0) as u32 };
                        self.burst = shots * GUNS[RIFLE as usize].fire_ticks;
                        self.rest = 24 + (self.random() * 24.0) as u32;
                        firing = true;
                    }
                }
            }
            if firing {
                keys |= keys::FIRE;
            }
            // Still to shoot (as people do: moving throws shots off); side-stepping otherwise,
            // but a sniper keeps still.
            if !firing && self.gun != SNIPER && !self.crouching {
                if self.strafe_for == 0 {
                    self.strafe = if self.random() < 0.5 { -1.0 } else { 1.0 };
                    self.strafe_for = 20 + (self.random() * 40.0) as u32;
                }
                self.strafe_for -= 1;
                keys |= if self.strafe > 0.0 { keys::RIGHT } else { keys::LEFT };
            }
            if self.crouching {
                keys |= keys::CROUCH;
            }
        } else {
            self.target = None;
            self.burst = 0;
            // Going somewhere: whoever shot it, where it last saw someone, or anywhere,
            // leaning toward the other side.
            if let Some(from) = self.shot_from {
                self.goal = Some(from);
            } else if self.goal.is_none() {
                self.goal = self.last_seen.take().or_else(|| {
                    let side = if me.team == 0 { 1.0 } else { -1.0 };
                    let side = if self.random() < 0.6 { side } else { 0.0 };
                    nav.somewhere(&mut self.rng, side)
                });
            }
            if let Some(goal) = self.goal {
                if self.path.is_empty() {
                    self.path = nav.path(me.pos, goal);
                    if self.path.is_empty() {
                        self.goal = None; // can't get there: somewhere else, next tick
                        self.shot_from = None;
                    }
                }
            }
            while let Some(&p) = self.path.first() {
                let d = sub(p, me.pos);
                if d[0] * d[0] + d[2] * d[2] < 0.35 * 0.35 {
                    self.path.remove(0);
                } else {
                    break;
                }
            }
            if self.path.is_empty() && self.goal.is_some() {
                self.goal = None;
                self.shot_from = None;
            }
            if let Some(&p) = self.path.first() {
                let d = sub(p, me.pos);
                // Look the way it's going (or back at whoever shot it), and walk.
                let (yaw, _) = match self.shot_from {
                    Some(from) => angles(eye, from),
                    None => angles(me.pos, p),
                };
                self.look(yaw, 0.0);
                keys |= move_keys(self.yaw, d);
            } else {
                self.look(self.yaw, 0.0);
            }
            // Reload while there's no one about.
            let n = me.gun as usize;
            if me.ammo[n] < GUNS[n].mag / 2 {
                keys |= keys::RELOAD;
            }
            // Stuck on something (a corner, a crate, someone else): hop, and find another way.
            let moved = sub(me.pos, self.was_at);
            if dot(moved, moved) < 0.3 * 0.3 && keys & (keys::FORWARD | keys::BACK | keys::LEFT | keys::RIGHT) != 0 {
                self.still += 1;
                if self.still > 45 {
                    self.still = 0;
                    self.path.clear();
                    self.goal = None;
                    keys |= keys::JUMP;
                }
            } else {
                self.still = 0;
                self.was_at = me.pos;
            }
        }
        if me.ammo[me.gun as usize] == 0 {
            keys |= keys::RELOAD;
        }

        // Its aim, held down against the recoil (most of it).
        Input { seq: 0, keys, yaw: self.yaw - me.punch[1] * 0.8, pitch: self.pitch - me.punch[0] * 0.8, gun: self.gun }
    }

    /// Turns toward a yaw and pitch, no faster than a person could.
    fn look(&mut self, yaw: f32, pitch: f32) {
        let dy = turn(self.yaw, yaw);
        self.yaw += (dy * 0.35).clamp(-TURN, TURN);
        self.pitch += ((pitch - self.pitch) * 0.35).clamp(-TURN, TURN);
    }
}
