<!doctype html>
<html>
<head>
  <meta charset="utf-8">
  <meta name="csrf-token" content="{{ csrf_token() }}">
  <title>@unscripted/ui features</title>
</head>
<body>
  {{-- Checks @unscripted/ui's features in a browser. --}}
  <nav><a u-navigate id="to-counters" href="/counters">counters</a> <a u-navigate id="to-todos" href="/todos">todos</a></nav>
  <div u-persist="player" id="player">persisted</div>
  @unscripted('features')
  @unscripted('listener', ['id' => 'listener'])
  @unscripted('counters')
  @unscripted('counter', ['id' => 'lazy-counter', 'lazy' => true, 'placeholder' => '<p id="lazy-placeholder">Loading…</p>'])
  <script>
    window.toldInJs = [];
    window.addEventListener('told', (e) => window.toldInJs.push(e.detail.n));
  </script>
  <script type="text/php" src="/unscripted/ui.php"></script>
  <script src="{{ config('unscripted.runtime') }}" data-php="{{ config('unscripted.pack') }}"></script>
</body>
</html>
