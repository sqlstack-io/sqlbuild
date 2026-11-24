<script lang="ts">
	import { createCopyState } from '$lib/browser/copy-state.svelte';
	import CheckIcon from '@lucide/svelte/icons/check';
	import CopyIcon from '@lucide/svelte/icons/copy';

	// Copies `value`; shows a tick for a moment. Stops click propagation so it works inside clickable rows.
	// `showValue` names the value in the tooltip and label; turn it off for long values such as SQL.
	let { value, label = 'Copy', text, showValue = true }: { value: string; label?: string; text?: string; showValue?: boolean } = $props();

	const copier = createCopyState();
	const copied = $derived(copier.copied);

	async function copy(e: MouseEvent) {
		e.stopPropagation();
		e.preventDefault();
		await copier.copy(value, e.currentTarget as Element);
	}
</script>

<button type="button" class="copy" class:text={!!text} onclick={copy} title={copied ? 'Copied' : showValue ? `${label}: ${value}` : label} aria-label={showValue ? `${label} ${value}` : label}>
	{#if copied}<CheckIcon size={12} />{:else}<CopyIcon size={12} />{/if}{#if text}<span>{copied ? 'copied' : text}</span>{/if}
</button>

<style>
	.copy {
		display: inline-grid;
		height: 20px;
		min-width: 20px;
		place-items: center;
		border-radius: 2px;
		color: var(--sb-text-faint);
	}
	.copy.text {
		display: inline-flex;
		gap: 4px;
		padding: 0 6px;
		font-family: var(--font-mono);
		font-size: 11px;
	}
	.copy:hover {
		background: var(--sb-hover);
		color: var(--foreground);
	}
</style>
