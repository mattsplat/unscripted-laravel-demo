<!doctype html>
<html lang="en">
<head>
  <meta charset="utf-8">
  <meta name="viewport" content="width=device-width, initial-scale=1">
  <title>Examples · Unscripted</title>
  <style>
    :root { --bg: #0b0e14; --card: #141923; --line: #232a37; --text: #e8ebf1; --dim: #8c95a6; --gold: #e8b04a; }
    * { box-sizing: border-box; }
    body { margin: 0; background: var(--bg); color: var(--text);
      font-family: "Avenir Next", "Segoe UI", "Helvetica Neue", sans-serif; line-height: 1.5; }
    main { max-width: 1080px; margin: 0 auto; padding: 56px 24px 72px; }
    header h1 { font-size: 44px; margin: 0 0 8px; letter-spacing: -.02em; }
    header h1 span { color: var(--gold); }
    header p { max-width: 640px; margin: 0; color: var(--dim); font-size: 18px; }
    header p a { color: var(--gold); }
    header .back { margin-bottom: 18px; font-size: 15px; }
    header .back a { color: var(--dim); text-decoration: none; }
    header .back a:hover { color: var(--text); }
    .cta { margin-top: 22px !important; display: flex; gap: 12px; }
    .cta a { display: inline-block; padding: 10px 18px; border-radius: 8px; background: var(--gold); color: #111; font-weight: 600; text-decoration: none; font-size: 15px; }
    .cta a.ghost { background: none; color: var(--text); border: 1px solid var(--line); }
    .cta a.ghost:hover { border-color: var(--gold); }
    h2 { font-size: 13px; letter-spacing: .14em; text-transform: uppercase; color: var(--dim); margin: 48px 0 16px; }
    .grid { display: grid; grid-template-columns: repeat(auto-fill, minmax(300px, 1fr)); gap: 20px; }
    .card { display: flex; flex-direction: column; background: var(--card); border: 1px solid var(--line);
      border-radius: 10px; overflow: hidden; color: inherit; text-decoration: none;
      transition: transform .15s, border-color .15s; }
    .card:hover { transform: translateY(-3px); border-color: var(--gold); }
    .card img { display: block; width: 100%; aspect-ratio: 16 / 10; object-fit: cover; background: #000; }
    .card div { padding: 14px 16px 18px; }
    .card h3 { margin: 0 0 4px; font-size: 20px; }
    .card h3 small { font-size: 12px; font-weight: normal; color: var(--gold); margin-left: 6px; }
    .card p { margin: 0; color: var(--dim); font-size: 14px; }
    .small { grid-template-columns: repeat(auto-fill, minmax(220px, 1fr)); }
    .small .card div { padding: 14px 16px; }
    footer { margin-top: 56px; color: var(--dim); font-size: 14px; }
    footer a { color: var(--text); }
  </style>
</head>
<body>
<main>
  <header>
    <p class="back"><a href="/">← Unscripted</a></p>
    <h1>Unscripted <span>examples</span></h1>
    <p>PHP components that run in the browser, compiled to WebAssembly, with games written in
      Rust around them. Pick one to play, or <a href="/docs">read the docs</a> and build your own.</p>
    <p class="cta"><a href="/docs">Read the docs</a> <a href="/docs/quickstart" class="ghost">Quickstart</a></p>
  </header>

  <h2>Games</h2>
  <section class="grid">
    @foreach ([
      ['/outpost', 'outpost', 'Outpost', 'Multiplayer', 'A team shooter. Play with friends, against bots or both.'],
      ['/rally', 'rally', 'Rally', 'vs computer', 'Table tennis with spin, against the computer.'],
      ['/rally-3d', 'rally-3d', 'Rally 3D', 'three-d', 'The same Rally, drawn with the three-d renderer.'],
      ['/voxel', 'voxel', 'Voxel', 'Multiplayer', 'A shared block world: build and dig together.'],
      ['/spread', 'spread', 'Spread', 'Multiplayer', 'Paint one board with everyone, in 60-second rounds.'],
    ] as [$href, $shot, $name, $tag, $about])
      <a class="card" href="{{ $href }}">
        <img src="/images/screens/{{ $shot }}.jpg" alt="{{ $name }}" loading="lazy">
        <div><h3>{{ $name }}<small>{{ $tag }}</small></h3><p>{{ $about }}</p></div>
      </a>
    @endforeach
  </section>

  <h2>Components</h2>
  <section class="grid small">
    @foreach ([
      ['/todos', 'Todos', 'Server loaders, validation and live updates.'],
      ['/counters', 'Counters', 'Nested components, props and events.'],
      ['/features', 'Features', 'Every framework feature on one page.'],
    ] as [$href, $name, $about])
      <a class="card" href="{{ $href }}"><div><h3>{{ $name }}</h3><p>{{ $about }}</p></div></a>
    @endforeach
  </section>

  <footer>Built with <a href="https://github.com/mattsplat/unscripted-js">Unscripted</a> on Laravel.</footer>
</main>
</body>
</html>
