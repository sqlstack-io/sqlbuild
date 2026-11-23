<script lang="ts">
	import Button from '$lib/components/ui/button/button.svelte';
	import CopyButton from '$lib/components/copy-button.svelte';
	import type { RunStatement } from '$lib/api';
	import { app } from '$lib/state/app.svelte';
	import { warehouseQueryUrl } from './utils';

	// A query, in place: the statement as it ran, its outcome, rows, duration and query ID,
	// with copy, and for a failure its error. "In Statements" is a small secondary link; only it changes tab.
	let { statement: st, onstatements }: { statement: RunStatement; onstatements?: () => void } = $props();

	let sql = $state<{ state: 'loading' } | { state: 'ok'; sql: string | null } | { state: 'error'; message: string }>({ state: 'loading' });
	$effect(() => {
		const id = st.id;
		const scope = app.scope;
		if (!scope || !st.sqlRecorded || !app.can('view.sql')) {
			sql = { state: 'ok', sql: null };
			return;
		}
		app.client.getStatementSql(scope, st.runId, id).then(
			(res) => (sql = { state: 'ok', sql: res.sql }),
			(e) => (sql = { state: 'error', message: e instanceof Error ? e.message : String(e) })
		);
	});
	const OUTCOME = { completed: 'Finished', failed: 'Failed', running: 'Running', cancelled: 'Cancelled' } as const;
	const took = $derived(`${(((st.endedAt ? Date.parse(st.endedAt) : app.now) - Date.parse(st.startedAt)) / 1000).toFixed(2)}s${st.endedAt ? '' : ' so far'}`);
	const rows = $derived(st.rowsAffected != null ? `${st.rowsAffected.toLocaleString('en-GB')} rows` : st.rowsSoFar != null ? `${st.rowsSoFar.toLocaleString('en-GB')} rows so far` : null);
	const queryUrl = $derived(st.queryId ? warehouseQueryUrl(app.location?.target.adapter, st.queryId) : null);
	let fullError = $state(false);
	const errorLines = $derived(st.error?.split('\n') ?? []);
</script>

<div class="query" class:failed={st.status === 'failed'}>
	<div class="meta">
		<span class="outcome">{OUTCOME[st.status]}</span>
		<span class="kind mono">{st.kind}</span>
		{#if rows}<span>{rows}</span>{/if}
		<span>{took}</span>
		{#if st.queryId}
			<span class="qid">Query <code>{st.queryId}</code><CopyButton value={st.queryId} label="Copy query ID" />{#if queryUrl}<a href={queryUrl} target="_blank" rel="noreferrer">open in warehouse ↗</a>{/if}</span>
		{/if}
		{#if onstatements}<Button variant="link" size="inline" class="secondary text-[11.5px]" onclick={onstatements}>In Statements →</Button>{/if}
	</div>
	{#if st.error}
		<!-- The failure's error, right here: its first lines, the rest on request. -->
		<pre class="error">{fullError || errorLines.length <= 8 ? st.error : errorLines.slice(0, 8).join('\n')}</pre>
		{#if errorLines.length > 8}<Button variant="link" size="inline" class="text-[12px]" onclick={() => (fullError = !fullError)}>{fullError ? 'Show less' : `Show all ${errorLines.length} lines`}</Button>{/if}
	{/if}
	{#if sql.state === 'ok' && sql.sql}
		<div class="sql"><pre>{sql.sql}</pre><CopyButton value={sql.sql} label="Copy SQL" /></div>
	{:else if sql.state === 'ok'}<p class="faint">The SQL wasn't recorded.</p>
	{:else if sql.state === 'error'}<p class="faint">Couldn't load the SQL: {sql.message}</p>
	{:else}<p class="faint">Loading the SQL…</p>{/if}
</div>

<style>
	.query {
		display: flex;
		min-width: 0;
		flex-direction: column;
		gap: 6px;
		padding: 6px 10px;
		border-left: 2px solid var(--border-subtle);
		font-size: 12px;
	}
	.query.failed {
		border-left-color: color-mix(in srgb, var(--sb-error) 60%, transparent);
	}
	.meta {
		display: flex;
		flex-wrap: wrap;
		align-items: baseline;
		gap: 4px 14px;
		color: var(--sb-text-faint);
	}
	.outcome {
		color: var(--muted-foreground);
		font-weight: 500;
	}
	.query.failed .outcome {
		color: var(--sb-error);
	}
	.qid {
		display: inline-flex;
		align-items: center;
		gap: 6px;
	}
	.qid a {
		color: var(--primary);
	}
	.meta :global(.secondary) {
		color: var(--sb-text-faint);
	}
	code,
	pre,
	.mono {
		font-family: var(--font-mono);
		font-size: 12px;
	}
	pre {
		margin: 0;
		white-space: pre-wrap;
		word-break: break-word;
	}
	pre.error {
		color: var(--sb-mono-err, var(--sb-error));
	}
	.sql {
		display: flex;
		align-items: flex-start;
		gap: 8px;
	}
	.sql pre {
		color: var(--muted-foreground);
	}
	.faint {
		margin: 0;
		color: var(--sb-text-faint);
	}
</style>
