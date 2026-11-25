<script lang="ts">
	import DataTable from '$lib/components/list/data-table.svelte';
	import type { TableColumn } from '$lib/components/list/table-columns';
	import CopyButton from '$lib/components/copy-button.svelte';
	import InspectorSection from '$lib/components/lineage/inspector-section.svelte';
	import Button from '$lib/components/ui/button/button.svelte';
	import type { Run } from '$lib/api';
	import { formatAgo, formatDuration, formatStamp } from '$lib/format/time';
	import { shortRunId } from '$lib/operate/labels';
	import { to } from '$lib/paths';
	import { shortImage } from '$lib/platform/labels';
	import { app } from '$lib/state/app.svelte';
	import { runStatusLabel, runStatusTone } from '$lib/status';
	import RunDrawer from './run-drawer.svelte';
	import type { RunDetailState } from './state.svelte';
	import { attemptLabel, costSummary } from './utils';

	// The run's details in the right-side panel, in three short sections: what ran, where it ran, and its
	// cost and place in the queue. Mono only for what is copied (commands, IDs); copy shows on hover.
	let { view }: { view: RunDetailState } = $props();

	const d = $derived(view.detail);
	const p = $derived(d?.provenance ?? null);
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
	const selection = $derived(p?.selection.length ? p.selection : (d?.selection ?? []));
	const invocationColumns: TableColumn[] = [
		{ id: 'invocation', label: 'Invocation', minWidth: 160 },
		{ id: 'attempt', label: 'Attempt', width: '70px' },
		{ id: 'started', label: 'Started', width: '150px' },
		{ id: 'exit', label: 'Exit', width: '50px' }
	];
</script>

{#snippet copyable(value: string, label: string, shown: string = value)}
	<span class="copyable"><code title={value}>{shown}</code><CopyButton {value} {label} showValue={false} /></span>
{/snippet}

{#if d && view.showDetails}
	<RunDrawer label="Run details" testid="details-drawer" onclose={() => view.toggleDetails()}>
		{#snippet header()}
			<span class="title">Details</span>
			<div class="sub">Run {shortRunId(d.run.id)}{d.run.jobKey ? ` · ${d.run.jobKey}` : ''}</div>
		{/snippet}

		<InspectorSection title="What ran">
			<dl>
				{#if p?.command}<dt>Command</dt><dd>{@render copyable(p.command, 'Copy command')}</dd>{/if}
				{#if selection.length}
					<dt>Selection</dt>
					<dd>
						{#if selection.length > 12}<details><summary>{selection.length} nodes</summary><code>{selection.join(' ')}</code></details>{:else}{@render copyable(selection.join(' '), 'Copy selection')}{/if}
						{#if p?.exclude.length}<span class="muted"> excluding </span><code>{p.exclude.join(' ')}</code>{/if}
					</dd>
				{/if}
				{#if p}
					<dt>Project</dt>
					<dd>Version {p.snapshotGeneration}{#if p.gitDirty}<span class="muted" title="Published with uncommitted changes"> · uncommitted changes</span>{/if}</dd>
					{#if p.gitRevision}<dt>Commit</dt><dd>{@render copyable(p.gitRevision, 'Copy commit')}</dd>{/if}
					<dt>Snapshot</dt><dd>{@render copyable(p.snapshotId, 'Copy snapshot ID')}</dd>
				{/if}
				<dt>Run ID</dt><dd>{@render copyable(d.run.id, 'Copy run ID')}</dd>
			</dl>
		</InspectorSection>

		<InspectorSection title="Where">
			<dl>
				{#if d.liveness}
					<dt>Worker</dt>
					<dd>
						{@render copyable(d.liveness.workerInstance ?? '—', 'Copy worker')}
						{#if d.liveness.lastHeartbeatAt}<div class="muted">Last heartbeat {formatAgo(d.liveness.lastHeartbeatAt, app.now)}</div>{/if}
					</dd>
					{#if d.liveness.containerId}<dt>Container</dt><dd>{@render copyable(d.liveness.containerId, 'Copy container ID')}</dd>{/if}
				{/if}
				{#if p}<dt>{p.imageId ? 'Image' : 'sqb'}</dt><dd>{#if p.imageId}{@render copyable(p.imageId, 'Copy image', shortImage(p.imageId))}<div class="muted">sqb {p.sqbVersion}</div>{:else}{p.sqbVersion}{/if}</dd>{/if}
				{#if d.invocations.length === 1}
					{@const inv = d.invocations[0]}
					<dt>Invocation</dt>
					<dd>{@render copyable(inv.invocationId, 'Copy invocation ID')}<div class="muted">{formatDuration(inv.startedAt, inv.endedAt, app.now)}{inv.exitCode != null ? ` · exit ${inv.exitCode}` : ''}</div></dd>
				{/if}
			</dl>
			{#if d.invocations.length > 1}
				<!-- Several attempts: one row each. -->
				<DataTable class="static compact" embedded columns={invocationColumns} rows={d.invocations} key={(inv) => inv.invocationId} empty="No invocations.">
					{#snippet row(inv)}
						<tr>
							<td><span class="copyable"><code>{inv.invocationId}</code><CopyButton value={inv.invocationId} label="Copy invocation ID" showValue={false} /></span></td>
							<td>{inv.attempt}</td>
							<td class="muted">{formatStamp(inv.startedAt)}</td>
							<td>{inv.exitCode ?? '—'}</td>
						</tr>
					{/snippet}
				</DataTable>
			{/if}
		</InspectorSection>

		<InspectorSection title="Cost & queue">
			<dl>
				{#if view.cost && view.cost.status !== 'none'}<dt>Cost</dt><dd><Button variant="link" size="inline" onclick={() => (view.tab = 'cost')}>{costSummary(view.cost).text}</Button></dd>{/if}
				<dt>Priority</dt><dd>{d.run.priority}</dd>
				<dt>Rebuilds history</dt><dd title={d.run.rewriteHistory ? 'Full refresh or a set window; rerunning needs run.rewrite_history' : undefined}>{d.run.rewriteHistory ? 'Yes' : 'No'}</dd>
				<dt>Queued</dt><dd>{formatStamp(d.run.queuedAt)}</dd>
				{#if d.run.tickScheduledFor}<dt>Scheduled for</dt><dd>{formatStamp(d.run.tickScheduledFor)}</dd>{/if}
				{#if d.run.runKey}<dt>Sensor run key</dt><dd>{@render copyable(d.run.runKey, 'Copy run key')}</dd>{/if}
				{#if related.length}
					<dt>Related runs</dt>
					<dd>
						<ul>
							{#each related as r (`${r.role}-${r.run.id}`)}
								<li>
									<a href={to(`/runs/${r.run.id}`)} title="{r.role} {r.run.id} · {runStatusLabel[r.run.status]}">
										<span class="muted">{r.role}</span>
										<i style:background={runStatusTone[r.run.status]}></i>
										{shortRunId(r.run.id)}{#if r.role === 'Job run' && r.run.jobKey} · {r.run.jobKey}{/if}
										{#if r.role === 'Job run'}{@const attempt = attemptLabel(r.run, d.children)}{#if attempt}<span class="muted" data-testid="member-attempt">{attempt}</span>{/if}{/if}
									</a>
								</li>
							{/each}
						</ul>
					</dd>
				{/if}
			</dl>
		</InspectorSection>
	</RunDrawer>
{/if}

<style>
	.title {
		font-size: 14px;
		font-weight: 600;
	}
	.sub {
		margin-top: 4px;
		color: var(--muted-foreground);
		font-size: 12px;
	}
	dl {
		display: grid;
		grid-template-columns: 110px 1fr;
		gap: 8px 12px;
		padding: 10px 6px 12px;
	}
	dt {
		color: var(--muted-foreground);
	}
	dd {
		min-width: 0;
		margin: 0;
		overflow-wrap: anywhere;
	}
	.muted {
		color: var(--muted-foreground);
	}
	code {
		font-family: var(--font-mono);
		font-size: 12px;
	}
	/* Copy shows on hover or focus, beside the value it copies. */
	.copyable {
		display: inline-flex;
		max-width: 100%;
		align-items: flex-start;
		gap: 4px;
	}
	/* Long values (a command, a selection) wrap rather than being cut off. */
	.copyable code {
		overflow-wrap: anywhere;
	}
	.copyable :global(.copy) {
		opacity: 0;
	}
	.copyable:hover :global(.copy),
	.copyable :global(.copy:focus-visible) {
		opacity: 1;
	}
	summary {
		width: fit-content;
		color: var(--primary);
		cursor: pointer;
	}
	ul {
		display: flex;
		flex-direction: column;
		gap: 4px;
	}
	li a {
		display: inline-flex;
		align-items: center;
		gap: 6px;
	}
	li a:hover {
		color: var(--primary);
	}
	i {
		width: 7px;
		height: 7px;
		border-radius: 50%;
	}
</style>
