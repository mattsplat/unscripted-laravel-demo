# Outpost

A team shooter (Raiders against Guards, team deathmatch) on one fixed map. Open `/outpost`,
with the game server running: `npm run outpost:server` (port 8082).

- `outpost/src/sim.rs`: the rules: movement and crouching, the three guns (the `GUNS` table: rifle, magnum, sniper), what a shot hits, and the messages
  the game and its server send each other. Both build it.
- `outpost/server/`: the game server (native Rust: tokio, WebSockets). It runs the game 60
  times a second and decides every move and hit. `cargo test` in there tests its rules.
- `outpost/src/lib.rs`: the game in the browser: drawing (WebGL 2), and predicting your own
  moves. Build it with `npm run build:outpost`.
- `resources/components/outpost.blade.php`: the HUD (joining, health, kill feed, scores). It
  hears the game's events and makes no server calls of its own.
- `resources/views/outpost.blade.php`: the page, the HUD's static parts and the sounds.
- `resources/outpost/map.json`: the map, as boxes, written by `outpost/map.py`. To change the
  map:
  1. Edit `outpost/map.py`.
  2. Run `python3 outpost/map.py resources/outpost/map.json`.
  3. Restart the game server.

## How it plays over the network

1. **Pages send only inputs.** Each one is a tick's keys and aim, 60 a second.
2. **The server decides.** It runs everyone's inputs through the rules and sends everyone a
   snapshot of where everyone is, 30 a second.
3. **Your own moves show at once.** The page runs the same rules on your inputs straight
   away. When a snapshot comes, it takes the server's word for where you are, then replays
   the inputs the server hasn't run yet on top of it.
4. **Everyone else is shown 100 ms in the past**, between the two snapshots around that
   moment, so they move smoothly.
5. **Lag compensation:** each input says which moment the page was showing. A shot is checked
   against where people were then, up to 0.3 s back.
6. **No cheating with inputs.** The spread of each shot comes from a seed, so the server works
   it out the same way. A page can't steer its bullets, move faster than the rules allow, or
   send more than 60 inputs a second.

## Earlier versions

1. **Trust the client.** Fine among friends, but trivial to cheat.
2. **The Laravel server checks the hits.** Moves and shots went through PHP calls over
   Reverb. A copy of that version is in `storage/app/outpost-level2/`.

## Bots

The game server fills each team up to 2 players with computer players (`outpost/server/src/bots.rs`),
and a bot steps out whenever a person joins its team. `OUTPOST_BOTS=5 npm run outpost:server`
changes the team size; `OUTPOST_BOTS=0` turns bots off. They play through the same rules as
everyone else (a tick's `Input` each, like a page). How good they are is set by `REACT`, `TURN` and
the starting aim error in `bots.rs`.

## Later, maybe

- rounds, with planting and defusing
- a buy phase and money
- more weapons (add a row to `GUNS` in `sim.rs`, and its models in `lib.rs`)
- a second zoom level on the sniper's scope
- a radar
- names over heads
- reconnecting by itself when the socket drops (today you reload the page)
