<?php

namespace App\Livewire\Compare;

use App\Support\Products;
use Livewire\Component;

class Search extends Component
{
    public string $query = '';

    public function render()
    {
        $matches = Products::search(Products::all(), $this->query);

        return view('livewire.compare.search', [
            'matches' => array_slice($matches, 0, 50),
            'total' => count($matches),
        ]);
    }
}
