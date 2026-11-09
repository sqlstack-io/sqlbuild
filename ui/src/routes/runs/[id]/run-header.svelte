<script lang="ts">
	import CopyButton from '$lib/components/copy-button.svelte';
	import RunStatusPill from '$lib/components/operate/run-status-pill.svelte';
	import { formatAgo, formatDuration, formatStamp } from '$lib/format/time';
	import { shortRunId } from '$lib/operate/labels';
	import { app } from '$lib/state/app.svelte';
	import { runKindLabel, runOriginLabel, runStatusTone } from '$lib/status';
	import RerunButton from './rerun-button.svelte';
	import type { RunDetailState } from './state.svelte';

	let { view, runId }: { view: RunDetailState; runId: string } = $props();

	const run = $derived(view.run);
	const pct = $derived(view.progress.total ? (view.progress.done / view.progress.total) * 100 : 0);
	const what = $derived(run?.jobKey ?? (view.detail?.selection.join(' ') || 'whole project'));
	const cancelRequested = $derived(!!view.detail?.cancel && !view.detail.cancel.terminatedAt);
</script>

<div class="head">
	<div class="left">
		<nav class="crumbs" aria-label="Breadcrumb">
			<a href="/runs">Runs</a>
			{#if run?.parentRunId}<span>/</span><a href="/runs/{run.parentRunId}" title="Group run {run.parentRunId}">group {shortRunId(run.parentRunId)}</a>{/if}
			<span>/</span>
			<span class="mono" title={runId}>{shortRunId(runId)}</span>
			<CopyButton value={runId} label="Copy run ID" />
		</nav>
		{#if run}
			<div class="title-row">
				<h2>
					{#if run.jobKey}<a href="/jobs/{run.jobKey}">{run.jobKey}</a>{:else}<span class="adhoc">ad hoc: <code>{what}</code></span>{/if}
				</h2>
				<RunStatusPill status={run.status} size="md" />
				<!-- The kind is a fact about this run; the origin opens what started it, when that has a page. -->
				<span class="chip" title="{runKindLabel[run.kind]} run">{runKindLabel[run.kind].toLowerCase()}</span>
				{#if run.triggerId}
					<a class="chip link" href="/triggers/{run.triggerId}" title="Open the {runOriginLabel[run.origin].toLowerCase()} that started this run">{runOriginLabel[run.origin].toLowerCase()}{#if run.triggerName}: {run.triggerName}{/if}</a>
				{:else if run.rerunOfRunId}
					<a class="chip link" href="/runs/{run.rerunOfRunId}" title="Open the run this one repeats">rerun of {shortRunId(run.rerunOfRunId)}</a>
				{:else}
					<span class="chip">{runOriginLabel[run.origin].toLowerCase()}</span>
				{/if}
			</div>
			<div class="meta">
				{#if run.startedAt}
					<span title={formatStamp(run.startedAt)}>Started {formatAgo(run.startedAt, app.now)}</span>
					<span>·</span>
					<span class:live={view.active}>{view.active ? 'running for ' : 'took '}{formatDuration(run.startedAt, run.endedAt, app.now)}</span>
				{:else}
					<span title={formatStamp(run.queuedAt)}>Queued {formatAgo(run.queuedAt, app.now)}</span>
				{/if}
				<span>·</span>
				<span title="{run.requestedBy.displayName} ({run.requestedBy.name})">by {run.requestedBy.displayName}</span>
				{#if view.progress.total}
					<span class="progress" title="{view.progress.done} of {view.progress.total} nodes finished">
						<span class="bar"><i style:width="{pct}%" style:background={runStatusTone[run.status]}></i></span>
						{view.progress.done}/{view.progress.total} nodes
					</span>
				{/if}
				<button class="details-btn" onclick={() => view.toggleDetails()} aria-expanded={view.showDetails} title="Version, command, image, worker and other technical details">{view.showDetails ? 'Hide details' : 'Details'}</button>
			</div>
		{:else if view.detailRes.error}
			<p class="err">Couldn't load this run: {view.detailRes.error}</p>
		{:else if view.detailRes.data === null}
			<p class="faint">There's no run {runId} in this location. It may belong to another location.</p>
		{/if}
	</div>
	{#if run}
		<div class="actions">
			{#if view.active}
				<span title={!app.can('run.cancel') ? "You don't have permission to cancel (run.cancel)" : cancelRequested ? 'Cancel already requested; see the progress below' : 'Stops the run: running queries are cancelled, then the run is stopped'}>
					<button
						class="cancel"
						class:armed={view.cancelArmed}
						disabled={!app.can('run.cancel') || cancelRequested || view.busy !== null}
						aria-busy={view.busy === 'cancel'}
						onclick={() => view.cancel()}
						onblur={() => view.disarmCancel()}
					>
						{cancelRequested ? 'Cancelling…' : view.cancelArmed ? 'Confirm cancel' : 'Cancel run'}
					</button>
				</span>
			{/if}
			<RerunButton {view} />
		</div>
	{/if}
</div>
{#if view.actionError}<div class="action-error" role="alert">{view.actionError}</div>{/if}

<style>
	.head {
		display: flex;
		align-items: flex-start;
		gap: 16px;
		padding: 12px 24px 12px;
		border-bottom: 1px solid var(--border);
	}
	.left {
		min-width: 0;
		flex: 1;
	}
	.crumbs {
		display: flex;
		align-items: center;
		gap: 6px;
		color: var(--muted-foreground);
		font-size: 12.5px;
	}
	.crumbs a:hover {
		color: var(--foreground);
	}
	.mono,
	code {
		font-family: var(--font-mono);
		font-size: 12px;
	}
	.crumbs .mono {
		color: var(--foreground);
	}
	.title-row {
		display: flex;
		flex-wrap: wrap;
		align-items: center;
		gap: 10px;
		margin-top: 4px;
	}
	h2 {
		font-family: var(--font-display);
		font-size: 18px;
		font-weight: 600;
	}
	h2 a:hover {
		color: var(--primary);
	}
	.adhoc {
		font-size: 15px;
	}
	.chip {
		padding: 0 8px;
		border: 1px solid var(--border);
		border-radius: 2px;
		color: var(--muted-foreground);
		font-size: 12px;
		line-height: 20px;
	}
	.chip.link:hover {
		color: var(--foreground);
		background: var(--sb-hover);
	}
	.meta {
		display: flex;
		flex-wrap: wrap;
		align-items: center;
		gap: 6px;
		margin-top: 6px;
		color: var(--muted-foreground);
		font-size: 12.5px;
	}
	.live {
		color: var(--sb-running);
		font-variant-numeric: tabular-nums;
	}
	.progress {
		display: inline-flex;
		align-items: center;
		gap: 8px;
		margin-left: 10px;
		font-variant-numeric: tabular-nums;
	}
	.bar {
		display: block;
		overflow: hidden;
		width: 140px;
		height: 6px;
		border-radius: 3px;
		background: var(--sb-hover);
	}
	.bar i {
		display: block;
		height: 100%;
		transition: width 0.4s ease;
	}
	.details-btn {
		margin-left: 10px;
		color: var(--primary);
		font-size: 12.5px;
	}
	.actions {
		display: flex;
		align-items: center;
		gap: 8px;
		padding-top: 18px;
	}
	.cancel {
		height: 32px;
		padding: 0 12px;
		border: 1px solid var(--border);
		border-radius: 6px;
		background: var(--background);
		font-size: 13px;
		font-weight: 500;
	}
	.cancel:hover:not(:disabled) {
		background: var(--sb-hover);
	}
	.cancel.armed {
		border-color: var(--sb-error);
		background: color-mix(in srgb, var(--sb-error) 14%, var(--background));
		color: var(--sb-error);
	}
	.cancel:disabled {
		opacity: 0.55;
	}
	.err,
	.action-error {
		color: var(--sb-error);
		font-size: 12.5px;
	}
	.action-error {
		padding: 6px 24px;
		border-bottom: 1px solid var(--border);
	}
	.faint {
		margin-top: 6px;
		color: var(--muted-foreground);
		font-size: 12.5px;
	}
</style>
