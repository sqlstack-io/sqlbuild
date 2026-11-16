<script lang="ts">
	import Button from '$lib/components/ui/button/button.svelte';
	import SearchIcon from '@lucide/svelte/icons/search';
	import XIcon from '@lucide/svelte/icons/x';
	import type { Component, Snippet } from 'svelte';

	/** `reserve` keeps a zero count's space (invisible, ahead of the others), so a count that comes and
	 *  goes as rows are switched doesn't change the toolbar's width and wrap it onto another row.
	 *  A count with `onclick` is a button that filters the list to those rows (`pressed` while it does),
	 *  marked with `icon` instead of a coloured dot when given. */
	export type ToolbarCount = {
		tone?: string;
		count: number;
		label: string;
		reserve?: boolean;
		icon?: Component<{ class?: string }>;
		onclick?: () => void;
		pressed?: boolean;
	};

	let {
		query = $bindable(''),
		placeholder = 'Search',
		counts = [],
		total,
		loading = false,
		filters,
		actions
	}: {
		query?: string;
		placeholder?: string;
		counts?: ToolbarCount[];
		total: string;
		/** Nothing loaded yet: the totals would read as zero, so they wait for the list. */
		loading?: boolean;
		filters?: Snippet;
		actions?: Snippet;
	} = $props();
</script>

<div class="list-toolbar">
	<label class="list-search">
		<SearchIcon size={14} />
		<input bind:value={query} {placeholder} />
		{#if query}
			<Button variant="ghost" size="icon-xs" onclick={() => (query = '')} aria-label="Clear search"><XIcon /></Button>
		{/if}
	</label>
	{#if filters}<div class="list-filters">{@render filters()}</div>{/if}
	{#if actions}{@render actions()}{/if}
	<div class="list-summary" class:invisible={loading}>
		{#each counts.filter((c) => c.count > 0 || c.reserve) as c (c.label)}
			{#if c.onclick}
				<Button
					variant="outline"
					size="sm"
					class={c.count === 0 ? 'summary-filter reserved' : 'summary-filter'}
					aria-hidden={c.count === 0 ? 'true' : undefined}
					tabindex={c.count === 0 ? -1 : undefined}
					aria-pressed={c.pressed ?? false}
					title={c.pressed ? 'Show all rows' : `Show only the ${c.count} ${c.label}`}
					onclick={c.onclick}
				>
					{#if c.icon}<c.icon class="size-3.5" />{:else}<i style:background={c.tone}></i>{/if}{c.count} {c.label}
				</Button>
			{:else}
				<span class:reserved={c.count === 0} aria-hidden={c.count === 0 ? 'true' : undefined} title="{c.count} {c.label}"><i style:background={c.tone}></i>{c.count} {c.label}</span>
			{/if}
		{/each}
		<b>{total}</b>
	</div>
</div>

<style>
	/* The search box shrinks first; past that the toolbar wraps onto more rows instead of clipping. */
	.list-toolbar {
		display: flex;
		min-height: 50px;
		flex: 0 0 auto;
		flex-wrap: wrap;
		align-items: center;
		gap: 8px 6px;
		padding: 8px 20px;
		border-bottom: 1px solid var(--border);
	}
	.list-search {
		display: flex;
		flex: 1 1 180px;
		max-width: 280px;
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
	.list-filters {
		display: flex;
		flex-wrap: wrap;
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
	.list-summary span.reserved,
	.list-summary :global(.summary-filter.reserved) {
		order: -1;
		visibility: hidden;
	}
	.list-summary i {
		width: 7px;
		height: 7px;
		border-radius: 50%;
	}
	.list-summary :global(.summary-filter) {
		color: var(--muted-foreground);
		font-size: 12px;
		font-weight: 400;
	}
	.list-summary :global(.summary-filter[aria-pressed='true']) {
		color: var(--foreground);
	}
	.list-summary b {
		color: var(--foreground);
		font-weight: 500;
	}
</style>
