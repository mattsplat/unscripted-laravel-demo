<!doctype html>
<html>
<head>
  <meta charset="utf-8">
  <meta name="csrf-token" content="{{ csrf_token() }}">
  <title>Rally</title>
  <style>
    html, body { margin: 0; height: 100%; overflow: hidden; background: #0a0c11; color: #fff;
      font-family: "Avenir Next", "Segoe UI", "Helvetica Neue", sans-serif; }
    #world { position: fixed; inset: 0; width: 100%; height: 100%; display: block; }
    #engine { position: fixed; right: 12px; top: 10px; font: 12px ui-monospace, Menlo, monospace; color: #8a93a6; }
    #engine a { color: #b9c2d3; }
    #fps { position: fixed; left: 10px; top: 8px; font: 12px ui-monospace, Menlo, monospace; color: #8fd18f; text-shadow: 0 1px 2px #000; pointer-events: none; }

    /* The scoreboard: you on the left, the computer on the right; a dot for whoever serves. */
    .score { position: fixed; top: 14px; left: 50%; transform: translateX(-50%); display: flex; gap: 2px; pointer-events: none; }
    .score .side { display: grid; grid-template-columns: auto auto; grid-template-rows: auto auto; align-items: center; column-gap: 14px;
      background: #0c111d; padding: 6px 16px 8px; min-width: 150px; box-shadow: 0 4px 14px rgba(0,0,0,.4); }
    .score .side:first-child { border-radius: 6px 0 0 6px; border-top: 3px solid #4f9dff; }
    .score .cpu { border-radius: 0 6px 6px 0; border-top: 3px solid #f07a2c; direction: rtl; }
    .score .who { font-size: 12px; letter-spacing: .08em; text-transform: uppercase; color: #b9c2d3; grid-column: 1 / 3; direction: ltr; }
    .score .cpu .who { text-align: right; }
    .score b { font-size: 40px; line-height: 1; font-variant-numeric: tabular-nums; }
    .score .games { display: flex; gap: 5px; }
    .score .games i { width: 9px; height: 9px; border-radius: 50%; border: 1.5px solid #6b7486; }
    .score .games i.won { background: #ffd35a; border-color: #ffd35a; }
    .serve { display: inline-block; width: 8px; height: 8px; border-radius: 50%; background: #fff; margin: 0 4px; box-shadow: 0 0 6px #fff; vertical-align: 1px; }
    .message { position: fixed; top: 96px; left: 50%; transform: translateX(-50%); margin: 0; padding: 6px 14px; border-radius: 4px;
      background: rgba(10, 14, 24, .6); font-size: 15px; letter-spacing: .02em; white-space: nowrap; pointer-events: none; animation: rise .25s ease-out; }
    @keyframes rise { from { opacity: 0; transform: translate(-50%, 6px); } }

    .card { position: fixed; left: 50%; top: 50%; transform: translate(-50%, -50%); width: min(520px, calc(100vw - 40px)); max-height: calc(100vh - 40px); overflow: auto;
      background: rgba(10, 14, 24, .88); padding: 26px 30px; border-radius: 10px; border-top: 4px solid #4f9dff; box-sizing: border-box; }
    .card h1 { margin: 0; font-size: 30px; letter-spacing: .02em; }
    .card h1.title { font-size: 34px; letter-spacing: .14em; }
    .card h1.won { color: #ffd35a; }
    .card .sub { margin: 4px 0 18px; color: #b9c2d3; }
    .card form { display: flex; flex-wrap: wrap; gap: 8px; }
    .card input { flex: 1 1 100%; font: inherit; font-size: 16px; padding: 9px 11px; background: #161c29; color: #fff; border: 1px solid #323b4e; border-radius: 4px; }
    .levels { display: flex; flex: 1; }
    .levels button { flex: 1; font: inherit; font-size: 14px; padding: 9px; background: #161c29; color: #b9c2d3; border: 1px solid #323b4e; cursor: pointer; }
    .levels button:first-child { border-radius: 4px 0 0 4px; }
    .levels button:last-child { border-radius: 0 4px 4px 0; }
    .levels button.picked { background: #24324d; color: #fff; border-color: #4f9dff; }
    .card .go { font: inherit; font-size: 15px; font-weight: 600; letter-spacing: .08em; text-transform: uppercase; padding: 9px 22px; background: #4f9dff; color: #07101f; border: 0; border-radius: 4px; cursor: pointer; }
    .error { color: #ff7a6b; font-size: 13px; margin: 8px 0 0; }
    .how { color: #8a93a6; font-size: 13px; line-height: 1.5; margin: 16px 0 0; }
    .lists { display: grid; grid-template-columns: 1.3fr 1fr; gap: 20px; margin-top: 18px; border-top: 1px solid #252d3d; padding-top: 14px; }
    .lists h2 { font-size: 12px; letter-spacing: .1em; text-transform: uppercase; color: #8a93a6; margin: 0 0 8px; font-weight: 600; }
    .lists ul, .lists ol { margin: 0; padding: 0; list-style: none; font-size: 14px; }
    .lists ol { counter-reset: n; }
    .lists li { padding: 3px 0; white-space: nowrap; overflow: hidden; text-overflow: ellipsis; }
    .lists ol li::before { counter-increment: n; content: counter(n) "."; color: #6b7486; margin-right: 6px; }
    .lists i { font-style: normal; color: #8a93a6; font-size: 12px; }
    .lists time { color: #6b7486; font-size: 12px; float: right; }
    .res { display: inline-block; width: 16px; text-align: center; font-size: 11px; font-weight: 700; border-radius: 3px; background: #4a2a2a; color: #ffb3a8; margin-right: 4px; }
    .res.won { background: #23402b; color: #9be3a9; }
  </style>
</head>
<body>
  <canvas id="world"></canvas>
  <div id="fps"></div>
  @php($engine ??= 'webgl')
  {{-- Two builds of the same game, to compare: rally/ draws with hand-written WebGL, rally3d/
       with the three-d crate. Both share rally/src/sim.rs. --}}
  <div id="engine">drawn with {{ $engine === 'three-d' ? 'three-d' : 'WebGL' }} ·
    <a href="{{ $engine === 'three-d' ? '/rally' : '/rally-3d' }}">try {{ $engine === 'three-d' ? 'WebGL' : 'three-d' }}</a></div>

  {{-- The game (built into public/wasm/): it all runs here. Sounds are made here too, as it asks. --}}
  <script type="module">
    import init, * as game from '{{ $engine === 'three-d' ? '/wasm/rally3d/rally3d.js' : '/wasm/rally/rally.js' }}';
    let ready = false, waiting = null;
    window.rally = { play: (level) => ready ? game.play(level) : (waiting = level) };

    // Sounds, made up as needed: a hollow tock for the table, a sharper one for a paddle.
    let audio;
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
    const play = (kind, volume) => {
      audio ??= new AudioContext();
      if (audio.state === 'suspended') audio.resume();
      const t = audio.currentTime;
      const v = Math.max(0.05, volume);
      if (kind === 0) { tone(t, 1900, 1200, 0.035, 0.32 * v); tone(t, 420, 300, 0.05, 0.25 * v, 'triangle'); }
      if (kind === 1) { tone(t, 1300, 700, 0.03, 0.5 * v, 'triangle'); tone(t, 260, 180, 0.06, 0.35 * v); }
      if (kind === 2) tone(t, 220, 120, 0.08, 0.25 * v, 'triangle');
      if (kind === 3) tone(t, 700, 450, 0.04, 0.15 * v, 'triangle');
      if (kind === 4) { tone(t, 660, 660, 0.09, 0.12); tone(t + 0.09, 990, 990, 0.16, 0.12); }
      if (kind === 5) { tone(t, 330, 300, 0.12, 0.1); tone(t + 0.1, 247, 220, 0.2, 0.1); }
      if (kind === 6) tone(t, 520, 520, 0.12, 0.08);
    };
    window.addEventListener('rally-sound', (e) => play(...e.detail));

    // Frames a second, counted over each half second, and the game's own work per frame: its
    // average and the slowest, in milliseconds (the CPU's share; the screen caps the FPS).
    const fps = document.getElementById('fps');
    let frames = 0, since = performance.now();
    const count = (now) => {
      frames++;
      if (now - since >= 500) {
        const [n, average, worst] = ready ? game.stats() : [0, 0, 0];
        const work = n ? ` · ${average.toFixed(2)} ms, worst ${worst.toFixed(1)}` : '';
        fps.textContent = `${Math.round(frames * 1000 / (now - since))} FPS${work}`;
        frames = 0;
        since = now;
      }
      requestAnimationFrame(count);
    };
    requestAnimationFrame(count);

    await init();
    game.start('world');
    ready = true;

    // The logo on the table: its SVG drawn onto a canvas for the game to take.
    const logo = new Image();
    logo.onload = () => {
      const c = document.createElement('canvas');
      c.width = c.height = 512;
      c.getContext('2d').drawImage(logo, 0, 0, 512, 512);
      game.logo(c);
    };
    logo.src = '/images/rally-logo.svg';
    if (waiting !== null) game.play(waiting);
  </script>

  {{-- Results are shared over Reverb: needs UNSCRIPTED_TRANSPORT=reverb and `php artisan reverb:start`. --}}
  @unscripted('rally')
  <script type="text/php" src="/unscripted/ui.php"></script>
  <script src="{{ config('unscripted.runtime') }}" data-php="{{ config('unscripted.pack') }}"></script>
</body>
</html>
