<?php

use Illuminate\Support\Facades\Route;

Route::view('/', 'home');
Route::view('/todos', 'todos');

// Spread, a multiplayer game: open it in two windows.
Route::view('/spread', 'spread');
Route::view('/voxel', 'voxel');
Route::view('/outpost', 'outpost');
Route::view('/rally', 'rally');
Route::view('/rally-3d', 'rally', ['engine' => 'three-d']);

// Checks @unscripted/ui's features in a browser.
Route::view('/features', 'features');

// A component as a whole page (unscripted/ui-server's built-in page).
Route::unscripted('/counters', 'counters', ['title' => 'Counters']);
