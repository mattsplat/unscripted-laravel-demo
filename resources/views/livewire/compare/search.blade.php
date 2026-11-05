<div class="demo search">
    <input type="search" wire:model.live="query" placeholder="Search 500 products">
    <p class="meta">{{ $total }} matches</p>
    <ul>
        @foreach ($matches as $p)
            <li wire:key="{{ $p['id'] }}"><span>{{ $p['name'] }}</span><small>{{ $p['category'] }}</small><b>${{ number_format($p['price'] / 100, 2) }}</b></li>
        @endforeach
    </ul>
</div>
