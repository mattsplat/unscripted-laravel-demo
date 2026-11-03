//! Rally's rules: the ball's flight (gravity, drag, spin), its bounces off the table and the net,
//! hitting it, the scoring, and the computer player. One `tick` is 1/60 s; the ball moves in eight
//! steps a tick, so a fast one can't pass through the net or a paddle between two.
//!
//! In metres. x is across the table, y up, z along it: the net is at z = 0, you're at +z and the
//! computer's at -z.
//!
//! Hitting is helped: when the ball meets your paddle, the shot is worked out from your swing
//! (how fast forward: power; sideways: where it goes and its sidespin; right button: backspin),
//! then flown for real. Where you met the ball on the paddle, and swinging too hard, add error,
//! which is what sends a ball long, wide or into the net.

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
pub fn len(a: V3) -> f32 {
    dot(a, a).sqrt()
}
pub fn normalize(a: V3) -> V3 {
    mul(a, 1.0 / len(a).max(1e-6))
}
pub fn lerp(a: V3, b: V3, t: f32) -> V3 {
    add(a, mul(sub(b, a), t))
}
fn mix(a: f32, b: f32, t: f32) -> f32 {
    a + (b - a) * t
}

pub const TICK: f32 = 1.0 / 60.0;
const STEPS: usize = 8;
const DT: f32 = TICK / STEPS as f32;

// A regulation table.
pub const TABLE_Y: f32 = 0.76;
pub const HALF_W: f32 = 0.7625;
pub const HALF_L: f32 = 1.37;
pub const NET_TOP: f32 = TABLE_Y + 0.1525;
pub const NET_HALF: f32 = 0.915;
pub const BALL_R: f32 = 0.02;
/// The paddle's blade, and how far from its middle the ball can be and still be hit.
pub const PADDLE_R: f32 = 0.08;
const REACH: f32 = PADDLE_R + BALL_R + 0.02;
/// Where a paddle rests, over the floor, when there's nothing to hit.
const REST_Y: f32 = 0.95;
/// Where your paddle can go: x either way, z from over the table's end to well back.
pub const YOUR_X: f32 = 1.2;
pub const YOUR_Z: [f32; 2] = [1.15, 2.6];

const GRAVITY: f32 = 9.81;
/// Air drag (per metre: a = -DRAG |v| v), and the pull of spin (a = MAGNUS w x v).
const DRAG: f32 = 0.12;
const MAGNUS: f32 = 0.004;
/// The table's bounce, and its grip on a spinning ball: how much of the bottom's slip it takes.
const BOUNCE: f32 = 0.88;
const GRIP: f32 = 0.25;

pub const YOU: usize = 0;
pub const CPU: usize = 1;

/// Which way z goes from the net to `side`'s end.
pub fn sign(side: usize) -> f32 {
    if side == YOU {
        1.0
    } else {
        -1.0
    }
}

#[derive(Clone, Copy, Debug)]
pub struct Ball {
    pub p: V3,
    pub v: V3,
    /// Spin, rad/s, about its axis.
    pub w: V3,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Touch {
    Table(V3),
    Net,
    Floor,
}

impl Ball {
    /// One step of flight. `wide`: the table goes on forever to the sides (for aiming).
    pub fn step(&mut self, wide: bool) -> Option<Touch> {
        let prev = self.p;
        let pull = add(mul(self.v, -DRAG * len(self.v)), mul(cross(self.w, self.v), MAGNUS));
        self.v = add(self.v, mul(add([0.0, -GRAVITY, 0.0], pull), DT));
        self.p = add(self.p, mul(self.v, DT));
        self.w = mul(self.w, 1.0 - 0.4 * DT);

        let crossed = (prev[2] > 0.0) != (self.p[2] > 0.0);
        if crossed && self.p[0].abs() < NET_HALF && self.p[1] - BALL_R < NET_TOP && self.p[1] > TABLE_Y {
            self.p[2] = if prev[2] > 0.0 { BALL_R } else { -BALL_R };
            self.v = [self.v[0] * 0.4, self.v[1] * 0.4, -self.v[2] * 0.12];
            self.w = mul(self.w, 0.3);
            return Some(Touch::Net);
        }
        let on_table = (wide || self.p[0].abs() <= HALF_W) && self.p[2].abs() <= HALF_L;
        if self.v[1] < 0.0 && self.p[1] < TABLE_Y + BALL_R && prev[1] >= TABLE_Y + BALL_R - 0.002 && on_table {
            self.p[1] = TABLE_Y + BALL_R;
            self.v[1] *= -BOUNCE;
            // The table's grip on the bottom of the ball: topspin kicks on, backspin holds back.
            // (A quarter of the way to rolling: 0.4 of the slip would get there.)
            let rim = cross(self.w, [0.0, -BALL_R, 0.0]);
            let slip = [self.v[0] + rim[0], 0.0, self.v[2] + rim[2]];
            let dv = mul(slip, -GRIP);
            self.v = add(self.v, dv);
            self.w = add(self.w, mul(cross([0.0, 1.0, 0.0], dv), -1.5 / BALL_R));
            return Some(Touch::Table(self.p));
        }
        if self.v[1] < 0.0 && self.p[1] < BALL_R {
            self.p[1] = BALL_R;
            self.v = [self.v[0] * 0.7, -self.v[1] * 0.55, self.v[2] * 0.7];
            self.w = mul(self.w, 0.5);
            return Some(Touch::Floor);
        }
        None
    }
}

/// Where a ball hit from `from`'s side comes down past the net: how far past (into the far side,
/// going on past its end), and across. -1 if it doesn't get there (the net, or its own side),
/// or if it's a serve that doesn't bounce on its own side first.
fn landing(mut b: Ball, from: usize, serve: bool) -> (f32, f32) {
    let s = sign(from);
    let mut own = false;
    for _ in 0..(1.6 / DT) as usize {
        let above = b.p[1] >= TABLE_Y + BALL_R;
        match b.step(true) {
            Some(Touch::Table(p)) if p[2] * s > 0.0 => {
                if serve && !own {
                    own = true;
                    continue;
                }
                return (-1.0, p[0]);
            }
            Some(Touch::Table(p)) => return (if serve && !own { -1.0 } else { -p[2] * s }, p[0]),
            Some(Touch::Floor) => return (-1.0, b.p[0]),
            _ => {}
        }
        // Past the far end: where it comes down through the table's height.
        if above && b.p[1] < TABLE_Y + BALL_R && b.v[1] < 0.0 && -b.p[2] * s > HALF_L {
            return (if serve && !own { -1.0 } else { -b.p[2] * s }, b.p[0]);
        }
        if b.p[2] * s > HALF_L + 0.5 && b.p[1] < TABLE_Y {
            return (-1.0, b.p[0]);
        }
    }
    (-1.0, b.p[0])
}

/// The velocity that sends a ball from `p`, spinning `w`, at `speed` m/s along the table, down at
/// `target`: x across, and how far past the net. If it can't come down there that fast (from
/// low down, say), it goes slower.
pub fn aim(p: V3, w: V3, speed: f32, target: [f32; 2], from: usize, serve: bool) -> V3 {
    let s = sign(from);
    let mut speed = speed;
    let mut v = [0.0; 3];
    for _ in 0..6 {
        let mut x = target[0];
        let mut close = false;
        for _ in 0..2 {
            let d = normalize([x - p[0], 0.0, -s * target[1] - p[2]]);
            let fly = |vy: f32| landing(Ball { p, v: [d[0] * speed, vy, d[2] * speed], w }, from, serve);
            // Upward speeds that land somewhere come in one band (lower is into the net, or for
            // a serve past its own side's end; higher, for a serve, is over its own side): find
            // two either side of the target in it, then close in between.
            let tried: Vec<(f32, f32)> = (0..29).map(|i| -7.0 + i as f32 * 0.5).map(|vy| (vy, fly(vy).0)).collect();
            let off = |f: f32| if f < 0.0 { f32::MAX } else { (f - target[1]).abs() };
            let pair = tried.windows(2).find(|w| w[0].1 >= 0.0 && w[1].1 >= 0.0 && (w[0].1 - target[1]) * (w[1].1 - target[1]) <= 0.0);
            let vy = match pair {
                Some(w) => {
                    let (mut a, mut b) = (w[0].0, w[1].0);
                    let rising = w[1].1 > w[0].1;
                    for _ in 0..16 {
                        let mid = (a + b) / 2.0;
                        if (fly(mid).0 < target[1]) == rising {
                            a = mid;
                        } else {
                            b = mid;
                        }
                    }
                    (a + b) / 2.0
                }
                None => tried.iter().min_by(|x, y| off(x.1).total_cmp(&off(y.1))).unwrap().0,
            };
            v = [d[0] * speed, vy, d[2] * speed];
            let (depth, across) = fly(vy);
            close = (depth - target[1]).abs() < 0.12;
            x += target[0] - across; // sidespin curves it: aim off by however far it went
        }
        if close || target[1] > HALF_L || target[1] < 0.1 {
            break;
        }
        speed *= 0.85;
    }
    v
}

// ---- Paddles, shots and the computer ---------------------------------------------------------

#[derive(Clone, Copy, Debug)]
pub struct Paddle {
    pub p: V3,
    /// Its speed, smoothed over a few ticks (what a swing is judged by).
    pub v: V3,
}

/// A swing as it meets the ball.
#[derive(Clone, Copy, Debug)]
pub struct Shot {
    /// 0 a block, 1 a full drive; past that, harder and riskier.
    pub power: f32,
    /// Backspin, low and safe.
    pub chop: bool,
    /// Where across: -1 to 1 (the table's sides).
    pub aim: f32,
    /// The swing's sideways speed (m/s): sidespin.
    pub side: f32,
    /// How well it was met: 1 in the paddle's middle, 0 on its edge.
    pub quality: f32,
}

/// What the page plays sounds for and shows.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Event {
    Bounce { speed: f32 },
    Hit { who: usize, power: f32 },
    Net { speed: f32 },
    Floor { speed: f32 },
    Point { winner: usize },
    Let,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Phase {
    /// Before a match.
    Idle,
    /// The server has the ball.
    Ready { since: u32 },
    /// Tossed up, to be hit.
    Tossed,
    Rally,
    /// A point's been won; the next serve at `until`.
    Point { until: u32 },
    /// A game's been won; the next at `until`.
    Game { until: u32 },
    Over,
}

/// How good the computer is.
#[derive(Clone, Copy, Debug)]
pub struct Level {
    /// Paddle speed, m/s.
    pub speed: f32,
    /// Ticks before it reacts to your shot.
    pub react: u32,
    /// How far off (m) it judges where the ball's going.
    pub misjudge: f32,
    pub power: f32,
    /// How much its shots stray, 0 to 1.
    pub sloppy: f32,
}

pub const LEVELS: [Level; 3] = [
    Level { speed: 2.4, react: 18, misjudge: 0.06, power: 0.25, sloppy: 0.75 },
    Level { speed: 3.4, react: 12, misjudge: 0.035, power: 0.45, sloppy: 0.55 },
    Level { speed: 4.6, react: 7, misjudge: 0.015, power: 0.62, sloppy: 0.35 },
];

/// One tick's controls from you: where you want your paddle (x, z), the right button, a click.
#[derive(Clone, Copy, Debug, Default)]
pub struct Input {
    pub paddle: [f32; 2],
    pub chop: bool,
    pub toss: bool,
}

pub struct Rally {
    pub ball: Ball,
    pub paddles: [Paddle; 2],
    /// Played by the computer: the computer's side, and in tests yours too.
    pub level: [Option<Level>; 2],
    pub phase: Phase,
    pub score: [u32; 2],
    pub games: [u32; 2],
    /// Who served first this game.
    pub first: usize,
    /// Who last hit the ball, and its bounces on each side since.
    pub hitter: usize,
    pub bounces: [u8; 2],
    /// The ball's a serve not yet over the net, and whether it touched the net.
    pub serving: bool,
    pub netted: bool,
    /// When it was last hit (ticks).
    hit_at: u32,
    pub why: &'static str,
    pub tick: u32,
    pub events: Vec<Event>,
    /// Something the page shows has changed: the phase, the score.
    pub changed: bool,
    /// Where each computer player's going to meet the ball, and when it may start moving.
    plan: [Option<V3>; 2],
    wake: [u32; 2],
    seed: u32,
}

impl Rally {
    pub fn new(seed: u32) -> Rally {
        let rest = |side: usize| Paddle { p: [0.25 * sign(side), REST_Y, 1.7 * sign(side)], v: [0.0; 3] };
        Rally {
            ball: Ball { p: [0.0, TABLE_Y + BALL_R, 0.5], v: [0.0; 3], w: [0.0; 3] },
            paddles: [rest(YOU), rest(CPU)],
            level: [None, Some(LEVELS[1])],
            phase: Phase::Idle,
            score: [0, 0],
            games: [0, 0],
            first: YOU,
            hitter: YOU,
            bounces: [0, 0],
            serving: false,
            netted: false,
            hit_at: 0,
            why: "",
            tick: 0,
            events: Vec::new(),
            changed: true,
            plan: [None, None],
            wake: [0, 0],
            seed: seed | 1,
        }
    }

    fn random(&mut self) -> f32 {
        self.seed ^= self.seed << 13;
        self.seed ^= self.seed >> 17;
        self.seed ^= self.seed << 5;
        (self.seed as f64 / u32::MAX as f64) as f32
    }

    /// About normally spread, around 0, with a spread of 1.
    fn gauss(&mut self) -> f32 {
        (self.random() + self.random() + self.random() - 1.5) * 2.0
    }

    /// A new match against the computer at `level` (0 easy, 1 normal, 2 hard).
    pub fn start(&mut self, level: usize) {
        self.level[CPU] = Some(LEVELS[level.min(2)]);
        self.score = [0, 0];
        self.games = [0, 0];
        self.first = if self.random() < 0.5 { YOU } else { CPU };
        self.ready();
    }

    /// Who serves: two serves each, then one each from 10-all.
    pub fn server(&self) -> usize {
        let played = self.score[0] + self.score[1];
        let turn = if self.score[0] >= 10 && self.score[1] >= 10 { played } else { played / 2 };
        self.first ^ (turn as usize % 2)
    }

    fn ready(&mut self) {
        self.phase = Phase::Ready { since: self.tick };
        self.plan = [None, None];
        self.changed = true;
        self.hold();
    }

    /// The server holds the ball up, in front of their paddle.
    fn hold(&mut self) {
        let server = self.server();
        let paddle = self.paddles[server].p;
        let z = paddle[2].abs().max(HALF_L + 0.1) * sign(server);
        self.ball = Ball { p: [paddle[0], TABLE_Y + 0.22, z - sign(server) * 0.12], v: [0.0; 3], w: [0.0; 3] };
    }

    /// One tick: your controls, the computer's, then the ball in steps.
    pub fn tick(&mut self, input: &Input) {
        self.tick += 1;
        let before = [self.paddles[0].p, self.paddles[1].p];
        for side in [YOU, CPU] {
            let want = match self.level[side] {
                Some(level) => self.think(side, level),
                None => [input.paddle[0], input.paddle[1]],
            };
            self.reach(side, want);
        }
        for side in [YOU, CPU] {
            let v = mul(sub(self.paddles[side].p, before[side]), 1.0 / TICK);
            self.paddles[side].v = lerp(self.paddles[side].v, v, 0.5);
        }

        match self.phase {
            Phase::Idle | Phase::Over => {}
            Phase::Ready { since } => {
                let server = self.server();
                self.hold();
                let toss = match self.level[server] {
                    Some(_) => self.tick - since > 50,
                    None => input.toss,
                };
                if toss {
                    self.ball.v = [0.0, 2.4, 0.0];
                    self.phase = Phase::Tossed;
                    self.changed = true;
                }
            }
            Phase::Point { until } | Phase::Game { until } if self.tick >= until => {
                if let Phase::Game { .. } = self.phase {
                    self.score = [0, 0];
                    self.first ^= 1;
                }
                self.ready();
            }
            _ => {}
        }
        // The computer serves as the ball comes down.
        if self.phase == Phase::Tossed {
            let server = self.server();
            if let Some(level) = self.level[server] {
                if self.ball.v[1] < 0.0 && self.ball.p[1] < TABLE_Y + 0.32 {
                    let shot = self.computer_shot(server, level, 1.0);
                    self.strike(server, shot, true);
                }
            }
        }

        // A ball that's stopped somewhere (on the table, say) is out of play.
        if self.phase == Phase::Rally && self.tick - self.hit_at > 240 {
            self.ended();
        }
        let after = [self.paddles[0].p, self.paddles[1].p];
        if matches!(self.phase, Phase::Ready { .. } | Phase::Idle | Phase::Over) {
            return;
        }
        for i in 0..STEPS {
            let t = (i + 1) as f32 / STEPS as f32;
            let at = [lerp(before[0], after[0], t), lerp(before[1], after[1], t)];
            let was = self.ball.p;
            let touch = self.ball.step(false);
            self.touched(touch);
            self.meet(was, at, input);
            // Dropped it (onto the table, the floor, anywhere): toss again.
            if self.phase == Phase::Tossed && (touch.is_some() || self.ball.p[1] < TABLE_Y - 0.05) {
                self.ready();
                return;
            }
        }
    }

    /// Moves `side`'s paddle toward `want` (x, z): yours as fast as the mouse, the computer's
    /// at its speed. Its height follows the ball's on the way in.
    fn reach(&mut self, side: usize, want: [f32; 2]) {
        let s = sign(side);
        let behind = matches!(self.phase, Phase::Ready { .. } | Phase::Tossed) && self.server() == side;
        let near = if behind { HALF_L + 0.1 } else { YOUR_Z[0] };
        let want = [want[0].clamp(-YOUR_X, YOUR_X), (want[1] * s).clamp(near, YOUR_Z[1]) * s];
        let p = &mut self.paddles[side].p;
        let d = [want[0] - p[0], want[1] - p[2]];
        let far = (d[0] * d[0] + d[1] * d[1]).sqrt();
        let step = match self.level[side] {
            Some(level) => (level.speed * TICK / far.max(1e-6)).min(1.0),
            None => 1.0,
        };
        p[0] += d[0] * step;
        p[2] += d[1] * step;
        let y = match self.phase {
            Phase::Ready { .. } | Phase::Tossed if self.server() == side => self.ball.p[1],
            _ => self.coming(side).map_or(REST_Y, |at| at[1]),
        };
        let p = &mut self.paddles[side].p;
        p[1] += (y - p[1]).clamp(-6.0 * TICK, 6.0 * TICK);
    }

    /// Whether `side` may hit the ball now: it's bounced on their side, coming their way.
    fn hittable(&self, side: usize) -> bool {
        match self.phase {
            Phase::Tossed => self.server() == side && self.level[side].is_none(),
            Phase::Rally => self.hitter != side && self.bounces[side] >= 1,
            _ => false,
        }
    }

    /// Where the ball will meet `side`'s paddle (where it crosses its z), if it's theirs to hit.
    fn coming(&self, side: usize) -> Option<V3> {
        if self.phase != Phase::Rally || self.hitter == side {
            return None;
        }
        let s = sign(side);
        let z = self.paddles[side].p[2];
        let mut b = self.ball;
        let mut bounced = self.bounces[side] >= 1;
        for _ in 0..(1.5 / DT) as usize {
            match b.step(false) {
                Some(Touch::Table(p)) if p[2] * s > 0.0 => bounced = true,
                Some(_) => return None,
                None => {}
            }
            if bounced && (b.p[2] - z) * s >= -BALL_R {
                return Some(b.p);
            }
        }
        None
    }

    /// The ball meets a paddle, if it's there: `at` is where both paddles are this step.
    fn meet(&mut self, was: V3, at: [V3; 2], input: &Input) {
        for side in [YOU, CPU] {
            if !self.hittable(side) {
                continue;
            }
            let s = sign(side);
            let rel_was = (was[2] - at[side][2]) * s;
            let rel = (self.ball.p[2] - at[side][2]) * s;
            let off = ((self.ball.p[0] - at[side][0]).powi(2) + (self.ball.p[1] - at[side][1]).powi(2)).sqrt();
            if rel >= -BALL_R && rel_was < 0.05 && rel > rel_was - 1e-5 && off < REACH {
                let quality = 1.0 - (off / REACH).powi(2);
                let serve = self.phase == Phase::Tossed;
                let shot = match self.level[side] {
                    Some(level) => self.computer_shot(side, level, quality),
                    None => {
                        let v = self.paddles[side].v;
                        Shot {
                            power: ((-v[2] * s - 0.4) / 3.6).clamp(0.0, 1.3),
                            chop: input.chop,
                            aim: (v[0] / 2.5).clamp(-1.0, 1.0),
                            side: v[0],
                            quality,
                        }
                    }
                };
                self.strike(side, shot, serve);
                return;
            }
        }
    }

    /// `who` hits the ball with `shot`.
    pub fn strike(&mut self, who: usize, shot: Shot, serve: bool) {
        let s = sign(who);
        let p = shot.power.clamp(0.0, 1.3);
        let full = p.min(1.0);
        let (speed, depth, spin) = match (serve, shot.chop) {
            (true, true) => (mix(3.4, 5.4, full), mix(0.75, 1.15, full), -45.0),
            (true, false) => (mix(3.6, 6.4, full), mix(0.75, 1.15, full), 20.0),
            (false, true) => (mix(4.0, 7.5, p), mix(0.85, 1.15, full), -55.0 - 25.0 * full),
            (false, false) => (mix(4.8, 15.5, p), mix(0.7, 1.2, full), 25.0 + 95.0 * p),
        };
        // Topspin turns the ball over toward where it's going: about -x going to -z.
        let w = [-s * spin, -s * shot.side.clamp(-3.0, 3.0) * 15.0, 0.0];
        let q = shot.quality.clamp(0.0, 1.0);
        let mut miss_long = 0.07 + 0.14 * p + 0.8 * (1.0 - q).powi(2) + 0.6 * (p - 0.9).max(0.0);
        let low = TABLE_Y + 0.05 - self.ball.p[1];
        if low > 0.0 && !serve {
            miss_long += low * 1.2; // dug out from under the table's height
        }
        let miss_wide = 0.05 + 0.1 * p + 0.35 * (1.0 - q);
        let target = [
            shot.aim.clamp(-1.0, 1.0) * 0.62 + self.gauss() * miss_wide,
            depth + self.gauss() * miss_long,
        ];
        let mut v = aim(self.ball.p, w, speed, target, who, serve);
        if target[1] < 0.12 {
            v[1] -= 0.8; // a mistake into the net
        }
        self.ball.v = v;
        self.ball.w = w;
        self.hitter = who;
        self.bounces = [0, 0];
        self.serving = serve;
        self.netted = false;
        self.hit_at = self.tick;
        self.phase = Phase::Rally;
        self.plan = [None, None];
        self.wake[1 - who] = self.tick + self.level[1 - who].map_or(0, |l| l.react);
        self.events.push(Event::Hit { who, power: p });
        self.changed |= serve;
    }

    /// The computer's shot, meeting the ball with `quality`.
    fn computer_shot(&mut self, side: usize, level: Level, quality: f32) -> Shot {
        let high = self.ball.p[1] > TABLE_Y + 0.32;
        let low = self.ball.p[1] < TABLE_Y + 0.1;
        let mut power = level.power + (self.random() - 0.5) * 0.3;
        if high {
            power += 0.45 * level.power / 0.62; // put it away
        }
        if self.phase == Phase::Tossed {
            power = self.random() * 0.7;
        }
        // Away from where you are, mostly.
        let you = self.paddles[1 - side].p[0];
        let away = if you > 0.0 { -1.0 } else { 1.0 };
        let aim = if self.random() < 0.65 { away * (0.3 + 0.55 * self.random()) } else { self.random() * 2.0 - 1.0 };
        Shot {
            power: power.max(0.0),
            chop: low && self.random() < 0.4,
            aim,
            side: (self.random() - 0.5) * 1.5,
            quality: quality * (1.0 - level.sloppy * self.random()),
        }
    }

    /// Where the computer playing `side` moves its paddle: to meet the ball where it's going
    /// (as it judges it, once it's reacted), else back to the middle.
    fn think(&mut self, side: usize, level: Level) -> [f32; 2] {
        let s = sign(side);
        let home = [0.0, 1.75 * s];
        if matches!(self.phase, Phase::Ready { .. } | Phase::Tossed) {
            return if self.server() == side { [0.2 * s, 1.55 * s] } else { home };
        }
        if self.phase != Phase::Rally || self.hitter == side || self.tick < self.wake[side] {
            return match self.plan[side] {
                Some(p) if self.hitter != side => [p[0], p[2]],
                _ => home,
            };
        }
        if self.plan[side].is_none() {
            let mut b = self.ball;
            let mut bounced = self.bounces[side] >= 1;
            let mut best: Option<V3> = None;
            for _ in 0..(1.6 / DT) as usize {
                match b.step(false) {
                    Some(Touch::Table(p)) if p[2] * s > 0.0 => bounced = true,
                    Some(_) => break,
                    None => {}
                }
                // Meet it on the way down from its bounce, or as it passes 1.6 m back.
                if bounced && b.p[2] * s > HALF_L - 0.25 && (b.v[1] < 0.0 || b.p[2] * s > 1.6) {
                    best = Some(b.p);
                    break;
                }
            }
            let miss = self.gauss() * level.misjudge;
            self.plan[side] = Some(best.map_or([0.0, REST_Y, 1.75 * s], |p| [p[0] + miss, p[1], p[2] + 0.02 * s]));
        }
        let p = self.plan[side].unwrap();
        [p[0], p[2]]
    }

    /// The rules, as the ball touches something.
    fn touched(&mut self, touch: Option<Touch>) {
        let speed = len(self.ball.v);
        match touch {
            // (Not the tiny bounces of a ball settling on the table: those would tick away.)
            Some(Touch::Table(_)) if self.ball.v[1] > 0.25 => self.events.push(Event::Bounce { speed }),
            Some(Touch::Net) => self.events.push(Event::Net { speed }),
            Some(Touch::Floor) if self.ball.v[1] > 0.25 => self.events.push(Event::Floor { speed }),
            _ => {}
        }
        if self.phase != Phase::Rally {
            return;
        }
        let (hitter, other) = (self.hitter, 1 - self.hitter);
        let far = self.ball.p[0].abs() > 4.0 || self.ball.p[2].abs() > 6.0;
        match touch {
            Some(Touch::Net) => self.netted = true,
            Some(Touch::Table(p)) => {
                let side = if p[2] > 0.0 { YOU } else { CPU };
                if self.serving {
                    if side == hitter && self.bounces[hitter] == 0 {
                        self.bounces[hitter] = 1;
                    } else if side == hitter || self.bounces[hitter] == 0 {
                        self.point(other, "Fault");
                    } else if self.netted {
                        self.why = "Let";
                        self.events.push(Event::Let);
                        self.ready();
                    } else {
                        self.serving = false;
                        self.bounces = [0, 0];
                        self.bounces[side] = 1;
                    }
                } else if side == hitter {
                    self.point(other, if self.netted { "Net" } else { "Fault" });
                } else if self.bounces[side] == 0 {
                    self.bounces[side] = 1;
                } else {
                    self.point(hitter, "Missed");
                }
            }
            Some(Touch::Floor) => self.ended(),
            None if far => self.ended(),
            None => {}
        }
    }

    /// The ball's off the table: whoever let it get there loses the point.
    fn ended(&mut self) {
        let (hitter, other) = (self.hitter, 1 - self.hitter);
        if !self.serving && self.bounces[other] >= 1 {
            self.point(hitter, "Missed");
        } else if self.ball.p[2] * sign(hitter) > 0.0 {
            self.point(other, "Net");
        } else {
            self.point(other, if self.serving { "Fault" } else { "Out" });
        }
    }

    fn point(&mut self, winner: usize, why: &'static str) {
        self.score[winner] += 1;
        self.why = why;
        self.events.push(Event::Point { winner });
        self.changed = true;
        let (a, b) = (self.score[winner], self.score[1 - winner]);
        if a >= 11 && a >= b + 2 {
            self.games[winner] += 1;
            self.phase = if self.games[winner] >= 2 { Phase::Over } else { Phase::Game { until: self.tick + 150 } };
        } else {
            self.phase = Phase::Point { until: self.tick + 80 };
        }
    }

    /// Who won the match, once it's over.
    pub fn winner(&self) -> Option<usize> {
        (self.phase == Phase::Over).then(|| if self.games[YOU] > self.games[CPU] { YOU } else { CPU })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fly(b: &mut Ball, seconds: f32) -> Vec<Touch> {
        (0..(seconds / DT) as usize).filter_map(|_| b.step(false)).collect()
    }

    #[test]
    fn a_dropped_ball_bounces_back_most_of_the_way() {
        let mut b = Ball { p: [0.0, TABLE_Y + 0.3, 0.5], v: [0.0; 3], w: [0.0; 3] };
        let mut top: f32 = 0.0;
        let mut bounced = false;
        for _ in 0..(1.0 / DT) as usize {
            if let Some(Touch::Table(_)) = b.step(false) {
                bounced = true;
            }
            if bounced {
                top = top.max(b.p[1] - TABLE_Y - BALL_R);
            }
        }
        assert!(bounced);
        assert!((0.2..0.26).contains(&top), "came back up {top} of 0.3");
    }

    #[test]
    fn topspin_kicks_on_off_the_table_and_backspin_holds_back() {
        let after = |spin: f32| {
            let mut b = Ball { p: [0.0, TABLE_Y + 0.1, 1.0], v: [0.0, -2.0, -6.0], w: [-spin, 0.0, 0.0] };
            let touches = fly(&mut b, 0.06);
            assert!(matches!(touches.first(), Some(Touch::Table(_))));
            -b.v[2]
        };
        assert!(after(120.0) > after(0.0) + 0.3);
        assert!(after(-120.0) < after(0.0) - 0.3);
    }

    #[test]
    fn a_ball_low_into_the_net_drops_back() {
        let mut b = Ball { p: [0.0, NET_TOP - 0.05, 0.5], v: [0.0, 0.0, -8.0], w: [0.0; 3] };
        let touches = fly(&mut b, 1.0);
        assert_eq!(touches.first(), Some(&Touch::Net));
        assert!(b.p[2] > 0.0, "it stays on its own side");
    }

    #[test]
    fn shots_come_down_where_theyre_aimed() {
        for (from, speed, spin, target) in [
            (YOU, 8.0, 60.0, [0.3, 1.0]),
            (YOU, 14.0, 110.0, [-0.5, 1.1]),
            (YOU, 5.0, -60.0, [0.0, 0.9]),
            (CPU, 9.0, 40.0, [0.4, 1.0]),
        ] {
            let s = sign(from);
            let p = [0.0, TABLE_Y + 0.25, 1.7 * s];
            let w = [-s * spin, 0.0, 0.0];
            let mut b = Ball { p, v: aim(p, w, speed, target, from, false), w };
            let first = fly(&mut b, 2.0).into_iter().find_map(|t| match t {
                Touch::Table(p) => Some(p),
                _ => None,
            });
            let at = first.expect("it lands");
            assert!((-at[2] * s - target[1]).abs() < 0.15 && (at[0] - target[0]).abs() < 0.1, "{at:?} for {target:?}");
        }
    }

    #[test]
    fn a_serve_bounces_on_both_sides() {
        let p = [0.1, TABLE_Y + 0.15, 1.5];
        let w = [-20.0, 0.0, 0.0];
        let mut b = Ball { p, v: aim(p, w, 5.0, [0.2, 1.0], YOU, true), w };
        let sides: Vec<bool> = fly(&mut b, 2.0)
            .into_iter()
            .filter_map(|t| match t {
                Touch::Table(p) => Some(p[2] > 0.0),
                _ => None,
            })
            .take(2)
            .collect();
        assert_eq!(sides, [true, false]);
    }

    #[test]
    fn a_dropped_toss_that_lands_on_the_table_is_tossed_again() {
        let mut r = Rally::new(1);
        r.start(1);
        r.first = YOU;
        r.ready();
        // Your paddle as close to the table as it goes, so the ball's held over its end.
        let mut input = Input { paddle: [0.0, 0.0], chop: false, toss: false };
        for _ in 0..30 {
            r.tick(&input);
        }
        input.toss = true;
        r.tick(&input);
        assert_eq!(r.phase, Phase::Tossed);
        input.toss = false;
        let mut back = false;
        for _ in 0..120 {
            r.tick(&input);
            back |= matches!(r.phase, Phase::Ready { .. });
        }
        assert!(back, "still {:?} with the ball at {:?}", r.phase, r.ball.p);
        assert_eq!(r.score, [0, 0]);
    }

    #[test]
    fn games_go_to_eleven_by_two_and_the_serve_goes_round() {
        let mut r = Rally::new(1);
        r.start(1);
        r.first = YOU;
        let mut servers = Vec::new();
        for _ in 0..4 {
            servers.push(r.server());
            r.point(YOU, "Out");
            r.score[CPU] += 1; // even
        }
        assert_eq!(servers, [YOU, CPU, YOU, CPU], "two each");
        r.score = [10, 10];
        let deuce = [r.server(), {
            r.score[YOU] += 1;
            r.server()
        }];
        assert_ne!(deuce[0], deuce[1], "one each from 10-all");
        r.score = [11, 10];
        r.point(YOU, "Out");
        assert_eq!((r.games, r.score[YOU]), ([1, 0], 12));
        assert!(matches!(r.phase, Phase::Game { .. }));
    }

    fn play_until_point(r: &mut Rally) -> usize {
        let before = r.score;
        for _ in 0..60 * 30 {
            r.tick(&Input::default());
            if r.score != before {
                return if r.score[YOU] > before[YOU] { YOU } else { CPU };
            }
        }
        panic!("no point in 30 s: {:?}", r.phase);
    }

    #[test]
    fn a_ball_into_the_net_loses_the_point_and_a_missed_ball_does_too() {
        let mut r = Rally::new(7);
        r.level = [Some(LEVELS[2]), Some(LEVELS[2])];
        r.start(2);
        r.first = YOU;
        r.ready();
        // Hit into the net.
        r.ball = Ball { p: [0.0, NET_TOP - 0.03, 0.4], v: [0.0; 3], w: [0.0; 3] };
        r.phase = Phase::Rally;
        r.hitter = YOU;
        r.serving = false;
        r.bounces = [0, 0];
        r.ball.v = [0.0, 0.0, -6.0];
        r.level = [None, None];
        let won = play_until_point(&mut r);
        assert_eq!((won, r.why), (CPU, "Net"));

        // A good ball nobody reaches.
        r.ready();
        r.ball = Ball { p: [0.0, TABLE_Y + 0.3, 1.6], v: [0.0; 3], w: [0.0; 3] };
        r.ball.v = aim(r.ball.p, [0.0; 3], 7.0, [0.5, 1.0], YOU, false);
        r.phase = Phase::Rally;
        r.hitter = YOU;
        r.paddles[CPU].p = [-1.0, REST_Y, -2.4];
        let won = play_until_point(&mut r);
        assert_eq!((won, r.why), (YOU, "Missed"));
    }

    #[test]
    fn the_computer_plays_rallies_and_finishes_a_match() {
        for level in 0..3 {
            let mut r = Rally::new(11 + level as u32);
            r.level = [Some(LEVELS[level]), Some(LEVELS[level])];
            r.start(level);
            let mut hits = 0;
            let mut points = 0;
            let mut why = std::collections::BTreeMap::new();
            for _ in 0..60 * 60 * 30 {
                r.tick(&Input::default());
                for e in r.events.drain(..) {
                    match e {
                        Event::Hit { .. } => hits += 1,
                        Event::Point { .. } => {
                            points += 1;
                            *why.entry(r.why).or_insert(0) += 1;
                        }
                        _ => {}
                    }
                }
                if r.phase == Phase::Over {
                    break;
                }
            }
            assert_eq!(r.phase, Phase::Over, "level {level}: {:?} {:?}", r.games, r.score);
            let per_point = hits as f32 / points as f32;
            println!("level {level}: {points} points, {per_point:.1} hits a point, {why:?}");
            assert!(per_point > 2.0 + level as f32, "level {level}: {per_point:.1} hits a point");
        }
    }

    /// Plays your side as a person would with the mouse: gets in line with the ball, waits back,
    /// and pushes forward through it as it comes; tosses and swings to serve.
    #[test]
    fn swinging_through_the_ball_returns_it() {
        let mut r = Rally::new(5);
        r.start(1);
        let mut want = [0.0f32, 2.0f32];
        let (mut returned, mut chances, mut serves, mut good_serves) = (0, 0, 0, 0);
        for _ in 0..60 * 60 * 4 {
            let mut input = Input { paddle: want, chop: false, toss: false };
            match r.phase {
                Phase::Ready { since } if r.server() == YOU && r.tick - since > 20 => {
                    want = [0.0, 1.8];
                    input.toss = true;
                }
                Phase::Tossed if r.server() == YOU => {
                    // Swing as it drops back to the paddle.
                    if r.ball.v[1] < 0.0 && r.ball.p[1] < TABLE_Y + 0.25 {
                        want[1] -= 3.0 * TICK;
                    }
                }
                Phase::Rally if r.hitter == CPU => {
                    if let Some(at) = r.coming(YOU) {
                        want[0] = at[0];
                        let gap = at[2] - r.ball.p[2];
                        let closing = r.ball.v[2].max(0.5);
                        if gap / closing < 0.12 {
                            want[1] -= 3.0 * TICK; // swing
                        } else {
                            want[1] = (want[1] + 2.0 * TICK).min(1.85); // wait back
                        }
                    }
                }
                _ => want[1] = (want[1] + 2.0 * TICK).min(1.85),
            }
            input.paddle = want;
            let before = (r.phase, r.hitter, r.bounces, r.serving);
            r.tick(&input);
            for e in r.events.drain(..) {
                if let Event::Hit { who: YOU, .. } = e {
                    if before.0 == Phase::Tossed {
                        serves += 1;
                    } else {
                        returned += 1;
                    }
                }
            }
            // A serve of yours that's landed on their side.
            if before.3 && r.hitter == YOU && !r.serving && r.phase == Phase::Rally {
                good_serves += 1;
            }
            // A ball of theirs that bounced on your side: a chance to return it.
            if before.1 == CPU && before.2[YOU] == 0 && r.hitter == CPU && r.bounces[YOU] == 1 {
                chances += 1;
            }
            if r.phase == Phase::Over {
                r.start(1);
            }
        }
        println!("returned {returned} of {chances}; {good_serves} of {serves} serves in");
        assert!(chances > 30 && returned as f32 > chances as f32 * 0.8, "returned {returned} of {chances}");
        assert!(serves > 10 && good_serves as f32 > serves as f32 * 0.7, "{good_serves} of {serves} serves in");
    }
}
