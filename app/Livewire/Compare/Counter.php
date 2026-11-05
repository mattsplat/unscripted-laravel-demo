<?php

namespace App\Livewire\Compare;

use Livewire\Component;

class Counter extends Component
{
    public int $count = 0;

    public function add(int $by = 1): void
    {
        $this->count += $by;
    }

    public function render()
    {
        return view('livewire.compare.counter');
    }
}
