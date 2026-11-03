<?php

use Illuminate\Database\Migrations\Migration;
use Illuminate\Database\Schema\Blueprint;
use Illuminate\Support\Facades\Schema;

// Spread, the multiplayer demo (resources/components/spread.blade.php).
return new class extends Migration
{
    public function up(): void
    {
        Schema::create('spread_players', function (Blueprint $table) {
            $table->id();
            $table->string('name', 16);
            $table->string('color', 7);
            $table->string('token', 64)->unique();
            $table->unsignedBigInteger('moved_at')->default(0); // ms, for the cooldown
            $table->timestamps();
        });
        Schema::create('spread_cells', function (Blueprint $table) {
            $table->unsignedTinyInteger('x');
            $table->unsignedTinyInteger('y');
            $table->foreignId('player_id')->constrained('spread_players')->cascadeOnDelete();
            $table->primary(['x', 'y']);
        });
    }

    public function down(): void
    {
        Schema::dropIfExists('spread_cells');
        Schema::dropIfExists('spread_players');
    }
};
