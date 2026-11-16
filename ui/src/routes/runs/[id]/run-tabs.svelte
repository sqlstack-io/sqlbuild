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

	let { view }: { view: RunDetailState } = $props();

	const sel = $derived(new Set(view.selected));
	const filtering = $derived(sel.size > 0);
	const entries = $derived(logEntries(view.eventsRes.data ?? [], view.outputRes.data ?? []));
	const statements = $derived(filtering ? view.statements.filter((s) => sel.has(s.nodeKey)) : view.statements);
	// Printed lines name no node, so a node filter keeps the lines that mention one of the selected nodes.
	const shownEntries = $derived(filtering ? entries.filter((e) => (e.nodeKey ? sel.has(e.nodeKey) : view.selected.some((k) => e.text.includes(k)))) : entries);
	const grouped = $derived((view.detail?.children.length ?? 0) > 0);
	// Kept here so the choice survives switching tabs, and the Log count matches the rows shown.
	let printed = $state(false);
	const logCount = $derived(shownEntries.filter((e) => printed || e.category !== 'output' || e.pinned).length);

	const TABS = $derived(bottomTabs(app.client, (c) => app.can(c), !!view.cost && view.cost.status !== 'none'));
	$effect(() => {
		if (TABS.length && !TABS.some((t) => t.value === view.tab)) view.tab = TABS[0].value;
	});
	const count = (t: BottomTab) => (t === 'statements' ? statements.length : t === 'cost' ? (view.cost?.resources.length ?? 0) : logCount);
	const chipText = $derived(view.selected.length === 1 ? view.selected[0] : `${view.selected.length} nodes`);
</script>

<div class="bar">
	<PageTabs
		flush
		bind:value={view.tab}
		tabs={TABS.map((t) => ({ value: t.value, label: t.label, count: count(t.value), title: t.value === 'statements' ? 'SQL statements the run sent to the warehouse' : t.value === 'cost' ? 'Estimated query cost of this run, by node and warehouse' : undefined }))}
	/>
	{#if filtering}
		<span class="chip" title={view.selected.join(', ')}>
			Filtered: <b>{chipText}</b>
			<Button variant="ghost" size="icon-xs" class="rounded-full" onclick={() => view.clearSelection()} aria-label="Clear the node filter"><XIcon /></Button>
		</span>
	{/if}
</div>

<PageBody padding="0">
	{#if view.tab === 'statements'}
		<StatementsTab {statements} total={view.statements.length} {grouped} />
	{:else if view.tab === 'cost'}
		{#if view.cost}
			<CostTab cost={view.cost} filteredBy={view.selected} onnode={(key) => view.focusNode(key, 'cost')} />
		{:else if view.costRes.error}
			<p class="err">Couldn't load this run's cost: {view.costRes.error}</p>
		{/if}
	{:else}
		<LogTab entries={shownEntries} {grouped} bind:printed runId={view.run?.id ?? ''} filteredBy={view.selected} onstatement={app.client.supports('listRunStatements') ? (key) => view.focusNode(key, 'statements') : undefined} />
	{/if}
</PageBody>

<style>
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
