<?php
use Unscripted\Ui\{Component, Computed, Server, ServerError, State, Url};
use Unscripted\UiServer\Upload;

// Exercises @unscripted/ui's features, for checking them in a browser (/features).
new class extends Component {
    #[State, Url(as: 'q')] public string $q = '';
    #[State] public string $lazyText = '';
    #[State] public string $slow = '';
    #[State] public array $form = ['amount' => 0, 'colors' => []];
    #[State] public bool $agree = false;
    public int $renders = 0;
    public int $clicks = 0;
    public bool $menu = false;
    public array $keys = [];
    public bool $polling = false;
    public int $polls = 0;
    public bool $inited = false;
    public ?string $failure = null;
    public string $napped = '';
    public int $told = 0;

    #[Server] public static function boom(): void { abort(403, 'Nope'); }
    #[Server] public static function nap(): string { usleep(600_000); return 'rested'; }
    #[Server] public static function story(string $topic): Generator
    {
        foreach (['Once', 'upon', 'a', 'time,', 'a', $topic, 'streamed.'] as $word) {
            usleep(200_000);
            yield "$word ";
        }
        return 'the end';
    }
    #[State] public ?array $photo = null;
    public string $kept = '';
    #[Server] public static function keep(array $photo): string
    {
        $upload = Upload::find($photo) ?? abort(422, "That upload isn't yours, or it's gone");
        return $upload->name . ', ' . $upload->size . ' bytes, stored at ' . $upload->store('features');
    }
    public function save(): Generator { $this->kept = yield static::keep($this->photo); }
    public string $story = '';
    public string $storyResult = '';
    public function tellStory(): Generator { $this->story = ''; $this->storyResult = yield static::story('server')->into('story'); }

    protected function rendered(): void { $this->renders++; }
    protected function failed(ServerError $e): void { $this->failure = "{$e->status} {$e->getMessage()}"; }
    #[Computed] public function shout(): string { return strtoupper($this->q); }

    public function key(string $key): void { $this->keys[] = $key; }
    public function toggleMenu(): void { $this->menu = !$this->menu; }
    public function close(): void { $this->menu = false; }
    public function click(): void { $this->clicks++; }
    public function poll(): void { $this->polls++; }
    public function togglePolling(): void { $this->polling = !$this->polling; }
    public function init(): void { $this->inited = true; }
    public function fail(): Generator { yield static::boom(); }
    public function rest(): Generator { $this->napped = yield static::nap(); }
    public function tell(): void { $this->dispatch('told', ['n' => ++$this->told]); }
    public function agreed(): void {}
};
?>
<p><input id="q" u-model="q"> <span id="shout">{{ $this->shout }}</span></p>
<p><input id="lazy" u-model.blur="lazyText"> <span id="lazy-out">{{ $lazyText }}</span></p>
<p><input id="slow" u-model.debounce.400ms="slow"> <span id="slow-out">{{ $slow }}</span></p>
<p><input id="amount" u-model.number="form.amount"> <span id="amount-out">{{ var_export($form['amount'], true) }}</span></p>
<p><select id="colors" multiple u-model="form.colors"><option>red</option><option>green</option><option>blue</option></select> <span id="colors-out">{{ implode(',', $form['colors']) }}</span></p>
<p><label><input id="agree" type="checkbox" u-model="agree" u-on:change="agreed"> agree</label> renders: <span id="renders">{{ $renders }}</span></p>
<p><input id="keys" u-on:keydown.enter='key("enter")' u-on:keydown.ctrl.k='key("ctrl-k")'> <span id="keys-out">{{ implode(',', $keys) }}</span></p>
<div u-on:keydown.window.escape="close"></div>
<div id="menu-wrap" u-on:click.outside="close"><button id="menu-btn" u-on:click="toggleMenu">menu</button>@if ($menu)<ul id="menu"><li>item</li></ul>@endif</div>
<p><button id="once" u-on:click.once="click">once</button> <button id="confirm" u-confirm="Sure?" u-on:click="click">confirm</button> <span id="clicks">{{ $clicks }}</span></p>
<p><button id="fail" u-on:click="fail">fail</button> <span id="failure">{{ $failure }}</span></p>
<p><button id="rest" u-on:click="rest">rest</button> <span id="resting" u-loading="rest">resting…</span> <span id="idle" u-loading.remove="rest">idle</span> <span id="napped">{{ $napped }}</span></p>
<p><button id="poll-toggle" u-on:click="togglePolling">poll</button> @if ($polling)<span u-poll.300ms="poll"></span>@endif <span id="polls">{{ $polls }}</span></p>
<p><span id="init" u-init="init">{{ $inited ? 'inited' : 'not yet' }}</span> <span id="offline" u-offline>offline</span></p>
<div id="ignored" u-ignore>from the server</div>
<p><button id="tell" u-on:click="tell">tell</button></p>
<p><input id="photo" type="file" u-model="photo">
    @if ($this->progress('photo') !== null) <span id="progress">{{ $this->progress('photo') }}%</span> @endif
    @if ($photo) <img id="preview" src="{{ $photo['url'] }}" height="24"> <button id="keep" u-on:click="save">keep</button> @endif
    @error('photo') <span id="photo-error">{{ $message }}</span> @enderror <span id="kept">{{ $kept }}</span></p>
<p><button id="story-btn" u-on:click="tellStory">story</button> <span id="story">{{ $story }}</span> <b id="story-result">{{ $storyResult }}</b></p>
