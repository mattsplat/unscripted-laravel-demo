<?php
use Unscripted\Ui\Component;

new class extends Component {
    public int $count = 0;

    public function add(int $by = 1): void
    {
        $this->count += $by;
    }
};
?>

<div class="demo counter">
    <button u-on:click="add(-1)">−</button>
    <output>{{ $count }}</output>
    <button u-on:click="add(1)">+</button>
</div>
