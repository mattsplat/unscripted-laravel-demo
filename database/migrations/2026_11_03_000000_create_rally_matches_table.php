<?php

use Illuminate\Database\Migrations\Migration;
use Illuminate\Database\Schema\Blueprint;
use Illuminate\Support\Facades\Schema;

// Rally's results (resources/components/rally.blade.php): one row a match against the computer.
return new class extends Migration
{
    public function up(): void
    {
        Schema::create('rally_matches', function (Blueprint $table) {
            $table->id();
            $table->string('name', 16);
            $table->unsignedTinyInteger('level'); // 0 easy, 1 normal, 2 hard
            $table->boolean('won');
            $table->unsignedTinyInteger('games_won');
            $table->unsignedTinyInteger('games_lost');
            $table->unsignedSmallInteger('points_won');
            $table->unsignedSmallInteger('points_lost');
            $table->timestamps();
        });
    }

    public function down(): void
    {
        Schema::dropIfExists('rally_matches');
    }
};
