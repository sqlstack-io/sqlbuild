<script lang="ts">
	import { tick, untrack } from 'svelte';
	import DataTable from '$lib/components/list/data-table.svelte';
	import type { TableColumn } from '$lib/components/list/table-columns';
	import DisclosureButton from '$lib/components/disclosure-button.svelte';
	import Button from '$lib/components/ui/button/button.svelte';
	import CircleCheckIcon from '@lucide/svelte/icons/circle-check';
	import CircleXIcon from '@lucide/svelte/icons/circle-x';
	import CircleSlashIcon from '@lucide/svelte/icons/circle-slash';
	import LoaderIcon from '@lucide/svelte/icons/loader-circle';
	import type { RunStatement } from '$lib/api';
	import SqlBlock from '$lib/components/code/sql-block.svelte';
	import CopyButton from '$lib/components/copy-button.svelte';
	import { formatStamp } from '$lib/format/time';
	import { clockTime, shortRunId } from '$lib/operate/labels';
	import { app } from '$lib/state/app.svelte';
	import { formatSpan } from './utils';

	/** `focusId`: a statement to open and scroll to, as when a check's "In Statements" was clicked. */
	let { statements, total, grouped, focusId = null }: { statements: RunStatement[]; total: number; grouped: boolean; focusId?: string | null } = $props();

	type SqlState = { state: 'loading' } | { state: 'ok'; sql: string | null } | { state: 'error'; message: string };
	let open = $state<Set<string>>(new Set());
	let sql = $state<Record<string, SqlState>>({});
	let fullError = $state<Set<string>>(new Set());

	async function loadSql(st: RunStatement) {
		const scope = app.scope;
		if (!scope || sql[st.id]?.state === 'ok' || sql[st.id]?.state === 'loading') return;
		sql = { ...sql, [st.id]: { state: 'loading' } };
		try {
			const res = await app.client.getStatementSql(scope, st.runId, st.id);
			sql = { ...sql, [st.id]: { state: 'ok', sql: res.sql } };
		} catch (e) {
			sql = { ...sql, [st.id]: { state: 'error', message: e instanceof Error ? e.message : String(e) } };
		}
	}

	function retry(st: RunStatement) {
		const next = { ...sql };
		delete next[st.id];
		sql = next;
		void loadSql(st);
	}

	let focused = '';
	$effect(() => {
		const st = focusId ? statements.find((x) => x.id === focusId) : null;
		if (!st || focused === st.id) return;
		focused = st.id;
		untrack(() => {
			if (!open.has(st.id)) toggle(st);
		});
		void tick().then(() => document.querySelector(`tr[data-sid="${st.id}"]`)?.scrollIntoView({ block: 'center' }));
	});

	function toggle(st: RunStatement) {
		const next = new Set(open);
		if (next.has(st.id)) next.delete(st.id);
		else {
			next.add(st.id);
			if (st.sqlRecorded && app.can('view.sql')) void loadSql(st);
		}
		open = next;
	}

	function toggleError(id: string) {
		const next = new Set(fullError);
		if (next.has(id)) next.delete(id);
		else next.add(id);
		fullError = next;
	}

	const elapsed = (st: RunStatement) => ((st.endedAt ? Date.parse(st.endedAt) : app.now) - Date.parse(st.startedAt)) / 1000;
	const outcome = { completed: 'Finished', failed: 'Failed', running: 'Running', cancelled: 'Cancelled' } as const;
	const statementColumns = $derived<TableColumn[]>([
		{ id: 'caret', label: '', width: '40px' },
		{ id: 'seq', label: '#', width: '50px', align: 'right' },
		{ id: 'time', label: 'Time', width: '100px' },
		...(grouped ? [{ id: 'run', label: 'Job run', width: '100px' }] : []),
		{ id: 'node', label: 'Node', minWidth: 200 },
		{ id: 'kind', label: 'Statement', width: '120px' },
		{ id: 'outcome', label: 'Outcome', width: '110px' },
		{ id: 'rows', label: 'Rows', width: '110px', align: 'right' },
		{ id: 'took', label: 'Took', width: '80px', align: 'right' },
		{ id: 'qid', label: 'Query ID', width: '170px' }
	]);
</script>

<DataTable class="compact statements" embedded columns={statementColumns} rows={statements} key={(st) => st.id} empty={total ? 'No queries match the filter.' : 'No queries yet.'}>
	{#snippet row(st)}
				<tr class="row" class:open={open.has(st.id)} class:focused={st.id === focusId} data-sid={st.id} onclick={() => toggle(st)}>
					<td class="caret-col"><DisclosureButton open={open.has(st.id)}><span class="sr-only">{open.has(st.id) ? 'Hide' : 'Show'} statement {st.sequence}</span></DisclosureButton></td>
					<td class="num faint">{st.sequence}</td>
					<td class="faint nowrap" title={formatStamp(st.startedAt)}>{clockTime(st.startedAt)}</td>
					{#if grouped}<td class="mono faint">{shortRunId(st.runId)}</td>{/if}
					<td class="mono">{#if st.nodeKey}{st.nodeKey}{:else}<span class="faint" title="Run-level query, such as setup or state bookkeeping, not tied to one node">run</span>{/if}</td>
					<td class="mono faint">{st.kind}</td>
					<td>
						<span class="outcome {st.status}" title="{st.kind} · {st.status}">
							{#if st.status === 'completed'}<CircleCheckIcon size={13} />{:else if st.status === 'failed'}<CircleXIcon size={13} />{:else if st.status === 'running'}<LoaderIcon size={13} class="spin" />{:else}<CircleSlashIcon size={13} />{/if}
							{outcome[st.status]}
						</span>
					</td>
					<td class="num">{st.rowsAffected != null ? st.rowsAffected.toLocaleString('en-GB') : st.rowsSoFar != null ? `${st.rowsSoFar.toLocaleString('en-GB')} so far` : '—'}</td>
					<td class="num faint">{formatSpan(elapsed(st))}</td>
					<td class="qid">
						{#if st.queryId}
							<span class="mono faint" title={st.queryId}>{st.queryId.slice(0, 13)}…</span>
							<CopyButton value={st.queryId} label="Copy query ID" />
						{:else}
							<span class="faint" title="This warehouse doesn't report query IDs">—</span>
						{/if}
					</td>
				</tr>
				{#if st.error}
					<tr class="err-row">
						<td colspan={grouped ? 10 : 9}>
							<!-- raw-button: a failed statement's error text, clickable to show it in full -->
							<button class="err" class:clamp={!fullError.has(st.id)} onclick={() => toggleError(st.id)} title={fullError.has(st.id) ? 'Show less' : 'Show the full error'}>{st.error}</button>
						</td>
					</tr>
				{/if}
				{#if open.has(st.id)}
					{@const loaded = sql[st.id]}
					<tr class="sql-row">
						<td colspan={grouped ? 10 : 9}>
							{#if !app.can('view.sql')}
								<p class="note">You need permission to see SQL (view.sql).</p>
							{:else if !st.sqlRecorded || (loaded?.state === 'ok' && loaded.sql === null)}
								<p class="note">SQL was not recorded for this statement. Capture may have been off, or the text has passed its retention.</p>
							{:else if loaded?.state === 'ok' && loaded.sql !== null}
								<SqlBlock sql={loaded.sql} caption="statement {st.sequence} · {st.nodeKey}" />
							{:else if loaded?.state === 'error'}
								<p class="note err-text">Couldn't load the SQL: {loaded.message} <Button variant="link" size="inline" onclick={() => retry(st)}>Retry</Button></p>
							{:else}
								<p class="note">Loading SQL…</p>
							{/if}
						</td>
					</tr>
				{/if}
	{/snippet}
</DataTable>

<style>
	tr.focused td {
		background: color-mix(in srgb, var(--primary) 10%, transparent);
	}
	td {
		padding: 6px 12px;
		border-bottom: 1px solid var(--border-subtle);
		vertical-align: middle;
	}
	.row {
		cursor: pointer;
	}
	.row:hover td,
	.row.open td {
		background: var(--sb-hover);
	}
	.caret-col {
		padding-right: 0;
		padding-left: 14px;
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
	.outcome {
		display: inline-flex;
		align-items: center;
		gap: 5px;
	}
	.outcome.completed {
		color: var(--sb-success);
	}
	.outcome.failed {
		color: var(--sb-error);
	}
	.outcome.running {
		color: var(--sb-running);
	}
	.outcome.cancelled {
		color: var(--muted-foreground);
	}
	.outcome :global(.spin) {
		animation: spin 1s linear infinite;
	}
	@keyframes spin {
		to {
			transform: rotate(360deg);
		}
	}
	.qid {
		white-space: nowrap;
	}
	.err-row td,
	.sql-row td {
		padding: 2px 12px 8px 54px;
	}
	.err {
		display: block;
		max-width: 100%;
		color: var(--sb-error);
		font-family: var(--font-mono);
		font-size: 11.5px;
		text-align: left;
		white-space: pre-wrap;
	}
	.err.clamp {
		display: -webkit-box;
		overflow: hidden;
		-webkit-box-orient: vertical;
		-webkit-line-clamp: 2;
		line-clamp: 2;
	}
	.err:hover {
		text-decoration: underline dotted;
	}
	.note {
		padding: 8px 0;
		color: var(--muted-foreground);
	}
	.err-text {
		color: var(--sb-error);
	}
	.empty {
		padding: 32px;
		color: var(--sb-text-faint);
		font-size: 12.5px;
		text-align: center;
	}
</style>
