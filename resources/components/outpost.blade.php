<?php
use Unscripted\Ui\{Component, On, State};
use Unscripted as U;

/*
 * Outpost's HUD: joining, health, the kill feed, the scores. The game is Rust (outpost/, drawn
 * with WebGL), and talks to its own game server (outpost/server) over its own socket: that's
 * where moves and hits are decided. This hears what it says through the game's CustomEvents
 * (#[On]) and draws the rest of the screen. No server calls of its own.
 */
new class extends Component {
    /** By team number: the CSS class, and the name. */
    const TEAMS = [['raiders', 'Raiders'], ['guards', 'Guards']];
    /** By gun number: its kill-feed icon. */
    const GUNS = ['rifle', 'pistol', 'sniper'];
    /** The bots' levels, for the whole match: anyone can change it, and everyone sees. */
    const LEVELS = ['Easy', 'Normal', 'Hard'];

    #[State] public string $name = '';
    public ?string $error = null;
    /** The game server's socket: connecting, open or closed. */
    public string $status = 'connecting';
    /** You, once in: id and team. */
    public ?array $me = null;
    public int $hp = 100;
    public bool $alive = true;
    /** Seconds left to look around after spawning: unseen, unhittable, and not moving or shooting. */
    public int $shield = 0;
    /** The bots' level (an index into LEVELS), from the game server. */
    public int $level = 1;
    /** Whether the game's paused (the mouse let go, with Esc): the time to change things. */
    public bool $paused = true;
    /** Everyone in, by id: name, team, kills, deaths, alive. */
    public array $players = [];
    /** Recent kills, newest last: killer, victim (name and team each), the gun, whether a headshot, when. */
    public array $feed = [];
    public bool $scores = false;
    /** When you were last hit (ms), for the red flash. */
    public float $hurtAt = 0;

    protected function mounted(): void
    {
        // The game may have said things (connected, the bots' level) before this was listening.
        U::window()->outpost->sync();
        // Kills leave the feed after 6 seconds; the hurt flash goes.
        U::setInterval(function () {
            $this->feed = array_values(array_filter($this->feed, fn($k) => self::now() - $k['at'] < 6000));
            $this->refresh();
        }, 1000);
    }

    public function join(): void
    {
        $name = trim($this->name);
        if ($name === '') { // the input stops at 16 letters, and the game server cuts any more
            $this->error = 'Pick a callsign.';
            return;
        }
        $this->error = null;
        U::window()->outpost->join($name);
    }

    public function bots(int $level): void
    {
        U::window()->outpost->level($level);
    }

    #[On('outpost-level')]
    public function levelChanged(array $level): void
    {
        $this->level = (int) $level[0];
    }

    #[On('outpost-paused')]
    public function pausedChanged(array $paused): void
    {
        $this->paused = (bool) $paused[0];
    }

    #[On('outpost-status')]
    public function connection(string $json): void
    {
        $this->status = json_decode($json);
    }

    #[On('outpost-welcome')]
    public function welcomed(string $json): void
    {
        $this->me = json_decode($json, true);
        [$this->hp, $this->alive] = [100, true];
    }

    #[On('outpost-roster')]
    public function roster(string $json): void
    {
        $this->players = array_column(json_decode($json, true), null, 'id');
    }

    #[On('outpost-hp')]
    public function health(array $health): void
    {
        [$hp, $alive, $shield] = $health + [2 => 0];
        if ($hp < $this->hp) {
            $this->hurtAt = self::now();
        }
        [$this->hp, $this->alive, $this->shield] = [(int) $hp, (bool) $alive, (int) $shield];
    }

    #[On('outpost-kill')]
    public function killed(string $json): void
    {
        $kill = json_decode($json, true);
        $who = fn($id) => $this->players[$id] ?? ['name' => '?', 'team' => 0];
        $this->feed[] = [
            'killer' => $who($kill['killer']),
            'victim' => $who($kill['victim']),
            'gun' => self::GUNS[$kill['gun'] ?? 0] ?? 'rifle',
            'head' => $kill['head'],
            'at' => self::now(),
        ];
        $this->feed = array_slice($this->feed, -5);
    }

    #[On('outpost-scores')]
    public function tab(array $held): void
    {
        $this->scores = (bool) $held[0];
    }

    /** Kills by team. */
    private function teamScores(): array
    {
        $scores = [0, 0];
        foreach ($this->players as $p) {
            $scores[$p['team']] += $p['kills'];
        }
        return $scores;
    }

    /** Players on `$team`, most kills first. */
    private function team(int $team): array
    {
        $players = array_filter($this->players, fn($p) => $p['team'] === $team);
        uasort($players, fn($a, $b) => [$b['kills'], $a['deaths']] <=> [$a['kills'], $b['deaths']]);
        return $players;
    }

    /** Now, in ms: a float, as the browser's PHP is 32-bit. */
    private static function now(): float
    {
        return microtime(true) * 1000;
    }
};
?>
@php($score = $this->teamScores())
<div class="score">
    <span class="t-raiders">{{ $score[0] }}</span>
    <span class="teams">Raiders <i>vs</i> Guards</span>
    <span class="t-guards">{{ $score[1] }}</span>
</div>

<ul class="feed">
    @foreach ($feed as $kill)
        <li u-key="{{ $kill['at'] }}">
            <span class="t-{{ self::TEAMS[$kill['killer']['team']][0] }}">{{ $kill['killer']['name'] }}</span>
            <span class="{{ $kill['gun'] }}"></span>
            @if ($kill['head']) <span class="head">◎</span> @endif
            <span class="t-{{ self::TEAMS[$kill['victim']['team']][0] }}">{{ $kill['victim']['name'] }}</span>
        </li>
    @endforeach
</ul>

@if ($status === 'closed')
    <div class="join">
        <h1>OUTPOST</h1>
        <p class="error">Can't reach the game server. Start it with <code>npm run outpost:server</code>, then reload.</p>
    </div>
@elseif ($me === null)
    <div class="join">
        <h1>OUTPOST</h1>
        <p>Team deathmatch. You'll join the smaller side.</p>
        <form u-on:submit.prevent="join">
            <input u-model="name" placeholder="Callsign" maxlength="16" autofocus>
            <button>Deploy</button>
        </form>
        @if ($error) <p class="error">{{ $error }}</p> @endif
        @include('outpost-bots', ['levels' => self::LEVELS])
    </div>
@else
    @php([$class, $label] = self::TEAMS[$me['team']])
    <div @class(['vitals', 'low' => $hp <= 25])>
        <span class="plus">✚</span><b>{{ $hp }}</b>
        <span class="you t-{{ $class }}">{{ $players[$me['id']]['name'] ?? $name }} · {{ $label }}</span>
    </div>
    @if (!$alive)
        <div class="dead">You were killed. Back in a moment…</div>
    @elseif ($shield > 0)
        <div class="looking" u-key="look-{{ $shield }}">Look around · <b>{{ $shield }}</b><small>Unseen. You can't move or shoot yet.</small></div>
    @endif
    @if ($paused)
        <div class="paused">
            <p>Paused · click to play</p>
            @include('outpost-bots', ['levels' => self::LEVELS])
        </div>
    @endif
    @if (self::now() - $hurtAt < 600)
        <div class="hurt" u-key="{{ $hurtAt }}"></div>
    @endif
@endif

@if ($scores)
    <div class="board">
        @foreach (self::TEAMS as $team => [$class, $label])
            <table class="t-{{ $class }}">
                <tr><th>{{ $label }}</th><th>{{ $score[$team] }}</th><th>K</th><th>D</th></tr>
                @foreach ($this->team($team) as $id => $p)
                    <tr u-key="{{ $id }}" @class(['me' => $id === ($me['id'] ?? null), 'down' => !$p['alive']])>
                        <td colspan="2">{{ $p['name'] }}</td><td>{{ $p['kills'] }}</td><td>{{ $p['deaths'] }}</td>
                    </tr>
                @endforeach
            </table>
        @endforeach
    </div>
@endif
