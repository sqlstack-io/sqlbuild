<script lang="ts">
	import SearchIcon from '@lucide/svelte/icons/search';
	import XIcon from '@lucide/svelte/icons/x';
	import type { Snippet } from 'svelte';

	export type ToolbarCount = { tone: string; count: number; label: string };

	let {
		query = $bindable(''),
		placeholder = 'Search',
		counts = [],
		total,
		filters,
		actions
	}: {
		query?: string;
		placeholder?: string;
		counts?: ToolbarCount[];
		total: string;
		filters?: Snippet;
		actions?: Snippet;
	} = $props();
</script>

<div class="list-toolbar">
	<label class="list-search">
		<SearchIcon size={14} />
		<input bind:value={query} {placeholder} />
		{#if query}
			<button onclick={() => (query = '')} aria-label="Clear search"><XIcon size={13} /></button>
		{/if}
	</label>
	{#if filters}<div class="list-filters">{@render filters()}</div>{/if}
	{#if actions}{@render actions()}{/if}
	<div class="list-summary">
		{#each counts.filter((c) => c.count > 0) as c (c.label)}
			<span title="{c.count} {c.label}"><i style:background={c.tone}></i>{c.count} {c.label}</span>
		{/each}
		<b>{total}</b>
	</div>
</div>

<style>
	.list-toolbar {
		display: flex;
		height: 50px;
		flex: 0 0 50px;
		align-items: center;
		gap: 6px;
		padding: 0 20px;
		border-bottom: 1px solid var(--border);
	}
	.list-search {
		display: flex;
		width: 280px;
		height: 30px;
		align-items: center;
		gap: 7px;
		margin-right: 6px;
		padding: 0 9px;
		border: 1px solid var(--border);
		border-radius: 2px;
		background: var(--background);
		color: var(--sb-text-faint);
	}
	.list-search:focus-within {
		border-color: color-mix(in srgb, var(--primary) 55%, var(--border));
	}
	.list-search input {
		min-width: 0;
		flex: 1;
		border: 0;
		outline: 0;
		background: transparent;
		color: var(--foreground);
		font-size: 12.5px;
	}
	.list-search input::placeholder {
		color: var(--sb-text-faint);
	}
	.list-search button {
		display: grid;
		place-items: center;
		border: 0;
		background: transparent;
		color: var(--sb-text-faint);
	}
	.list-filters {
		display: flex;
		align-items: center;
		gap: 2px;
	}
	.list-summary {
		display: flex;
		align-items: center;
		gap: 14px;
		margin-left: auto;
		color: var(--muted-foreground);
		font-size: 12px;
		font-variant-numeric: tabular-nums;
		white-space: nowrap;
	}
	.list-summary span {
		display: inline-flex;
		align-items: center;
		gap: 6px;
	}
	.list-summary i {
		width: 7px;
		height: 7px;
		border-radius: 50%;
	}
	.list-summary b {
		color: var(--foreground);
		font-weight: 500;
	}
</style>
