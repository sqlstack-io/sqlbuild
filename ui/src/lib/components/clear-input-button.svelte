<script lang="ts">
	import XIcon from '@lucide/svelte/icons/x';

	// The ✕ inside a text field that holds anything: always shown while it does, not only on hover or
	// focus. It keeps the field's focus (the press doesn't blur it), so typing can carry on. One
	// component, so every field's clear looks and behaves alike.
	let { label, onclear }: { label: string; onclear: () => void } = $props();
</script>

<!-- raw-button: a field's clear control, inside the field's own frame -->
<button type="button" class="clear-input" aria-label={label} title="{label} (Esc)" onmousedown={(e) => e.preventDefault()} onclick={onclear}><XIcon size={12} strokeWidth={2.5} /></button>

<style>
	.clear-input {
		display: inline-flex;
		width: 18px;
		height: 18px;
		flex: none;
		align-items: center;
		justify-content: center;
		align-self: center;
		margin-left: auto;
		border-radius: 999px;
		background: var(--muted);
		color: var(--foreground);
		opacity: 0.75;
	}
	.clear-input:hover,
	.clear-input:focus-visible {
		opacity: 1;
	}
	.clear-input:focus-visible {
		outline: 2px solid var(--primary);
		outline-offset: 1px;
	}
</style>
