<script lang="ts">
	import DataTable from '$lib/components/list/data-table.svelte';
	import type { TableColumn } from '$lib/components/list/table-columns';
	import { to } from '$lib/paths';
	import type { Run } from '$lib/api';
	import CopyButton from '$lib/components/copy-button.svelte';
	import { formatAgo, formatStamp } from '$lib/format/time';
	import { shortImage } from '$lib/platform/labels';
	import { shortRunId } from '$lib/operate/labels';
	import { app } from '$lib/state/app.svelte';
	import { runStatusLabel, runStatusTone } from '$lib/status';
	import type { RunDetailState } from './state.svelte';
	import { attemptLabel, costSummary } from './utils';
	import Button from '$lib/components/ui/button/button.svelte';

	let { view }: { view: RunDetailState } = $props();

	const d = $derived(view.detail);
	type Related = { role: string; run: Run };
	const related = $derived.by((): Related[] => {
		if (!d) return [];
		return [
			...(d.parent ? [{ role: 'Group run', run: d.parent }] : []),
			...(d.rerunOf ? [{ role: 'Rerun of', run: d.rerunOf }] : []),
			...d.reruns.map((run) => ({ role: 'Rerun', run })),
			...d.children.map((run) => ({ role: 'Job run', run }))
		];
	});
	const invocationColumns: TableColumn[] = [
		{ id: 'invocation', label: 'sqb invocation', minWidth: 200 },
		{ id: 'run', label: 'Run', width: '110px' },
		{ id: 'attempt', label: 'Attempt', width: '80px' },
		{ id: 'started', label: 'Started', width: '170px' },
		{ id: 'ended', label: 'Ended', width: '170px' },
		{ id: 'exit', label: 'Exit code', width: '90px' }
	];
</script>

{#if d}
	<!-- A long selection folds behind its count, so the rest of the details stay in view. -->
{#snippet selection(keys: string[])}
	{#if keys.length > 12}
		<details><summary>{keys.length} nodes</summary><code>{keys.join(' ')}</code></details>
	{:else}
		<code>{keys.join(' ') || '—'}</code>
	{/if}
{/snippet}

{#if view.showDetails}
		{@const p = d.provenance}
		<div class="details">
			<dl>
				<!-- Moved here from the header (CHI-1591): nodes, cost and related runs. -->
				{#if view.progress.total}<div><dt>Nodes</dt><dd>{view.progress.done}/{view.progress.total} finished</dd></div>{/if}
				{#if view.cost && view.cost.status !== 'none'}
					<div><dt>Cost</dt><dd><Button variant="link" size="inline" onclick={() => (view.tab = 'cost')}>{costSummary(view.cost).text}</Button></dd></div>
				{/if}
				{#if related.length}
					<div>
						<dt>Related runs</dt>
						<dd>
			<ul>
				{#each related as r (`${r.role}-${r.run.id}`)}
					<li>
						<a href={to(`/runs/${r.run.id}`)} title="{r.role} {r.run.id} · {runStatusLabel[r.run.status]} · queued {formatStamp(r.run.queuedAt)}">
							<span class="faint">{r.role}</span>
							<i style:background={runStatusTone[r.run.status]}></i>
							<span class="mono">{shortRunId(r.run.id)}</span>
							{#if r.role === 'Job run' && r.run.jobKey}<span>{r.run.jobKey}</span>{/if}
							{#if r.role === 'Job run'}
								{@const attempt = attemptLabel(r.run, d.children)}
								{#if attempt}<span class="faint" data-testid="member-attempt">{attempt}</span>{/if}
							{/if}
							<span class="faint">{r.run.status === 'skipped' && r.run.parentRunId && !r.run.startedAt ? 'not started' : runStatusLabel[r.run.status].toLowerCase()}</span>
						</a>
					</li>
				{/each}
			</ul>
						</dd>
					</div>
				{/if}
				{#if p}
				<div><dt>Project version</dt><dd>Version {p.snapshotGeneration}{#if p.gitRevision} · commit <code>{p.gitRevision}</code>{/if}{#if p.gitDirty}<span title="Published with uncommitted changes"> · dirty</span>{/if} <span class="faint">· snapshot <code>{p.snapshotId}</code></span></dd></div>
				{#if p.command}<div><dt>Command</dt><dd><code>{p.command}</code> <CopyButton value={p.command} label="Copy command" /></dd></div>{/if}
				{#if p.selection.length || p.exclude.length}
					<div><dt>Selection</dt><dd>{@render selection(p.selection)}{#if p.exclude.length} excluding <code>{p.exclude.join(' ')}</code>{/if}</dd></div>
				{/if}
				<div><dt>{p.imageId ? 'Image' : 'Published by'}</dt><dd>{#if p.imageId}<code title={p.imageId}>{shortImage(p.imageId)}</code> · {/if}sqb {p.sqbVersion}</dd></div>
				{:else if d.selection.length}
				<div><dt>Selection</dt><dd>{@render selection(d.selection)}</dd></div>
				{/if}
				<div><dt>Rebuilds history</dt><dd title={d.run.rewriteHistory ? 'Full refresh or set window; rerun needs run.rewrite_history' : undefined}>{d.run.rewriteHistory ? 'Yes' : 'No'}</dd></div>
				<div><dt>Priority</dt><dd>{d.run.priority}</dd></div>
				<div><dt>Run ID</dt><dd><code>{d.run.id}</code> <CopyButton value={d.run.id} label="Copy run ID" /></dd></div>
				{#if d.run.runKey}<div><dt>Sensor run key</dt><dd><code>{d.run.runKey}</code></dd></div>{/if}
				{#if d.run.tickScheduledFor}<div><dt>Scheduled for</dt><dd>{formatStamp(d.run.tickScheduledFor)} <span class="faint">· tick <code>{d.run.tickId}</code></span></dd></div>{/if}
				{#if d.liveness}
					<div>
						<dt>Worker</dt>
						<dd>
							<code>{d.liveness.workerInstance ?? '—'}</code> · container <code>{d.liveness.containerId ?? '—'}</code>
							{#if d.liveness.lastHeartbeatAt}<span class="faint"> · last heartbeat {formatAgo(d.liveness.lastHeartbeatAt, app.now)}</span>{/if}
							{#if d.liveness.exitCode != null}<span class="faint"> · exit code {d.liveness.exitCode}</span>{/if}
						</dd>
					</div>
				{/if}
			</dl>
			{#if d.invocations.length}
				<DataTable class="static compact" embedded columns={invocationColumns} rows={d.invocations} key={(inv) => inv.invocationId} empty="No sqb invocations.">
					{#snippet row(inv)}
							<tr>
								<td class="mono">{inv.invocationId}</td>
								<td class="mono"><a href={to(`/runs/${inv.runId}`)}>{shortRunId(inv.runId)}</a></td>
								<td>{inv.attempt}</td>
								<td>{formatStamp(inv.startedAt)}</td>
								<td>{inv.endedAt ? formatStamp(inv.endedAt) : '—'}</td>
								<td>{inv.exitCode ?? '—'}</td>
							</tr>
					{/snippet}
				</DataTable>
			{/if}
		</div>
	{/if}
{/if}

<style>
	ul {
		display: flex;
		min-width: 0;
		flex: 1;
		flex-wrap: wrap;
		gap: 6px;
	}
	li a {
		display: inline-flex;
		align-items: center;
		gap: 6px;
		padding: 1px 8px;
		border: 1px solid var(--border);
		border-radius: 2px;
		line-height: 20px;
	}
	li a:hover {
		background: var(--sb-hover);
	}
	i {
		width: 7px;
		height: 7px;
		border-radius: 50%;
	}
	.faint {
		color: var(--muted-foreground);
	}
	.mono,
	code {
		font-family: var(--font-mono);
		font-size: 12px;
	}
	.details {
		flex: 0 0 auto;
		max-height: 300px;
		overflow: auto;
		padding: 12px 24px;
		border-bottom: 1px solid var(--border-subtle);
		background: var(--sb-surface-low);
		font-size: 12.5px;
	}
	dl div {
		display: grid;
		grid-template-columns: 150px 1fr;
		padding: 2px 0;
	}
	dt {
		color: var(--muted-foreground);
	}
	summary {
		width: fit-content;
		color: var(--primary);
		cursor: pointer;
	}
	a:hover {
		color: var(--primary);
	}
</style>
