//! Outpost's game server. Pages send their inputs (keys and aim, a tick at a time); this runs
//! everyone's through the rules (../src/sim.rs) 60 times a second and sends everyone where
//! everyone is 30 times a second. It decides every move and every hit: a page only predicts.
//!
//! Lag compensation: a page shows everyone else a little in the past, and says which server
//! tick it was showing when it fired. The shot's checked against where people were then (up to
//! 0.3 s back), so what you hit on your screen is what you hit.
//!
//! Computer players (bots.rs) fill each team up to OUTPOST_BOTS players (2; 0 for none),
//! making way as people join.
//!
//! Usage: outpost-server [map.json] (default ../../resources/outpost/map.json); OUTPOST_PORT
//! sets the port (8082).

#[path = "../../src/sim.rs"]
#[allow(dead_code)]
mod sim;
mod bots;

use futures_util::{SinkExt, StreamExt};
use sim::*;
use std::collections::{HashMap, VecDeque};
use std::time::Duration;
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::mpsc::{unbounded_channel, UnboundedSender};
use tokio_tungstenite::tungstenite::Message;

/// Snapshots go out every this many ticks: 30 a second.
const SNAPSHOT_EVERY: u32 = 2;
/// How far back a shot may be checked, in ticks (0.3 s).
const MAX_REWIND: f32 = 18.0;
/// Ticks of everyone's positions kept, for rewinding.
const HISTORY: usize = 64;
/// Inputs a player may have waiting; more are dropped (no playing faster than everyone else).
const MAX_QUEUE: usize = 30;
/// Inputs a player's run through per tick: one, and one more to catch up after a stall.
const PER_TICK: usize = 2;
/// Bots' ids start here, clear of people's.
const FIRST_BOT: u32 = 1_000_000;
const BOT_NAMES: [&str; 12] = ["Ada", "Bram", "Cole", "Dax", "Edda", "Finn", "Gale", "Hale", "Iris", "Jett", "Kai", "Lux"];

/// Where someone was at a tick: feet, alive, how crouched.
type Pose = (V3, bool, f32);

enum From {
    Open(UnboundedSender<String>),
    Message(ToServer),
    Closed,
}

struct Client {
    out: UnboundedSender<String>,
    playing: Option<Playing>,
    /// A bot's: what it's thinking (and nothing's sent to it).
    brain: Option<bots::Brain>,
}

struct Playing {
    name: String,
    state: Player,
    queue: VecDeque<Input>,
    /// The last input queued, and the last one run.
    queued: u32,
    ack: u32,
    /// The server tick their page shows everyone else at.
    view: f32,
    kills: u32,
    deaths: u32,
    /// Ticks until they're back in, while dead.
    dead: u32,
}

struct Server {
    map: Map,
    /// Each team's spawns: feet, yaw.
    spawns: [Vec<(V3, f32)>; 2],
    clients: HashMap<u32, Client>,
    tick: u32,
    /// Everyone's feet, whether alive and how crouched, by tick.
    history: VecDeque<(u32, HashMap<u32, Pose>)>,
    events: Vec<Event>,
    roster_changed: bool,
    spawn_turn: usize,
    /// Where bots can walk; how many players each team's filled up to; the next bot's id.
    nav: bots::Nav,
    team_size: usize,
    next_bot: u32,
    /// The bots' level, for the whole match: an index into `bots::SKILLS`. Anyone can change it.
    level: u8,
}

impl Server {
    fn new(map: Map, spawns: [Vec<(V3, f32)>; 2], team_size: usize) -> Server {
        let nav = bots::Nav::new(&map, spawns[0][0].0);
        let mut server = Server {
            nav,
            team_size,
            next_bot: FIRST_BOT,
            level: 1,
            map,
            spawns,
            clients: HashMap::new(),
            tick: 0,
            history: VecDeque::new(),
            events: Vec::new(),
            roster_changed: false,
            spawn_turn: 0,
        };
        server.balance();
        server
    }

    /// People and bots on each team.
    fn counts(&self) -> [[usize; 2]; 2] {
        let mut counts = [[0; 2]; 2];
        for c in self.clients.values() {
            if let Some(p) = &c.playing {
                counts[p.state.team as usize][c.brain.is_some() as usize] += 1;
            }
        }
        counts
    }

    /// Bots in or out, so each team has `team_size` players (people count first).
    fn balance(&mut self) {
        for team in 0..2u8 {
            loop {
                let [people, bots] = self.counts()[team as usize];
                if people + bots < self.team_size {
                    let id = self.next_bot;
                    self.next_bot += 1;
                    let (out, _) = unbounded_channel();
                    self.clients.insert(id, Client { out, playing: None, brain: Some(bots::Brain::new(id)) });
                    let name = format!("BOT {}", BOT_NAMES[(id - FIRST_BOT) as usize % BOT_NAMES.len()]);
                    self.add_player(id, name, team);
                } else if bots > 0 && people + bots > self.team_size {
                    let bot = self.clients.iter().find(|(_, c)| c.brain.is_some() && c.playing.as_ref().is_some_and(|p| p.state.team == team));
                    let Some((&id, _)) = bot else { break };
                    self.clients.remove(&id);
                    self.roster_changed = true;
                } else {
                    break;
                }
            }
        }
    }

    /// Puts client `id` in the game on `team`, at a spawn.
    fn add_player(&mut self, id: u32, name: String, team: u8) {
        let mut state = Player::new(id, team);
        let (at, yaw) = self.spawn_point(team, id);
        state.spawn(at, yaw);
        let view = self.tick as f32;
        let Some(client) = self.clients.get_mut(&id) else { return };
        if client.brain.is_some() {
            state.shield = 0; // the look-around after spawning is for people
        }
        client.playing = Some(Playing { name, state, queue: VecDeque::new(), queued: 0, ack: 0, view, kills: 0, deaths: 0, dead: 0 });
        send(&client.out, &ToClient::Welcome { id, team });
        self.roster_changed = true;
    }

    fn playing(&mut self, id: u32) -> Option<&mut Playing> {
        self.clients.get_mut(&id).and_then(|c| c.playing.as_mut())
    }

    /// Ids of everyone in the game, in order.
    fn players(&self) -> Vec<u32> {
        let mut ids: Vec<u32> = self.clients.iter().filter(|(_, c)| c.playing.is_some()).map(|(&id, _)| id).collect();
        ids.sort();
        ids
    }

    /// Where everyone but `id` who's out (alive, done looking around) is standing.
    fn others(&self, id: u32) -> Vec<V3> {
        self.clients
            .iter()
            .filter(|&(&other, _)| other != id)
            .filter_map(|(_, c)| c.playing.as_ref())
            .filter(|p| p.state.exposed())
            .map(|p| p.state.pos)
            .collect()
    }

    fn receive(&mut self, id: u32, message: From) {
        match message {
            From::Open(out) => {
                send(&out, &ToClient::Level { level: self.level });
                self.clients.insert(id, Client { out, playing: None, brain: None });
            }
            From::Message(ToServer::Level { level }) => {
                if (level as usize) < bots::SKILLS.len() && level != self.level {
                    self.level = level;
                    for client in self.clients.values().filter(|c| c.brain.is_none()) {
                        send(&client.out, &ToClient::Level { level });
                    }
                }
            }
            From::Closed => {
                if self.clients.remove(&id).is_some_and(|c| c.playing.is_some()) {
                    self.roster_changed = true;
                    self.balance();
                }
            }
            From::Message(ToServer::Join { name }) => {
                if self.clients.get(&id).is_none_or(|c| c.playing.is_some()) {
                    return;
                }
                let name: String = name.trim().chars().take(16).collect();
                let name = if name.is_empty() { format!("Player {id}") } else { name };
                // The side with fewer people (then fewer players); a bot there makes way.
                let [a, b] = self.counts();
                let team = if (a[0], a[0] + a[1]) <= (b[0], b[0] + b[1]) { 0 } else { 1 };
                self.add_player(id, name, team);
                self.balance();
            }
            From::Message(ToServer::Input { view, inputs }) => {
                if let Some(p) = self.playing(id) {
                    for input in inputs {
                        if input.seq > p.queued && p.queue.len() < MAX_QUEUE {
                            p.queued = input.seq;
                            p.queue.push_back(input);
                        }
                    }
                    p.view = view;
                }
            }
        }
    }

    /// One of the team's spawns, taking turns, skipping any someone's standing on.
    fn spawn_point(&mut self, team: u8, id: u32) -> (V3, f32) {
        let others = self.others(id);
        let spawns = &self.spawns[team as usize];
        for i in 0..spawns.len() {
            let (at, yaw) = spawns[(self.spawn_turn + i) % spawns.len()];
            if others.iter().all(|o| dot(sub(*o, at), sub(*o, at)) > 1.0) {
                self.spawn_turn += i + 1;
                return (at, yaw);
            }
        }
        self.spawn_turn += 1;
        spawns[self.spawn_turn % spawns.len()]
    }

    fn step(&mut self) {
        self.tick += 1;
        self.think();
        for id in self.players() {
            let p = self.playing(id).unwrap();
            if !p.state.alive {
                p.dead = p.dead.saturating_sub(1);
                if p.dead == 0 {
                    let team = p.state.team;
                    let (at, yaw) = self.spawn_point(team, id);
                    let bot = self.clients.get(&id).is_some_and(|c| c.brain.is_some());
                    let state = &mut self.playing(id).unwrap().state;
                    state.spawn(at, yaw);
                    if bot {
                        state.shield = 0; // the look-around after spawning is for people
                    }
                    self.roster_changed = true;
                }
            }
            for _ in 0..PER_TICK {
                let others = self.others(id);
                let map = &self.map;
                let Some(p) = self.clients.get_mut(&id).and_then(|c| c.playing.as_mut()) else { break };
                let Some(input) = p.queue.pop_front() else { break };
                p.ack = input.seq;
                if let Some(shot) = p.state.step(&input, map, &others) {
                    let view = p.view;
                    self.shoot(id, shot, view);
                }
            }
        }

        let now: HashMap<u32, Pose> = self
            .clients
            .iter()
            .filter_map(|(&id, c)| c.playing.as_ref().map(|p| (id, (p.state.pos, p.state.exposed(), p.state.crouch))))
            .collect();
        self.history.push_back((self.tick, now));
        if self.history.len() > HISTORY {
            self.history.pop_front();
        }

        if self.tick % SNAPSHOT_EVERY == 0 {
            self.snapshot();
        }
        if self.roster_changed {
            self.roster_changed = false;
            self.roster();
        }
    }

    /// Each bot makes up this tick's input, from where everyone is now.
    fn think(&mut self) {
        let others: Vec<bots::Other> = self
            .clients
            .iter()
            .filter_map(|(&id, c)| c.playing.as_ref().map(|p| (id, p)))
            .map(|(id, p)| bots::Other { id, pos: p.state.pos, crouch: p.state.crouch, team: p.state.team, alive: p.state.exposed() })
            .collect();
        let skill = bots::SKILLS[self.level as usize];
        let Server { clients, map, nav, tick, .. } = self;
        for (&id, client) in clients.iter_mut() {
            let (Some(brain), Some(p)) = (client.brain.as_mut(), client.playing.as_mut()) else { continue };
            let mut input = brain.think(id, &p.state, &others, map, nav, *tick, skill);
            p.queued += 1;
            input.seq = p.queued;
            p.queue.push_back(input);
            p.view = *tick as f32;
        }
    }

    /// Where everyone was at tick `at` (fractional: between two ticks), as far as it's kept.
    fn rewound(&self, at: f32) -> HashMap<u32, Pose> {
        let Some((_, latest)) = self.history.back() else { return HashMap::new() };
        let before = self.history.iter().rev().find(|(t, _)| *t as f32 <= at);
        let after = self.history.iter().find(|(t, _)| *t as f32 >= at);
        match (before, after) {
            (Some((t0, a)), Some((t1, b))) if t1 > t0 => {
                let k = (at - *t0 as f32) / (*t1 - *t0) as f32;
                let between = |&(pb, alive, cb): &Pose, &(pa, alive_a, ca): &Pose| (lerp(pa, pb, k), alive_a && alive, ca + (cb - ca) * k);
                b.iter().map(|(id, now)| (*id, a.get(id).map_or(*now, |then| between(now, then)))).collect()
            }
            (Some((_, a)), _) => a.clone(),
            (None, Some((_, b))) => b.clone(),
            _ => latest.clone(),
        }
    }

    /// Player `id` fired, seeing everyone else as they were at tick `view`.
    fn shoot(&mut self, id: u32, shot: Shot, view: f32) {
        let now = self.tick as f32;
        let seen = self.rewound(view.clamp(now - MAX_REWIND, now));
        let team = self.playing(id).map_or(0, |p| p.state.team);
        let mut targets: Vec<(u32, (V3, f32))> = Vec::new();
        for (&other, &(at, was_alive, crouch)) in &seen {
            let Some(p) = self.clients.get(&other).and_then(|c| c.playing.as_ref()) else { continue };
            // Not yourself, not your own side, and not someone who's died (or respawned) since.
            if other != id && was_alive && p.state.exposed() && p.state.team != team {
                targets.push((other, (at, crouch)));
            }
        }
        let positions: Vec<(V3, f32)> = targets.iter().map(|t| t.1).collect();
        let (t, _, hit) = trace(&self.map, shot.origin, shot.dir, &positions);
        let gun = shot.gun.min(GUNS.len() as u8 - 1);
        self.events.push(Event::Shot { id, from: shot.origin, to: add(shot.origin, mul(shot.dir, t)), gun });
        if let Some((i, part)) = hit {
            self.damage(id, targets[i].0, GUNS[gun as usize].damage[part as usize], part == 1, gun);
        }
    }

    fn damage(&mut self, by: u32, victim: u32, amount: i32, head: bool, gun: u8) {
        let from = self.playing(by).map(|p| p.state.pos);
        // Bots hit people softer (each other at full strength, so their fights still end).
        let bot = |id: u32| self.clients.get(&id).is_some_and(|c| c.brain.is_some());
        let share = bots::SKILLS[self.level as usize].damage;
        let amount = if bot(by) && !bot(victim) { (amount as f32 * share).ceil() as i32 } else { amount };
        if let Some(brain) = self.clients.get_mut(&victim).and_then(|c| c.brain.as_mut()) {
            brain.shot_from = from;
        }
        let Some(v) = self.playing(victim) else { return };
        v.state.hp = (v.state.hp - amount).max(0);
        if v.state.hp > 0 {
            let hp = v.state.hp;
            self.events.push(Event::Hit { target: victim, by, hp });
            return;
        }
        v.state.alive = false;
        v.dead = RESPAWN_TICKS;
        v.deaths += 1;
        if let Some(killer) = self.playing(by) {
            killer.kills += 1;
        }
        self.events.push(Event::Kill { killer: by, victim, head, gun });
        self.roster_changed = true;
    }

    fn snapshot(&mut self) {
        let events = std::mem::take(&mut self.events);
        let players: Vec<Seen> = self
            .clients
            .iter()
            .filter_map(|(&id, c)| c.playing.as_ref().map(|p| (id, p)))
            // Someone still looking around after spawning isn't there, as far as anyone else knows.
            .filter(|(_, p)| p.state.shield == 0)
            .map(|(id, p)| {
                let s = &p.state;
                Seen { id, pos: s.pos, yaw: s.yaw, pitch: s.pitch, team: s.team, alive: s.alive, crouch: s.crouch, gun: s.gun }
            })
            .collect();
        for client in self.clients.values().filter(|c| c.brain.is_none()) {
            let (ack, you) = client.playing.as_ref().map_or((0, None), |p| (p.ack, Some(p.state.clone())));
            let message = ToClient::Snapshot { tick: self.tick, ack, you, players: players.clone(), events: events.clone() };
            send(&client.out, &message);
        }
    }

    fn roster(&self) {
        let mut players: Vec<Entry> = self
            .clients
            .iter()
            .filter_map(|(&id, c)| c.playing.as_ref().map(|p| (id, p)))
            .map(|(id, p)| Entry { id, name: p.name.clone(), team: p.state.team, kills: p.kills, deaths: p.deaths, alive: p.state.alive })
            .collect();
        players.sort_by_key(|e| e.id);
        let message = ToClient::Roster { players };
        for client in self.clients.values().filter(|c| c.brain.is_none()) {
            send(&client.out, &message);
        }
    }
}

fn send(out: &UnboundedSender<String>, message: &ToClient) {
    if let Ok(text) = serde_json::to_string(message) {
        let _ = out.send(text);
    }
}

/// The map file: its boxes, and each team's spawns.
fn load(path: &str) -> Result<(Map, [Vec<(V3, f32)>; 2]), String> {
    let text = std::fs::read_to_string(path).map_err(|e| format!("can't read {path}: {e}"))?;
    let json: serde_json::Value = serde_json::from_str(&text).map_err(|e| format!("{path}: {e}"))?;
    let numbers = |v: &serde_json::Value| -> Vec<f32> {
        v.as_array().map(|a| a.iter().filter_map(|n| n.as_f64()).map(|n| n as f32).collect()).unwrap_or_default()
    };
    let flat: Vec<f32> = json["boxes"].as_array().ok_or("no boxes")?.iter().flat_map(numbers).collect();
    let spawns = |team: &str| -> Vec<(V3, f32)> {
        json["spawns"][team].as_array().map_or(Vec::new(), |a| {
            a.iter().map(numbers).filter(|s| s.len() == 4).map(|s| ([s[0], s[1], s[2]], s[3])).collect()
        })
    };
    let spawns = [spawns("raiders"), spawns("guards")];
    if spawns.iter().any(|s| s.is_empty()) {
        return Err(format!("{path}: each team needs spawns"));
    }
    Ok((Map::new(&flat), spawns))
}

async fn connection(stream: TcpStream, id: u32, tx: UnboundedSender<(u32, From)>) {
    let _ = stream.set_nodelay(true);
    let Ok(ws) = tokio_tungstenite::accept_async(stream).await else { return };
    let (mut write, mut read) = ws.split();
    let (out, mut outgoing) = unbounded_channel::<String>();
    if tx.send((id, From::Open(out))).is_err() {
        return;
    }
    let writer = tokio::spawn(async move {
        while let Some(text) = outgoing.recv().await {
            if write.send(Message::text(text)).await.is_err() {
                break;
            }
        }
    });
    while let Some(Ok(message)) = read.next().await {
        match message {
            Message::Text(text) => {
                if let Ok(message) = serde_json::from_str::<ToServer>(&text) {
                    let _ = tx.send((id, From::Message(message)));
                }
            }
            Message::Close(_) => break,
            _ => {}
        }
    }
    let _ = tx.send((id, From::Closed));
    writer.abort();
}

#[tokio::main(flavor = "current_thread")]
async fn main() {
    let path = std::env::args().nth(1).unwrap_or_else(|| "../../resources/outpost/map.json".into());
    let port: u16 = std::env::var("OUTPOST_PORT").ok().and_then(|p| p.parse().ok()).unwrap_or(8082);
    let team_size: usize = std::env::var("OUTPOST_BOTS").ok().and_then(|n| n.parse().ok()).unwrap_or(2);
    let (map, spawns) = load(&path).unwrap_or_else(|e| {
        eprintln!("{e}");
        std::process::exit(1);
    });
    let listener = TcpListener::bind(("0.0.0.0", port)).await.unwrap_or_else(|e| {
        eprintln!("can't listen on port {port}: {e}");
        std::process::exit(1);
    });
    println!("Outpost server on 0.0.0.0:{port} ({TICK_RATE} ticks a second), map {path}, bots filling teams to {team_size}");

    let (tx, mut rx) = unbounded_channel();
    tokio::spawn(async move {
        let mut next = 1;
        loop {
            if let Ok((stream, _)) = listener.accept().await {
                tokio::spawn(connection(stream, next, tx.clone()));
                next += 1;
            }
        }
    });

    let mut server = Server::new(map, spawns, team_size);
    // OUTPOST_BOT_LEVEL: the bots' level to start at (0 easy, 1 normal, 2 hard); players change it.
    if let Some(level) = std::env::var("OUTPOST_BOT_LEVEL").ok().and_then(|n| n.parse::<u8>().ok()).filter(|&n| n < 3) {
        server.level = level;
    }
    let mut clock = tokio::time::interval(Duration::from_micros(1_000_000 / TICK_RATE as u64));
    loop {
        clock.tick().await;
        while let Ok((id, message)) = rx.try_recv() {
            server.receive(id, message);
        }
        server.step();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn server() -> Server {
        let (map, spawns) = load(concat!(env!("CARGO_MANIFEST_DIR"), "/../../resources/outpost/map.json")).unwrap();
        Server::new(map, spawns, 0)
    }

    /// Joins player `id`, and puts them at x, z facing +x.
    fn join(s: &mut Server, id: u32, x: f32, z: f32) {
        let (out, _) = unbounded_channel();
        s.receive(id, From::Open(out));
        s.receive(id, From::Message(ToServer::Join { name: format!("P{id}") }));
        let p = s.playing(id).unwrap();
        p.state.pos = [x, 0.0, z];
        p.state.yaw = std::f32::consts::FRAC_PI_2;
        p.state.shield = 0; // out already: these tests are about standing players
    }

    fn fire_along_x(s: &mut Server, id: u32, view: f32) {
        let origin = s.playing(id).unwrap().state.eye();
        s.shoot(id, Shot { origin, dir: [1.0, 0.0, 0.0], gun: RIFLE }, view);
    }

    fn hp(s: &mut Server, id: u32) -> i32 {
        s.playing(id).unwrap().state.hp
    }

    #[test]
    fn teams_take_turns() {
        let mut s = server();
        for id in 1..=4 {
            join(&mut s, id, 0.0, 0.0);
        }
        let teams: Vec<u8> = (1..=4).map(|id| s.playing(id).unwrap().state.team).collect();
        assert_eq!(teams, [0, 1, 0, 1]);
    }

    #[test]
    fn a_shot_in_the_open_hits_and_one_through_a_wall_doesnt() {
        let mut s = server();
        join(&mut s, 1, -9.0, 2.0);
        join(&mut s, 2, -4.0, 2.0);
        s.step();
        let now = s.tick as f32;
        fire_along_x(&mut s, 1, now);
        assert_eq!(hp(&mut s, 2), 0, "at eye height: the head, which kills");
        // Now with the spawn wall (x 60..63) between them.
        let mut s = server();
        join(&mut s, 1, 56.0, 0.0);
        join(&mut s, 2, 67.0, 0.0);
        s.step();
        let now = s.tick as f32;
        fire_along_x(&mut s, 1, now);
        assert_eq!(hp(&mut s, 2), 100);
    }

    #[test]
    fn just_spawned_players_look_around_unseen_unhurt_and_still() {
        let mut s = server();
        join(&mut s, 1, -9.0, 2.0);
        join(&mut s, 2, -4.0, 2.0);
        // 2 comes back in right where 1 is aiming: looking around, so out of reach.
        s.playing(2).unwrap().state.spawn([-4.0, 0.0, 2.0], 0.0);
        s.step();
        let now = s.tick as f32;
        fire_along_x(&mut s, 1, now);
        assert_eq!(hp(&mut s, 2), 100, "can't be hit");
        assert!(s.others(1).is_empty(), "isn't in anyone's way");
        let seen = s.rewound(s.tick as f32);
        assert!(!seen[&2].1, "isn't a target in the history shots are checked against");

        // Meanwhile it can look, but not walk or shoot.
        let mut me = Player::new(7, 0);
        me.spawn([-4.0, 0.0, 2.0], 0.0);
        let held = Input { keys: keys::FORWARD | keys::FIRE, yaw: 1.0, pitch: 0.2, ..Default::default() };
        for _ in 0..SHIELD_TICKS - 1 {
            assert!(me.step(&held, &s.map, &[]).is_none(), "no shots while looking around");
        }
        assert_eq!((me.pos[0], me.pos[2]), (-4.0, 2.0), "didn't move");
        assert_eq!((me.yaw, me.pitch), (1.0, 0.2), "did look");
        me.step(&held, &s.map, &[]);
        assert_eq!(me.shield, 0);
        // Out: walking works, and a trigger held since the look-around isn't a new click.
        let walk = Input { keys: keys::FORWARD, yaw: 1.0, ..Default::default() };
        for _ in 0..10 {
            me.step(&walk, &s.map, &[]);
        }
        assert!(me.pos[0] != -4.0 || me.pos[2] != 2.0, "walks once it's out");

        // And 1 can hit 2 once 2 is out.
        s.playing(2).unwrap().state.shield = 0;
        s.step();
        let now = s.tick as f32;
        fire_along_x(&mut s, 1, now);
        assert!(hp(&mut s, 2) < 100, "fair game once out");
    }

    #[test]
    fn bots_get_no_look_around() {
        let mut s = with_bots(2);
        s.step();
        let bots: Vec<&Playing> = s.clients.values().filter(|c| c.brain.is_some()).filter_map(|c| c.playing.as_ref()).collect();
        assert_eq!(bots.len(), 4);
        assert!(bots.iter().all(|p| p.state.alive && p.state.shield == 0));
        // And again after dying.
        let id = *s.clients.iter().find(|(_, c)| c.brain.is_some()).unwrap().0;
        s.damage(1, id, 1000, false, RIFLE);
        for _ in 0..RESPAWN_TICKS + 1 {
            s.step();
        }
        let p = s.playing(id).unwrap();
        assert!(p.state.alive && p.state.shield == 0);
    }

    #[test]
    fn the_first_rifle_and_magnum_shot_goes_where_its_aimed() {
        let s = server();
        for gun in [RIFLE, PISTOL] {
            let mut p = Player::new(3, 0);
            p.spawn([-9.0, 0.0, 2.0], 0.0);
            p.shield = 0;
            p.gun = gun;
            // Running, which throws other shots off.
            let run = |keys| Input { keys, yaw: 0.4, pitch: 0.1, gun, ..Default::default() };
            for _ in 0..30 {
                p.step(&run(keys::FORWARD), &s.map, &[]);
            }
            let aim = forward(0.4, 0.1);
            let shot = p.step(&run(keys::FORWARD | keys::FIRE), &s.map, &[]).expect("fires");
            assert!(dot(shot.dir, aim) > 0.99999, "gun {gun}: the first shot is exact");
            // Later ones in the string aren't.
            let mut off = 0.0f32;
            for _ in 0..40 {
                p.step(&run(keys::FORWARD), &s.map, &[]); // the magnum fires once a click
                if let Some(shot) = p.step(&run(keys::FORWARD | keys::FIRE), &s.map, &[]) {
                    off = off.max(1.0 - dot(shot.dir, forward(p.yaw + p.punch[1], p.pitch + p.punch[0])));
                }
            }
            assert!(off > 1e-5, "gun {gun}: the rest spread");
        }
    }

    #[test]
    fn no_hurting_your_own_team() {
        let mut s = server();
        join(&mut s, 1, -9.0, 2.0);
        join(&mut s, 2, 0.0, 0.0); // the other team, out of the way
        join(&mut s, 3, -4.0, 2.0); // Alpha's team
        s.step();
        let now = s.tick as f32;
        fire_along_x(&mut s, 1, now);
        assert_eq!(hp(&mut s, 3), 100);
    }

    #[test]
    fn shots_are_checked_where_the_shooter_saw_people() {
        let mut s = server();
        join(&mut s, 1, -9.0, 2.0);
        join(&mut s, 2, -4.0, 2.0);
        s.step();
        let seen_at = s.tick as f32;
        // Bravo steps out of the line of fire; Alpha's page hasn't shown that yet.
        for _ in 0..6 {
            s.playing(2).unwrap().state.pos[2] = 4.0;
            s.step();
        }
        fire_along_x(&mut s, 1, seen_at);
        assert!(hp(&mut s, 2) < 100, "hit where Alpha saw Bravo");
        // But not further back than 0.3 s.
        let mut s = server();
        join(&mut s, 1, -9.0, 2.0);
        join(&mut s, 2, -4.0, 2.0);
        s.step();
        let seen_at = s.tick as f32;
        for _ in 0..40 {
            s.playing(2).unwrap().state.pos[2] = 4.0;
            s.step();
        }
        fire_along_x(&mut s, 1, seen_at);
        assert_eq!(hp(&mut s, 2), 100);
    }

    #[test]
    fn three_body_shots_kill_and_they_come_back() {
        let mut s = server();
        join(&mut s, 1, -9.0, 2.0);
        join(&mut s, 2, -4.0, 2.0);
        s.step();
        for _ in 0..3 {
            let origin = [-9.0, 1.2, 2.0];
            s.shoot(1, Shot { origin, dir: [1.0, 0.0, 0.0], gun: RIFLE }, s.tick as f32);
        }
        assert!(!s.playing(2).unwrap().state.alive);
        assert_eq!(s.playing(1).unwrap().kills, 1);
        for _ in 0..RESPAWN_TICKS {
            s.step();
        }
        assert!(s.playing(2).unwrap().state.alive);
        assert_eq!(hp(&mut s, 2), 100);
    }

    #[test]
    fn a_sniper_kills_with_one_to_the_body_but_not_the_legs() {
        let body = [-9.0, 1.2, 2.0];
        let legs = [-9.0, 0.5, 2.0];
        for (origin, dies) in [(body, true), (legs, false)] {
            let mut s = server();
            join(&mut s, 1, -9.0, 2.0);
            join(&mut s, 2, -4.0, 2.0);
            s.step();
            s.shoot(1, Shot { origin, dir: [1.0, 0.0, 0.0], gun: SNIPER }, s.tick as f32);
            assert_eq!(!s.playing(2).unwrap().state.alive, dies);
        }
    }

    #[test]
    fn crouching_ducks_under_a_shot_at_head_height() {
        let mut s = server();
        join(&mut s, 1, -9.0, 2.0);
        join(&mut s, 2, -4.0, 2.0);
        s.playing(2).unwrap().state.crouch = 1.0;
        s.step();
        let now = s.tick as f32;
        fire_along_x(&mut s, 1, now); // from a standing eye, level
        assert_eq!(hp(&mut s, 2), 100);
    }

    /// Steps a lone player with `keys` held, `ticks` times. How many shots they fired.
    fn hold(p: &mut Player, map: &Map, keys: u16, gun: u8, ticks: u32) -> u32 {
        let mut shots = 0;
        for seq in 0..ticks {
            let input = Input { seq, keys, yaw: 0.0, pitch: 0.0, gun };
            shots += p.step(&input, map, &[]).is_some() as u32;
        }
        shots
    }

    #[test]
    fn the_pistol_fires_once_a_click_and_the_rifle_for_as_long_as_its_held() {
        let s = server();
        let mut p = Player::new(1, 0);
        p.spawn([-9.0, 0.0, 2.0], 0.0);
        p.shield = 0; // out already: this is about the guns and moving, not spawning
        hold(&mut p, &s.map, 0, PISTOL, 30); // get it out
        assert_eq!(hold(&mut p, &s.map, keys::FIRE, PISTOL, 60), 1);
        hold(&mut p, &s.map, 0, RIFLE, 40);
        assert_eq!(hold(&mut p, &s.map, keys::FIRE, RIFLE, 60), 10);
    }

    #[test]
    fn the_sniper_is_only_accurate_scoped_and_scopes_again_after_the_bolt() {
        let s = server();
        let mut p = Player::new(1, 0);
        p.spawn([-9.0, 0.0, 2.0], 0.0);
        p.shield = 0; // out already: this is about the guns and moving, not spawning
        hold(&mut p, &s.map, 0, SNIPER, 90);
        let unscoped = p.spread();
        hold(&mut p, &s.map, keys::SCOPE, SNIPER, 1);
        assert!(p.scoped && p.spread() < unscoped / 50.0);
        assert_eq!(hold(&mut p, &s.map, keys::FIRE, SNIPER, 1), 1);
        assert!(!p.scoped, "off the scope to work the bolt");
        hold(&mut p, &s.map, 0, SNIPER, GUNS[SNIPER as usize].fire_ticks);
        assert!(p.scoped, "and back on it");
        assert_eq!(p.ammo[SNIPER as usize], 9);
    }

    #[test]
    fn crouching_is_slow_and_lowers_your_eye() {
        let s = server();
        let mut p = Player::new(1, 0);
        p.spawn([-9.0, 0.0, 2.0], 0.0);
        p.shield = 0; // out already: this is about the guns and moving, not spawning
        hold(&mut p, &s.map, keys::CROUCH | keys::FORWARD, RIFLE, 60);
        assert!(p.crouch == 1.0 && p.eye()[1] < 1.2);
        assert!(p.speed() < RUN * 0.4);
        hold(&mut p, &s.map, 0, RIFLE, 30);
        assert_eq!(p.crouch, 0.0);
    }

    fn with_bots(team_size: usize) -> Server {
        let (map, spawns) = load(concat!(env!("CARGO_MANIFEST_DIR"), "/../../resources/outpost/map.json")).unwrap();
        Server::new(map, spawns, team_size)
    }

    #[test]
    fn bots_fill_the_teams_and_make_way_for_people() {
        let mut s = with_bots(3);
        assert_eq!(s.counts(), [[0, 3], [0, 3]]);
        let (out, _) = unbounded_channel();
        s.receive(1, From::Open(out));
        s.receive(1, From::Message(ToServer::Join { name: "Alpha".into() }));
        assert_eq!(s.counts(), [[1, 2], [0, 3]]);
        s.receive(1, From::Closed);
        assert_eq!(s.counts(), [[0, 3], [0, 3]]);
    }

    #[test]
    fn there_is_a_way_from_one_side_to_the_other() {
        let s = with_bots(0);
        let (from, to) = (s.spawns[0][0].0, s.spawns[1][0].0);
        let path = s.nav.path(from, to);
        assert!(!path.is_empty());
        let end = path.last().unwrap();
        assert!((end[0] - to[0]).abs() < 1.0 && (end[2] - to[2]).abs() < 1.0);
    }

    /// How deadly the bots are, to tune them by: a player standing in the open mid-map, who never shoots, against
    /// two bots a side, for three minutes. `cargo test --release -- --ignored --nocapture`.
    #[test]
    #[ignore]
    fn how_deadly_the_bots_are() {
        for level in 0..3u8 {
        let (mut deaths, mut kills) = (0, 0);
        for seed in 0..4 {
            let mut s = with_bots(2);
            s.level = level;
            for _ in 0..seed * 37 {
                s.step();
            }
            let (out, _) = unbounded_channel();
            s.receive(1, From::Open(out));
            s.receive(1, From::Message(ToServer::Join { name: "Still".into() }));
            for _ in 0..180 * TICK_RATE {
                // Standing in the open in the middle, beside the well.
                let p = s.playing(1).unwrap();
                if p.state.alive {
                    p.state.pos = [-7.0, 0.0, 0.0];
                }
                let (seq, view) = (p.queued + 1, s.tick as f32);
                let input = Input { seq, yaw: std::f32::consts::FRAC_PI_2, ..Default::default() };
                s.receive(1, From::Message(ToServer::Input { view, inputs: vec![input] }));
                s.step();
            }
            deaths += s.clients[&1].playing.as_ref().unwrap().deaths;
            kills += s.clients.values().filter_map(|c| c.playing.as_ref()).map(|p| p.kills).sum::<u32>();
        }
        println!("level {level}: a player standing in the middle died {:.1} times a minute; {:.1} kills a minute in all", deaths as f32 / 12.0, kills as f32 / 12.0);
        }
    }

    #[test]
    fn anyone_can_change_the_bots_level_and_everyone_hears() {
        let mut s = server();
        let (out1, mut in1) = unbounded_channel();
        let (out2, mut in2) = unbounded_channel();
        s.receive(1, From::Open(out1));
        s.receive(2, From::Open(out2));
        let level = |rx: &mut tokio::sync::mpsc::UnboundedReceiver<_>| {
            let mut last = None;
            while let Ok(m) = rx.try_recv() {
                if let Ok(ToClient::Level { level }) = serde_json::from_str::<ToClient>(&String::from(m)) {
                    last = Some(level);
                }
            }
            last
        };
        assert_eq!((level(&mut in1), level(&mut in2)), (Some(1), Some(1)), "told on connecting");
        s.receive(2, From::Message(ToServer::Level { level: 0 }));
        assert_eq!(s.level, 0);
        assert_eq!((level(&mut in1), level(&mut in2)), (Some(0), Some(0)), "everyone told");
        s.receive(1, From::Message(ToServer::Level { level: 9 }));
        assert_eq!(s.level, 0, "no such level");
    }

    #[test]
    fn bots_find_each_other_and_fight() {
        let mut s = with_bots(3);
        for _ in 0..60 * TICK_RATE {
            s.step();
        }
        let kills: u32 = s.clients.values().filter_map(|c| c.playing.as_ref()).map(|p| p.kills).sum();
        println!("kills in a minute: {kills}");
        assert!(kills >= 3, "only {kills} kills in a minute");
    }
}
