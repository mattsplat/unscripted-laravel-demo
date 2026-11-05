<?php

namespace Tests\Feature;

use Illuminate\Foundation\Testing\RefreshDatabase;
use Tests\TestCase;

class ExampleTest extends TestCase
{
    use RefreshDatabase; // the page renders the todos on the server

    /**
     * A basic test example.
     */
    public function test_the_application_returns_a_successful_response(): void
    {
        $this->get('/')->assertOk()->assertSee('run in the browser')->assertSee('href="/examples"', false)->assertSee('href="/docs"', false);
        $this->get('/examples')->assertOk()->assertSee('Outpost');
        $this->get('/todos')->assertOk();
    }
}
