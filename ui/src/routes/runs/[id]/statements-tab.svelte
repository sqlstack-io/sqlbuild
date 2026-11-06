<script lang="ts">
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

	let { statements, total, grouped, runId }: { statements: RunStatement[]; total: number; grouped: boolean; runId: string } = $props();

	type SqlState = { state: 'loading' } | { state: 'ok'; sql: string | null } | { state: 'error'; message: string };
	let open = $state<Set<string>>(new Set());
	let sql = $state<Record<string, SqlState>>({});
	let fullError = $state<Set<string>>(new Set());

	async function loadSql(st: RunStatement) {
		const scope = app.scope;
		if (!scope || sql[st.id]?.state === 'ok' || sql[st.id]?.state === 'loading') return;
		sql = { ...sql, [st.id]: { state: 'loading' } };
		try {
			const res = await app.client.getStatementSql(scope, runId, st.id);
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
</script>

{#if statements.length}
	<table class="st">
		<thead>
			<tr>
				<th class="caret-col"></th>
				<th class="num">#</th>
				<th>Time</th>
				{#if grouped}<th>Job run</th>{/if}
				<th>Node</th>
				<th>Outcome</th>
				<th class="num">Rows</th>
				<th class="num">Took</th>
				<th>Query ID</th>
			</tr>
		</thead>
		<tbody>
			{#each statements as st (st.id)}
				<tr class="row" class:open={open.has(st.id)} onclick={() => toggle(st)}>
					<td class="caret-col"><button class="caret" aria-expanded={open.has(st.id)} aria-label="{open.has(st.id) ? 'Hide' : 'Show'} statement {st.sequence}">{open.has(st.id) ? '▾' : '▸'}</button></td>
					<td class="num faint">{st.sequence}</td>
					<td class="faint nowrap" title={formatStamp(st.startedAt)}>{clockTime(st.startedAt)}</td>
					{#if grouped}<td class="mono faint">{shortRunId(st.runId)}</td>{/if}
					<td class="mono">{st.nodeKey}</td>
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
						<td colspan={grouped ? 9 : 8}>
							<button class="err" class:clamp={!fullError.has(st.id)} onclick={() => toggleError(st.id)} title={fullError.has(st.id) ? 'Show less' : 'Show the full error'}>{st.error}</button>
						</td>
					</tr>
				{/if}
				{#if open.has(st.id)}
					{@const loaded = sql[st.id]}
					<tr class="sql-row">
						<td colspan={grouped ? 9 : 8}>
							{#if !app.can('view.sql')}
								<p class="note">You need permission to see SQL (view.sql).</p>
							{:else if !st.sqlRecorded || (loaded?.state === 'ok' && loaded.sql === null)}
								<p class="note">SQL was not recorded for this statement. Capture may have been off, or the text has passed its retention.</p>
							{:else if loaded?.state === 'ok' && loaded.sql !== null}
								<SqlBlock sql={loaded.sql} caption="statement {st.sequence} · {st.nodeKey}" />
							{:else if loaded?.state === 'error'}
								<p class="note err-text">Couldn't load the SQL: {loaded.message} <button class="retry" onclick={() => retry(st)}>Retry</button></p>
							{:else}
								<p class="note">Loading SQL…</p>
							{/if}
						</td>
					</tr>
				{/if}
			{/each}
		</tbody>
	</table>
{:else}
	<div class="empty">{total ? 'No queries match the filter.' : 'No queries yet. They appear here as the run sends them to the warehouse.'}</div>
{/if}

<style>
	.st {
		width: 100%;
		border-collapse: collapse;
		font-size: 12px;
	}
	th {
		position: sticky;
		top: 0;
		z-index: 1;
		padding: 7px 12px;
		border-bottom: 1px solid var(--border);
		background: var(--background);
		color: var(--sb-text-faint);
		font-size: 11.5px;
		font-weight: 500;
		text-align: left;
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
		width: 28px;
		padding-right: 0;
		padding-left: 14px;
	}
	.caret {
		color: var(--sb-text-faint);
	}
	.num {
		text-align: right;
		font-variant-numeric: tabular-nums;
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
	.retry {
		color: var(--primary);
	}
	.empty {
		padding: 32px;
		color: var(--sb-text-faint);
		font-size: 12.5px;
		text-align: center;
	}
</style>
