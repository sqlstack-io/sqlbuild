<script lang="ts" generics="T extends string">
	let {
		value = $bindable(),
		tabs
	}: {
		value: T;
		tabs: { value: T; label: string; count?: number }[];
	} = $props();
</script>

<div class="page-tabs" role="tablist">
	{#each tabs as t (t.value)}
		<button role="tab" aria-selected={value === t.value} class:on={value === t.value} onclick={() => (value = t.value)}>
			{t.label}{#if t.count != null}<span>{t.count}</span>{/if}
		</button>
	{/each}
</div>

<style>
	.page-tabs {
		display: flex;
		flex: 0 0 auto;
		align-items: center;
		gap: 4px;
		padding: 0 16px;
		border-bottom: 1px solid var(--border);
	}
	button {
		display: inline-flex;
		align-items: center;
		gap: 6px;
		padding: 10px 8px;
		border-bottom: 2px solid transparent;
		color: var(--muted-foreground);
		font-size: 12.5px;
	}
	button:hover {
		color: var(--foreground);
	}
	button.on {
		border-color: var(--primary);
		color: var(--foreground);
	}
	span {
		color: var(--sb-text-faint);
		font-size: 11.5px;
		font-variant-numeric: tabular-nums;
	}
</style>
