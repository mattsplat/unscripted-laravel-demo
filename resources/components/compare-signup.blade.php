<?php
use Unscripted\Ui\{Component, Server, State, Validate};

new class extends Component {
    #[State] public array $form = ['name' => '', 'email' => '', 'password' => '', 'plan' => 'free'];
    public bool $sent = false;

    // The same rules as the Livewire form. With `as: 'form'` the browser checks each field as
    // it changes; the server checks them all again when the form is sent.
    #[Server]
    public static function register(
        #[Validate([
            'name' => 'required|min:2|max:50',
            'email' => 'required|email',
            'password' => 'required|min:8',
            'plan' => 'required|in:free,team,business',
        ], as: 'form')] array $form,
    ): void {
        // A real app would create the account here.
    }

    public function save(): Generator
    {
        if (!$this->validate()) {
            return;
        }
        yield static::register($this->form);
        $this->sent = true;
    }
};
?>

<div class="demo signup">
    @if ($sent)
        <p class="done">Signed up as {{ $form['email'] }}.</p>
    @else
        <form u-on:submit.prevent="save">
            <label>Name <input u-model="form.name"></label>
            @error('form.name') <p class="error">{{ $message }}</p> @enderror
            <label>Email <input u-model="form.email" type="email"></label>
            @error('form.email') <p class="error">{{ $message }}</p> @enderror
            <label>Password <input u-model="form.password" type="password"></label>
            @error('form.password') <p class="error">{{ $message }}</p> @enderror
            <label>Plan
                <select u-model="form.plan">
                    <option value="free">Free</option><option value="team">Team</option><option value="business">Business</option>
                </select>
            </label>
            <button>Sign up</button>
        </form>
    @endif
</div>
