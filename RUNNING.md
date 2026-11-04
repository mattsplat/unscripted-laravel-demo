# Running the demo apps

This Laravel app hosts several small apps. Most need only the web server and Reverb; Outpost
also needs its own game server.

| App | URL | Needs running |
|---|---|---|
| Todos | <http://localhost:8010/> | web server, Reverb |
| Counters | <http://localhost:8010/counters> | web server, Reverb |
| Features check | <http://localhost:8010/features> | web server, Reverb |
| Spread (multiplayer board game) | <http://localhost:8010/spread> | web server, Reverb |
| Voxel (shared block world) | <http://localhost:8010/voxel> | web server, Reverb |
| Outpost (team shooter) | <http://localhost:8010/outpost> | web server, Outpost game server |
| Rally (table tennis) | <http://localhost:8010/rally> | web server; Reverb for the results list |
| Rally, drawn with three-d | <http://localhost:8010/rally-3d> | the same |

Reverb is needed wherever a page calls the server. `.env` sets `UNSCRIPTED_TRANSPORT=reverb`, so
those calls go over Reverb's WebSocket instead of HTTP. Set it to `http` to run the apps
without Reverb, but you'll lose the live updates in Spread, Voxel and Rally.

## What you need installed

- **PHP 8.4** and **Composer**.
- **Node**, through nvm: `nvm use` (the default, v20, works).
- **Optional: the `unscripted-js` checkout** next to this folder, at `../unscripted-js`. The
  pages load the runtime and PHP from the CDN (`@mtjncn/runtime`, `@mtjncn/php84`). To work on
  them, load them from the checkout instead, through the links in `public/vendor/unscripted/`:

  ```ini
  UNSCRIPTED_RUNTIME=/vendor/unscripted/runtime/auto.js
  UNSCRIPTED_PACK=/vendor/unscripted/php84/php_pack.wasm
  ```
- **Rust**, only to rebuild the games or run the Outpost server:

  ```sh
  rustup target add wasm32-unknown-unknown
  cargo install wasm-bindgen-cli --version 0.2.130   # must match the crates' wasm-bindgen
  ```

- A browser with **WebGL 2** (any current Chrome, Firefox, Safari or Edge) for Voxel,
  Outpost and Rally.

## First-time setup

From this folder:

```sh
composer install
npm install
cp .env.example .env        # if there's no .env yet
php artisan key:generate
touch database/database.sqlite
php artisan migrate
```

Then make sure `.env` has the Reverb settings. Pick your own random values for the app
ID, key and secret:

```ini
BROADCAST_CONNECTION=reverb
REVERB_APP_ID=your-app-id
REVERB_APP_KEY=your-app-key
REVERB_APP_SECRET=your-app-secret
REVERB_HOST=127.0.0.1
REVERB_PORT=8081
REVERB_SCHEME=http
REVERB_SERVER_PORT=8081
UNSCRIPTED_TRANSPORT=reverb
```

### Building the games

The built games live in `public/wasm/`. Build them the first time, and again after changing
their Rust code:

```sh
npm run build:voxel     # voxel/    -> public/wasm/voxel
npm run build:outpost   # outpost/  -> public/wasm/outpost
npm run build:rally     # rally/    -> public/wasm/rally
npm run build:rally3d   # rally3d/  -> public/wasm/rally3d (the same game, drawn with three-d)
```

The Outpost game server builds itself the first time it starts (below).

## Starting everything

Use a terminal for each:

```sh
# 1. The web server: every app's pages.
php artisan serve --host=0.0.0.0 --port=8010

# 2. Reverb: page-to-server calls and live updates (port 8081).
php artisan reverb:start --port=8081

# 3. Only for Outpost: its game server (port 8082).
npm run outpost:server
```

Then open the URLs in the table above.

Including `--host=0.0.0.0` lets other machines on your network play too. Open
`http://<this machine's IP>:8010/...` on them. Reverb and the Outpost server are reached at the
same address the page came from.

### Outpost options

- `OUTPOST_BOTS=0 npm run outpost:server` turns off computer players. The default fills each
  team up to 2 players, and bots make way as people join.
- `OUTPOST_PORT` changes the game server's port (8082 by default). The page always connects to
  8082, so change both together.
- After editing the Rust in `outpost/src/sim.rs`, rebuild the browser side
  (`npm run build:outpost`) and restart the game server, as both use those rules.

## Tests

```sh
php artisan test                  # the Laravel side
(cd rally && cargo test --release)            # Rally's rules and physics
(cd outpost/server && cargo test --release)   # Outpost's rules, server and bots
```

## When something doesn't work

- **Outpost says "Can't reach the game server".** Start it with `npm run outpost:server`, then
  reload the page.
- **Buttons do nothing, or lists never load (Todos, Spread, Voxel, Rally's results).** Reverb
  isn't running: start it, or set `UNSCRIPTED_TRANSPORT=http` in `.env`.
- **A game shows a black screen.** The browser has no WebGL 2, or the game hasn't been built:
  run its `npm run build:...`.
- **A change to `ui-server` doesn't show.** It's installed from GitHub now, not linked: in
  `unscripted-js`, commit it and run `npm run split-ui-server`; then here,
  `composer update unscripted/ui-server`.
- **A change to a game's Rust doesn't show.** Rebuild it, then hard-reload the page.
- **No sound.** Browsers start sound only after you click the page, so click into the game.
- **"Address already in use".** Something else has that port (8010, 8081 or 8082). Stop it,
  or choose another port with `--port`.

## Deploying (Laravel Cloud)

Build commands: `composer install --no-dev --no-interaction --prefer-dist --optimize-autoloader`,
`npm ci`, `npm run build`. Don't run the `build:` game scripts there: there's no Rust, and the
built games are committed in `public/wasm/`. Rebuild them here after changing their Rust, and
commit the result.

- Leave `UNSCRIPTED_RUNTIME` and `UNSCRIPTED_PACK` unset, for the CDN.
- `UNSCRIPTED_TRANSPORT=reverb` needs Cloud's WebSockets (Reverb) set up, with the `REVERB_*`
  variables; otherwise set it to `http` (Voxel's live updates need the socket).
- Outpost's game server is a separate Rust program, which Cloud doesn't run: the Outpost page
  loads, but can't reach a server.
