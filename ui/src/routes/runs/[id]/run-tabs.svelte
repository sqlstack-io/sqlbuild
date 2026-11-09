<script lang="ts">
	import XIcon from '@lucide/svelte/icons/x';
	import { app } from '$lib/state/app.svelte';
	import LogTab from './log-tab.svelte';
	import type { RunDetailState } from './state.svelte';
	import StatementsTab from './statements-tab.svelte';
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

	const TABS = $derived(bottomTabs(app.client, (c) => app.can(c)));
	$effect(() => {
		if (TABS.length && !TABS.some((t) => t.value === view.tab)) view.tab = TABS[0].value;
	});
	const count = (t: BottomTab) => (t === 'statements' ? statements.length : logCount);
	const chipText = $derived(view.selected.length === 1 ? view.selected[0] : `${view.selected.length} nodes`);
</script>

<div class="bar">
	{#each TABS as t (t.value)}
		<button class:on={view.tab === t.value} onclick={() => (view.tab = t.value)} title={t.value === 'statements' ? 'SQL statements the run sent to the warehouse' : undefined}>
			{t.label} <span>{count(t.value)}</span>
		</button>
	{/each}
	{#if filtering}
		<span class="chip" title={view.selected.join(', ')}>
			Filtered: <b>{chipText}</b>
			<button onclick={() => view.clearSelection()} aria-label="Clear the node filter"><XIcon size={12} /></button>
		</span>
	{:else}
		<span class="hint">Click a node above to filter</span>
	{/if}
</div>

<div class="body sb-scroll">
	{#if view.tab === 'statements'}
		<StatementsTab {statements} total={view.statements.length} {grouped} />
	{:else}
		<LogTab entries={shownEntries} {grouped} bind:printed runId={view.run?.id ?? ''} filteredBy={view.selected} onstatement={app.client.supports('listRunStatements') ? (key) => view.focusNode(key, 'statements') : undefined} />
	{/if}
</div>

<style>
	.bar {
		display: flex;
		flex: 0 0 auto;
		align-items: center;
		gap: 2px;
		padding: 0 16px;
		border-bottom: 1px solid var(--border);
	}
	.bar > button {
		padding: 9px 12px;
		border-bottom: 2px solid transparent;
		color: var(--muted-foreground);
		font-size: 12.5px;
		font-weight: 500;
	}
	.bar > button span {
		color: var(--sb-text-faint);
		font-weight: 400;
	}
	.bar > button.on {
		border-color: var(--primary);
		color: var(--foreground);
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
	.chip button {
		display: grid;
		width: 18px;
		height: 18px;
		place-items: center;
		border-radius: 50%;
		color: var(--muted-foreground);
	}
	.chip button:hover {
		background: var(--sb-hover);
		color: var(--foreground);
	}
	.hint {
		margin-left: 12px;
		color: var(--sb-text-faint);
		font-size: 12px;
	}
	.body {
		min-height: 0;
		flex: 1;
		overflow: auto;
	}
	.sb-scroll {
		scrollbar-width: thin;
		scrollbar-color: var(--border-strong) transparent;
	}
</style>
