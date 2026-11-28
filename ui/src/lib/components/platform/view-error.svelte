<script lang="ts">
	import Button from '$lib/components/ui/button/button.svelte';

	/**
	 * "View error": a long error stays out of the page until asked for, then shows in full below,
	 * in muted monospace. `label` names something other than an error, such as "View output".
	 */
	let { error, label = 'View error' }: { error: string; label?: string } = $props();
	let open = $state(false);
</script>

<Button variant="link" size="inline" class="view-error-toggle text-[12px]" aria-expanded={open} onclick={(e: MouseEvent) => (e.stopPropagation(), (open = !open))}>{open ? 'Hide error' : label}</Button>
{#if open}<pre class="view-error">{error}</pre>{/if}

<style>
	.view-error {
		order: 10;
		flex-basis: 100%;
		max-height: 240px;
		margin: 4px 0 0;
		overflow: auto;
		padding: 6px 8px;
		border-radius: 2px;
		background: var(--sb-inset);
		color: var(--muted-foreground);
		font-family: var(--font-mono);
		font-size: 11.5px;
		white-space: pre-wrap;
		overflow-wrap: anywhere;
	}
</style>
