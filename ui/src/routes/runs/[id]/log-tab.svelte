<script lang="ts">
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
		onstatement
	}: {
		entries: LogEntry[];
		grouped: boolean;
		runId: string;
		filteredBy: string[];
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
	let categories = $state<Set<EventCategory>>(new Set(['run', 'nodes', 'queries', 'checks', 'output']));
	let query = $state('');
	// Long printed blocks fold to their first lines.
	const FOLD = 4;
	let open = $state<Set<string>>(new Set());

	const present = $derived(EVENT_CATEGORIES.filter((c) => c.value !== 'output' || entries.some((e) => e.category === 'output')));
	const q = $derived(query.trim().toLowerCase());
	const byCategory = $derived(entries.filter((e) => categories.has(e.category)));
	const byLevel = $derived(entries.filter((e) => levels.has(e.level)));
	const shown = $derived(entries.filter((e) => levels.has(e.level) && categories.has(e.category) && (!q || `${e.nodeKey ?? ''} ${e.text} ${e.title}`.toLowerCase().includes(q))));
	const levelCount = (l: RunEventLevel) => byCategory.filter((e) => e.level === l).length;
	const categoryCount = (c: EventCategory) => byLevel.filter((e) => e.category === c).length;

	function toggle<T>(set: Set<T>, value: T): Set<T> {
		const next = new Set(set);
		if (next.has(value)) next.delete(value);
		else next.add(value);
		return next;
	}

	function download() {
		const text = shown.map((e) => [e.at, grouped ? e.runId : null, e.level, e.nodeKey ?? '', e.text].filter((v) => v !== null).join('\t')).join('\n');
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
					{/if}
					{#if e.statementId && e.nodeKey && onstatement && e.category === 'queries'}
						<button class="sql" onclick={() => onstatement(e.nodeKey!)} title="Show this node's queries and their SQL">SQL</button>
					{/if}
				</td>
			</tr>
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
