<?php
use Unscripted\Ui\{Component, On};

// Listens for the 'told' event, from the features component or from JavaScript (/features).
new class extends Component {
    public array $heard = [];
    #[On('told')] public function heard(?array $detail): void { $this->heard[] = $detail['n'] ?? null; }
};
?>
<p id="heard">heard: {{ implode(',', $heard) }}</p>
