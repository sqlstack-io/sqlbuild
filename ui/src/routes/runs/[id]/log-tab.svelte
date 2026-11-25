<script lang="ts">
	import { countBy } from '$lib/list/count-by';
	import { plural } from '$lib/format/plural';
	import DataTable from '$lib/components/list/data-table.svelte';
	import type { TableColumn } from '$lib/components/list/table-columns';
	import Button from '$lib/components/ui/button/button.svelte';
	import { to } from '$lib/paths';
	import DisclosureButton from '$lib/components/disclosure-button.svelte';
	import DownloadIcon from '@lucide/svelte/icons/download';
	import type { RunEventLevel, RunStatement } from '$lib/api';
	import CheckList from './check-list.svelte';
	import QueryPanel from './query-panel.svelte';
	import TracebackPanel from './traceback-panel.svelte';
	import { formatStamp } from '$lib/format/time';
	import { nodeLabel, shortRunId } from '$lib/operate/labels';
	import type { LogEntry } from './utils';
	import { CHECK_FAILED_CODE, fullStamp, tracebackOf } from './utils';
	import SegmentedFilter from '$lib/components/list/segmented-filter.svelte';
	import * as DropdownMenu from '$lib/components/ui/dropdown-menu';
	import LogFilter, { type FilterOption } from './log-filter.svelte';
	import ChevronDownIcon from '@lucide/svelte/icons/chevron-down';
	import XIcon from '@lucide/svelte/icons/x';
	import { cn } from '$lib/utils';
	import { buttonVariants } from '$lib/components/ui/button/button.svelte';

	let {
		entries,
		grouped,
		runId,
		filteredBy,
		printed = $bindable(false),
		memberOf = () => null,
		memory,
		statements = [],
		onquery,
		onframe,
		onnode,
		onstatement,
		request = null
	}: {
		/** A filter set from outside the log, such as the failure summary's Show in log. */
		request?: { query: string; seq: number } | null;
		entries: LogEntry[];
		grouped: boolean;
		runId: string;
		filteredBy: string[];
		/** Whether what the run printed is shown; a failing run's last printed block always is. */
		printed?: boolean;
		/** A node name clicked: the log shows only that node. */
		onnode?: (nodeKey: string) => void;
		/** The page's memory, so Back restores the log as it was. */
		memory?: Record<string, unknown>;
		/** A frame in a node's own code: show that line in the drawer. */
		onframe?: (nodeKey: string, line: number) => void;
		/** The run's statements, for a check's query ID and duration. */
		statements?: readonly RunStatement[];
		/** Shows a check's query in the Statements tab. */
		onquery?: (statementId: string, nodeKey: string) => void;
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
	// One source of truth: the filter text. The Levels and Kinds menus and the counts only write
	// `level:` and `kind:` into it.
	// Back to the page restores the log as it was: its filter, stream and open rows.
	const mem = <T>(key: string, fallback: T): T => (memory && key in memory ? (memory[key] as T) : fallback);
	// svelte-ignore state_referenced_locally
	let query = $state(mem('logQuery', ''));
	// Long printed blocks fold to their first lines.
	const FOLD = 4;
	// svelte-ignore state_referenced_locally
	let open = $state<Set<string>>(new Set(mem<string[]>('logFolded', [])));
	// Nodes whose queries are listed under their outcome row.
	// svelte-ignore state_referenced_locally
	let expanded = $state<Set<string>>(new Set(mem<string[]>('logQueries', [])));

	// Events | Output: the run's events, or what sqb and the nodes printed (`printed`).
	const hasOutput = $derived(entries.some((e) => e.category === 'output'));
	const inMode = (e: LogEntry) => (printed ? e.category === 'output' : e.category !== 'output');
	// Expected lookups (a schema not created yet) stay out of the log until asked for.
	// svelte-ignore state_referenced_locally
	let showExpected = $state(mem('logExpected', false));
	const expectedCount = $derived(entries.filter((e) => e.expected && inMode(e)).length);
	const modeEntries = $derived(entries.filter((e) => inMode(e) && (showExpected || !e.expected)));
	// svelte-ignore state_referenced_locally
	let stream = $state<'all' | 'stdout' | 'stderr'>(mem('logStream', 'all'));
	const kindOf = (e: LogEntry) => e.tag?.text.replace(/ /g, '-') ?? null;
	type Term = { key: string; value: string };
	const terms = $derived(
		query
			.trim()
			.split(/\s+/)
			.filter(Boolean)
			.map((t): Term => {
				const m = t.match(/^(node|kind|level|job|stream):(.*)$/i);
				return m ? { key: m[1].toLowerCase(), value: m[2].toLowerCase() } : { key: 'text', value: t.toLowerCase() };
			})
	);
	const holds = (e: LogEntry, t: Term) => {
		if (!t.value) return true;
		if (t.key === 'node') return !!e.nodeKey && nodeLabel(e.nodeKey).toLowerCase() === t.value;
		if (t.key === 'kind') return kindOf(e) === t.value;
		// Errors are failures: a failed node, check or query, not the run's own summary line.
		if (t.key === 'level') return e.level === (t.value === 'warning' ? 'warn' : t.value) && (t.value !== 'error' || e.category !== 'run');
		if (t.key === 'job') return memberOf(e.runId)?.toLowerCase() === t.value;
		if (t.key === 'stream') return e.stream === t.value;
		return [e.nodeKey ?? '', e.text, e.title, ...e.queries.map((x) => x.text)].join(' ').toLowerCase().includes(t.value);
	};
	const matchesExcept = (e: LogEntry, skip: string | null) => terms.every((t) => t.key === skip || holds(e, t)) && (!printed || stream === 'all' || e.stream === stream);
	const shown = $derived(modeEntries.filter((e) => matchesExcept(e, null)));
	const termValue = (key: string) => terms.find((t) => t.key === key && t.value)?.value ?? null;
	const level = $derived((termValue('level') === 'warning' ? 'warn' : termValue('level')) ?? 'all');
	const kind = $derived(termValue('kind') ?? 'all');
	/** Writes `key:value` into the filter text, in place of any earlier one; null takes it out. */
	function setTerm(key: string, value: string | null) {
		const kept = query.split(/\s+/).filter((t) => t && !t.toLowerCase().startsWith(`${key}:`));
		query = [...kept, ...(value ? [`${key}:${value}`] : [])].join(' ') + (kept.length || value ? ' ' : '');
	}
	const uniq = (xs: (string | null | undefined)[]) => [...new Set(xs.filter((x): x is string => !!x))].sort();
	const kindValues = $derived(printed ? [] : uniq(modeEntries.map(kindOf)));
	const filterOptions = $derived<FilterOption[]>([
		{ prefix: 'node', hint: 'a node', values: uniq(entries.map((e) => (e.nodeKey ? nodeLabel(e.nodeKey) : null))) },
		{ prefix: 'kind', hint: 'built, merged, failed, check, log…', values: kindValues },
		{ prefix: 'level', hint: 'error, warn, info', values: uniq(modeEntries.map((e) => e.level)) },
		{ prefix: 'job', hint: 'a member job', values: grouped ? uniq(entries.map((e) => memberOf(e.runId))) : [] },
		{ prefix: 'stream', hint: 'stdout or stderr', values: printed ? uniq(entries.map((e) => e.stream)) : [] }
	]);
	// Counts for each menu choice: what the rest of the filter leaves, by level or by kind.
	const levelCounts = $derived(countBy(modeEntries.filter((e) => matchesExcept(e, 'level') && !(e.level === 'error' && e.category === 'run')), (e) => e.level));
	const kindCounts = $derived(countBy(modeEntries.filter((e) => matchesExcept(e, 'kind')), (e) => kindOf(e) ?? ''));
	// A count shows only when it matters: errors and warnings in what the switch shows, none when zero.
	const errors = $derived(levelCounts.get('error') ?? 0);
	const warnings = $derived(levelCounts.get('warn') ?? 0);
	const levelText = $derived(LEVELS.find((l) => l.value === level)?.label ?? 'Levels');
	const levelOptions = $derived(LEVELS.map((l) => ({ ...l, count: l.value === 'all' ? [...levelCounts.values()].reduce((a, b) => a + b, 0) : (levelCounts.get(l.value) ?? 0) })));
	const kindOptions = $derived([{ value: 'all', label: 'All kinds', count: [...kindCounts.values()].reduce((a, b) => a + b, 0) }, ...kindValues.map((k) => ({ value: k, label: k, count: kindCounts.get(k) ?? 0 }))]);
	// Rows whose structured details are open in place; long text stays folded until "show more".
	// svelte-ignore state_referenced_locally
	let detailOpen = $state<Set<string>>(new Set(mem<string[]>('logDetails', [])));
	// svelte-ignore state_referenced_locally
	let longOpen = $state<Set<string>>(new Set(mem<string[]>('logLong', [])));
	let applied = 0;
	$effect(() => {
		if (!request || request.seq === applied) return;
		applied = request.seq;
		query = `${request.query} `;
	});
	$effect(() => {
		if (memory) Object.assign(memory, { logExpected: showExpected, logQuery: query, logStream: stream, logFolded: [...open], logQueries: [...expanded], logDetails: [...detailOpen], logLong: [...longOpen] });
	});

	/** A node line's queries (its build statements, not its checks'), or a query row's own statement. */
	function queriesOf(e: LogEntry): RunStatement[] {
		if (e.statementId && e.category === 'queries') return statements.filter((q) => q.id === e.statementId);
		if (!e.node) return [];
		return statements.filter((q) => q.nodeKey === e.node!.nodeKey && q.runId === e.runId && q.kind !== 'audit');
	}

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
		// Full timestamps, never cropped.
		{ id: 'time', label: 'Time', width: '206px' },
		// A group run's member job is a quiet tag beside the node, not a column of its own.
		{ id: 'node', label: 'Node', width: grouped ? '320px' : '230px' },
		...(printed ? [] : [{ id: 'tag', label: '', width: '78px' }]),
		{ id: 'what', label: 'What happened', minWidth: 300, truncate: false }
	]);
</script>

<!-- Controls right after what they act on, from the left: what to show, then narrow it. -->
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
	{#if printed}
		<SegmentedFilter
			label="Stream"
			bind:value={stream}
			options={[
				{ value: 'all', label: 'All' },
				{ value: 'stdout', label: 'stdout' },
				{ value: 'stderr', label: 'stderr' }
			]}
		/>
	{/if}
	<LogFilter bind:value={query} options={filterOptions} />
	<DropdownMenu.Root>
		<DropdownMenu.Trigger class={cn(buttonVariants({ variant: 'ghost', size: 'sm' }), 'menu-trigger')} aria-label="Levels">{level === 'all' ? 'Levels' : levelText} <ChevronDownIcon size={13} /></DropdownMenu.Trigger>
		<DropdownMenu.Content class="w-44">
			<DropdownMenu.RadioGroup value={level} onValueChange={(v) => setTerm('level', v === 'all' ? null : v)}>
				{#each levelOptions as l (l.value)}<DropdownMenu.RadioItem value={l.value}>{l.label}<span class="menu-count">{l.count}</span></DropdownMenu.RadioItem>{/each}
			</DropdownMenu.RadioGroup>
		</DropdownMenu.Content>
	</DropdownMenu.Root>
	{#if !printed && kindValues.length > 1}
		<DropdownMenu.Root>
			<DropdownMenu.Trigger class={cn(buttonVariants({ variant: 'ghost', size: 'sm' }), 'menu-trigger')} aria-label="Kinds">{kind === 'all' ? 'Kinds' : kind} <ChevronDownIcon size={13} /></DropdownMenu.Trigger>
			<DropdownMenu.Content class="w-44">
				<DropdownMenu.RadioGroup value={kind} onValueChange={(v) => setTerm('kind', v === 'all' ? null : v)}>
					{#each kindOptions as k (k.value)}<DropdownMenu.RadioItem value={k.value}>{k.label}<span class="menu-count">{k.count}</span></DropdownMenu.RadioItem>{/each}
				</DropdownMenu.RadioGroup>
			</DropdownMenu.Content>
		</DropdownMenu.Root>
	{/if}
	{#if errors || level === 'error'}<Button variant={level === 'error' ? 'outline' : 'ghost'} size="sm" class={level === 'error' ? 'count err on' : 'count err'} aria-pressed={level === 'error'} onclick={() => setTerm('level', level === 'error' ? null : 'error')} title={level === 'error' ? 'Show every event' : 'Show only failures'}>Errors {errors}</Button>{/if}
	{#if warnings}<Button variant="link" size="inline" class="count warn" onclick={() => setTerm('level', level === 'warn' ? null : 'warn')} title="Show only warnings">Warnings {warnings}</Button>{/if}
	{#if expectedCount && !printed}<Button variant="link" size="inline" class="expected" aria-pressed={showExpected} onclick={() => (showExpected = !showExpected)} title="Failed lookups the run expects, such as a schema not created yet">{showExpected ? 'Hide' : 'Show'} {plural(expectedCount, 'expected lookup')}</Button>{/if}
	<span class="grow"></span>
	<Button variant="ghost" size="sm" onclick={download} disabled={!shown.length} title="Download the rows shown as a text file"><DownloadIcon />Download</Button>
</div>

{#if level === 'error'}
	<div class="only" data-testid="errors-only">
		Showing errors only · {shown.length} of {plural(modeEntries.length, 'event')}
		<Button variant="ghost" size="icon-xs" onclick={() => setTerm('level', null)} aria-label="Show every event" title="Show every event"><XIcon /></Button>
	</div>
{/if}

<DataTable class="static compact log" embedded columns={logColumns} rows={shown} key={(e) => e.id} empty={entries.length ? 'Nothing in the log matches these filters.' : 'Nothing logged yet.'}>
	{#snippet row(e)}
			<tr class:error={e.level === 'error'} class:warn={e.level === 'warn'} class:stderr={e.stream === 'stderr'}>
				<td class="faint nowrap mono stamp" title={formatStamp(e.at)}>{fullStamp(e.at)}</td>
				<td class="mono" class:faint={!e.nodeKey} title={e.nodeKey ? `Show only ${e.nodeKey} in the log` : undefined}>
					<!-- A real link: it filters the log to the node, and reads as the node's name anywhere. -->
					{#if e.nodeKey && onnode}<a
							class="node-link"
							href="?node={encodeURIComponent(e.nodeKey)}"
							onclick={(ev) => {
								ev.preventDefault();
								onnode(e.nodeKey!);
							}}>{nodeLabel(e.nodeKey)}</a
						>{:else}{e.nodeKey ? nodeLabel(e.nodeKey) : '—'}{/if}
					{#if grouped && memberOf(e.runId)}<a class="member" href={to(`/runs/${e.runId}`)} title="In member job run {shortRunId(e.runId)}">{memberOf(e.runId)}</a>{/if}
				</td>
				{#if !printed}<td class="tagcell">{#if e.tag && !e.tag.repeats}<span class="tag" style:--tone={e.tag.tone}>{e.tag.text}</span>{/if}</td>{/if}
				<td class:out={e.stream} class:opens={!!e.details?.length} onclick={() => e.details?.length && (detailOpen = toggle(detailOpen, e.id))}>
					{#if e.omitted && e.nodeKey}
						<!-- The live view is capped: earlier lines are in the paged viewer, not here. -->
						<a class="omitted" href={to(`/runs/${e.runId}/output/${e.nodeKey}`)}>+{e.omitted.toLocaleString('en-GB')} lines · open in viewer →</a>
					{:else if e.stream}
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
						{#if queriesOf(e).length}
							<!-- The node's queries open in place, under its line; only "In Statements" changes tab. -->
							<DisclosureButton
								open={expanded.has(e.id)}
								onclick={(ev: MouseEvent) => {
									ev.stopPropagation();
									expanded = toggle(expanded, e.id);
								}}
								class="ml-2 gap-0.5 text-[11.5px]">{plural(queriesOf(e).length, 'query', 'queries')}</DisclosureButton
							>
						{/if}
					{/if}
					{#if e.statementId && e.category === 'queries' && queriesOf(e).length}
						<Button
							variant="link"
							size="inline"
							class="ml-2 font-mono text-[11px]"
							onclick={(ev: MouseEvent) => {
								ev.stopPropagation();
								expanded = toggle(expanded, e.id);
							}}
							title="Show this query here">SQL {expanded.has(e.id) ? '▾' : '▸'}</Button
						>
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
						{#if e.checks?.length}
							<div class="checks-wrap"><CheckList checks={e.checks} runId={e.runId} {statements} onstatement={onquery && e.nodeKey ? (id) => onquery(id, e.nodeKey!) : undefined} /></div>
						{/if}
						{#if e.node && tracebackOf(e.long)}
							<!-- A Python failure in place: the exception and the last of its traceback; the output viewer for the rest. -->
							<div class="inline-wrap"><TracebackPanel detail={tracebackOf(e.long)!} file={e.node.errorLocation?.file ?? null} runId={e.runId} nodeKey={e.node.nodeKey} onframe={onframe ? (line) => onframe(e.node!.nodeKey, line) : undefined} /></div>
						{:else if e.node?.status === 'failed' && queriesOf(e).some((q) => q.status === 'failed')}
							<!-- A SQL failure in place: the statement as it ran and its error. -->
							{#each queriesOf(e).filter((q) => q.status === 'failed') as q (q.id)}
								<div class="inline-wrap"><QueryPanel statement={q} onstatements={onquery ? () => onquery(q.id, q.nodeKey) : undefined} /></div>
							{/each}
						{:else if e.long && e.node?.errorCode !== CHECK_FAILED_CODE}
							{@const lines = e.long.split('\n')}
							<pre>{longOpen.has(e.id) ? e.long : lines.slice(0, 3).join('\n')}</pre>
							{#if lines.length > 3}<Button variant="link" size="inline" class="text-[12px]" onclick={() => (longOpen = toggle(longOpen, e.id))}>{longOpen.has(e.id) ? 'Show less' : `Show more (${lines.length} lines)`}</Button>{/if}
						{/if}
					</td>
				</tr>
			{/if}
			{#if expanded.has(e.id) && queriesOf(e).length}
				<tr class="detail">
					<td></td>
					<td colspan={printed ? 2 : 3}>
						{#each queriesOf(e) as q (q.id)}
							<div class="inline-wrap"><QueryPanel statement={q} onstatements={onquery ? () => onquery(q.id, q.nodeKey) : undefined} /></div>
						{/each}
					</td>
				</tr>
			{/if}
	{/snippet}
</DataTable>

<style>
	/* Calmer: no rules between rows, one text size, muted secondary text, room around the toolbar. */
	.filters {
		position: sticky;
		top: 0;
		z-index: 6;
		display: flex;
		flex-wrap: wrap;
		align-items: center;
		gap: 8px 12px;
		padding: 10px 24px;
		background: var(--background);
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
	/* Pressed: the filter is on. Tinted and bordered in the error colour, so it reads as held down. */
	.filters :global(.count.err.on) {
		border-color: color-mix(in srgb, var(--sb-error) 55%, var(--border));
		background: color-mix(in srgb, var(--sb-error) 20%, transparent);
		box-shadow: inset 0 1px 3px color-mix(in srgb, black 35%, transparent);
		font-weight: 600;
	}
	.filters :global(.expected.expected) {
		color: var(--sb-text-faint);
		font-size: 12px;
	}
	.only {
		display: flex;
		align-items: center;
		gap: 6px;
		padding: 3px 24px;
		border-block: 1px solid color-mix(in srgb, var(--sb-error) 25%, var(--border-subtle));
		background: color-mix(in srgb, var(--sb-error) 5%, var(--background));
		color: var(--muted-foreground);
		font-size: 12px;
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
	/* What a node or sqb wrote to stderr: tinted, so warnings and tracebacks stand out from stdout. */
	tr.stderr td.out {
		background: color-mix(in srgb, var(--sb-warning) 5%, transparent);
		color: color-mix(in srgb, var(--sb-warning) 70%, var(--foreground));
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
	.node-link {
		color: var(--primary);
		font-family: var(--font-mono);
		font-size: 12px;
	}
	.node-link:hover {
		text-decoration: underline;
	}
	.inline-wrap {
		margin: 8px 0 2px;
		max-width: 980px;
	}
	.checks-wrap {
		margin: 8px 0 0;
		padding: 8px 12px;
		border-left: 2px solid var(--border-subtle);
	}
	.omitted {
		color: var(--primary);
		font-family: var(--font-sans);
	}
	.omitted:hover {
		text-decoration: underline;
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
