<!doctype html>
<html lang="en">
<head>
  <meta charset="utf-8">
  <meta name="viewport" content="width=device-width, initial-scale=1">
  <title>{{ $title }} · Unscripted docs</title>
  <link rel="stylesheet" href="https://cdn.jsdelivr.net/gh/highlightjs/cdn-release@11.10.0/build/styles/atom-one-dark.min.css">
  <style>
    :root {
      --ink: #1b1f27; --dim: #5d6573; --faint: #9aa1ad; --line: #e8e9ec; --wash: #f7f7f5;
      --accent: #c97a12; --accent-soft: #fdf3e3; --code: #1d2230;
    }
    * { box-sizing: border-box; }
    html { scroll-padding-top: 80px; }
    body { margin: 0; color: var(--ink); background: #fff;
      font: 16px/1.7 "Avenir Next", "Segoe UI", "Helvetica Neue", sans-serif; }
    a { color: inherit; }

    /* The bar along the top. */
    .top { position: sticky; top: 0; z-index: 10; height: 60px; display: flex; align-items: center; gap: 28px;
      padding: 0 28px; background: rgba(255,255,255,.92); backdrop-filter: blur(8px); border-bottom: 1px solid var(--line); }
    .brand { display: flex; align-items: center; gap: 10px; font-weight: 700; font-size: 18px; text-decoration: none; letter-spacing: -.01em; }
    .brand i { width: 22px; height: 22px; border-radius: 6px; background: linear-gradient(135deg, #e8b04a, var(--accent)); display: inline-block;
      box-shadow: inset -5px -5px 0 rgba(0,0,0,.12); }
    .brand small { font-weight: 500; color: var(--faint); font-size: 14px; }
    .top nav { margin-left: auto; display: flex; gap: 22px; font-size: 14px; }
    .top nav a { text-decoration: none; color: var(--dim); }
    .top nav a:hover { color: var(--ink); }

    .wrap { display: grid; grid-template-columns: 260px minmax(0, 1fr) 220px; max-width: 1360px; margin: 0 auto; }

    /* The sidebar: every page, by section. */
    .side { position: sticky; top: 60px; height: calc(100vh - 60px); overflow-y: auto; padding: 28px 20px 40px 28px; border-right: 1px solid var(--line); }
    .side h4 { margin: 22px 0 6px; font-size: 12px; text-transform: uppercase; letter-spacing: .09em; color: var(--faint); font-weight: 600; }
    .side h4:first-child { margin-top: 0; }
    .side a { display: block; padding: 4px 10px; margin-left: -10px; border-radius: 6px; font-size: 14.5px; color: var(--dim); text-decoration: none; }
    .side a:hover { color: var(--ink); background: var(--wash); }
    .side a.here { color: var(--accent); background: var(--accent-soft); font-weight: 600; }

    /* The page. */
    main { padding: 44px 56px 80px; min-width: 0; }
    .crumb { font-size: 13px; color: var(--accent); font-weight: 600; letter-spacing: .02em; margin-bottom: 6px; }
    main h1 { font-size: 40px; line-height: 1.15; letter-spacing: -.025em; margin: 0 0 28px; }
    .prose { max-width: 760px; }
    .prose h2 { font-size: 26px; letter-spacing: -.015em; margin: 52px 0 14px; padding-top: 4px; }
    .prose h3 { font-size: 19px; margin: 34px 0 10px; }
    .prose .heading-permalink { margin-left: 8px; color: var(--faint); text-decoration: none; opacity: 0; font-weight: 400; }
    .prose h2:hover .heading-permalink, .prose h3:hover .heading-permalink { opacity: 1; }
    .prose p, .prose li { color: #333a46; }
    .prose a { color: var(--accent); text-decoration: underline; text-decoration-color: #ecc98f; text-underline-offset: 3px; }
    .prose a:hover { text-decoration-color: var(--accent); }
    .prose ul, .prose ol { padding-left: 22px; }
    .prose li { margin: 4px 0; }
    .prose blockquote { margin: 22px 0; padding: 12px 18px; border-left: 3px solid var(--accent); background: var(--accent-soft); border-radius: 0 8px 8px 0; }
    .prose blockquote p { margin: 4px 0; }
    .prose :not(pre) > code { font: 0.86em/1 "JetBrains Mono", "SF Mono", Menlo, monospace; background: #f1f0ec; padding: 2px 6px; border-radius: 5px; color: #7a4a0b; }
    .prose pre { margin: 22px 0; border-radius: 10px; overflow-x: auto; background: var(--code); box-shadow: 0 1px 0 rgba(0,0,0,.04), 0 6px 20px -10px rgba(20,25,40,.4); }
    .prose pre code { display: block; padding: 18px 22px; font: 14px/1.6 "JetBrains Mono", "SF Mono", Menlo, monospace; background: none !important; }
    .prose table { width: 100%; border-collapse: collapse; margin: 22px 0; font-size: 14.5px; }
    .prose th { text-align: left; font-weight: 600; border-bottom: 2px solid var(--line); padding: 8px 10px; }
    .prose td { border-bottom: 1px solid var(--line); padding: 8px 10px; vertical-align: top; }
    .prose hr { border: 0; border-top: 1px solid var(--line); margin: 40px 0; }

    .pager { max-width: 760px; display: grid; grid-template-columns: 1fr 1fr; gap: 16px; margin-top: 64px; }
    .pager a { display: block; padding: 14px 18px; border: 1px solid var(--line); border-radius: 10px; text-decoration: none; }
    .pager a:hover { border-color: var(--accent); }
    .pager small { display: block; color: var(--faint); font-size: 12px; text-transform: uppercase; letter-spacing: .08em; }
    .pager span { font-weight: 600; }
    .pager .next { grid-column: 2; text-align: right; }

    /* On this page. */
    .toc { position: sticky; top: 60px; height: calc(100vh - 60px); overflow-y: auto; padding: 44px 24px; font-size: 13.5px; }
    .toc h4 { margin: 0 0 10px; font-size: 12px; text-transform: uppercase; letter-spacing: .09em; color: var(--faint); font-weight: 600; }
    .toc a { display: block; padding: 3px 0 3px 12px; border-left: 2px solid var(--line); color: var(--dim); text-decoration: none; }
    .toc a:hover { color: var(--accent); border-left-color: var(--accent); }

    .menu { display: none; }
    @media (max-width: 1100px) { .wrap { grid-template-columns: 240px minmax(0, 1fr); } .toc { display: none; } }
    @media (max-width: 760px) {
      .wrap { display: block; } main { padding: 28px 20px 60px; }
      .side { display: none; position: static; height: auto; border: 0; border-bottom: 1px solid var(--line); }
      .side.open { display: block; }
      .menu { display: inline-block; margin-left: auto; font: inherit; font-size: 14px; background: none; border: 1px solid var(--line); border-radius: 6px; padding: 4px 10px; }
      .top nav { display: none; }
    }
  </style>
</head>
<body>
  <header class="top">
    <a class="brand" href="/docs"><i></i>Unscripted <small>docs</small></a>
    <nav>
      <a href="/docs/quickstart">Quickstart</a>
      <a href="/">Demos</a>
      <a href="https://github.com/mattsplat/unscripted-ui-server">GitHub</a>
    </nav>
    <button class="menu" onclick="document.querySelector('.side').classList.toggle('open')">Menu</button>
  </header>

  <div class="wrap">
    <aside class="side">
      <h4>Start here</h4>
      <a href="/docs" @class(['here' => $page === 'README'])>Introduction</a>
      @foreach ($nav as $section)
        <h4>{{ $section['title'] }}</h4>
        @foreach ($section['links'] as $link)
          <a href="{{ $link['href'] }}" @class(['here' => $link['href'] === "/docs/$page"])>{{ $link['title'] }}</a>
        @endforeach
      @endforeach
    </aside>

    <main>
      @php($section = collect($nav)->first(fn($s) => collect($s['links'])->contains('href', "/docs/$page")))
      <div class="crumb">{{ $section['title'] ?? 'Unscripted for Laravel' }}</div>
      <h1>{{ $title }}</h1>
      <article class="prose">{!! $html !!}</article>

      <nav class="pager">
        @if ($previous)
          <a href="{{ $previous['href'] }}"><small>Previous</small><span>{{ $previous['title'] }}</span></a>
        @endif
        @if ($next)
          <a class="next" href="{{ $next['href'] }}"><small>Next</small><span>{{ $next['title'] }}</span></a>
        @endif
      </nav>
    </main>

    <aside class="toc">
      @if ($toc)
        <h4>On this page</h4>
        @foreach ($toc as $item)
          <a href="#{{ $item['id'] }}">{{ $item['title'] }}</a>
        @endforeach
      @endif
    </aside>
  </div>

  <script src="https://cdn.jsdelivr.net/gh/highlightjs/cdn-release@11.10.0/build/highlight.min.js"></script>
  <script>
    // Blade's highlighted as PHP in HTML.
    document.querySelectorAll('pre code.language-blade').forEach((c) => c.className = 'language-php-template');
    hljs.highlightAll();
  </script>
</body>
</html>
