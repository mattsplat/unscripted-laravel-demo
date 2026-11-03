<?php

namespace Database\Seeders;

use App\Models\Todo;
use Illuminate\Database\Seeder;

class DatabaseSeeder extends Seeder
{
    public function run(): void
    {
        foreach (['Buy milk', 'Write the server package', 'Try #[Server] methods', 'Walk the dog'] as $i => $title) {
            Todo::create(['title' => $title, 'done' => $i === 0]);
        }
    }
}
