<script lang="ts">
	import CheckIcon from '@lucide/svelte/icons/check';
	import CopyIcon from '@lucide/svelte/icons/copy';

	// Copies `value`; shows a tick for a moment. Stops click propagation so it works inside clickable rows.
	let { value, label = 'Copy', text }: { value: string; label?: string; text?: string } = $props();

	let copied = $state(false);
	let timer: ReturnType<typeof setTimeout> | undefined;

	async function copy(e: MouseEvent) {
		e.stopPropagation();
		e.preventDefault();
		await navigator.clipboard?.writeText(value).catch(() => undefined);
		copied = true;
		clearTimeout(timer);
		timer = setTimeout(() => (copied = false), 1200);
	}
</script>

<button type="button" class="copy" class:text={!!text} onclick={copy} title={copied ? 'Copied' : `${label}: ${value}`} aria-label="{label} {value}">
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
