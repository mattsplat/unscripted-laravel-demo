<?php
use Illuminate\Support\Facades\Cache;
use Illuminate\Support\Facades\DB;
use Unscripted\Ui\{Component, Live, Server, State, Validate, Watch};
use Unscripted\UiServer\Live as Changed;
use Unscripted as U;

/*
 * Spread: everyone paints one 12×12 board in 60-second rounds. Your first cell can be anywhere;
 * after that, an empty cell must touch one of yours, and another player's cell must touch two.
 * Most cells when the time runs out wins.
 */
new class extends Component {
    const SIZE = 12;
    const COLORS = ['#e6194b', '#3cb44b', '#4363d8', '#f58231', '#911eb4', '#42d4f4', '#f032e6', '#9a6324'];

    #[State] public string $name = '';
    /** This player: id, name, color and the token the server knows them by. */
    public ?array $me = null;
    /** The board, from the server: cells ("x,y" => player id), players (by id), endsAt (ms). */
    public array $board = ['cells' => [], 'players' => [], 'endsAt' => null];

    // ---- On the server -------------------------------------------------------------------

    // Loaded on mount, after each of this page's moves, and whenever anyone's move changes it.
    #[Server, Watch, Live('spread')]
    public static function board(): array
    {
        $cells = [];
        foreach (DB::table('spread_cells')->get() as $cell) {
            $cells["$cell->x,$cell->y"] = $cell->player_id;
        }
        $players = DB::table('spread_players')
            ->where('created_at', '>', now()->subHour())
            ->orWhereIn('id', array_unique($cells))
            ->get(['id', 'name', 'color'])
            ->keyBy('id')
            ->map(fn($p) => ['name' => $p->name, 'color' => $p->color])
            ->all();
        return ['cells' => $cells, 'players' => $players, 'endsAt' => Cache::get('spread.ends_at')];
    }

    #[Server]
    public static function enter(#[Validate('required|max:16')] string $name): array
    {
        $count = DB::table('spread_players')->count();
        $player = [
            'name' => trim($name),
            'color' => self::COLORS[$count % count(self::COLORS)],
            'token' => bin2hex(random_bytes(16)),
            'created_at' => now(),
            'updated_at' => now(),
        ];
        $player['id'] = DB::table('spread_players')->insertGetId($player);
        Changed::changed('spread');
        return array_intersect_key($player, array_flip(['id', 'name', 'color', 'token']));
    }

    /** Starts a round, unless one's going: the board is cleared. */
    #[Server]
    public static function begin(): void
    {
        if (self::running(Cache::get('spread.ends_at'))) {
            return;
        }
        DB::table('spread_cells')->delete();
        Cache::forever('spread.ends_at', (int) self::now() + 60_000);
        Changed::changed('spread');
    }

    /** Claims cell x,y for the player with `$token`, if the rules allow: whether it did. */
    #[Server]
    public static function take(
        string $token,
        #[Validate('integer|between:0,11')] int $x,
        #[Validate('integer|between:0,11')] int $y,
    ): bool {
        $player = DB::table('spread_players')->where('token', $token)->first() ?? abort(403, 'Join first');
        $now = self::now();
        // A short cooldown, so holding the mouse down isn't a strategy.
        if (!self::running(Cache::get('spread.ends_at')) || $now - $player->moved_at < 150) {
            return false;
        }
        if (!self::allowed(self::board()['cells'], $player->id, $x, $y)) {
            return false;
        }
        DB::table('spread_cells')->upsert(['x' => $x, 'y' => $y, 'player_id' => $player->id], ['x', 'y'], ['player_id']);
        DB::table('spread_players')->where('id', $player->id)->update(['moved_at' => (int) $now]);
        Changed::changed('spread');
        return true;
    }

    // ---- In both: the rules, which the browser uses to show your moves -------------------

    /** Whether player `$id` may claim x,y on `$cells`. */
    private static function allowed(array $cells, int $id, int $x, int $y): bool
    {
        $owner = $cells["$x,$y"] ?? null;
        if ($owner === $id) {
            return false;
        }
        if ($owner === null && !in_array($id, $cells, true)) {
            return true; // your first cell
        }
        $mine = 0;
        foreach ([[1, 0], [-1, 0], [0, 1], [0, -1]] as [$dx, $dy]) {
            $mine += ($cells[($x + $dx) . ',' . ($y + $dy)] ?? null) === $id ? 1 : 0;
        }
        return $mine >= ($owner === null ? 1 : 2);
    }

    private static function running(int|float|null $endsAt): bool
    {
        return $endsAt !== null && $endsAt > self::now();
    }

    /** Now, in ms. A float: the browser's PHP is 32-bit, where an int stops at about 2.1 billion
     * (and a timestamp in ms from JSON arrives as a float). */
    private static function now(): float
    {
        return microtime(true) * 1000;
    }

    // ---- In the browser -------------------------------------------------------------------

    protected function mounted(): void
    {
        // Re-render for the countdown. Loaders don't run again unless what they watch changed.
        U::setInterval(fn() => $this->refresh(), 250);
    }

    public function join(): Generator
    {
        $this->me = yield static::enter($this->name);
    }

    public function start(): Generator
    {
        yield static::begin();
    }

    public function claim(int $x, int $y): Generator
    {
        if (!$this->canClaim($x, $y)) {
            return;
        }
        // Shown at once; the board loaded after the call says whether the server agreed.
        $this->board['cells']["$x,$y"] = $this->me['id'];
        yield static::take($this->me['token'], $x, $y);
    }

    private function canClaim(int $x, int $y): bool
    {
        return $this->me !== null && $this->secondsLeft() > 0 && self::allowed($this->board['cells'], $this->me['id'], $x, $y);
    }

    private function secondsLeft(): int
    {
        return max(0, (int) ceil((($this->board['endsAt'] ?? 0) - self::now()) / 1000));
    }

    /** Players with their cell counts, most first. */
    private function scores(): array
    {
        $counts = array_count_values($this->board['cells']);
        $scores = [];
        foreach ($this->board['players'] as $id => $player) {
            $scores[] = [...$player, 'id' => $id, 'cells' => $counts[$id] ?? 0];
        }
        usort($scores, fn($a, $b) => $b['cells'] <=> $a['cells']);
        return $scores;
    }
};
?>
@php($left = $this->secondsLeft())
@php($scores = $this->scores())
<header>
    @if ($left > 0)
        <strong class="timer">{{ $left }}s</strong>
    @elseif ($board['endsAt'] !== null && $scores !== [] && $scores[0]['cells'] > 0)
        <strong>Round over: <span style="color: {{ $scores[0]['color'] }}">{{ $scores[0]['name'] }}</span> wins with {{ $scores[0]['cells'] }} cells</strong>
    @else
        <strong>No round going</strong>
    @endif
    @if ($left === 0)
        <button u-on:click="start" @disabled($this->loading('start'))>New round</button>
    @endif
</header>

@if ($me === null)
    <form u-on:submit.prevent="join">
        <input u-model="name" placeholder="Your name" maxlength="16">
        <button @disabled($this->loading('join'))>Join</button>
    </form>
    @error('name') <p class="error">{{ $message }}</p> @enderror
@else
    <p>You're <strong style="color: {{ $me['color'] }}">{{ $me['name'] }}</strong>. Outlined cells are the ones you can take.</p>
@endif

<div class="board">
    @for ($y = 0; $y < self::SIZE; $y++)
        @for ($x = 0; $x < self::SIZE; $x++)
            @php($owner = $board['cells']["$x,$y"] ?? null)
            <button
                u-key="{{ $x }},{{ $y }}"
                u-on:click="claim({{ $x }}, {{ $y }})"
                @class(['cell', 'open' => $this->canClaim($x, $y)])
                style="background: {{ $owner === null ? '#f2f2f2' : $board['players'][$owner]['color'] ?? '#999' }}"
                aria-label="{{ $x }},{{ $y }}"
            ></button>
        @endfor
    @endfor
</div>

<ol class="scores">
    @foreach ($scores as $player)
        <li u-key="{{ $player['id'] }}"><span class="swatch" style="background: {{ $player['color'] }}"></span>{{ $player['name'] }} <b>{{ $player['cells'] }}</b></li>
    @endforeach
</ol>
