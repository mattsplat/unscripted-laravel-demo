{{-- The bots' level, for the whole match: whoever changes it, everyone sees. --}}
<div class="bots">
    <span>Bots</span>
    @foreach ($levels as $i => $label)
        <button type="button" u-on:click="bots({{ $i }})" @class(['picked' => $i === $level])>{{ $label }}</button>
    @endforeach
    <small>For everyone in the match</small>
</div>
