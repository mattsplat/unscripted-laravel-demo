<?php

namespace App\Livewire\Compare;

use Livewire\Attributes\Validate;
use Livewire\Component;

class Signup extends Component
{
    #[Validate('required|min:2|max:50')]
    public string $name = '';

    #[Validate('required|email')]
    public string $email = '';

    #[Validate('required|min:8')]
    public string $password = '';

    #[Validate('required|in:free,team,business')]
    public string $plan = 'free';

    public bool $sent = false;

    public function save(): void
    {
        $this->validate();
        // A real app would create the account here.
        $this->sent = true;
    }

    public function render()
    {
        return view('livewire.compare.signup');
    }
}
