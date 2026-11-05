<?php

namespace Tests\Feature;

use App\Livewire\Compare\Search;
use App\Livewire\Compare\Signup;
use Livewire\Livewire;
use Tests\TestCase;

class CompareTest extends TestCase
{
    public function test_the_page_renders_both_sides(): void
    {
        $this->get('/compare')->assertOk()
            ->assertSee('wire:snapshot', false)
            ->assertSee('unscripted-compare-counter', false)
            ->assertSee('500 matches');
        $this->get('/big-list')->assertOk();
    }

    public function test_the_livewire_search_and_signup_work(): void
    {
        Livewire::test(Search::class)->set('query', 'pro')->assertSee('48 matches');
        Livewire::test(Signup::class)->set('email', 'nope')->assertHasErrors(['email' => 'email'])
            ->set('name', 'Ada')->set('email', 'ada@example.com')->set('password', 'correct-horse')
            ->call('save')->assertHasNoErrors()->assertSet('sent', true);
    }
}
