<?php

return [
    // 'http' (POST /unscripted/call) or 'reverb' (calls over Reverb's WebSocket: run
    // `php artisan reverb:start` too).
    'transport' => env('UNSCRIPTED_TRANSPORT', 'http'),

    // The address the browser opens Reverb's socket at, when it isn't the broadcasting
    // connection's: on an https site, a wss:// proxy in front of Reverb (see RUNNING.md).
    'socket_url' => env('UNSCRIPTED_SOCKET_URL'),

    // The runtime and the PHP pack, for every page here (and Route::unscripted()'s): from the CDN,
    // or, for working on unscripted-js beside this app, from its checkout through the links in
    // public/vendor/unscripted (set both in .env: see RUNNING.md).
    'runtime' => env('UNSCRIPTED_RUNTIME', 'https://cdn.jsdelivr.net/npm/@mtjncn/runtime@0.1/auto.js'),
    'pack' => env('UNSCRIPTED_PACK', 'https://cdn.jsdelivr.net/npm/@mtjncn/php84@0.1/php_pack.wasm'),
];
