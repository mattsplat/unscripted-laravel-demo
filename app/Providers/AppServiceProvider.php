<?php

namespace App\Providers;

use App\Models\Todo;
use App\Models\User;
use Illuminate\Support\Facades\Gate;
use Illuminate\Support\ServiceProvider;

class AppServiceProvider extends ServiceProvider
{
    /**
     * Register any application services.
     */
    public function register(): void
    {
        //
    }

    /**
     * Bootstrap any application services.
     */
    public function boot(): void
    {
        // For #[Server(can: 'remove-todo')]: gets the call's arguments. The demo has no logins,
        // so guests (?User) are allowed; only finished todos can be removed.
        Gate::define('remove-todo', fn (?User $user, int $id) => Todo::find($id)?->done === true);

        // Running Reverb here (REVERB_HOST is this machine), a page's socket goes to it at the host
        // the page came from, so phones and other machines on the network can play too, whatever
        // this machine's IP is today. Artisan, Reverb's own process included, keeps REVERB_HOST,
        // and so does a Reverb elsewhere (Laravel Cloud's).
        $local = in_array(config('broadcasting.connections.reverb.options.host'), ['127.0.0.1', 'localhost', '0.0.0.0'], true);
        if ($local && ! $this->app->runningInConsole()) {
            config(['broadcasting.connections.reverb.options.host' => request()->getHost()]);
        }
    }
}
