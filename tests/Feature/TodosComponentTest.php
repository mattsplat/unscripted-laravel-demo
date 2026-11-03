<?php

namespace Tests\Feature;

use App\Models\Todo;
use Illuminate\Foundation\Testing\RefreshDatabase;
use Tests\TestCase;
use Unscripted\UiServer\Components;

// resources/components/todos.blade.php, driven without a browser.
class TodosComponentTest extends TestCase
{
    use RefreshDatabase;

    public function test_it_loads_adds_and_validates(): void
    {
        Todo::create(['title' => 'Buy milk']);
        $todos = app(Components::class)->test('todos');

        $todos->assertSee('Buy milk')->assertSee('1 left');
        $todos->set('draft', 'Walk the dog')->call('add')
            ->assertSee('Walk the dog')->assertSet('draft', '')->assertHasNoErrors();
        $this->assertDatabaseHas('todos', ['title' => 'Walk the dog']);

        $todos->call('add')->assertHasErrors('title')->assertSee('The title field is required.');
    }

    public function test_only_finished_todos_can_be_removed(): void
    {
        $todo = Todo::create(['title' => 'Not done']);
        $todos = app(Components::class)->test('todos');

        $todos->call('remove', $todo->id)->assertFailed(403); // the gate says no
        $this->assertDatabaseHas('todos', ['id' => $todo->id]);

        $todos->call('toggle', $todo->id)->call('remove', $todo->id);
        $this->assertDatabaseMissing('todos', ['id' => $todo->id]);
    }
}
