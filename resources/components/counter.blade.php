<?php
use Unscripted\Ui\{Component, Server, Watch};

// A child of counters.blade.php: its label comes from the parent, its count is its own.
new class extends Component {
    public string $label = '';
    public int $count = 0;
    public string $loaded = '';

    #[Server, Watch] public static function loaded(): string { return 'loaded'; }
    public function bump(): void { $this->count++; $this->dispatch('counted'); }
};
?>
<span class="counter"><b>{{ $label }}</b>: <button u-on:click="bump">{{ $count }}</button> <i>{{ $loaded }}</i></span>
