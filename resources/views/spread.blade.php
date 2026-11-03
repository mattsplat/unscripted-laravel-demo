<!doctype html>
<html>
<head>
  <meta charset="utf-8">
  <meta name="csrf-token" content="{{ csrf_token() }}">
  <title>Spread</title>
  <style>
    body { font-family: sans-serif; max-width: 560px; margin: 32px auto; padding: 0 16px; color: #222; }
    header { display: flex; align-items: center; justify-content: space-between; min-height: 36px; }
    .timer { font-size: 24px; font-variant-numeric: tabular-nums; }
    form { display: flex; gap: 8px; margin: 12px 0; }
    form input { flex: 1; font-size: 16px; padding: 6px 8px; }
    button { font-size: 14px; padding: 4px 12px; }
    .error { color: #b00020; }
    .board { display: grid; grid-template-columns: repeat(12, 1fr); gap: 2px; margin: 12px 0; }
    .cell { aspect-ratio: 1; padding: 0; border: none; border-radius: 3px; cursor: default; transition: background .15s; }
    .cell.open { outline: 2px dashed #888; outline-offset: -3px; cursor: pointer; }
    .scores { padding-left: 20px; }
    .scores li { margin: 4px 0; }
    .swatch { display: inline-block; width: 12px; height: 12px; border-radius: 2px; margin-right: 6px; vertical-align: middle; }
  </style>
</head>
<body>
  <h1>Spread</h1>
  {{-- Multiplayer: needs UNSCRIPTED_TRANSPORT=reverb and `php artisan reverb:start`. --}}
  @unscripted('spread')
  <script type="text/php" src="/unscripted/ui.php"></script>
  <script src="/vendor/unscripted/runtime/auto.js" data-php="/vendor/unscripted/php84/php_pack.wasm"></script>
</body>
</html>
