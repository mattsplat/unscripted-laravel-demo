<?php

return [
    // 'http' (POST /unscripted/call) or 'reverb' (calls over Reverb's WebSocket: run
    // `php artisan reverb:start` too).
    'transport' => env('UNSCRIPTED_TRANSPORT', 'http'),

    // For Route::unscripted()'s page: the runtime and the PHP pack, from the checkout beside this
    // app (a real app would leave these out, for the CDN).
    'runtime' => '/vendor/unscripted/runtime/auto.js',
    'pack' => '/vendor/unscripted/php84/php_pack.wasm',
];
