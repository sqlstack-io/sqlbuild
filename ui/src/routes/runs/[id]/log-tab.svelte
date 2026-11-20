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
	import * as DropdownMenu from '$lib/components/ui/dropdown-menu';
	import ChevronDownIcon from '@lucide/svelte/icons/chevron-down';

	let {
		entries,
		grouped,
		runId,
		filteredBy,
		printed = $bindable(false),
		memberOf = () => null,
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
		/** A group run's member job, by member run, for the quiet tag beside a node. */
		memberOf?: (runId: string) => string | null;
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

	// Events | Output: the run's events, or what sqb printed (`printed`). A kind shows in the menu only when the log has it.
	const present = $derived(EVENT_CATEGORIES.filter((c) => c.value !== 'output' && entries.some((e) => e.category === c.value)));
	const hasOutput = $derived(entries.some((e) => e.category === 'output'));
	const kindShown = (e: LogEntry) => (printed ? e.category === 'output' : e.category !== 'output' && (category === 'all' || e.category === category));
	const levelShown = (e: LogEntry) => level === 'all' || e.level === level;
	const q = $derived(query.trim().toLowerCase());
	const matches = (e: LogEntry) => !q || [e.nodeKey ?? '', e.text, e.title, ...e.queries.map((x) => x.text)].join(' ').toLowerCase().includes(q);
	const byCategory = $derived(entries.filter(kindShown));
	const byLevel = $derived(entries.filter((e) => levelShown(e) && (printed ? e.category === 'output' : e.category !== 'output')));
	const levelText = $derived(LEVELS.find((l) => l.value === level)?.label ?? 'All');
	const kindText = $derived(category === 'all' ? 'All kinds' : (present.find((c) => c.value === category)?.label ?? 'All kinds'));
	// Rows whose structured details are open in place; long text stays folded until "show more".
	let detailOpen = $state<Set<string>>(new Set());
	let longOpen = $state<Set<string>>(new Set());
	const shown = $derived(entries.filter((e) => levelShown(e) && kindShown(e) && matches(e)));
	const levelCounts = $derived(countBy(byCategory, (e) => e.level));
	const categoryCounts = $derived(countBy(byLevel, (e) => e.category));
	// A count shows only when it matters: errors and warnings in what the switch shows, none when zero.
	const errors = $derived(levelCounts.get('error') ?? 0);
	const warnings = $derived(levelCounts.get('warn') ?? 0);
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
		// A group run's member job is a quiet tag beside the node, not a column of its own.
		{ id: 'node', label: 'Node', width: grouped ? '320px' : '230px' },
		...(printed ? [] : [{ id: 'tag', label: '', width: '78px' }]),
		{ id: 'what', label: 'What happened', minWidth: 300, truncate: false }
	]);
</script>

<!-- Controls right after what they act on, from the left: what to show, then narrow it (CHI-1591). -->
<div class="filters">
	{#if hasOutput}
		<SegmentedFilter
			label="Show"
			value={printed ? 'output' : 'events'}
			onselect={(v: string) => (printed = v === 'output')}
			options={[
				{ value: 'events', label: 'Events' },
				{ value: 'output', label: 'Output' }
			]}
		/>
	{/if}
	<label class="search">
		<SearchIcon size={13} />
		<input bind:value={query} placeholder="Filter…" aria-label="Filter the log" />
	</label>
	<DropdownMenu.Root>
		<DropdownMenu.Trigger class="menu-trigger" aria-label="Levels">{level === 'all' ? 'Levels' : levelText} <ChevronDownIcon size={13} /></DropdownMenu.Trigger>
		<DropdownMenu.Content class="w-44">
			<DropdownMenu.RadioGroup value={level} onValueChange={(v) => (level = v as RunEventLevel | 'all')}>
				{#each levelOptions as l (l.value)}<DropdownMenu.RadioItem value={l.value}>{l.label}<span class="menu-count">{l.count}</span></DropdownMenu.RadioItem>{/each}
			</DropdownMenu.RadioGroup>
		</DropdownMenu.Content>
	</DropdownMenu.Root>
	{#if !printed && present.length > 1}
		<DropdownMenu.Root>
			<DropdownMenu.Trigger class="menu-trigger" aria-label="Kinds">{category === 'all' ? 'Kinds' : kindText} <ChevronDownIcon size={13} /></DropdownMenu.Trigger>
			<DropdownMenu.Content class="w-44">
				<DropdownMenu.RadioGroup value={category} onValueChange={(v) => (category = v as EventCategory | 'all')}>
					{#each kindOptions as k (k.value)}<DropdownMenu.RadioItem value={k.value}>{k.label}<span class="menu-count">{k.count}</span></DropdownMenu.RadioItem>{/each}
				</DropdownMenu.RadioGroup>
			</DropdownMenu.Content>
		</DropdownMenu.Root>
	{/if}
	{#if errors}<Button variant="link" size="inline" class="count err" onclick={() => (level = level === 'error' ? 'all' : 'error')} title="Show only errors">Errors {errors}</Button>{/if}
	{#if warnings}<Button variant="link" size="inline" class="count warn" onclick={() => (level = level === 'warn' ? 'all' : 'warn')} title="Show only warnings">Warnings {warnings}</Button>{/if}
	<span class="grow"></span>
	<Button variant="ghost" size="sm" onclick={download} disabled={!shown.length} title="Download the rows shown as a text file"><DownloadIcon />Download</Button>
</div>

<DataTable class="static compact log" embedded columns={logColumns} rows={shown} key={(e) => e.id} empty={entries.length ? 'Nothing in the log matches these filters.' : 'Nothing logged yet.'}>
	{#snippet row(e)}
			<tr class:error={e.level === 'error'} class:warn={e.level === 'warn'}>
				<td class="faint nowrap mono stamp" title={formatStamp(e.at)}>{fullStamp(e.at)}</td>
				<td class="mono" class:faint={!e.nodeKey} title={e.nodeKey ? `Show only ${e.nodeKey} in the log` : undefined}>
					{#if e.nodeKey && onnode}<Button variant="link" size="inline" class="font-mono text-[12px]" onclick={() => onnode(e.nodeKey!)}>{nodeLabel(e.nodeKey)}</Button>{:else}{e.nodeKey ? nodeLabel(e.nodeKey) : (e.stream ?? 'run')}{/if}
					{#if grouped && memberOf(e.runId)}<a class="member" href={to(`/runs/${e.runId}`)} title="In member job run {shortRunId(e.runId)}">{memberOf(e.runId)}</a>{/if}
				</td>
				{#if !printed}<td class="tagcell">{#if e.tag}<span class="tag" style:--tone={e.tag.tone}>{e.tag.text}</span>{/if}</td>{/if}
				<td class:out={e.stream} class:opens={!!e.details?.length} onclick={() => e.details?.length && (detailOpen = toggle(detailOpen, e.id))}>
					{#if e.stream}
						{@const lines = e.text.split('\n')}
						<span title={e.title}>{open.has(e.id) || lines.length <= FOLD ? e.text : lines.slice(0, FOLD).join('\n')}</span>
						{#if lines.length > FOLD}<Button variant="link" size="inline" class="mt-0.5 flex font-sans text-[11.5px]" onclick={() => (open = toggle(open, e.id))}>{open.has(e.id) ? 'Show less' : `Show all ${lines.length} lines`}</Button>{/if}
					{:else}
						{#if e.category === 'nodes' && e.id.startsWith('node-')}
							<!-- A node's outcome line: its glyph in its colour, then what it did. -->
							<span><b class="glyph" style:color={e.tone}>{e.text.slice(0, 1)}</b>{e.text.slice(1)}</span>
						{:else}
							<span>{e.text}</span>
						{/if}
						{#if e.details?.length}<span class="caret" class:open={detailOpen.has(e.id)} aria-hidden="true">▸</span>{/if}
						{#if e.queries.length}
							<DisclosureButton open={expanded.has(e.id)} onclick={() => (expanded = toggle(expanded, e.id))} class="ml-2 gap-0.5 text-[11.5px]">{plural(e.queries.length, 'query', 'queries')}</DisclosureButton>
						{/if}
					{/if}
					{#if e.statementId && e.nodeKey && onstatement && e.category === 'queries'}
						<Button variant="link" size="inline" class="ml-2 font-mono text-[11px]" onclick={() => onstatement(e.nodeKey!)} title="Show this node's queries and their SQL">SQL</Button>
					{/if}
				</td>
			</tr>
			{#if detailOpen.has(e.id) && e.details?.length}
				<!-- The row's details, in place: a small key/value table, long text behind "show more". -->
				<tr class="detail">
					<td></td>
					<td colspan={printed ? 2 : 3}>
						<dl>
							{#each e.details as [k, v], i (i)}<dt>{k}</dt><dd>{v}</dd>{/each}
						</dl>
						{#if e.long}
							{@const lines = e.long.split('\n')}
							<pre>{longOpen.has(e.id) ? e.long : lines.slice(0, 3).join('\n')}</pre>
							{#if lines.length > 3}<Button variant="link" size="inline" class="text-[12px]" onclick={() => (longOpen = toggle(longOpen, e.id))}>{longOpen.has(e.id) ? 'Show less' : `Show more (${lines.length} lines)`}</Button>{/if}
						{/if}
					</td>
				</tr>
			{/if}
			{#if expanded.has(e.id)}
				{#each e.queries as x (x.id)}
					<tr class="query" class:error={x.level === 'error'}>
						<td class="faint nowrap mono stamp" title={formatStamp(x.at)}>{fullStamp(x.at)}</td>
						<td></td>
						{#if !printed}<td class="tagcell"></td>{/if}
						<td>
							<span title={x.title}>{x.text}</span>
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
	/* Calmer (CHI-1591): no rules between rows, one text size, muted secondary text, room around the toolbar. */
	.filters {
		position: sticky;
		top: 0;
		z-index: 2;
		display: flex;
		flex-wrap: wrap;
		align-items: center;
		gap: 8px 12px;
		padding: 10px 24px;
		background: var(--background);
	}
	.search {
		display: flex;
		align-items: center;
		gap: 6px;
		padding: 3px 8px;
		border: 1px solid var(--border-subtle);
		border-radius: 2px;
		color: var(--sb-text-faint);
	}
	.search input {
		width: 180px;
		border: 0;
		outline: 0;
		background: transparent;
		color: var(--foreground);
		font-size: 12px;
	}
	.filters :global(.menu-trigger) {
		display: inline-flex;
		align-items: center;
		gap: 4px;
		padding: 3px 4px;
		color: var(--muted-foreground);
		font-size: 12px;
	}
	.filters :global(.menu-trigger:hover) {
		color: var(--foreground);
	}
	:global(.menu-count) {
		margin-left: auto;
		color: var(--sb-text-faint);
		font-size: 11.5px;
	}
	.filters :global(.count) {
		font-size: 12px;
		font-weight: 500;
	}
	.filters :global(.count.err) {
		color: var(--sb-error);
	}
	.filters :global(.count.warn) {
		color: var(--sb-warning);
	}
	.grow {
		flex: 1;
	}
	:global(.sb-table.log) td {
		border-bottom-color: transparent;
	}
	td {
		padding: 5px 16px;
		border-bottom: 0;
		font-size: 12px;
		vertical-align: top;
	}
	tr:hover td {
		background: var(--sb-hover);
	}
	tr.error td {
		background: color-mix(in srgb, var(--sb-error) 6%, transparent);
	}
	tr.warn td {
		background: color-mix(in srgb, var(--sb-warning) 4%, transparent);
	}
	td.out {
		color: var(--muted-foreground);
		font-family: var(--font-mono);
		white-space: pre-wrap;
		word-break: break-word;
	}
	td.opens {
		cursor: pointer;
	}
	.tagcell {
		padding-right: 0;
		padding-left: 0;
	}
	.tag {
		display: inline-block;
		min-width: 52px;
		padding: 0 6px;
		border-radius: 2px;
		background: color-mix(in srgb, var(--tone) 12%, transparent);
		color: var(--tone);
		font-size: 11px;
		line-height: 17px;
		text-align: center;
	}
	.caret {
		display: inline-block;
		margin-left: 8px;
		color: var(--sb-text-faint);
		transition: transform 0.12s;
	}
	.caret.open {
		transform: rotate(90deg);
	}
	tr.detail td {
		padding-top: 0;
		padding-bottom: 10px;
		background: transparent;
	}
	tr.detail:hover td {
		background: transparent;
	}
	dl {
		display: grid;
		grid-template-columns: max-content 1fr;
		gap: 2px 16px;
		margin: 2px 0 0;
		padding: 8px 12px;
		border-left: 2px solid var(--border-subtle);
		color: var(--muted-foreground);
	}
	dt {
		color: var(--sb-text-faint);
	}
	dd {
		margin: 0;
		font-family: var(--font-mono);
		overflow-wrap: anywhere;
	}
	pre {
		margin: 6px 0 2px 14px;
		color: var(--muted-foreground);
		font-family: var(--font-mono);
		font-size: 12px;
		white-space: pre-wrap;
	}
	tr.query td {
		padding-top: 3px;
		padding-bottom: 3px;
		color: var(--muted-foreground);
	}
	.member {
		margin-left: 8px;
		color: var(--sb-text-faint);
		font-family: var(--font-sans);
		font-size: 11px;
	}
	.member:hover {
		color: var(--primary);
	}
	.glyph {
		display: inline-block;
		width: 1.1em;
		font-weight: 600;
	}
	.mono {
		font-family: var(--font-mono);
		font-size: 12px;
	}
	.stamp {
		color: var(--sb-text-faint);
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
