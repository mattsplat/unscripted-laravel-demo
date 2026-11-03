<?php
use Unscripted\Ui\{Component, On, State};

// A parent with @child counters (/features): each keeps its own count; the parent names them.
new class extends Component {
    public array $names = ['a' => 'Apples', 'b' => 'Bananas'];
    public int $total = 0;
    #[State] public string $suffix = '';

    public function rename(): void { $this->names['a'] = 'Apricots'; }
    public function drop(): void { unset($this->names['b']); }
    public function add(): void { $this->names['c'] = 'Cherries'; }
    #[On('counted')] public function counted(): void { $this->total++; }
};
?>
<div id="counters">
    @foreach ($names as $key => $label)
        @child('counter', ['label' => $label . $suffix], $key)
    @endforeach
</div>
<p>total clicks: <span id="total">{{ $total }}</span></p>
<p><input id="suffix" u-model="suffix"> <button id="rename" u-on:click="rename">rename</button> <button id="drop" u-on:click="drop">drop b</button> <button id="add" u-on:click="add">add c</button></p>
