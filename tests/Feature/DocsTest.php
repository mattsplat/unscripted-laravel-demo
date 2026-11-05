<?php

namespace Tests\Feature;

use Tests\TestCase;

class DocsTest extends TestCase
{
    public function test_the_docs_show_the_packages_pages_with_links_kept_here(): void
    {
        $this->get('/docs')->assertOk()->assertSee('Quickstart')->assertSee('href="/docs/quickstart"', false);
        $this->get('/docs/actions')->assertOk()->assertSee('<h1>Actions</h1>', false)->assertSee('On this page');
        $this->get('/docs/no-such-page')->assertNotFound();
    }
}
