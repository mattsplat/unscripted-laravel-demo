<!doctype html>
<html>
<head>
  <meta charset="utf-8">
  <meta name="csrf-token" content="{{ csrf_token() }}">
  <title>Voxel</title>
  <style>
    html, body { margin: 0; height: 100%; overflow: hidden; font-family: sans-serif; }
    #world { position: fixed; inset: 0; width: 100%; height: 100%; display: block; }
    .crosshair { position: fixed; left: 50%; top: 50%; width: 16px; height: 16px; margin: -8px 0 0 -8px; pointer-events: none;
      background: linear-gradient(#fff, #fff) center / 2px 100% no-repeat, linear-gradient(#fff, #fff) center / 100% 2px no-repeat; mix-blend-mode: difference; }
    .hud { position: fixed; top: 12px; left: 12px; padding: 10px 14px; background: rgba(255,255,255,.85); border-radius: 8px; min-width: 200px; }
    .hud p { margin: 4px 0; }
    .hud form { display: flex; gap: 6px; }
    .hud input { flex: 1; font-size: 15px; padding: 4px 6px; }
    .hint { color: #555; font-size: 13px; }
    .error { color: #b00020; font-size: 13px; }
    .players { list-style: none; padding: 0; margin: 6px 0 0; }
    .swatch { display: inline-block; width: 10px; height: 10px; border-radius: 2px; margin-right: 6px; }
    .blocks { position: fixed; bottom: 12px; left: 50%; transform: translateX(-50%); display: flex; gap: 6px; list-style: none; padding: 0; margin: 0; }
    .blocks li { background: rgba(255,255,255,.75); padding: 6px 10px; border-radius: 6px; font-size: 13px; }
    .blocks li.picked { background: #fff; outline: 3px solid #222; }
    .help { position: fixed; top: 12px; right: 12px; color: #fff; font-size: 13px; text-align: right; text-shadow: 0 1px 2px #000; }
  </style>
</head>
<body>
  <canvas id="world"></canvas>
  <div class="crosshair"></div>
  <div class="help">Click to look around · WASD to move · space to jump<br>Left click breaks · right click places · 1–7 picks a block · Esc lets go</div>

  {{-- The game (voxel/, built into public/wasm/voxel). window.voxel takes calls before it's
       loaded and passes them on once it has. --}}
  <script type="module">
    import init, * as game from '/wasm/voxel/voxel.js';
    const waiting = [];
    let ready = false;
    const call = (name, convert = (...args) => args) => (...args) =>
      ready ? game[name](...convert(...args)) : waiting.push([name, convert(...args)]);
    window.voxel = {
      load: call('load', (json) => [Int32Array.from(JSON.parse(json))]),
      setBlock: call('set_block'),
      setPlayer: call('set_player'),
      removePlayer: call('remove_player'),
    };
    await init();
    game.start('world');
    ready = true;
    for (const [name, args] of waiting) game[name](...args);
  </script>

  {{-- Multiplayer: needs UNSCRIPTED_TRANSPORT=reverb and `php artisan reverb:start`. --}}
  @unscripted('voxel')
  <script type="text/php" src="/unscripted/ui.php"></script>
  <script src="/vendor/unscripted/runtime/auto.js" data-php="/vendor/unscripted/php84/php_pack.wasm"></script>
</body>
</html>
