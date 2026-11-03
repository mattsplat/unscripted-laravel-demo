<?php
use App\Models\Todo;
use Unscripted\Ui\{Component, Server, State, Validate, Watch};

new class extends Component {
    #[State] public string $draft = '';
    #[State] public string $query = '';
    #[State] public array $todos = [];

    // Fills $todos on mount, whenever $query changes, and after each action's server calls.
    #[Server, Watch('query', debounce: 250)]
    public static function todos(string $query): array
    {
        return Todo::where('title', 'like', '%' . $query . '%')
            ->orderBy('id')
            ->get(['id', 'title', 'done'])
            ->toArray();
    }

    // Checked on the server before it runs; a failure shows under @error('title').
    #[Server]
    public static function create(#[Validate('required|max:100')] string $title): array
    {
        return Todo::create(['title' => trim($title)])->only('id', 'title', 'done');
    }

    #[Server]
    public static function toggle(int $id): void
    {
        $todo = Todo::findOrFail($id);
        $todo->update(['done' => !$todo->done]);
    }

    #[Server(can: 'remove-todo')]
    public static function remove(int $id): void
    {
        Todo::findOrFail($id)->delete();
    }

    public function add(): Generator
    {
        yield static::create($this->draft);
        $this->draft = '';
    }
};
?>

<form u-on:submit.prevent="add">
    <input u-model="draft" placeholder="What needs doing?">
    <button @disabled($this->loading('add'))>Add</button>
</form>
@error('title')
    <p class="error">{{ $message }}</p>
@enderror

<input type="search" u-model="query" placeholder="Search">

<ul>
    @forelse ($todos as $todo)
        <li u-key="{{ $todo['id'] }}" @class(['done' => $todo['done']])>
            <label>
                <input type="checkbox" @checked($todo['done']) u-on:change="toggle({{ $todo['id'] }})">
                {{ $todo['title'] }}
            </label>
            @if ($todo['done'])
                <button class="remove" u-on:click="remove({{ $todo['id'] }})" title="Remove">×</button>
            @endif
        </li>
    @empty
        <li class="empty">{{ $this->loading('todos') ? 'Loading…' : 'Nothing here.' }}</li>
    @endforelse
</ul>

<footer>
    <span>{{ count(array_filter($todos, fn($todo) => !$todo['done'])) }} left</span>
    @if ($this->loading())
        <span class="busy">Syncing…</span>
    @endif
</footer>
