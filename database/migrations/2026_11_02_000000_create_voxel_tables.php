<?php

use Illuminate\Database\Migrations\Migration;
use Illuminate\Database\Schema\Blueprint;
use Illuminate\Support\Facades\Schema;

// Voxel, the block world demo (resources/components/voxel.blade.php).
return new class extends Migration
{
    public function up(): void
    {
        Schema::create('voxel_players', function (Blueprint $table) {
            $table->id();
            $table->string('name', 16);
            $table->string('color', 7);
            $table->string('token', 64)->unique();
            $table->timestamps();
        });
        // Only the blocks people changed: the rest is generated, the same on every page.
        Schema::create('voxel_edits', function (Blueprint $table) {
            $table->unsignedTinyInteger('x');
            $table->unsignedTinyInteger('y');
            $table->unsignedTinyInteger('z');
            $table->unsignedTinyInteger('kind'); // 0: air
            $table->primary(['x', 'y', 'z']);
        });
    }

    public function down(): void
    {
        Schema::dropIfExists('voxel_edits');
        Schema::dropIfExists('voxel_players');
    }
};
