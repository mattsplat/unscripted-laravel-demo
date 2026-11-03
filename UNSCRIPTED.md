# @unscripted/ui on Laravel

A demo of `#[Server]` methods: `resources/components/todos.blade.php`, mounted by
`resources/views/todos.blade.php`. Composer installs `unscripted/ui-server` from its
own repo (`mattsplat/unscripted-ui-server`). The browser side still needs the
`unscripted-js` checkout next to this folder (`../unscripted-js`), for `public/vendor/unscripted`'s
links.

```sh
php artisan migrate:fresh --seed
php artisan serve --port=8010    # http://127.0.0.1:8010/
php artisan reverb:start         # calls go over Reverb's socket (port 8081): UNSCRIPTED_TRANSPORT=reverb in .env
```

## Spread, a multiplayer game

<http://127.0.0.1:8010/spread>, in two or more windows (with `reverb:start` running). Everyone
paints one 12×12 board in 60-second rounds: your first cell can be anywhere, an empty cell must
touch one of yours, and another player's must touch two. It's all in
`resources/components/spread.blade.php`: the rules are one function the server enforces and
the browser uses to outline your moves, and `#[Live('spread')]` updates every player's board.
