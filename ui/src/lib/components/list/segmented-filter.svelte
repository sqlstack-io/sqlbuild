<script lang="ts" generics="T extends string">
	let {
		value = $bindable(),
		options
	}: {
		value: T;
		options: { value: T; label: string; count?: number }[];
	} = $props();
</script>

<div class="segmented-filter" role="tablist">
	{#each options as o (o.value)}
		<button role="tab" aria-selected={value === o.value} class:on={value === o.value} onclick={() => (value = o.value)}>
			{o.label}{#if o.count != null}<span>{o.count}</span>{/if}
		</button>
	{/each}
</div>

<style>
	.segmented-filter {
		display: flex;
		gap: 2px;
		padding: 2px;
		border-radius: 2px;
		background: var(--sb-hover);
	}
	.segmented-filter button {
		display: inline-flex;
		height: 26px;
		align-items: center;
		gap: 6px;
		padding: 0 10px;
		border: 0;
		border-radius: 2px;
		background: transparent;
		color: var(--muted-foreground);
		font-size: 12.5px;
	}
	.segmented-filter button:hover {
		color: var(--foreground);
	}
	.segmented-filter button.on {
		background: var(--card);
		color: var(--foreground);
		box-shadow: 0 1px 2px rgba(0, 0, 0, 0.14);
	}
	.segmented-filter span {
		color: var(--sb-text-faint);
		font-size: 11.5px;
		font-variant-numeric: tabular-nums;
	}
</style>
