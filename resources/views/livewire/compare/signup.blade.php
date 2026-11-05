<div class="demo signup">
    @if ($sent)
        <p class="done">Signed up as {{ $email }}.</p>
    @else
        <form wire:submit="save">
            <label>Name <input wire:model.live="name"></label>
            @error('name') <p class="error">{{ $message }}</p> @enderror
            <label>Email <input wire:model.live="email" type="email"></label>
            @error('email') <p class="error">{{ $message }}</p> @enderror
            <label>Password <input wire:model.live="password" type="password"></label>
            @error('password') <p class="error">{{ $message }}</p> @enderror
            <label>Plan
                <select wire:model.live="plan">
                    <option value="free">Free</option><option value="team">Team</option><option value="business">Business</option>
                </select>
            </label>
            <button>Sign up</button>
        </form>
    @endif
</div>
