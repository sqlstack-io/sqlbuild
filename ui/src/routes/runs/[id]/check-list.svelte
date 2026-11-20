<script lang="ts">
	import Button from '$lib/components/ui/button/button.svelte';
	import CopyButton from '$lib/components/copy-button.svelte';
	import type { CheckSample, RunStatement } from '$lib/api';
	import { app } from '$lib/state/app.svelte';
	import type { CheckResult } from './utils/checks';

	// A node's checks as a small table (CHI-1591): problems first, each with its SQL as it ran and, for
	// a problem, the failing rows it found. Used in a log line's details and in the drawer.
	let {
		checks,
		runId,
		statements = [],
		onstatement
	}: {
		checks: CheckResult[];
		runId: string;
		/** The run's statements, for a check's query ID and duration. */
		statements?: readonly RunStatement[];
		/** Shows the check's query in the Statements tab. */
		onstatement?: (statementId: string) => void;
	} = $props();

	const problems = $derived(checks.filter((c) => c.outcome !== 'passed'));
	const passed = $derived(checks.filter((c) => c.outcome === 'passed'));
	let all = $state(false);
	/** Problems, then a passing check or three; the rest behind "… N more passed". */
	const shown = $derived(all ? checks : [...problems, ...passed.slice(0, problems.length ? 1 : 3)]);
	const hidden = $derived(checks.length - shown.length);

	type Loaded<T> = { state: 'loading' } | { state: 'ok'; value: T } | { state: 'error'; message: string };
	let sql = $state<Record<string, Loaded<string | null>>>({});
	let sample = $state<Record<string, Loaded<CheckSample | null>>>({});
	let openSql = $state<Set<string>>(new Set());
	let openRows = $state<Set<string>>(new Set());

	const toggled = (set: Set<string>, id: string) => {
		const next = new Set(set);
		if (next.has(id)) next.delete(id);
		else next.add(id);
		return next;
	};
	async function showSql(c: CheckResult) {
		openSql = toggled(openSql, c.id);
		const id = c.statementId;
		if (!id || !app.scope || sql[id]) return;
		sql = { ...sql, [id]: { state: 'loading' } };
		try {
			const res = await app.client.getStatementSql(app.scope, runId, id);
			sql = { ...sql, [id]: { state: 'ok', value: res.sql } };
		} catch (e) {
			sql = { ...sql, [id]: { state: 'error', message: e instanceof Error ? e.message : String(e) } };
		}
	}
	async function showRows(c: CheckResult) {
		openRows = toggled(openRows, c.id);
		const id = c.statementId;
		if (!id || !app.scope || !app.client.getCheckSample || sample[id]) return;
		sample = { ...sample, [id]: { state: 'loading' } };
		try {
			sample = { ...sample, [id]: { state: 'ok', value: await app.client.getCheckSample(app.scope, runId, id) } };
		} catch (e) {
			sample = { ...sample, [id]: { state: 'error', message: e instanceof Error ? e.message : String(e) } };
		}
	}
	const statementOf = (c: CheckResult) => statements.find((s) => s.id === c.statementId) ?? null;
	const took = (s: RunStatement) => (s.endedAt ? `${((Date.parse(s.endedAt) - Date.parse(s.startedAt)) / 1000).toFixed(2)}s` : 'running');
	const GLYPH = { failed: '✗', warned: '⚠', passed: '✓' } as const;
	const SEVERITY = { failed: 'error', warned: 'warn', passed: '' } as const;
</script>

<div class="checks">
	{#each shown as c (c.id)}
		{@const st = statementOf(c)}
		<div class="check {c.outcome}">
			<span class="glyph">{GLYPH[c.outcome]}</span>
			<span class="name">{c.name}</span>
			<span class="what">{c.outcome}{#if c.count != null}{` · ${c.count.toLocaleString('en-GB')} ${c.noun} (${SEVERITY[c.outcome]})`}{/if}</span>
			<span class="acts">
				{#if c.statementId}<Button variant="link" size="inline" class="text-[12px]" onclick={() => showSql(c)} aria-expanded={openSql.has(c.id)}>SQL {openSql.has(c.id) ? '▾' : '▸'}</Button>{/if}
				{#if c.outcome !== 'passed' && c.statementId}<Button variant="link" size="inline" class="text-[12px]" onclick={() => showRows(c)} aria-expanded={openRows.has(c.id)}>rows {openRows.has(c.id) ? '▾' : '▸'}</Button>{/if}
			</span>
		</div>
		{#if openSql.has(c.id) && c.statementId}
			{@const got = sql[c.statementId]}
			<div class="panel">
				<div class="meta">
					{#if st?.queryId}<span>Query <code>{st.queryId}</code></span>{/if}
					{#if st}<span>{took(st)}</span>{/if}
					{#if onstatement}<Button variant="link" size="inline" class="text-[12px]" onclick={() => onstatement(c.statementId!)}>In Statements →</Button>{/if}
				</div>
				{#if got?.state === 'ok' && got.value}
					<div class="sql"><pre>{got.value}</pre><CopyButton value={got.value} label="Copy SQL" /></div>
				{:else if got?.state === 'ok'}<p class="faint">The SQL wasn't recorded.</p>
				{:else if got?.state === 'error'}<p class="faint">Couldn't load the SQL: {got.message}</p>
				{:else}<p class="faint">Loading…</p>{/if}
			</div>
		{/if}
		{#if openRows.has(c.id) && c.statementId}
			{@const got = sample[c.statementId]}
			<div class="panel">
				{#if got?.state === 'ok' && got.value}
					<table>
						<thead><tr>{#each got.value.columns as col (col)}<th>{col}</th>{/each}</tr></thead>
						<tbody>
							{#each got.value.rows as row, i (i)}<tr>{#each row as v, j (j)}<td class:null={v == null}>{v ?? 'null'}</td>{/each}</tr>{/each}
						</tbody>
					</table>
					<p class="faint">{got.value.rows.length < got.value.total ? `${got.value.rows.length} of ${got.value.total.toLocaleString('en-GB')} failing rows` : `${got.value.total.toLocaleString('en-GB')} failing rows`}</p>
				{:else if got?.state === 'ok'}<p class="faint">No sample was kept.</p>
				{:else if got?.state === 'error'}<p class="faint">Couldn't load the rows: {got.message}</p>
				{:else}<p class="faint">Loading…</p>{/if}
			</div>
		{/if}
	{/each}
	{#if hidden > 0 || all}
		<Button variant="link" size="inline" class="more text-[12px]" onclick={() => (all = !all)}>{all ? 'Show fewer' : `… ${hidden} more passed`}</Button>
	{/if}
</div>

<style>
	.checks {
		display: flex;
		max-width: 780px;
		min-width: 0;
		flex-direction: column;
		gap: 2px;
		font-size: 12px;
	}
	.name,
	.what {
		overflow-wrap: anywhere;
	}
	.panel {
		min-width: 0;
		overflow-x: auto;
	}
	.check {
		display: grid;
		grid-template-columns: 16px minmax(0, 220px) minmax(0, 1fr) auto;
		align-items: baseline;
		gap: 10px;
	}
	.glyph {
		font-weight: 600;
	}
	.check.failed .glyph,
	.check.failed .what {
		color: var(--sb-error);
	}
	.check.warned .glyph,
	.check.warned .what {
		color: var(--sb-warning);
	}
	.check.passed .glyph {
		color: var(--sb-success);
	}
	.check.passed .what {
		color: var(--sb-text-faint);
	}
	.name {
		font-family: var(--font-mono);
	}
	.acts {
		display: inline-flex;
		gap: 12px;
	}
	.panel {
		margin: 2px 0 6px 26px;
		padding: 6px 10px;
		border-left: 2px solid var(--border-subtle);
	}
	.meta {
		display: flex;
		flex-wrap: wrap;
		align-items: baseline;
		gap: 4px 14px;
		color: var(--sb-text-faint);
	}
	code,
	pre,
	td,
	th {
		font-family: var(--font-mono);
		font-size: 12px;
	}
	.sql {
		display: flex;
		align-items: flex-start;
		gap: 8px;
		margin-top: 4px;
	}
	pre {
		margin: 0;
		color: var(--muted-foreground);
		white-space: pre-wrap;
	}
	/* Inside the log's table too: these beat its cell styles. */
	.checks table {
		width: auto;
		border-collapse: collapse;
	}
	.checks th,
	.checks td {
		padding: 1px 16px 1px 0;
		border: 0;
		background: transparent;
		height: auto;
	}
	.checks th {
		color: var(--sb-text-faint);
		font-weight: 500;
		text-align: left;
	}
	.checks td.null {
		color: var(--sb-text-faint);
		font-style: italic;
	}
	.faint {
		margin: 4px 0 0;
		color: var(--sb-text-faint);
	}
	.checks :global(.more) {
		align-self: flex-start;
		margin: 2px 0 0 26px;
	}
</style>
