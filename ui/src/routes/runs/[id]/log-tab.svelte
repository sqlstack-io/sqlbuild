<script lang="ts">
	import ChevronRightIcon from '@lucide/svelte/icons/chevron-right';
	import DownloadIcon from '@lucide/svelte/icons/download';
	import SearchIcon from '@lucide/svelte/icons/search';
	import type { RunEventLevel } from '$lib/api';
	import { formatStamp } from '$lib/format/time';
	import { clockTime, shortRunId } from '$lib/operate/labels';
	import type { EventCategory } from './types';
	import { EVENT_CATEGORIES, type LogEntry } from './utils';

	let {
		entries,
		grouped,
		runId,
		filteredBy,
		printed = $bindable(false),
		onstatement
	}: {
		entries: LogEntry[];
		grouped: boolean;
		runId: string;
		filteredBy: string[];
		/** Whether what the run printed is shown; a failing run's last printed block always is. */
		printed?: boolean;
		/** Opens a query's statement, with its SQL. */
		onstatement?: (nodeKey: string) => void;
	} = $props();

	const LEVELS: { value: RunEventLevel; label: string }[] = [
		{ value: 'error', label: 'Errors' },
		{ value: 'warn', label: 'Warnings' },
		{ value: 'info', label: 'Info' },
		{ value: 'debug', label: 'Debug' }
	];
	let levels = $state<Set<RunEventLevel>>(new Set(['info', 'warn', 'error']));
	let categories = $state<Set<EventCategory>>(new Set(['run', 'nodes', 'queries', 'checks']));
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
	const kindShown = (e: LogEntry) => (e.category === 'output' ? printed || e.pinned : categories.has(e.category));
	const q = $derived(query.trim().toLowerCase());
	const matches = (e: LogEntry) => !q || [e.nodeKey ?? '', e.text, e.title, ...e.queries.map((x) => x.text)].join(' ').toLowerCase().includes(q);
	const byCategory = $derived(entries.filter(kindShown));
	const byLevel = $derived(entries.filter((e) => levels.has(e.level)));
	const shown = $derived(entries.filter((e) => levels.has(e.level) && kindShown(e) && matches(e)));
	const levelCount = (l: RunEventLevel) => byCategory.filter((e) => e.level === l).length;
	const categoryCount = (c: EventCategory) => byLevel.filter((e) => e.category === c).length;

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
</script>

<div class="filters">
	<div class="chips" role="group" aria-label="Levels">
		{#each LEVELS as l (l.value)}
			<button class:on={levels.has(l.value)} onclick={() => (levels = toggle(levels, l.value))}>{l.label} <span>{levelCount(l.value)}</span></button>
		{/each}
	</div>
	<div class="chips" role="group" aria-label="Kinds">
		{#each present as c (c.value)}
			<button class:on={categories.has(c.value)} onclick={() => (categories = toggle(categories, c.value))}>{c.label} <span>{categoryCount(c.value)}</span></button>
		{/each}
	</div>
	{#if hasOutput}
		<label class="switch" title="What sqb printed while the run worked. A failing run's last block, with its failures, is always shown.">
			<input type="checkbox" bind:checked={printed} /> Show printed output <span>{hidden}</span>
		</label>
	{/if}
	<label class="search">
		<SearchIcon size={13} />
		<input bind:value={query} placeholder="Search the log" aria-label="Search the log" />
	</label>
	<button class="dl" onclick={download} disabled={!shown.length} title="Download the rows shown as a text file"><DownloadIcon size={13} /> Download</button>
</div>

<table class="log">
	<colgroup>
		<col style:width="110px" />
		{#if grouped}<col style:width="100px" />{/if}
		<col style:width="220px" />
		<col />
	</colgroup>
	<thead>
		<tr>
			<th>Time</th>
			{#if grouped}<th>Job run</th>{/if}
			<th>Node</th>
			<th>What happened</th>
		</tr>
	</thead>
	<tbody>
		{#each shown as e (e.id)}
			<tr class:error={e.level === 'error'} class:warn={e.level === 'warn'}>
				<td class="faint nowrap" title={formatStamp(e.at)}>{clockTime(e.at)}</td>
				{#if grouped}<td class="mono faint"><a href="/runs/{e.runId}">{shortRunId(e.runId)}</a></td>{/if}
				<td class="mono" class:faint={!e.nodeKey}>{e.nodeKey ?? e.stream ?? 'run'}</td>
				<td class:out={e.stream}>
					{#if e.stream}
						{@const lines = e.text.split('\n')}
						<span title={e.title}>{open.has(e.id) || lines.length <= FOLD ? e.text : lines.slice(0, FOLD).join('\n')}</span>
						{#if lines.length > FOLD}<button class="more" onclick={() => (open = toggle(open, e.id))}>{open.has(e.id) ? 'Show less' : `Show all ${lines.length} lines`}</button>{/if}
					{:else}
						<span class="dot" style:background={e.tone}></span><span title={e.title}>{e.text}</span>
						{#if e.queries.length}
							<button class="fold" class:open={expanded.has(e.id)} onclick={() => (expanded = toggle(expanded, e.id))} aria-expanded={expanded.has(e.id)}>
								<ChevronRightIcon size={12} />{e.queries.length} {e.queries.length === 1 ? 'query' : 'queries'}
							</button>
						{/if}
					{/if}
					{#if e.statementId && e.nodeKey && onstatement && e.category === 'queries'}
						<button class="sql" onclick={() => onstatement(e.nodeKey!)} title="Show this node's queries and their SQL">SQL</button>
					{/if}
				</td>
			</tr>
			{#if expanded.has(e.id)}
				{#each e.queries as x (x.id)}
					<tr class="query" class:error={x.level === 'error'}>
						<td class="faint nowrap" title={formatStamp(x.at)}>{clockTime(x.at)}</td>
						{#if grouped}<td></td>{/if}
						<td></td>
						<td>
							<span class="dot" style:background={x.tone}></span><span title={x.title}>{x.text}</span>
							{#if x.statementId && x.nodeKey && onstatement}
								<button class="sql" onclick={() => onstatement(x.nodeKey!)} title="Show this node's queries and their SQL">SQL</button>
							{/if}
						</td>
					</tr>
				{/each}
			{/if}
		{:else}
			<tr><td colspan={grouped ? 4 : 3} class="empty">{entries.length ? 'Nothing in the log matches these filters.' : 'Nothing logged yet.'}</td></tr>
		{/each}
	</tbody>
</table>

<style>
	.filters {
		position: sticky;
		top: 0;
		z-index: 2;
		display: flex;
		align-items: center;
		gap: 8px;
		padding: 6px 16px;
		border-bottom: 1px solid var(--border);
		background: var(--background);
	}
	.chips {
		display: flex;
		gap: 2px;
		padding: 2px;
		border-radius: 2px;
		background: var(--sb-hover);
	}
	.chips button {
		padding: 2px 8px;
		border-radius: 2px;
		color: var(--muted-foreground);
		font-size: 11.5px;
	}
	.chips button.on {
		background: var(--card);
		color: var(--foreground);
		box-shadow: 0 1px 2px rgba(0, 0, 0, 0.12);
	}
	.chips span {
		color: var(--sb-text-faint);
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
	.fold {
		display: inline-flex;
		align-items: center;
		gap: 2px;
		margin-left: 8px;
		color: var(--muted-foreground);
		font-size: 11.5px;
	}
	.fold:hover {
		color: var(--primary);
	}
	.fold :global(svg) {
		transition: transform 0.12s;
	}
	.fold.open :global(svg) {
		transform: rotate(90deg);
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
	.dl {
		display: inline-flex;
		align-items: center;
		gap: 5px;
		padding: 3px 8px;
		border: 1px solid var(--border);
		border-radius: 2px;
		color: var(--muted-foreground);
		font-size: 12px;
	}
	.dl:disabled {
		opacity: 0.5;
	}
	.log {
		width: 100%;
		border-collapse: collapse;
		font-size: 12px;
	}
	th {
		padding: 6px 16px;
		border-bottom: 1px solid var(--border);
		color: var(--sb-text-faint);
		font-size: 11.5px;
		font-weight: 500;
		text-align: left;
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
	.more {
		display: block;
		margin-top: 2px;
		color: var(--primary);
		font-family: var(--font-sans);
		font-size: 11.5px;
	}
	.dot {
		display: inline-block;
		width: 6px;
		height: 6px;
		margin-right: 8px;
		border-radius: 50%;
		vertical-align: middle;
	}
	.sql {
		margin-left: 8px;
		padding: 0 5px;
		border: 1px solid var(--border);
		border-radius: 2px;
		color: var(--muted-foreground);
		font-family: var(--font-mono);
		font-size: 10.5px;
	}
	.sql:hover {
		color: var(--primary);
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
	.empty {
		padding: 32px;
		color: var(--sb-text-faint);
		text-align: center;
	}
	a:hover {
		color: var(--primary);
	}
</style>
