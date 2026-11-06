<script lang="ts">
	import ChevronDownIcon from '@lucide/svelte/icons/chevron-down';

	let { label, value }: { label: string; value?: string } = $props();
</script>

<button class="filter-button" class:set={!!value}>
	{label}{#if value}<b>{value}</b>{/if}
	<ChevronDownIcon size={13} />
</button>

<style>
	.filter-button {
		display: inline-flex;
		height: 30px;
		align-items: center;
		gap: 5px;
		padding: 0 9px;
		border: 0;
		border-radius: 2px;
		background: transparent;
		color: var(--muted-foreground);
		font-size: 12.5px;
	}
	.filter-button:hover {
		background: var(--sb-hover);
		color: var(--foreground);
	}
	.filter-button b {
		color: var(--foreground);
		font-weight: 500;
	}
	.filter-button :global(svg) {
		color: var(--sb-text-faint);
	}
</style>
