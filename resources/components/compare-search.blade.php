<?php
use App\Support\Products;
use Unscripted\Ui\{Component, Computed, Server, State, Watch};

new class extends Component {
    #[State] public string $query = '';
    public array $products = [];

    // The 500 products, once, when the component mounts. The search runs in the browser.
    #[Server, Watch]
    public static function products(): array
    {
        return Products::all();
    }

    #[Computed]
    public function matches(): array
    {
        $query = strtolower(trim($this->query));
        return $query === '' ? $this->products : array_values(array_filter($this->products,
            fn($p) => str_contains(strtolower($p['name'] . ' ' . $p['category']), $query)));
    }
};
?>

<div class="demo search">
    <input type="search" u-model="query" placeholder="Search 500 products">
    <p class="meta">{{ count($this->matches) }} matches</p>
    <ul>
        @foreach (array_slice($this->matches, 0, 50) as $p)
            <li u-key="{{ $p['id'] }}"><span>{{ $p['name'] }}</span><small>{{ $p['category'] }}</small><b>${{ number_format($p['price'] / 100, 2) }}</b></li>
        @endforeach
    </ul>
</div>
