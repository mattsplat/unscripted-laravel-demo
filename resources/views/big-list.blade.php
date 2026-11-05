<!doctype html>
<html lang="en">
<head>
  <meta charset="utf-8">
  <meta name="viewport" content="width=device-width, initial-scale=1">
  <meta name="csrf-token" content="{{ csrf_token() }}">
  <title>Big list · Unscripted</title>
  <style>
    body { margin: 0; font: 14px/1.4 "Avenir Next", "Segoe UI", "Helvetica Neue", sans-serif; color: #1b1f27; background: #f7f7f5; }
    main { max-width: 1000px; margin: 0 auto; padding: 32px 20px 80px; }
    h1 { margin: 0 0 4px; font-size: 28px; letter-spacing: -.02em; }
    .intro { margin: 0 0 20px; color: #5d6573; max-width: 680px; }
    .intro a { color: #c97a12; }
    .controls { display: flex; gap: 10px; flex-wrap: wrap; align-items: center; margin-bottom: 10px; }
    .controls input { flex: 1; min-width: 240px; font-size: 15px; padding: 8px 10px; border: 1px solid #d5d7dc; border-radius: 8px; }
    .controls select { font-size: 14px; padding: 6px 8px; border: 1px solid #d5d7dc; border-radius: 8px; background: #fff; }
    .controls label { color: #5d6573; display: flex; gap: 6px; align-items: center; }
    .summary { color: #5d6573; margin: 6px 0 10px; }
    table { width: 100%; border-collapse: collapse; background: #fff; border: 1px solid #e8e9ec; border-radius: 10px; overflow: hidden; }
    th { position: sticky; top: 0; background: #fbfbfa; text-align: left; font-weight: 600; padding: 8px 12px; border-bottom: 1px solid #e8e9ec; cursor: pointer; user-select: none; white-space: nowrap; }
    th.sorted { color: #c97a12; }
    td { padding: 6px 12px; border-bottom: 1px solid #f0f0ee; }
    .num { text-align: right; font-variant-numeric: tabular-nums; }
    .status { font-size: 12px; padding: 2px 8px; border-radius: 99px; background: #eee; }
    .status.active { background: #e3f4e8; color: #1d6b36; }
    .status.trial { background: #e6effc; color: #1f4f99; }
    .status.paused { background: #fdf3e3; color: #8a5208; }
    .status.churned { background: #fbe7e7; color: #9b2222; }

    /* How long each change took, measured in the page. */
    #meter { position: fixed; right: 16px; bottom: 16px; background: #1b1f27; color: #e8ebf1; border-radius: 10px; padding: 10px 14px; font: 12px/1.5 "SF Mono", Menlo, monospace; min-width: 210px; box-shadow: 0 10px 30px -10px rgba(0,0,0,.5); }
    #meter b { color: #e8b04a; font-weight: 600; }
    #meter .dim { color: #8f98a9; }
  </style>
</head>
<body>
<main>
  <p><a href="/examples" style="color:#5d6573;text-decoration:none">← Examples</a></p>
  <h1>Big list</h1>
  <p class="intro">Thousands of rows, fetched from Laravel once. Searching, filtering and sorting
    run in the browser, in PHP, with no request. The box in the corner times each change, from
    the input to the table being updated and painted. Compare <code>wire:model.live</code>,
    which sends a request per keystroke and gets the table's HTML back.</p>

  @unscripted('big-list')
</main>

<div id="meter"><span class="dim">Type, filter or sort to time it.</span></div>

<script>
  // From an input event to the DOM being patched (PHP ran, rendered and morphed), then to the
  // next paint. The median is over the last 20 changes.
  (() => {
    const meter = document.getElementById('meter');
    const times = [];
    let started = 0;
    const start = () => { started = performance.now(); };
    ['input', 'change', 'click'].forEach((type) => document.addEventListener(type, start, true));

    new MutationObserver(() => {
      if (!started) return;
      const t0 = started;
      started = 0;
      const patched = performance.now() - t0;
      requestAnimationFrame(() => setTimeout(() => {
        const painted = performance.now() - t0;
        times.push(painted);
        if (times.length > 20) times.shift();
        const median = [...times].sort((a, b) => a - b)[Math.floor(times.length / 2)];
        const rows = document.querySelectorAll('.big-list tbody tr').length;
        meter.innerHTML =
          `updated <b>${patched.toFixed(1)} ms</b><br>painted <b>${painted.toFixed(1)} ms</b><br>` +
          `<span class="dim">median ${median.toFixed(1)} ms · ${rows.toLocaleString()} rows in the DOM</span>`;
      }));
    }).observe(document.querySelector('main'), { subtree: true, childList: true, characterData: true, attributes: true });
  })();
</script>
<script type="text/php" src="/unscripted/ui.php"></script>
<script src="{{ config('unscripted.runtime') }}" data-php="{{ config('unscripted.pack') }}"></script>
</body>
</html>
