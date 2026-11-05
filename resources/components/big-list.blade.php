<?php
use Unscripted\Ui\{Component, Computed, Server, State, Watch};

new class extends Component {
    #[State] public int $size = 5000;
    #[State] public string $query = '';
    #[State] public string $status = '';
    #[State] public int $limit = 100;    // rows shown; 0 is all of them
    public string $sort = 'id';
    public bool $desc = false;
    public array $rows = [];

    // The only server call: the whole list, once, and again when the size changes.
    // Searching, filtering and sorting it all happen in the browser.
    #[Server, Watch('size')]
    public static function rows(int $size): array
    {
        $size = max(100, min($size, 10000));
        mt_srand(42);
        $first = ['Ada', 'Ben', 'Cleo', 'Dev', 'Elif', 'Femi', 'Gus', 'Hana', 'Ivan', 'Jun', 'Kai', 'Lena', 'Milo', 'Nia', 'Omar', 'Pia', 'Quinn', 'Rosa', 'Sami', 'Tess', 'Umi', 'Vik', 'Wren', 'Yara', 'Zed'];
        $last = ['Abbott', 'Bauer', 'Chen', 'Diaz', 'Evans', 'Fischer', 'Garcia', 'Haddad', 'Ito', 'Jensen', 'Kowalski', 'Lopez', 'Moreau', 'Nakamura', 'Okafor', 'Patel', 'Rossi', 'Silva', 'Tanaka', 'Ueda', 'Varga', 'Weber', 'Young', 'Zhou'];
        $companies = ['Acme', 'Globex', 'Initech', 'Umbrella', 'Hooli', 'Vandelay', 'Stark', 'Wayne', 'Wonka', 'Tyrell', 'Cyberdyne', 'Soylent'];
        $cities = ['Austin', 'Berlin', 'Cairo', 'Denver', 'Lagos', 'Lima', 'Lisbon', 'Osaka', 'Oslo', 'Paris', 'Seoul', 'Sydney', 'Toronto', 'Zurich'];
        $statuses = ['active', 'trial', 'paused', 'churned'];

        $rows = [];
        for ($id = 1; $id <= $size; $id++) {
            $rows[] = [
                'id' => $id,
                'name' => $first[mt_rand(0, 24)] . ' ' . $last[mt_rand(0, 23)],
                'company' => $companies[mt_rand(0, 11)],
                'city' => $cities[mt_rand(0, 13)],
                'status' => $statuses[mt_rand(0, 3)],
                'amount' => mt_rand(100, 999999),   // cents
            ];
        }

        return $rows;
    }

    public function sortBy(string $column): void
    {
        $this->desc = $this->sort === $column ? !$this->desc : false;
        $this->sort = $column;
    }

    // The rows that match, in order. Runs in the browser on every keystroke.
    #[Computed]
    public function matching(): array
    {
        $query = strtolower(trim($this->query));
        $rows = array_filter($this->rows, fn($row) =>
            ($this->status === '' || $row['status'] === $this->status)
            && ($query === '' || str_contains(strtolower($row['name'] . ' ' . $row['company'] . ' ' . $row['city']), $query)));

        $sort = $this->sort;
        usort($rows, fn($a, $b) => $a[$sort] <=> $b[$sort]);

        return $this->desc ? array_reverse($rows) : $rows;
    }

    #[Computed]
    public function total(): float   // float: the sum passes 32-bit int in the browser
    {
        return array_sum(array_column($this->matching, 'amount'));
    }
};
?>

<div class="big-list">
    <div class="controls">
        <input type="search" u-model="query" placeholder="Search names, companies, cities" autofocus>
        <select u-model="status">
            <option value="">Any status</option>
            @foreach (['active', 'trial', 'paused', 'churned'] as $s)
                <option value="{{ $s }}">{{ ucfirst($s) }}</option>
            @endforeach
        </select>
        <label>Rows
            <select u-model="size">
                @foreach ([1000, 5000, 10000] as $n)
                    <option value="{{ $n }}">{{ number_format($n) }}</option>
                @endforeach
            </select>
        </label>
        <label>Show
            <select u-model="limit">
                @foreach ([100 => 'first 100', 500 => 'first 500', 0 => 'all'] as $n => $label)
                    <option value="{{ $n }}">{{ $label }}</option>
                @endforeach
            </select>
        </label>
    </div>

    <p class="summary">
        @if ($this->loading('rows'))
            Loading {{ number_format($size) }} rows…
        @else
            {{ number_format(count($this->matching)) }} of {{ number_format(count($rows)) }} rows
            · ${{ number_format($this->total / 100, 2) }}
        @endif
    </p>

    <table>
        <thead>
            <tr>
                @foreach (['id' => '#', 'name' => 'Name', 'company' => 'Company', 'city' => 'City', 'status' => 'Status', 'amount' => 'Amount'] as $column => $label)
                    <th u-on:click='sortBy("{{ $column }}")' @class(['sorted' => $sort === $column, 'num' => $column === 'amount' || $column === 'id'])>
                        {{ $label }}{{ $sort === $column ? ($desc ? ' ↓' : ' ↑') : '' }}
                    </th>
                @endforeach
            </tr>
        </thead>
        <tbody>
            @foreach ($limit ? array_slice($this->matching, 0, $limit) : $this->matching as $row)
                <tr u-key="{{ $row['id'] }}">
                    <td class="num">{{ $row['id'] }}</td>
                    <td>{{ $row['name'] }}</td>
                    <td>{{ $row['company'] }}</td>
                    <td>{{ $row['city'] }}</td>
                    <td><span class="status {{ $row['status'] }}">{{ $row['status'] }}</span></td>
                    <td class="num">${{ number_format($row['amount'] / 100, 2) }}</td>
                </tr>
            @endforeach
        </tbody>
    </table>
</div>
