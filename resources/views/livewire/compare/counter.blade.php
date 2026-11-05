<div class="demo counter">
    <button wire:click="add(-1)">−</button>
    <output>{{ $count }}</output>
    <button wire:click="add(1)">+</button>
</div>
