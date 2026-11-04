<?php
use Illuminate\Support\Facades\DB;
use Unscripted\Ui\{Component, Live, On, Server, State, Validate, Watch};
use Unscripted\UiServer\Live as Changed;
use Unscripted as U;

/*
 * Rally's scoreboard, start screen and results. The game is Rust (rally/, drawn with WebGL) and
 * runs here in the browser, computer and all; this hears what happens through its CustomEvents
 * (#[On]). Results go to the server when a match ends, and everyone's list of matches updates
 * at once over Reverb (#[Live]).
 */
new class extends Component {
    const LEVELS = ['Easy', 'Normal', 'Hard'];

    #[State] public string $name = '';
    #[State] public int $level = 1;
    public ?string $error = null;
    /** From the game: phase, score, games, server, why, last (who won the last point), paused. */
    public ?array $game = null;
    /** Your last match's result: games and points, yours first. */
    public ?array $result = null;
    /** From the server: recent matches, and who's won most. */
    public array $board = ['recent' => [], 'best' => []];

    // ---- On the server -------------------------------------------------------------------

    // Loaded on mount, and again whenever anyone's match is recorded.
    #[Server, Watch, Live('rally')]
    public static function board(): array
    {
        $recent = DB::table('rally_matches')->latest('id')->limit(8)->get()
            ->map(fn($m) => [
                'id' => $m->id,
                'name' => $m->name,
                'level' => $m->level,
                'won' => (bool) $m->won,
                'games' => "{$m->games_won}–{$m->games_lost}",
                'ago' => \Illuminate\Support\Carbon::parse($m->created_at)->diffForHumans(short: true),
            ])->all();
        $best = DB::table('rally_matches')
            ->select('name', DB::raw('sum(won) as wins'), DB::raw('sum(case when won and level = 2 then 1 else 0 end) as hard'), DB::raw('count(*) as played'))
            ->groupBy('name')->orderByDesc('hard')->orderByDesc('wins')->limit(5)->get()
            ->map(fn($b) => ['name' => $b->name, 'wins' => (int) $b->wins, 'hard' => (int) $b->hard, 'played' => (int) $b->played])->all();
        return ['recent' => $recent, 'best' => $best];
    }

    /** A match played: the game runs in the browser, so this takes its word for the score. */
    #[Server]
    public static function record(
        #[Validate('required|max:16')] string $name,
        #[Validate('integer|between:0,2')] int $level,
        #[Validate('integer|between:0,2')] int $gamesWon,
        #[Validate('integer|between:0,2')] int $gamesLost,
        #[Validate('integer|between:0,200')] int $pointsWon,
        #[Validate('integer|between:0,200')] int $pointsLost,
    ): void {
        DB::table('rally_matches')->insert([
            'name' => trim($name),
            'level' => $level,
            'won' => $gamesWon > $gamesLost,
            'games_won' => $gamesWon,
            'games_lost' => $gamesLost,
            'points_won' => $pointsWon,
            'points_lost' => $pointsLost,
            'created_at' => now(),
            'updated_at' => now(),
        ]);
        Changed::changed('rally');
    }

    // ---- In the browser -------------------------------------------------------------------

    public function pick(int $level): void
    {
        $this->level = $level;
    }

    public function play(): void
    {
        if (trim($this->name) === '') {
            $this->error = 'Pick a name, for the results.';
            return;
        }
        $this->error = null;
        $this->result = null;
        U::window()->rally->play($this->level);
    }

    #[On('rally-state')]
    public function changed(string $json): void
    {
        $this->game = json_decode($json, true);
    }

    #[On('rally-end')]
    public function ended(string $json): Generator
    {
        ['games' => $games, 'points' => $points] = json_decode($json, true);
        $this->result = ['games' => $games, 'points' => $points];
        yield static::record($this->name, $this->level, $games[0], $games[1], $points[0], $points[1]);
    }

    /** The line under the score: whose serve, what just happened. */
    private function message(): ?string
    {
        $g = $this->game;
        if ($g === null || in_array($g['phase'], ['idle', 'over'], true)) {
            return null;
        }
        if ($g['paused']) {
            return 'Paused · click the table to play';
        }
        $to = fn() => $g['last'] === 0 ? 'your point' : 'their point';
        return match ($g['phase']) {
            'ready' => $g['why'] === 'Let' && $g['server'] === 0 ? 'Let · serve again'
                : ($g['server'] === 0 ? 'Your serve · click to toss, then swing up through it' : 'Their serve'),
            'point' => match ($g['why']) {
                'Missed' => $g['last'] === 0 ? 'Winner! Your point' : 'Missed · their point',
                'Out' => 'Out · ' . $to(),
                'Net' => 'Into the net · ' . $to(),
                'Fault' => 'Fault · ' . $to(),
                default => ucfirst($to()),
            },
            'game' => ($g['last'] === 0 ? 'You take the game ' : 'They take the game ') . max($g['score']) . '–' . min($g['score']),
            default => null,
        };
    }
};
?>
@php($phase = $game['phase'] ?? 'idle')
@if (!in_array($phase, ['idle'], true))
    <div class="score">
        <div class="side">
            <span class="who">You @if (($game['server'] ?? 1) === 0 && $phase !== 'over') <i class="serve"></i> @endif</span>
            <b>{{ $game['score'][0] }}</b>
            <span class="games">@for ($i = 0; $i < 2; $i++)<i @class(['won' => $i < $game['games'][0]])></i>@endfor</span>
        </div>
        <div class="side cpu">
            <span class="who">@if (($game['server'] ?? 0) === 1 && $phase !== 'over') <i class="serve"></i> @endif Computer · {{ self::LEVELS[$level] }}</span>
            <b>{{ $game['score'][1] }}</b>
            <span class="games">@for ($i = 0; $i < 2; $i++)<i @class(['won' => $i < $game['games'][1]])></i>@endfor</span>
        </div>
    </div>
@endif
@if ($line = $this->message())
    <p class="message" u-key="{{ $line }}">{{ $line }}</p>
@endif

@if ($phase === 'idle' || $phase === 'over')
    <div class="card">
        @if ($phase === 'over' && $result)
            @php($won = $result['games'][0] > $result['games'][1])
            <h1 @class(['won' => $won])>{{ $won ? 'You win' : 'Computer wins' }} {{ max($result['games']) }}–{{ min($result['games']) }}</h1>
            <p class="sub">Points {{ $result['points'][0] }}–{{ $result['points'][1] }} on {{ self::LEVELS[$level] }}</p>
        @else
            <h1 class="title">RALLY</h1>
            <p class="sub">Table tennis against the computer. Games to 11, best of three.</p>
        @endif
        <form u-on:submit.prevent="play">
            <input u-model="name" placeholder="Your name" maxlength="16" autocomplete="off">
            <div class="levels">
                @foreach (self::LEVELS as $i => $label)
                    <button type="button" u-on:click="pick({{ $i }})" @class(['picked' => $i === $level])>{{ $label }}</button>
                @endforeach
            </div>
            <button class="go">{{ $phase === 'over' ? 'Play again' : 'Play' }}</button>
        </form>
        @if ($error) <p class="error">{{ $error }}</p> @endif
        <p class="how">Move the mouse to move your paddle, up toward the net. Get near the ball and your paddle meets it; swing up through it for pace, sideways to place it. Hold the right button for backspin. Esc pauses.</p>

        @if ($board['recent'])
            <div class="lists">
                <div>
                    <h2>Recent matches</h2>
                    <ul>
                        @foreach ($board['recent'] as $m)
                            <li u-key="{{ $m['id'] }}"><span @class(['res', 'won' => $m['won']])>{{ $m['won'] ? 'W' : 'L' }}</span> {{ $m['name'] }} <i>{{ $m['games'] }} · {{ self::LEVELS[$m['level']] }}</i> <time>{{ $m['ago'] }}</time></li>
                        @endforeach
                    </ul>
                </div>
                <div>
                    <h2>Most wins</h2>
                    <ol>
                        @foreach ($board['best'] as $b)
                            <li u-key="{{ $b['name'] }}">{{ $b['name'] }} <b>{{ $b['wins'] }}</b> <i>of {{ $b['played'] }}@if ($b['hard']) · {{ $b['hard'] }} on Hard @endif</i></li>
                        @endforeach
                    </ol>
                </div>
            </div>
        @endif
    </div>
@endif
