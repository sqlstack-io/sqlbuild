<script lang="ts">
	import RefreshCwIcon from '@lucide/svelte/icons/refresh-cw';
	import SearchIcon from '@lucide/svelte/icons/search';
	import XIcon from '@lucide/svelte/icons/x';
	import type { RunTab } from '$lib/api';
	import ChoiceFilter from '$lib/components/list/choice-filter.svelte';
	import SegmentedFilter from '$lib/components/list/segmented-filter.svelte';
	import { formatAgo } from '$lib/format/time';
	import { app } from '$lib/state/app.svelte';
	import type { RunsState } from './state.svelte';
	import type { RangePreset, RunsView } from './types';
	import { invalidTokens, parseTokens, removeToken, TOKEN_HELP } from './utils';

	let { list, queued }: { list: RunsState; queued: number | null } = $props();

	const p = $derived(list.params);

	// The box edits a local copy; the URL follows after a short pause.
	let draft = $state('');
	let typing = false;
	let timer: ReturnType<typeof setTimeout> | undefined;
	$effect(() => {
		const q = p.q;
		if (!typing) draft = q;
	});
	function onInput(value: string) {
		draft = value;
		typing = true;
		clearTimeout(timer);
		timer = setTimeout(() => {
			typing = false;
			list.update({ q: draft });
		}, 300);
	}

	const tokens = $derived(parseTokens(p.q).tokens);
	const invalid = $derived(new Set(invalidTokens(p.q).map((t) => `${t.key}:${t.value}`)));
	let showHelp = $state(false);

	const countText = (n: number | undefined) => (list.countsError ? 'unavailable' : n === undefined ? '…' : String(n));
	const tabs = $derived<{ value: RunTab; label: string; count?: string; title: string }[]>([
		{ value: 'all', label: 'All', title: 'Every run' },
		{ value: 'active', label: 'Active', count: countText(list.counts?.active), title: 'Queued, starting, running or with no signal' },
		{ value: 'queued', label: 'Queued', count: countText(list.counts?.queued), title: 'Waiting for their turn' },
		{ value: 'failed', label: 'Failed', title: 'Failed, partial, or lost contact' },
		{ value: 'cancelled', label: 'Cancelled', title: 'Cancelled runs' }
	]);

	const freshnessText = { live: 'Live', reconnecting: 'Reconnecting', stale: 'Stale' } as const;
	const freshnessTitle = {
		live: 'Updates arrive as runs change',
		reconnecting: 'Lost the live connection; trying again. The list may be behind.',
		stale: 'Not updating. Refresh to load the latest runs.'
	} as const;
</script>

<div class="tabs-bar">
	<div class="tabs" role="tablist">
		{#each tabs as t (t.value)}
			<button role="tab" aria-selected={p.tab === t.value} class:on={p.tab === t.value} title={t.title} onclick={() => list.update({ tab: t.value })}>
				{t.label}{#if t.count !== undefined}<span class="count" class:na={t.count === 'unavailable'} title={t.count === 'unavailable' ? "Couldn't load the count" : undefined}>{t.count}</span>{/if}
			</button>
		{/each}
	</div>
	<div class="right">
		<SegmentedFilter
			bind:value={() => p.view, (v: RunsView) => list.update({ view: v })}
			options={[
				{ value: 'list', label: 'List' },
				{ value: 'timeline', label: 'Timeline' },
				{ value: 'queue', label: 'Queue', count: queued ?? undefined }
			]}
		/>
		<span class="fresh {list.freshness}" title={freshnessTitle[list.freshness]}>
			{#if list.updatedAt}<span class="ago">updated {formatAgo(new Date(list.updatedAt).toISOString(), app.now)}</span>{/if}
			<i></i>{freshnessText[list.freshness]}
			<button class="refresh" onclick={() => list.refresh()} aria-label="Refresh now" title="Refresh now"><RefreshCwIcon size={12} /></button>
		</span>
	</div>
</div>

{#if p.view === 'list'}
	<div class="filter-bar">
		<label class="search">
			<SearchIcon size={14} />
			<input value={draft} oninput={(e) => onInput(e.currentTarget.value)} placeholder="Search, or filter with job:… status:… node:…" aria-label="Search and filter runs" />
			{#if draft}<button onclick={() => (onInput(''), list.update({ q: '' }))} aria-label="Clear search"><XIcon size={13} /></button>{/if}
		</label>
		<div class="help-wrap">
			<button class="help-btn" onclick={() => (showHelp = !showHelp)} aria-expanded={showHelp}>Filters?</button>
			{#if showHelp}
				<div class="help" role="dialog" aria-label="Filter tokens">
					<p>Type tokens, or click a job, schedule, sensor, person or kind in a row.</p>
					<dl>
						{#each Object.entries(TOKEN_HELP) as [key, help] (key)}<div><dt>{key}:</dt><dd>{help}</dd></div>{/each}
					</dl>
					<p class="faint">Repeat a key or use commas for “any of”: <code>status:failed,partial</code>.</p>
				</div>
			{/if}
		</div>
		<ChoiceFilter
			label="Started"
			bind:value={() => (p.range === 'any' ? '' : p.range), (v: RangePreset | '') => list.update({ range: v || 'any' })}
			all="Any time"
			options={[
				{ value: '24h', label: 'Last 24 hours' },
				{ value: '7d', label: 'Last 7 days' },
				{ value: '30d', label: 'Last 30 days' },
				{ value: 'custom', label: 'Between dates…' }
			]}
		/>
		{#if p.range === 'custom'}
			<input class="date" type="date" value={p.from} onchange={(e) => list.update({ from: e.currentTarget.value })} aria-label="From date" />
			<span class="faint">to</span>
			<input class="date" type="date" value={p.to} onchange={(e) => list.update({ to: e.currentTarget.value })} aria-label="To date" />
		{/if}
		<label class="toggle" title="Group runs start one run per job; show those as their own rows">
			<input type="checkbox" class="sb-check" checked={p.children} onchange={(e) => list.update({ children: e.currentTarget.checked })} />
			Show child runs
		</label>
	</div>
	{#if tokens.length}
		<div class="tokens">
			{#each tokens as t (`${t.key}:${t.value}`)}
				<span class="token" class:bad={invalid.has(`${t.key}:${t.value}`)} title={invalid.has(`${t.key}:${t.value}`) ? `Not a known value; ${TOKEN_HELP[t.key]}` : undefined}>
					<b>{t.key}</b>{t.value}
					<button onclick={() => list.update({ q: removeToken(p.q, t.key, t.value) })} aria-label="Remove filter {t.key}:{t.value}"><XIcon size={11} /></button>
				</span>
			{/each}
			<button class="clear" onclick={() => list.update({ q: parseTokens(p.q).text })}>Clear filters</button>
		</div>
	{/if}
{/if}

<style>
	.tabs-bar {
		display: flex;
		flex: 0 0 auto;
		align-items: center;
		gap: 12px;
		padding: 0 20px;
		border-bottom: 1px solid var(--border);
	}
	.tabs {
		display: flex;
		gap: 2px;
	}
	.tabs button {
		display: inline-flex;
		align-items: center;
		gap: 6px;
		padding: 12px 10px 10px;
		border-bottom: 2px solid transparent;
		color: var(--muted-foreground);
		font-size: 13px;
		font-weight: 500;
	}
	.tabs button:hover {
		color: var(--foreground);
	}
	.tabs button.on {
		border-color: var(--primary);
		color: var(--foreground);
	}
	.count {
		padding: 0 6px;
		border-radius: 999px;
		background: var(--sb-hover);
		color: var(--muted-foreground);
		font-size: 11.5px;
		font-variant-numeric: tabular-nums;
	}
	.count.na {
		font-style: italic;
	}
	.right {
		display: flex;
		align-items: center;
		gap: 14px;
		margin-left: auto;
	}
	.fresh {
		display: inline-flex;
		align-items: center;
		gap: 6px;
		color: var(--muted-foreground);
		font-size: 12px;
		white-space: nowrap;
	}
	.fresh .ago {
		color: var(--sb-text-faint);
	}
	.fresh i {
		width: 7px;
		height: 7px;
		border-radius: 50%;
		background: var(--sb-success);
	}
	.fresh.reconnecting i {
		background: var(--sb-warning);
	}
	.fresh.stale i {
		background: var(--sb-error);
	}
	.refresh {
		display: grid;
		width: 24px;
		height: 24px;
		place-items: center;
		border: 1px solid var(--border);
		border-radius: 2px;
	}
	.refresh:hover {
		background: var(--sb-hover);
	}
	.filter-bar {
		display: flex;
		flex: 0 0 auto;
		align-items: center;
		gap: 8px;
		padding: 9px 20px;
		border-bottom: 1px solid var(--border);
	}
	.search {
		display: flex;
		width: 380px;
		height: 30px;
		align-items: center;
		gap: 7px;
		padding: 0 9px;
		border: 1px solid var(--border);
		border-radius: 2px;
		background: var(--background);
		color: var(--sb-text-faint);
	}
	.search:focus-within {
		border-color: color-mix(in srgb, var(--primary) 55%, var(--border));
	}
	.search input {
		min-width: 0;
		flex: 1;
		border: 0;
		outline: 0;
		background: transparent;
		color: var(--foreground);
		font-family: var(--font-mono);
		font-size: 12px;
	}
	.help-wrap {
		position: relative;
	}
	.help-btn {
		color: var(--muted-foreground);
		font-size: 12px;
		text-decoration: underline dotted;
	}
	.help {
		position: absolute;
		top: calc(100% + 6px);
		left: 0;
		z-index: 60;
		width: 420px;
		padding: 12px;
		border: 1px solid var(--border);
		border-radius: 2px;
		background: var(--popover);
		box-shadow: var(--sb-elev);
		font-size: 12.5px;
	}
	.help dl div {
		display: grid;
		grid-template-columns: 80px 1fr;
		padding: 2px 0;
	}
	.help dt,
	code {
		font-family: var(--font-mono);
		font-size: 12px;
	}
	.help p {
		margin-bottom: 6px;
	}
	.faint {
		color: var(--sb-text-faint);
		font-size: 12px;
	}
	.date {
		height: 28px;
		padding: 0 6px;
		border: 1px solid var(--border);
		border-radius: 2px;
		background: var(--background);
		color: var(--foreground);
		font-size: 12px;
	}
	.toggle {
		display: inline-flex;
		align-items: center;
		gap: 6px;
		margin-left: auto;
		color: var(--muted-foreground);
		font-size: 12.5px;
		cursor: pointer;
	}
	.tokens {
		display: flex;
		flex: 0 0 auto;
		flex-wrap: wrap;
		align-items: center;
		gap: 6px;
		padding: 7px 20px;
		border-bottom: 1px solid var(--border);
	}
	.token {
		display: inline-flex;
		align-items: center;
		gap: 5px;
		padding: 1px 4px 1px 8px;
		border: 1px solid color-mix(in srgb, var(--primary) 35%, var(--border));
		border-radius: 2px;
		background: color-mix(in srgb, var(--primary) 7%, transparent);
		font-family: var(--font-mono);
		font-size: 11.5px;
	}
	.token b {
		color: var(--muted-foreground);
		font-weight: 500;
	}
	.token b::after {
		content: ':';
	}
	.token.bad {
		border-color: var(--sb-warning);
		text-decoration: line-through;
	}
	.token button {
		display: grid;
		place-items: center;
		color: var(--sb-text-faint);
	}
	.clear {
		color: var(--muted-foreground);
		font-size: 12px;
	}
	.clear:hover,
	.token button:hover {
		color: var(--foreground);
	}
</style>
