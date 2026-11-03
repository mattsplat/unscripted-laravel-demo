<!doctype html>
<html>
<head>
  <meta charset="utf-8">
  <meta name="csrf-token" content="{{ csrf_token() }}">
  <title>Outpost</title>
  <style>
    html, body { margin: 0; height: 100%; overflow: hidden; background: #000; color: #fff;
      font-family: "DIN Alternate", Bahnschrift, "Arial Narrow", "Helvetica Neue", sans-serif; }
    #world { position: fixed; inset: 0; width: 100%; height: 100%; display: block; }
    .t-raiders { color: #e8b04a; }
    .t-guards { color: #7fb2ff; }
    .shade { text-shadow: 0 1px 2px rgba(0,0,0,.9); }

    /* The crosshair: four lines, apart by --gap (the game sets it from your spread). */
    #crosshair { --gap: 4px; position: fixed; left: 50%; top: 50%; width: 0; height: 0; pointer-events: none; }
    #crosshair i { position: absolute; background: #4dff6a; box-shadow: 0 0 0 1px rgba(0,0,0,.55); }
    #crosshair i:nth-child(1) { width: 2px; height: 7px; left: -1px; bottom: var(--gap); }
    #crosshair i:nth-child(2) { width: 2px; height: 7px; left: -1px; top: var(--gap); }
    #crosshair i:nth-child(3) { width: 7px; height: 2px; top: -1px; right: var(--gap); }
    #crosshair i:nth-child(4) { width: 7px; height: 2px; top: -1px; left: var(--gap); }
    #hitmarker { position: fixed; left: 50%; top: 50%; width: 22px; height: 22px; margin: -11px 0 0 -11px; opacity: 0; pointer-events: none;
      background: linear-gradient(45deg, transparent 46%, #fff 46%, #fff 54%, transparent 54%), linear-gradient(-45deg, transparent 46%, #fff 46%, #fff 54%, transparent 54%);
      -webkit-mask: radial-gradient(circle, transparent 5px, #000 5px); mask: radial-gradient(circle, transparent 5px, #000 5px); }
    #ammo { position: fixed; right: 28px; bottom: 22px; font-size: 22px; color: #ddd; text-shadow: 0 1px 2px #000; }
    #ammo b { font-size: 44px; color: #fff; margin-right: 2px; }
    #ammo i { font-style: normal; font-size: 13px; letter-spacing: .12em; text-transform: uppercase; color: #bbb; margin-right: 12px; }
    /* Through the sniper's scope: black but for a circle, crossed by fine lines. */
    #scope { display: none; position: fixed; inset: 0; pointer-events: none;
      background: radial-gradient(circle at center, transparent 0, transparent calc(min(48vw, 48vh) - 2px), #000 min(48vw, 48vh)); }
    #scope::before, #scope::after { content: ""; position: absolute; background: #000; }
    #scope::before { left: 0; right: 0; top: 50%; height: 1px; }
    #scope::after { top: 0; bottom: 0; left: 50%; width: 1px; }
    #fps { position: fixed; left: 10px; top: 8px; font: 12px ui-monospace, Menlo, monospace; color: #4dff6a; text-shadow: 0 1px 2px #000; pointer-events: none; }
    .help { position: fixed; left: 50%; bottom: 12px; transform: translateX(-50%); font-size: 12px; color: rgba(255,255,255,.7); text-shadow: 0 1px 2px #000; }

    .score { position: fixed; top: 10px; left: 50%; transform: translateX(-50%); display: flex; align-items: center; gap: 14px;
      background: rgba(10,12,14,.55); padding: 4px 16px; border-radius: 3px; font-size: 26px; font-weight: bold; }
    .score .teams { font-size: 12px; letter-spacing: .12em; text-transform: uppercase; color: #bbb; font-weight: normal; }
    .score .teams i { color: #777; font-style: normal; }

    .feed { position: fixed; top: 12px; right: 14px; list-style: none; margin: 0; padding: 0; display: flex; flex-direction: column; gap: 4px; align-items: flex-end; }
    .feed li { background: rgba(10,12,14,.6); padding: 4px 10px; border-radius: 2px; font-size: 15px; display: flex; gap: 8px; align-items: center; }
    .feed .rifle { width: 34px; height: 7px; background: linear-gradient(90deg, #ddd 0 70%, transparent 70% 74%, #ddd 74%); border-radius: 1px; position: relative; }
    .feed .rifle::after { content: ""; position: absolute; left: 13px; top: 6px; width: 5px; height: 6px; background: #ddd; }
    .feed .head { color: #ff6b5a; }
    .feed .pistol { width: 20px; height: 7px; background: #ddd; border-radius: 1px; position: relative; }
    .feed .pistol::after { content: ""; position: absolute; left: 2px; top: 6px; width: 6px; height: 7px; background: #ddd; transform: skewX(-12deg); }
    .feed .sniper { width: 48px; height: 5px; background: #ddd; border-radius: 1px; position: relative; }
    .feed .sniper::before { content: ""; position: absolute; left: 14px; top: -5px; width: 16px; height: 4px; background: #ddd; border-radius: 2px; }
    .feed .sniper::after { content: ""; position: absolute; left: 6px; top: 4px; width: 5px; height: 6px; background: #ddd; }

    .vitals { position: fixed; left: 26px; bottom: 22px; display: flex; align-items: baseline; gap: 8px; text-shadow: 0 1px 2px #000; }
    .vitals .plus { font-size: 26px; color: #eee; }
    .vitals b { font-size: 44px; min-width: 70px; }
    .vitals.low b, .vitals.low .plus { color: #ff4d3d; }
    .vitals .you { font-size: 13px; margin-left: 12px; letter-spacing: .04em; }
    .dead { position: fixed; left: 50%; top: 62%; transform: translateX(-50%); font-size: 20px; letter-spacing: .06em; text-transform: uppercase; color: #ff6b5a; text-shadow: 0 1px 3px #000; }
    .hurt { position: fixed; inset: 0; pointer-events: none; background: radial-gradient(ellipse at center, transparent 45%, rgba(190,0,0,.55)); animation: fade .6s ease-out forwards; }
    @keyframes fade { from { opacity: 1; } to { opacity: 0; } }

    .join { position: fixed; left: 50%; top: 50%; transform: translate(-50%, -50%); background: rgba(12,14,16,.82); padding: 28px 34px; border-radius: 4px; min-width: 300px; border-top: 3px solid #e8b04a; }
    .join h1 { margin: 0 0 4px; font-size: 34px; letter-spacing: .3em; }
    .join p { margin: 0 0 16px; color: #aaa; font-size: 14px; }
    .join form { display: flex; gap: 8px; }
    .join input { flex: 1; font: inherit; font-size: 16px; padding: 8px 10px; background: #1c2024; color: #fff; border: 1px solid #3a4046; border-radius: 2px; }
    .join button { font: inherit; font-size: 15px; letter-spacing: .1em; text-transform: uppercase; padding: 8px 16px; background: #e8b04a; color: #111; border: 0; border-radius: 2px; cursor: pointer; }
    .error { color: #ff6b5a; font-size: 13px; margin: 8px 0 0; }

    .board { position: fixed; left: 50%; top: 50%; transform: translate(-50%, -50%); display: flex; gap: 18px; }
    .board table { background: rgba(10,12,14,.85); border-collapse: collapse; min-width: 280px; font-size: 15px; }
    .board th { text-align: left; padding: 8px 12px; border-bottom: 2px solid currentColor; font-size: 13px; letter-spacing: .1em; text-transform: uppercase; }
    .board td { padding: 5px 12px; color: #ddd; }
    .board th:nth-child(n+2), .board td:nth-child(n+2) { text-align: right; width: 40px; }
    .board tr.me td { color: #fff; background: rgba(255,255,255,.08); }
    .board tr.down td { color: #777; }
  </style>
</head>
<body>
  <canvas id="world"></canvas>
  <div id="crosshair"><i></i><i></i><i></i><i></i></div>
  <div id="hitmarker"></div>
  <div id="ammo"></div>
  <div id="scope"></div>
  <div id="fps"></div>
  <div class="help">Click to play · WASD move · shift crouch · space jump · mouse shoot · right-click scope · 1 rifle, 2 magnum, 3 sniper (or Q, wheel) · R reload · Tab scores · Esc lets go</div>

  {{-- The game (outpost/, built into public/wasm/outpost), and its game server (outpost/server:
       npm run outpost:server) on this host's port 8082. Sounds are made here, as the game asks. --}}
  <script type="module">
    import init, * as game from '/wasm/outpost/outpost.js';
    const map = @json(json_decode(file_get_contents(resource_path('outpost/map.json')), true));
    let ready = false, joining = null;
    window.outpost = { join: (name) => ready ? game.join(name) : (joining = name) };

    // Sounds, made up as needed: a crack and a thump for shots (bigger for bigger guns), clicks
    // for the rest.
    let audio, noise;
    const play = (kind, volume, gun = 0) => {
      audio ??= new AudioContext();
      if (audio.state === 'suspended') audio.resume();
      if (!noise) {
        noise = audio.createBuffer(1, audio.sampleRate, audio.sampleRate);
        const data = noise.getChannelData(0);
        for (let i = 0; i < data.length; i++) data[i] = Math.random() * 2 - 1;
      }
      const t = audio.currentTime;
      const burst = (at, length, gain, type, frequency) => {
        const source = audio.createBufferSource();
        const filter = audio.createBiquadFilter();
        const g = audio.createGain();
        source.buffer = noise;
        filter.type = type;
        filter.frequency.value = frequency;
        g.gain.setValueAtTime(gain, at);
        g.gain.exponentialRampToValueAtTime(0.001, at + length);
        source.connect(filter).connect(g).connect(audio.destination);
        source.start(at, Math.random() * 0.5, length + 0.02);
      };
      const tone = (at, from, to, length, gain, type = 'sine') => {
        const o = audio.createOscillator();
        const g = audio.createGain();
        o.type = type;
        o.frequency.setValueAtTime(from, at);
        o.frequency.exponentialRampToValueAtTime(to, at + length);
        g.gain.setValueAtTime(gain, at);
        g.gain.exponentialRampToValueAtTime(0.001, at + length);
        o.connect(g).connect(audio.destination);
        o.start(at);
        o.stop(at + length + 0.02);
      };
      if (kind === 0 && gun === 0) { burst(t, 0.16, 0.45, 'lowpass', 5000); tone(t, 150, 45, 0.12, 0.5); }
      if (kind === 0 && gun === 1) { burst(t, 0.22, 0.55, 'lowpass', 3500); tone(t, 120, 38, 0.17, 0.6); }
      if (kind === 0 && gun === 2) { burst(t, 0.5, 0.7, 'lowpass', 2600); tone(t, 90, 26, 0.4, 0.8); burst(t + 0.06, 0.9, 0.14, 'lowpass', 600); }
      if (kind === 1 && gun === 0) { burst(t, 0.22, 0.4 * volume, 'lowpass', 900 + 2500 * volume); tone(t, 110, 40, 0.14, 0.35 * volume); }
      if (kind === 1 && gun === 1) { burst(t, 0.26, 0.45 * volume, 'lowpass', 700 + 2000 * volume); tone(t, 95, 35, 0.18, 0.4 * volume); }
      if (kind === 1 && gun === 2) { burst(t, 0.7, 0.6 * volume, 'lowpass', 500 + 2000 * volume); tone(t, 70, 24, 0.45, 0.55 * volume); }
      if (kind === 2) tone(t, 2200, 1800, 0.05, 0.12, 'square');
      if (kind === 3) { burst(t + 0.5, 0.05, 0.3, 'highpass', 2500); burst(t + 1.9, 0.06, 0.35, 'highpass', 2000); }
      if (kind === 4) burst(t, 0.03, 0.3, 'highpass', 3000);
      if (kind === 5) { burst(t + 0.35, 0.04, 0.3, 'highpass', 1800); burst(t + 0.62, 0.05, 0.35, 'highpass', 1500); }
    };
    window.addEventListener('outpost-sound', (e) => play(...e.detail));

    // Frames a second, counted over each half second. Every animation-frame callback runs in the
    // same frame as the game's, so this counts the game's frames too.
    const fps = document.getElementById('fps');
    let frames = 0, since = performance.now();
    const count = (now) => {
      frames++;
      if (now - since >= 500) {
        fps.textContent = `${Math.round(frames * 1000 / (now - since))} FPS`;
        frames = 0;
        since = now;
      }
      requestAnimationFrame(count);
    };
    requestAnimationFrame(count);

    await init();
    game.start('world', new Float32Array(map.boxes.flat()), `ws://${location.hostname}:8082`);
    ready = true;
    if (joining) game.join(joining);
  </script>

  @unscripted('outpost')
  <script type="text/php" src="/unscripted/ui.php"></script>
  <script src="/vendor/unscripted/runtime/auto.js" data-php="/vendor/unscripted/php84/php_pack.wasm"></script>
</body>
</html>
