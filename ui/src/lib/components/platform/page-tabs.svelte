<script lang="ts" generics="T extends string">
	// The UI's one row of tabs under a page or panel header.
	let {
		value = $bindable(),
		tabs,
		flush = false,
		onselect
	}: {
		value: T;
		tabs: { value: T; label: string; count?: number | string; title?: string }[];
		/** Inside a header that already has the padding and bottom border. */
		flush?: boolean;
		/** Called with the tab picked, e.g. to keep it in the URL. */
		onselect?: (value: T) => void;
	} = $props();
</script>

<div class="page-tabs" class:flush role="tablist">
	{#each tabs as t (t.value)}
		<button role="tab" aria-selected={value === t.value} class:on={value === t.value} title={t.title} onclick={() => ((value = t.value), onselect?.(t.value))}>
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
	.page-tabs.flush {
		padding: 0;
		border-bottom: 0;
	}
	button {
		display: inline-flex;
		margin-bottom: -1px;
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
