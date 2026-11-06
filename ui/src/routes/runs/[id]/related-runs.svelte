<script lang="ts">
	import type { Run } from '$lib/api';
	import CopyButton from '$lib/components/copy-button.svelte';
	import { formatAgo, formatStamp } from '$lib/format/time';
	import { shortImage } from '$lib/platform/labels';
	import { shortRunId } from '$lib/operate/labels';
	import { app } from '$lib/state/app.svelte';
	import { runStatusLabel, runStatusTone } from '$lib/status';
	import type { RunDetailState } from './state.svelte';

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
</script>

{#if d}
	{#if related.length}
		<div class="strip">
			<span class="label">Related</span>
			<ul>
				{#each related as r (`${r.role}-${r.run.id}`)}
					<li>
						<a href="/runs/{r.run.id}" title="{r.role} {r.run.id} · {runStatusLabel[r.run.status]} · queued {formatStamp(r.run.queuedAt)}">
							<span class="faint">{r.role}</span>
							<i style:background={runStatusTone[r.run.status]}></i>
							<span class="mono">{shortRunId(r.run.id)}</span>
							{#if r.role === 'Job run' && r.run.jobKey}<span>{r.run.jobKey}</span>{/if}
							<span class="faint">{runStatusLabel[r.run.status].toLowerCase()}</span>
						</a>
					</li>
				{/each}
			</ul>
		</div>
	{/if}
	{#if view.showDetails}
		{@const p = d.provenance}
		<div class="details">
			<dl>
				<div><dt>Project version</dt><dd>Version {p.snapshotGeneration} · commit <code>{p.gitRevision}</code>{p.gitDirty ? ' (with uncommitted changes)' : ''} <span class="faint">· snapshot <code>{p.snapshotId}</code></span></dd></div>
				<div><dt>Command</dt><dd><code>{p.command}</code> <CopyButton value={p.command} label="Copy command" /></dd></div>
				{#if p.selection.length || p.exclude.length}
					<div><dt>Selection</dt><dd><code>{p.selection.join(' ') || '—'}</code>{#if p.exclude.length} excluding <code>{p.exclude.join(' ')}</code>{/if}</dd></div>
				{/if}
				<div><dt>Image</dt><dd><code title={p.imageId}>{shortImage(p.imageId)}</code> · sqb {p.sqbVersion}</dd></div>
				<div><dt>Rebuilds history</dt><dd>{d.run.rewriteHistory ? 'Yes: a full refresh or a set time window, so it needs extra permission to rerun' : 'No'}</dd></div>
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
				<table>
					<thead><tr><th>sqb invocation</th><th>Run</th><th>Attempt</th><th>Started</th><th>Ended</th><th>Exit code</th></tr></thead>
					<tbody>
						{#each d.invocations as inv (inv.invocationId)}
							<tr>
								<td class="mono">{inv.invocationId}</td>
								<td class="mono"><a href="/runs/{inv.runId}">{shortRunId(inv.runId)}</a></td>
								<td>{inv.attempt}</td>
								<td>{formatStamp(inv.startedAt)}</td>
								<td>{inv.endedAt ? formatStamp(inv.endedAt) : '—'}</td>
								<td>{inv.exitCode ?? '—'}</td>
							</tr>
						{/each}
					</tbody>
				</table>
			{/if}
		</div>
	{/if}
{/if}

<style>
	.strip {
		display: flex;
		flex: 0 0 auto;
		align-items: center;
		gap: 10px;
		padding: 6px 24px;
		border-bottom: 1px solid var(--border);
		font-size: 12.5px;
	}
	.label {
		color: var(--sb-text-faint);
		font-weight: 500;
	}
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
		padding: 10px 24px;
		border-bottom: 1px solid var(--border);
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
	table {
		margin-top: 10px;
		border-collapse: collapse;
		font-size: 12px;
	}
	th {
		padding: 4px 16px 4px 0;
		color: var(--sb-text-faint);
		font-weight: 500;
		text-align: left;
	}
	td {
		padding: 3px 16px 3px 0;
	}
	a:hover {
		color: var(--primary);
	}
</style>
