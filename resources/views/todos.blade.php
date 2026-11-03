<!doctype html>
<html>
<head>
  <meta charset="utf-8">
  <meta name="csrf-token" content="{{ csrf_token() }}">
  <title>@unscripted/ui + Laravel</title>
  <style>
    body { font-family: sans-serif; max-width: 520px; margin: 40px auto; padding: 0 16px; color: #222; }
    form { display: flex; gap: 8px; }
    input { font-size: 16px; padding: 6px 8px; }
    form input { flex: 1; }
    input[type=search] { width: 100%; box-sizing: border-box; margin-top: 12px; }
    button { font-size: 14px; padding: 4px 12px; border: 1px solid #888; border-radius: 6px; background: #f4f4f4; }
    ul { list-style: none; padding: 0; }
    li { display: flex; align-items: center; justify-content: space-between; padding: 6px 0; border-bottom: 1px solid #eee; }
    li.done label { text-decoration: line-through; color: #888; }
    .remove { border: none; background: none; font-size: 18px; color: #888; }
    .empty, footer { color: #666; }
    .error { color: #b00020; }
    footer { display: flex; justify-content: space-between; }
  </style>
</head>
<body>
  <h1>Todos</h1>
  <nav><a u-navigate id="to-features" href="/features">features</a></nav>
  <div u-persist="player" id="player">placeholder</div>
  {{-- Rendered here by unscripted/ui-server (loaders and all), then taken over in the browser.
       The #[Server] methods stay on the server. --}}
  @unscripted('todos')
  <script type="text/php" src="/unscripted/ui.php"></script>
  <script src="/vendor/unscripted/runtime/auto.js" data-php="/vendor/unscripted/php84/php_pack.wasm"></script>
</body>
</html>
