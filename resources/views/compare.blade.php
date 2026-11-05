<!doctype html>
<html lang="en">
<head>
  <meta charset="utf-8">
  <meta name="viewport" content="width=device-width, initial-scale=1">
  <meta name="csrf-token" content="{{ csrf_token() }}">
  <title>Livewire and Unscripted, side by side · Unscripted</title>
  <script>
    // Before either framework loads: every call either makes to the server goes through here,
    // so both get the same added delay, and each is counted against the demo it came from.
    (() => {
      const latency = () => Number(localStorage.getItem('compare-latency') ?? 150);
      const counted = (side, names) => {
        for (const name of names) {
          const demo = String(name).replace(/^compare[.-]/, '');
          window.dispatchEvent(new CustomEvent('compare:request', { detail: { side, demo } }));
        }
      };
      const wait = () => new Promise((r) => setTimeout(r, latency()));

      const fetch = window.fetch;
      window.fetch = async (input, init = {}) => {
        const url = String(input?.url ?? input);
        if (/\/livewire-[^/]+\/update/.test(url)) {
          try { counted('livewire', JSON.parse(init.body).components.map((c) => JSON.parse(c.snapshot).memo.name)); } catch {}
          await wait();
        } else if (url.includes('/unscripted/call')) {
          try { counted('unscripted', JSON.parse(init.body).calls.map((c) => c.component)); } catch {}
          await wait();
        }
        return fetch(input, init);
      };

      // Unscripted's calls can go over Reverb's socket instead (UNSCRIPTED_TRANSPORT=reverb).
      const send = WebSocket.prototype.send;
      WebSocket.prototype.send = function (data) {
        if (typeof data === 'string' && data.includes('"unscripted:call"')) {
          try { counted('unscripted', JSON.parse(data).data.calls.map((c) => c.component)); } catch {}
          setTimeout(() => send.call(this, data), latency());
          return;
        }
        return send.call(this, data);
      };
    })();
  </script>
  <style>
    :root { --ink: #1b1f27; --dim: #5d6573; --faint: #9aa1ad; --line: #e8e9ec; --wash: #f7f7f5; --accent: #c97a12; --lw: #fb70a9; }
    * { box-sizing: border-box; }
    body { margin: 0; font: 15px/1.5 "Avenir Next", "Segoe UI", "Helvetica Neue", sans-serif; color: var(--ink); background: var(--wash); }
    main { max-width: 1100px; margin: 0 auto; padding: 28px 20px 80px; }
    .back { color: var(--dim); text-decoration: none; font-size: 14px; }
    h1 { margin: 10px 0 6px; font-size: 32px; letter-spacing: -.02em; }
    .intro { margin: 0 0 22px; color: var(--dim); max-width: 760px; }
    .intro a { color: var(--accent); }

    .bar { position: sticky; top: 0; z-index: 5; display: flex; flex-wrap: wrap; align-items: center; gap: 18px; padding: 12px 16px; margin-bottom: 26px;
      background: #fff; border: 1px solid var(--line); border-radius: 12px; box-shadow: 0 6px 20px -14px rgba(0,0,0,.35); }
    .bar label { font-weight: 600; display: flex; gap: 10px; align-items: center; }
    .bar input[type=range] { width: 220px; accent-color: var(--accent); }
    .bar .totals { margin-left: auto; color: var(--dim); font-size: 14px; display: flex; gap: 16px; }
    .bar .totals b { color: var(--ink); font-variant-numeric: tabular-nums; }
    .bar button { font: inherit; font-size: 13px; border: 1px solid var(--line); background: #fff; border-radius: 7px; padding: 4px 10px; cursor: pointer; }

    section.row { margin-bottom: 34px; }
    .row h2 { font-size: 20px; margin: 0 0 2px; }
    .row > p { margin: 0 0 12px; color: var(--dim); font-size: 14px; max-width: 760px; }
    .pair { display: grid; grid-template-columns: 1fr 1fr; gap: 16px; }
    .panel { background: #fff; border: 1px solid var(--line); border-radius: 12px; overflow: hidden; display: flex; flex-direction: column; }
    .panel header { display: flex; align-items: center; gap: 10px; padding: 10px 14px; border-bottom: 1px solid var(--line); font-size: 13px; color: var(--dim); }
    .panel header strong { color: var(--ink); font-size: 14px; }
    .panel header i { width: 10px; height: 10px; border-radius: 3px; display: inline-block; }
    .panel.livewire header i { background: var(--lw); }
    .panel.unscripted header i { background: var(--accent); }
    .panel header .stats { margin-left: auto; display: flex; gap: 12px; font-variant-numeric: tabular-nums; }
    .panel header .stats b { color: var(--ink); }
    .panel .body { padding: 16px; flex: 1; }

    .demo button, .demo select, .demo input { font: inherit; }
    .counter { display: flex; align-items: center; gap: 16px; justify-content: center; padding: 10px 0; }
    .counter button { width: 44px; height: 44px; font-size: 22px; border-radius: 10px; border: 1px solid var(--line); background: var(--wash); cursor: pointer; }
    .counter output { font-size: 34px; font-weight: 700; min-width: 70px; text-align: center; font-variant-numeric: tabular-nums; }
    .search input { width: 100%; padding: 8px 10px; border: 1px solid #d5d7dc; border-radius: 8px; }
    .search .meta { margin: 8px 0; color: var(--dim); font-size: 13px; }
    .search ul { list-style: none; margin: 0; padding: 0; height: 260px; overflow-y: auto; border-top: 1px solid var(--line); }
    .search li { display: flex; gap: 10px; padding: 5px 2px; border-bottom: 1px solid #f2f2f0; font-size: 14px; }
    .search li span { flex: 1; } .search li small { color: var(--faint); width: 70px; } .search li b { font-weight: 500; font-variant-numeric: tabular-nums; }
    .signup form { display: grid; gap: 6px; }
    .signup label { display: grid; gap: 3px; font-size: 13px; color: var(--dim); }
    .signup input, .signup select { padding: 7px 9px; border: 1px solid #d5d7dc; border-radius: 8px; color: var(--ink); }
    .signup .error { margin: 0 0 4px; color: #b42318; font-size: 13px; }
    .signup button { margin-top: 8px; padding: 9px; border: 0; border-radius: 8px; background: var(--ink); color: #fff; cursor: pointer; }
    .signup .done { color: #1d6b36; font-weight: 600; }

    .notes { margin-top: 40px; padding: 18px 20px; background: #fff; border: 1px solid var(--line); border-radius: 12px; color: var(--dim); font-size: 14px; }
    .notes h3 { margin: 0 0 6px; color: var(--ink); font-size: 16px; }
    .notes ul { margin: 0; padding-left: 20px; }
    @media (max-width: 760px) { .pair { grid-template-columns: 1fr; } .bar .totals { margin-left: 0; } }
  </style>
</head>
<body>
<main>
  <a class="back" href="/examples">← Examples</a>
  <h1>Livewire and Unscripted, side by side</h1>
  <p class="intro">The same three components written twice: with <a href="https://livewire.laravel.com">Livewire</a> on the
    left and Unscripted on the right, on the same page and the same Laravel app. Both are good ways to
    build a Laravel interface; they make different trade-offs about where the component runs. Add some
    network delay with the slider and use both sides to feel the difference.</p>

  <div class="bar">
    <label>Added delay per server call
      <input type="range" id="latency" min="0" max="600" step="50">
      <output id="latency-out"></output>
    </label>
    <div class="totals">
      <span>Livewire requests <b id="total-livewire">0</b></span>
      <span>Unscripted requests <b id="total-unscripted">0</b></span>
      <button id="reset">Reset counts</button>
    </div>
  </div>

  @foreach ([
    'counter' => ['Counter', 'Click quickly. Livewire runs the action on the server, so each click is a request. While one is on its way, Livewire 4 queues the next and merges identical ones (a guard against double clicks), so quick clicks on a slow connection can count once. Unscripted runs the action in the browser.'],
    'search' => ['Live search', 'Type to filter 500 products. Livewire filters on the server as you type. Unscripted fetches the list once and filters in the browser.'],
    'signup' => ['Form with live validation', 'The same Laravel rules on both sides. Livewire checks each field on the server as it changes. Unscripted checks them in the browser as you type, and the server checks them all again on submit.'],
  ] as $demo => [$title, $about])
    <section class="row">
      <h2>{{ $title }}</h2>
      <p>{{ $about }}</p>
      <div class="pair">
        <div class="panel livewire" data-side="livewire" data-demo="{{ $demo }}">
          <header><i></i><strong>Livewire</strong>
            <span class="stats"><span>requests <b data-requests>0</b></span><span>last update <b data-last>–</b></span></span></header>
          <div class="body">@livewire("compare.$demo")</div>
        </div>
        <div class="panel unscripted" data-side="unscripted" data-demo="{{ $demo }}">
          <header><i></i><strong>Unscripted</strong>
            <span class="stats"><span>requests <b data-requests>0</b></span><span>last update <b data-last>–</b></span></span></header>
          <div class="body">@unscripted("compare-$demo")</div>
        </div>
      </div>
    </section>
  @endforeach

  <div class="notes">
    <h3>Reading the numbers</h3>
    <ul>
      <li><b>Requests</b> counts calls each side made to the server after the page loaded. Both searches got their first results with the page; Unscripted's product list came with it too.</li>
      <li><b>Last update</b> is the time from your click or keystroke to that panel's HTML changing (from the oldest one still waiting, when you're quicker than the server).</li>
      <li>The delay is added in this page before each call is sent, the same for both, so it stands in for a slower network.</li>
      <li>Unscripted's first visit downloads PHP for the browser (about 1.5 MB, then cached); Livewire's pages are lighter to start.</li>
    </ul>
  </div>
</main>

<script>
  (() => {
    const slider = document.getElementById('latency');
    const out = document.getElementById('latency-out');
    slider.value = localStorage.getItem('compare-latency') ?? 150;
    const show = () => { out.textContent = `${slider.value} ms`; localStorage.setItem('compare-latency', slider.value); };
    slider.addEventListener('input', show);
    show();

    const totals = { livewire: 0, unscripted: 0 };
    const panel = (side, demo) => document.querySelector(`.panel[data-side="${side}"][data-demo="${demo}"]`);
    addEventListener('compare:request', ({ detail: { side, demo } }) => {
      totals[side]++;
      document.getElementById(`total-${side}`).textContent = totals[side];
      const counter = panel(side, demo)?.querySelector('[data-requests]');
      if (counter) counter.textContent = Number(counter.textContent) + 1;
    });
    document.getElementById('reset').addEventListener('click', () => {
      totals.livewire = totals.unscripted = 0;
      document.querySelectorAll('[data-requests]').forEach((b) => b.textContent = '0');
      document.querySelectorAll('[id^=total-]').forEach((b) => b.textContent = '0');
    });

    // From an interaction in a panel to its HTML changing.
    document.querySelectorAll('.panel').forEach((p) => {
      const body = p.querySelector('.body');
      const last = p.querySelector('[data-last]');
      let started = 0;
      ['input', 'click', 'change', 'submit'].forEach((type) => body.addEventListener(type, () => {
        const now = performance.now();
        // A submit starts afresh; so does a change when the last one is old enough that it
        // simply didn't alter the HTML (a valid field, say).
        if (!started || type === 'submit' || now - started > 3000) started = now;
      }, true));
      new MutationObserver(() => {
        if (!started) return;
        last.textContent = `${Math.round(performance.now() - started)} ms`;
        started = 0;
      }).observe(body, { subtree: true, childList: true, characterData: true });
    });
  })();
</script>
<script type="text/php" src="/unscripted/ui.php"></script>
<script src="{{ config('unscripted.runtime') }}" data-php="{{ config('unscripted.pack') }}"></script>
</body>
</html>
