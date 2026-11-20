<script lang="ts">
	import { countBy } from '$lib/list/count-by';
	import { plural } from '$lib/format/plural';
	import DataTable from '$lib/components/list/data-table.svelte';
	import type { TableColumn } from '$lib/components/list/table-columns';
	import Button from '$lib/components/ui/button/button.svelte';
	import { to } from '$lib/paths';
	import DisclosureButton from '$lib/components/disclosure-button.svelte';
	import DownloadIcon from '@lucide/svelte/icons/download';
	import SearchIcon from '@lucide/svelte/icons/search';
	import type { RunEventLevel } from '$lib/api';
	import { formatStamp } from '$lib/format/time';
	import { nodeLabel, shortRunId } from '$lib/operate/labels';
	import type { EventCategory } from './types';
	import { EVENT_CATEGORIES, type LogEntry } from './utils';
	import { fullStamp } from './utils/proto';
	import SegmentedFilter from '$lib/components/list/segmented-filter.svelte';

	let {
		entries,
		grouped,
		runId,
		filteredBy,
		printed = $bindable(false),
		onnode,
		onstatement
	}: {
		entries: LogEntry[];
		grouped: boolean;
		runId: string;
		filteredBy: string[];
		/** Whether what the run printed is shown; a failing run's last printed block always is. */
		printed?: boolean;
		/** A node name clicked: the log shows only that node. */
		onnode?: (nodeKey: string) => void;
		/** Opens a query's statement, with its SQL. */
		onstatement?: (nodeKey: string) => void;
	} = $props();

	// One choice each, like the Checks page's outcome filter: All, or a single level or kind.
	const LEVELS: { value: RunEventLevel | 'all'; label: string }[] = [
		{ value: 'all', label: 'All' },
		{ value: 'error', label: 'Errors' },
		{ value: 'warn', label: 'Warnings' },
		{ value: 'info', label: 'Info' },
		{ value: 'debug', label: 'Debug' }
	];
	let level = $state<RunEventLevel | 'all'>('all');
	let category = $state<EventCategory | 'all'>('all');
	let query = $state('');
	// Long printed blocks fold to their first lines.
	const FOLD = 4;
	let open = $state<Set<string>>(new Set());
	// Nodes whose queries are listed under their outcome row.
	let expanded = $state<Set<string>>(new Set());

	// Printed output has its own switch; a kind chip shows only when the log has that kind.
	const present = $derived(EVENT_CATEGORIES.filter((c) => c.value !== 'output' && entries.some((e) => e.category === c.value)));
	const hasOutput = $derived(entries.some((e) => e.category === 'output'));
	const hidden = $derived(entries.filter((e) => e.category === 'output' && !e.pinned).length);
	const kindShown = (e: LogEntry) => (e.category === 'output' ? printed || e.pinned : category === 'all' || e.category === category);
	const levelShown = (e: LogEntry) => level === 'all' || e.level === level;
	const q = $derived(query.trim().toLowerCase());
	const matches = (e: LogEntry) => !q || [e.nodeKey ?? '', e.text, e.title, ...e.queries.map((x) => x.text)].join(' ').toLowerCase().includes(q);
	const byCategory = $derived(entries.filter(kindShown));
	const byLevel = $derived(entries.filter((e) => levelShown(e) && e.category !== 'output'));
	const shown = $derived(entries.filter((e) => levelShown(e) && kindShown(e) && matches(e)));
	const levelCounts = $derived(countBy(byCategory, (e) => e.level));
	const categoryCounts = $derived(countBy(byLevel, (e) => e.category));
	const levelOptions = $derived(LEVELS.map((l) => ({ ...l, count: l.value === 'all' ? byCategory.length : (levelCounts.get(l.value) ?? 0) })));
	const kindOptions = $derived([{ value: 'all' as const, label: 'All kinds', count: byLevel.length }, ...present.map((c) => ({ value: c.value, label: c.label, count: categoryCounts.get(c.value) ?? 0 }))]);

	function toggle<T>(set: Set<T>, value: T): Set<T> {
		const next = new Set(set);
		if (next.has(value)) next.delete(value);
		else next.add(value);
		return next;
	}

	function download() {
		const line = (e: LogEntry, indent = '') => [e.at, grouped ? e.runId : null, e.level, e.nodeKey ?? '', indent + e.text].filter((v) => v !== null).join('\t');
		const text = shown.flatMap((e) => [line(e), ...e.queries.map((x) => line(x, '  '))]).join('\n');
		const url = URL.createObjectURL(new Blob([text + '\n'], { type: 'text/plain' }));
		const a = document.createElement('a');
		a.href = url;
		a.download = `${runId}${filteredBy.length ? `-${filteredBy.join('-')}` : ''}.log`;
		a.click();
		URL.revokeObjectURL(url);
	}
	const logColumns = $derived<TableColumn[]>([
		// Full timestamps, never cropped (CHI-1591, change 6).
		{ id: 'time', label: 'Time', width: '206px' },
		...(grouped ? [{ id: 'run', label: 'Job run', width: '100px' }] : []),
		{ id: 'node', label: 'Node', width: '220px' },
		{ id: 'what', label: 'What happened', minWidth: 300, truncate: false }
	]);
</script>

<div class="filters">
	<SegmentedFilter bind:value={level} options={levelOptions} />
	{#if present.length > 1}<SegmentedFilter bind:value={category} options={kindOptions} />{/if}
	{#if hasOutput}
		<label class="switch" title="What sqb printed while the run worked. A failing run's last block, with its failures, is always shown.">
			<input type="checkbox" class="sb-check" bind:checked={printed} /> Show printed output <span>{hidden}</span>
		</label>
	{/if}
	<label class="search">
		<SearchIcon size={13} />
		<input bind:value={query} placeholder="Search the log" aria-label="Search the log" />
	</label>
	<Button size="sm" onclick={download} disabled={!shown.length} title="Download the rows shown as a text file"><DownloadIcon />Download</Button>
</div>

<DataTable class="static compact log" embedded columns={logColumns} rows={shown} key={(e) => e.id} empty={entries.length ? 'Nothing in the log matches these filters.' : 'Nothing logged yet.'}>
	{#snippet row(e)}
			<tr class:error={e.level === 'error'} class:warn={e.level === 'warn'}>
				<td class="faint nowrap mono stamp" title={formatStamp(e.at)}>{fullStamp(e.at)}</td>
				{#if grouped}<td class="mono faint"><a href={to(`/runs/${e.runId}`)}>{shortRunId(e.runId)}</a></td>{/if}
				<td class="mono" class:faint={!e.nodeKey} title={e.nodeKey ? `Show only ${e.nodeKey} in the log` : undefined}>
					{#if e.nodeKey && onnode}<Button variant="link" size="inline" class="font-mono text-[12px]" onclick={() => onnode(e.nodeKey!)}>{nodeLabel(e.nodeKey)}</Button>{:else}{e.nodeKey ? nodeLabel(e.nodeKey) : (e.stream ?? 'run')}{/if}
				</td>
				<td class:out={e.stream}>
					{#if e.stream}
						{@const lines = e.text.split('\n')}
						<span title={e.title}>{open.has(e.id) || lines.length <= FOLD ? e.text : lines.slice(0, FOLD).join('\n')}</span>
						{#if lines.length > FOLD}<Button variant="link" size="inline" class="mt-0.5 flex font-sans text-[11.5px]" onclick={() => (open = toggle(open, e.id))}>{open.has(e.id) ? 'Show less' : `Show all ${lines.length} lines`}</Button>{/if}
					{:else}
						<span class="dot" style:background={e.tone}></span><span title={e.title}>{e.text}</span>
						{#if e.queries.length}
							<DisclosureButton open={expanded.has(e.id)} onclick={() => (expanded = toggle(expanded, e.id))} class="ml-2 gap-0.5 text-[11.5px]">{plural(e.queries.length, 'query', 'queries')}</DisclosureButton>
						{/if}
					{/if}
					{#if e.statementId && e.nodeKey && onstatement && e.category === 'queries'}
						<Button variant="link" size="inline" class="ml-2 font-mono text-[11px]" onclick={() => onstatement(e.nodeKey!)} title="Show this node's queries and their SQL">SQL</Button>
					{/if}
				</td>
			</tr>
			{#if expanded.has(e.id)}
				{#each e.queries as x (x.id)}
					<tr class="query" class:error={x.level === 'error'}>
						<td class="faint nowrap mono stamp" title={formatStamp(x.at)}>{fullStamp(x.at)}</td>
						{#if grouped}<td></td>{/if}
						<td></td>
						<td>
							<span class="dot" style:background={x.tone}></span><span title={x.title}>{x.text}</span>
							{#if x.statementId && x.nodeKey && onstatement}
								<Button variant="link" size="inline" class="ml-2 font-mono text-[11px]" onclick={() => onstatement(x.nodeKey!)} title="Show this node's queries and their SQL">SQL</Button>
							{/if}
						</td>
					</tr>
				{/each}
			{/if}
	{/snippet}
</DataTable>

<style>
	.filters {
		position: sticky;
		top: 0;
		z-index: 2;
		display: flex;
		flex-wrap: wrap;
		align-items: center;
		gap: 6px 8px;
		padding: 6px 16px;
		border-bottom: 1px solid var(--border);
		background: var(--background);
	}
	.switch {
		display: inline-flex;
		align-items: center;
		gap: 5px;
		color: var(--muted-foreground);
		font-size: 11.5px;
		white-space: nowrap;
	}
	.switch span {
		color: var(--sb-text-faint);
	}
	tr.query td {
		padding-top: 3px;
		padding-bottom: 3px;
		border-bottom: 0;
		color: var(--muted-foreground);
		font-size: 11.5px;
	}
	.search {
		display: flex;
		align-items: center;
		gap: 6px;
		margin-left: auto;
		padding: 3px 8px;
		border: 1px solid var(--border);
		border-radius: 2px;
		color: var(--sb-text-faint);
	}
	.search input {
		width: 200px;
		border: 0;
		outline: 0;
		background: transparent;
		color: var(--foreground);
		font-size: 12px;
	}
	td {
		padding: 5px 16px;
		border-bottom: 1px solid var(--border-subtle);
		vertical-align: top;
	}
	tr:hover td {
		background: var(--sb-hover);
	}
	tr.error td {
		background: color-mix(in srgb, var(--sb-error) 9%, transparent);
		color: var(--sb-error);
	}
	tr.warn td {
		background: color-mix(in srgb, var(--sb-warning) 6%, transparent);
	}
	td.out {
		font-family: var(--font-mono);
		font-size: 11.5px;
		white-space: pre-wrap;
		word-break: break-word;
	}
	td.out {
		color: var(--muted-foreground);
	}
	.dot {
		display: inline-block;
		width: 6px;
		height: 6px;
		margin-right: 8px;
		border-radius: 50%;
		vertical-align: middle;
	}
	.mono {
		font-family: var(--font-mono);
		font-size: 11.5px;
	}
	.faint {
		color: var(--muted-foreground);
	}
	.nowrap {
		white-space: nowrap;
	}
	a:hover {
		color: var(--primary);
	}
</style>
