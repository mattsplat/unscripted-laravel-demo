@php
$code = <<<'BLADE'
<?php
use App\Models\Todo;
use Unscripted\Ui\{Component, Server, State};

new class extends Component {
    #[State] public string $draft = '';

    // Runs on your Laravel app, called like a function.
    #[Server]
    public static function create(string $title): void
    {
        Todo::create(['title' => $title]);
    }

    // Runs in the browser. `yield` waits for the server.
    public function add(): Generator
    {
        yield static::create($this->draft);
        $this->draft = '';
    }
};
?>

<form u-on:submit.prevent="add">
    <input u-model="draft" placeholder="What needs doing?">
    <button>Add</button>
</form>
BLADE;
@endphp
<!doctype html>
<html lang="en">
<head>
  <meta charset="utf-8">
  <meta name="viewport" content="width=device-width, initial-scale=1">
  <title>Unscripted · PHP components that run in the browser</title>
  <meta name="description" content="Livewire-style components in PHP and Blade, run in the browser by PHP compiled to WebAssembly.">
  <link rel="stylesheet" href="https://cdn.jsdelivr.net/gh/highlightjs/cdn-release@11.10.0/build/styles/atom-one-dark.min.css">
  <style>
    :root { --bg: #0b0e14; --panel: #131822; --line: #232a37; --text: #e8ebf1; --dim: #8f98a9; --gold: #e8b04a; --amber: #c97a12; }
    * { box-sizing: border-box; }
    body { margin: 0; background: var(--bg); color: var(--text); line-height: 1.6;
      font-family: "Avenir Next", "Segoe UI", "Helvetica Neue", sans-serif; }
    a { color: inherit; }
    .page { max-width: 1160px; margin: 0 auto; padding: 0 28px; }

    .top { display: flex; align-items: center; height: 72px; gap: 28px; }
    .brand { display: flex; align-items: center; gap: 10px; font-weight: 700; font-size: 19px; text-decoration: none; }
    .brand i { width: 24px; height: 24px; border-radius: 7px; background: linear-gradient(135deg, var(--gold), var(--amber)); box-shadow: inset -5px -5px 0 rgba(0,0,0,.15); }
    .top nav { margin-left: auto; display: flex; gap: 26px; font-size: 15px; }
    .top nav a { text-decoration: none; color: var(--dim); }
    .top nav a:hover { color: var(--text); }

    .hero { display: grid; grid-template-columns: 1fr 1.1fr; gap: 56px; align-items: center; padding: 64px 0 88px; }
    .tag { display: inline-block; font-size: 13px; color: var(--gold); border: 1px solid #3a3222; background: #1a1610; padding: 4px 12px; border-radius: 99px; margin-bottom: 22px; }
    h1 { font-size: 54px; line-height: 1.06; letter-spacing: -.035em; margin: 0 0 22px; }
    h1 em { font-style: normal; background: linear-gradient(90deg, var(--gold), #f3cf86); -webkit-background-clip: text; background-clip: text; color: transparent; }
    .lede { font-size: 19px; color: var(--dim); margin: 0 0 32px; max-width: 520px; }
    .lede strong { color: var(--text); font-weight: 600; }
    .buttons { display: flex; gap: 12px; flex-wrap: wrap; }
    .btn { display: inline-block; padding: 12px 22px; border-radius: 9px; font-weight: 600; text-decoration: none; font-size: 16px; }
    .btn.primary { background: var(--gold); color: #15110a; }
    .btn.primary:hover { background: #f0c063; }
    .btn.ghost { border: 1px solid var(--line); }
    .btn.ghost:hover { border-color: var(--gold); }
    .install { margin-top: 28px; font: 13px/1.5 "JetBrains Mono", "SF Mono", Menlo, monospace; color: var(--dim); }
    .install code { color: var(--text); }

    .code { background: #161b26; border: 1px solid var(--line); border-radius: 14px; overflow: hidden; box-shadow: 0 30px 60px -30px rgba(0,0,0,.7); }
    .code .bar { display: flex; align-items: center; gap: 7px; padding: 12px 16px; border-bottom: 1px solid var(--line); font-size: 13px; color: var(--dim); }
    .code .bar b { width: 11px; height: 11px; border-radius: 50%; background: #2b3240; }
    .code .bar span { margin-left: 10px; }
    .code pre { margin: 0; overflow-x: auto; }
    .code pre code { display: block; padding: 18px 22px; font: 13.5px/1.6 "JetBrains Mono", "SF Mono", Menlo, monospace; background: none !important; }

    section { padding: 72px 0; border-top: 1px solid var(--line); }
    h2 { font-size: 34px; letter-spacing: -.02em; margin: 0 0 12px; }
    .sub { color: var(--dim); font-size: 17px; margin: 0 0 40px; max-width: 640px; }
    .steps { display: grid; grid-template-columns: repeat(3, 1fr); gap: 20px; counter-reset: step; }
    .steps div, .features div { background: var(--panel); border: 1px solid var(--line); border-radius: 12px; padding: 22px 22px 24px; }
    .steps div::before { counter-increment: step; content: counter(step); display: inline-grid; place-items: center; width: 28px; height: 28px; border-radius: 8px; background: #2a2214; color: var(--gold); font-weight: 700; font-size: 14px; margin-bottom: 14px; }
    .steps h3, .features h3 { margin: 0 0 6px; font-size: 18px; }
    .steps p, .features p { margin: 0; color: var(--dim); font-size: 15px; }
    .features { display: grid; grid-template-columns: repeat(3, 1fr); gap: 20px; }
    code.inline { font: 0.85em "JetBrains Mono", "SF Mono", Menlo, monospace; color: var(--gold); }

    .shots { display: grid; grid-template-columns: repeat(3, 1fr); gap: 20px; margin-bottom: 28px; }
    .shots a { display: block; border-radius: 12px; overflow: hidden; border: 1px solid var(--line); text-decoration: none; background: var(--panel); transition: transform .15s, border-color .15s; }
    .shots a:hover { transform: translateY(-3px); border-color: var(--gold); }
    .shots img { display: block; width: 100%; aspect-ratio: 16 / 10; object-fit: cover; }
    .shots span { display: block; padding: 12px 16px; font-weight: 600; }
    .shots small { color: var(--dim); font-weight: 400; margin-left: 6px; }

    .end { text-align: center; padding: 88px 0 96px; }
    .end .buttons { justify-content: center; }
    footer { border-top: 1px solid var(--line); padding: 28px 0 40px; color: var(--dim); font-size: 14px; display: flex; gap: 24px; }
    footer a { text-decoration: none; }
    footer a:hover { color: var(--text); }

    @media (max-width: 900px) {
      .hero, .steps, .features, .shots { grid-template-columns: 1fr; }
      h1 { font-size: 40px; } .hero { padding-top: 32px; }
      .top nav a:not(.keep) { display: none; }
    }
  </style>
</head>
<body>
<div class="page">
  <header class="top">
    <a class="brand" href="/"><i></i>Unscripted</a>
    <nav>
      <a class="keep" href="/docs">Docs</a>
      <a class="keep" href="/examples">Examples</a>
      <a href="https://github.com/mattsplat/unscripted-js">GitHub</a>
    </nav>
  </header>

  <div class="hero">
    <div>
      <span class="tag">For Laravel · PHP 8.4 in WebAssembly</span>
      <h1>PHP components that <em>run in the browser</em>.</h1>
      <p class="lede">Write components in PHP and Blade, the way you would with Livewire. Unscripted runs
        them <strong>in the browser</strong>, so clicks, typing and re-renders need no request.
        Only the methods you mark <code class="inline">#[Server]</code> reach your Laravel app.</p>
      <div class="buttons">
        <a class="btn primary" href="/docs/quickstart">Get started</a>
        <a class="btn ghost" href="/examples">See the examples</a>
      </div>
      <p class="install">$ <code>composer require unscripted/ui-server:dev-main</code></p>
    </div>
    <div class="code">
      <div class="bar"><b></b><b></b><b></b><span>resources/components/todos.blade.php</span></div>
      <pre><code class="language-php-template">{{ $code }}</code></pre>
    </div>
  </div>

  <section>
    <h2>How it works</h2>
    <p class="sub">One component file, two places it runs.</p>
    <div class="steps">
      <div><h3>The server renders it first</h3><p>The page arrives as HTML, loaders and all, so it's there before any PHP loads in the browser.</p></div>
      <div><h3>The browser takes over</h3><p>PHP 8.4, compiled to WebAssembly, runs the same class. Actions, bindings and re-renders happen right there.</p></div>
      <div><h3>The server, when you ask</h3><p>A <code class="inline">#[Server]</code> method's body stays on Laravel; the browser gets a stub that calls it, with validation and permissions.</p></div>
    </div>
  </section>

  <section>
    <h2>What's in the box</h2>
    <p class="sub">The parts of Livewire you'd reach for, working without the round trips.</p>
    <div class="features">
      <div><h3>Familiar</h3><p>Blade templates, <code class="inline">u-model</code>, <code class="inline">u-on:click</code>, events, nested components and lifecycle hooks.</p></div>
      <div><h3>Forms</h3><p>Laravel's validation rules, checked as you type in the browser and again on the server.</p></div>
      <div><h3>Loaders</h3><p>Server methods that fill a property on mount and run again when what they watch changes.</p></div>
      <div><h3>Live updates</h3><p>Change data on the server and every open page showing it updates, over Reverb or polling.</p></div>
      <div><h3>Streaming and uploads</h3><p>Text that arrives as it's made, and uploads with progress, kept safe until you store them.</p></div>
      <div><h3>Navigation</h3><p>Move between pages without loading PHP again, keeping what you choose across pages.</p></div>
    </div>
  </section>

  <section>
    <h2>Examples</h2>
    <p class="sub">Games in Rust and WebGL, with Unscripted components for their menus, scores and results; and smaller demos of the framework itself.</p>
    <div class="shots">
      <a href="/outpost"><img src="/images/screens/outpost.jpg" alt="Outpost" loading="lazy"><span>Outpost<small>team shooter</small></span></a>
      <a href="/rally-3d"><img src="/images/screens/rally-3d.jpg" alt="Rally" loading="lazy"><span>Rally<small>table tennis</small></span></a>
      <a href="/spread"><img src="/images/screens/spread.jpg" alt="Spread" loading="lazy"><span>Spread<small>multiplayer board</small></span></a>
    </div>
    <a class="btn ghost" href="/examples">All the examples →</a>
  </section>

  <section class="end">
    <h2>Build your first component</h2>
    <p class="sub" style="margin: 0 auto 32px;">A counter, then a todo list with a database, in ten minutes.</p>
    <div class="buttons">
      <a class="btn primary" href="/docs/quickstart">Read the quickstart</a>
      <a class="btn ghost" href="/docs/coming-from-livewire">Coming from Livewire</a>
    </div>
  </section>

  <footer>
    <span>Unscripted</span>
    <a href="/docs">Docs</a>
    <a href="/examples">Examples</a>
    <a href="https://github.com/mattsplat/unscripted-js">unscripted-js</a>
    <a href="https://github.com/mattsplat/unscripted-ui-server">ui-server</a>
  </footer>
</div>
<script src="https://cdn.jsdelivr.net/gh/highlightjs/cdn-release@11.10.0/build/highlight.min.js"></script>
<script>hljs.highlightAll();</script>
</body>
</html>
