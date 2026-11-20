<script lang="ts">
	import PageBody from '$lib/components/page-body.svelte';
	import PageTabs from '$lib/components/platform/page-tabs.svelte';
	import Button from '$lib/components/ui/button/button.svelte';
	import XIcon from '@lucide/svelte/icons/x';
	import { app } from '$lib/state/app.svelte';
	import LogTab from './log-tab.svelte';
	import type { RunDetailState } from './state.svelte';
	import StatementsTab from './statements-tab.svelte';
	import CostTab from './cost-tab.svelte';
	import type { BottomTab } from './types';
	import { bottomTabs, logEntries } from './utils';
	import { tick } from 'svelte';
	import { plural } from '$lib/format/plural';
	import { skipCause } from './utils/proto';

	let { view }: { view: RunDetailState } = $props();

	// The log follows a summary figure or a node picked in the log, else the selection (until "Show all").
	let ignoreSelection = $state(false);
	let lastSelection = '';
	$effect(() => {
		const key = view.selected.join(',');
		if (key !== lastSelection) {
			lastSelection = key;
			ignoreSelection = false;
		}
	});
	const filterKeys = $derived(view.focus ? view.focus.keys : ignoreSelection ? [] : view.selected);
	const sel = $derived(new Set(filterKeys));
	const filtering = $derived(sel.size > 0);
	// One line per node outcome, with its checks folded in (CHI-1591, change 3); `app.now` keeps a building node's line current.
	const edges = $derived(view.detail?.edges ?? []);
	const entries = $derived(logEntries(view.eventsRes.data ?? [], view.outputRes.data ?? [], { nodes: view.nodes, skipCause: (n) => skipCause(n, view.nodes, edges), now: app.now }));
	const statements = $derived(filtering ? view.statements.filter((s) => sel.has(s.nodeKey)) : view.statements);
	// Printed lines carry their node when sqb printed them for one; others are kept when they mention it.
	const shownEntries = $derived(filtering ? entries.filter((e) => (e.nodeKey ? sel.has(e.nodeKey) : filterKeys.some((k) => e.text.includes(k)))) : entries);
	const grouped = $derived((view.detail?.children.length ?? 0) > 0);
	// Kept here so the choice survives switching tabs, and the Log count matches the rows shown.
	// On by default (CHI-1591, change 5): what sqb printed is the proof the run is moving.
	let printed = $state(true);
	const logCount = $derived(shownEntries.filter((e) => printed || e.category !== 'output' || e.pinned).length);

	const TABS = $derived(bottomTabs(app.client, (c) => app.can(c), !!view.cost && view.cost.status !== 'none'));
	$effect(() => {
		if (TABS.length && !TABS.some((t) => t.value === view.tab)) view.tab = TABS[0].value;
	});
	const count = (t: BottomTab) => (t === 'statements' ? statements.length : t === 'cost' ? (view.cost?.resources.length ?? 0) : logCount);
	const chipText = $derived(view.focus ? view.focus.label : filterKeys.length === 1 ? filterKeys[0] : `${filterKeys.length} selected nodes`);
	function showAll() {
		if (view.focus) view.clearFocus();
		else ignoreSelection = true;
	}

	// ---- the log follows new lines while it is at the bottom (CHI-1591, change 7) ----
	let scroller = $state<HTMLDivElement>();
	// A running run's log opens at its newest line; a finished run's at its first.
	let atBottom = $state(untrackActive());
	function untrackActive() {
		return view.active;
	}
	let seen = $state(0);
	const fresh = $derived(Math.max(0, logCount - seen));
	function onscroll() {
		if (!scroller) return;
		atBottom = scroller.scrollTop + scroller.clientHeight >= scroller.scrollHeight - 24;
		if (atBottom) seen = logCount;
	}
	async function toBottom() {
		await tick();
		if (!scroller) return;
		scroller.scrollTop = scroller.scrollHeight;
		atBottom = true;
		seen = logCount;
	}
	// A new filter or the printed-output switch changes what is shown, not what is new.
	let shownKey = '';
	$effect(() => {
		const key = `${filterKeys.join(',')}|${printed}`;
		if (key === shownKey) return;
		shownKey = key;
		seen = untrackCount();
	});
	function untrackCount() {
		return logCount;
	}
	$effect(() => {
		void logCount;
		if (view.tab !== 'log') return;
		// Only a run still going adds lines; a finished run's log arrives in parts, which are not new.
		if (!view.active) seen = logCount;
		if (atBottom) void toBottom();
	});
</script>

<div class="bar">
	<PageTabs
		flush
		bind:value={view.tab}
		tabs={TABS.map((t) => ({ value: t.value, label: t.label, count: count(t.value), title: t.value === 'statements' ? 'SQL statements the run sent to the warehouse' : t.value === 'cost' ? 'Estimated query cost of this run, by node and warehouse' : undefined }))}
	/>
	{#if filtering}
		<span class="chip" title={view.selected.join(', ')}>
			Filtered to <b>{chipText}</b>
			<Button variant="ghost" size="icon-xs" class="rounded-full" onclick={showAll} aria-label="Show all" title="Show the whole log"><XIcon /></Button>
		</span>
	{/if}
</div>

<div class="log-area">
<PageBody padding="0" bind:scroller {onscroll}>
	{#if view.tab === 'statements'}
		<StatementsTab {statements} total={view.statements.length} {grouped} />
	{:else if view.tab === 'cost'}
		{#if view.cost}
			<CostTab cost={view.cost} filteredBy={view.selected} onnode={(key) => view.focusNode(key, 'cost')} />
		{:else if view.costRes.error}
			<p class="err">Couldn't load this run's cost: {view.costRes.error}</p>
		{/if}
	{:else}
		<LogTab entries={shownEntries} {grouped} memberOf={(id) => view.detail?.children.find((c) => c.id === id)?.jobKey ?? null} bind:printed runId={view.run?.id ?? ''} filteredBy={filterKeys} onnode={(key) => view.focusNode(key, 'log')} onstatement={app.client.supports('listRunStatements') ? (key) => view.focusNode(key, 'statements') : undefined} />
	{/if}
</PageBody>
{#if view.tab === 'log' && !atBottom && fresh > 0}
	<!-- raw-button: the jump to the newest lines, floating over the log -->
	<button class="new-lines" onclick={toBottom} data-testid="new-lines">↓ {plural(fresh, 'new line')}</button>
{/if}
</div>

<style>
	.log-area {
		position: relative;
		display: flex;
		min-height: 0;
		flex: 1;
		flex-direction: column;
	}
	.new-lines {
		position: absolute;
		right: 18px;
		bottom: 12px;
		padding: 4px 12px;
		border-radius: 999px;
		background: var(--primary);
		box-shadow: 0 2px 8px rgb(0 0 0 / 0.25);
		color: var(--primary-foreground);
		font-size: 12px;
		font-weight: 500;
	}
	.err {
		padding: 12px 16px;
		color: var(--destructive);
		font-size: 12.5px;
	}
	.bar {
		display: flex;
		flex: 0 0 auto;
		align-items: center;
		gap: 2px;
		padding: 0 16px;
		border-bottom: 1px solid var(--border);
	}
	.chip {
		display: inline-flex;
		align-items: center;
		gap: 6px;
		margin-left: 12px;
		padding: 1px 4px 1px 9px;
		border: 1px solid color-mix(in srgb, var(--primary) 40%, var(--border));
		border-radius: 999px;
		background: color-mix(in srgb, var(--primary) 8%, transparent);
		font-size: 12px;
	}
	.chip b {
		font-family: var(--font-mono);
		font-size: 11.5px;
		font-weight: 500;
	}
</style>
