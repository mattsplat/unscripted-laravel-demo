<?php
use Illuminate\Support\Facades\Cache;
use Illuminate\Support\Facades\DB;
use Unscripted\Ui\{Component, Live, On, Server, ServerError, State, Validate, Watch};
use Unscripted\UiServer\Live as Push;
use Unscripted as U;

/*
 * Voxel: one shared block world. The game itself is Rust (voxel/, drawn with WebGL); this is
 * everything around it. It hears the game's CustomEvents (#[On]) and sends them to the server,
 * and hears other players' moves and blocks from the server (#[Live]) and hands them to the game
 * (window.voxel). The terrain is made from a fixed seed, so the server keeps only the changes.
 */
new class extends Component {
    const COLORS = ['#e6194b', '#3cb44b', '#4363d8', '#f58231', '#911eb4', '#42d4f4', '#f032e6', '#9a6324'];
    const BLOCKS = [1 => 'Grass', 2 => 'Dirt', 3 => 'Stone', 4 => 'Wood', 5 => 'Leaves', 6 => 'Sand', 7 => 'Planks'];

    #[State] public string $name = '';
    /** This player: id, name, color and the token the server knows them by. */
    public ?array $me = null;
    /** From the server: the changed blocks ([x, y, z, kind] each) and who's in the world. */
    public array $world = ['edits' => [], 'players' => []];
    /** Who's in the world, by id: name, color, and when we last heard from them (ms). */
    public array $players = [];
    public int $selected = 1;
    public ?string $notice = null;
    /** The world the game was last given. */
    private ?array $given = null;

    // ---- On the server -------------------------------------------------------------------

    // Loaded on mount, and again if the socket was down (moves and blocks sent then are lost).
    #[Server, Watch, Live('voxel')]
    public static function world(): array
    {
        $edits = DB::table('voxel_edits')->get()->map(fn($e) => [$e->x, $e->y, $e->z, $e->kind])->all();
        $players = [];
        foreach (DB::table('voxel_players')->where('created_at', '>', now()->subDay())->get() as $p) {
            if ($at = Cache::get("voxel.at.$p->id")) {
                $players[] = [$p->id, $p->name, $p->color, ...$at];
            }
        }
        return ['edits' => $edits, 'players' => $players];
    }

    #[Server]
    public static function enter(#[Validate('required|max:16')] string $name): array
    {
        $player = [
            'name' => trim($name),
            'color' => self::COLORS[DB::table('voxel_players')->count() % count(self::COLORS)],
            'token' => bin2hex(random_bytes(16)),
            'created_at' => now(),
            'updated_at' => now(),
        ];
        $player['id'] = DB::table('voxel_players')->insertGetId($player);
        return array_intersect_key($player, array_flip(['id', 'name', 'color', 'token']));
    }

    /** Sets block x, y, z to `$kind` (0 for air) for everyone. The bottom layer stays. */
    #[Server]
    public static function place(
        string $token,
        #[Validate('integer|between:0,63')] int $x,
        #[Validate('integer|between:1,31')] int $y,
        #[Validate('integer|between:0,63')] int $z,
        #[Validate('integer|between:0,7')] int $kind,
    ): void {
        self::player($token);
        DB::table('voxel_edits')->upsert(compact('x', 'y', 'z', 'kind'), ['x', 'y', 'z'], ['kind']);
        Push::send('voxel', ['block' => [$x, $y, $z, $kind]]);
    }

    /** Tells everyone where this player is. Kept for 10 seconds, for pages that load meanwhile. */
    #[Server]
    public static function move(string $token, float $x, float $y, float $z, float $yaw): void
    {
        $player = self::player($token);
        $at = [round($x, 2), round($y, 2), round($z, 2), round($yaw, 2)];
        Cache::put("voxel.at.$player->id", $at, 10);
        Push::send('voxel', ['player' => [$player->id, $player->name, $player->color, ...$at]]);
    }

    private static function player(string $token): object
    {
        return DB::table('voxel_players')->where('token', $token)->first(['id', 'name', 'color']) ?? abort(403, 'Join first');
    }

    // ---- In the browser -------------------------------------------------------------------

    protected function mounted(): void
    {
        // Players go quiet when they leave: anyone not heard from in 6 seconds has.
        U::setInterval(function () {
            foreach ($this->players as $id => $player) {
                if (self::now() - $player['seen'] > 6000) {
                    unset($this->players[$id]);
                    U::window()->voxel->removePlayer($id);
                }
            }
        }, 1000);
    }

    /** The game gets the world when it's loaded (again). */
    protected function rendered(): void
    {
        if ($this->world === $this->given) {
            return;
        }
        $this->given = $this->world;
        U::window()->voxel->load(json_encode(array_merge(...$this->world['edits'] ?: [[]])));
        foreach ($this->world['players'] as $player) {
            $this->show($player);
        }
    }

    public function join(): Generator
    {
        $this->me = yield static::enter($this->name);
        $this->notice = null;
    }

    /** A block you broke or placed: shown already; put back if the server says no. */
    #[On('voxel-edit')]
    public function edited(array $edit): Generator
    {
        [$x, $y, $z, $kind, $was] = $edit;
        if ($this->me === null) {
            U::window()->voxel->setBlock($x, $y, $z, $was);
            $this->notice = 'Join to build.';
            return;
        }
        try {
            yield static::place($this->me['token'], $x, $y, $z, $kind);
        } catch (ServerError $e) {
            U::window()->voxel->setBlock($x, $y, $z, $was);
            $this->notice = $e->getMessage();
        }
    }

    #[On('voxel-move')]
    public function walked(array $at): Generator
    {
        if ($this->me !== null) {
            yield static::move($this->me['token'], ...$at);
        }
    }

    #[On('voxel-select')]
    public function picked(array $detail): void
    {
        [$this->selected] = $detail;
    }

    /** Someone else's move or block. */
    #[Live('voxel')]
    public function heard(array $message): void
    {
        if (isset($message['block'])) {
            U::window()->voxel->setBlock(...$message['block']);
        }
        if (isset($message['player'])) {
            $this->show($message['player']);
        }
    }


    private function show(array $player): void
    {
        [$id, $name, $color, $x, $y, $z, $yaw] = $player;
        if ($id === ($this->me['id'] ?? null)) {
            return;
        }
        $this->players[$id] = ['name' => $name, 'color' => $color, 'seen' => self::now()];
        U::window()->voxel->setPlayer($id, $x, $y, $z, $yaw, $color);
    }

    /** Now, in ms: a float, as the browser's PHP is 32-bit. */
    private static function now(): float
    {
        return microtime(true) * 1000;
    }
};
?>
<div class="hud">
    @if ($me === null)
        <form u-on:submit.prevent="join">
            <input u-model="name" placeholder="Your name" maxlength="16">
            <button @disabled($this->loading('join'))>Join</button>
        </form>
        @error('name') <p class="error">{{ $message }}</p> @enderror
        <p class="hint">You can look around first: click the world.</p>
    @else
        <p>You're <strong style="color: {{ $me['color'] }}">{{ $me['name'] }}</strong></p>
    @endif
    @if ($notice)
        <p class="error">{{ $notice }}</p>
    @endif
    <ul class="players">
        @foreach ($players as $id => $player)
            <li u-key="{{ $id }}"><span class="swatch" style="background: {{ $player['color'] }}"></span>{{ $player['name'] }}</li>
        @endforeach
    </ul>
</div>
<ol class="blocks">
    @foreach (self::BLOCKS as $kind => $label)
        <li @class(['picked' => $kind === $selected])><b>{{ $kind }}</b> {{ $label }}</li>
    @endforeach
</ol>
