<script lang="ts">
	import Button from '$lib/components/ui/button/button.svelte';
	import { to } from '$lib/paths';
	import CopyButton from '$lib/components/copy-button.svelte';
	import RunStatusPill from '$lib/components/operate/run-status-pill.svelte';
	import { formatAgo, formatDuration, formatStamp } from '$lib/format/time';
	import { narrowingText, shortRunId } from '$lib/operate/labels';
	import { app } from '$lib/state/app.svelte';
	import { runKindLabel, runOriginLabel } from '$lib/status';
	import RunSummary from './run-summary.svelte';
	import RerunMenu from './rerun-menu.svelte';
	import { page } from '$app/state';
	import ChevronDownIcon from '@lucide/svelte/icons/chevron-down';
	import PanelRightIcon from '@lucide/svelte/icons/panel-right';
	import { ACTION_CLASS, actionLook } from './utils/action-looks';
	import { retriedMembers } from './utils';
	import type { RunDetailState } from './state.svelte';

	let { view, runId }: { view: RunDetailState; runId: string } = $props();

	// Mock-up: the header actions' look, switchable with ?actions=0|1|2|3|4 for comparison; 4 by default.
	const look = $derived(actionLook(page.url.searchParams.get('actions')));
	const detailsClass = ACTION_CLASS.details;

	const run = $derived(view.run);
	const what = $derived(run?.jobKey ?? (view.detail?.selection.join(' ') || 'whole project'));
	const cancelRequested = $derived(!!view.detail?.cancel && !view.detail.cancel.terminatedAt);
	const retried = $derived(retriedMembers(view.detail?.children ?? []));
	/** A group's member that never started: "not started", never "skipped". */
	const notStarted = $derived(!!run?.parentRunId && run.status === 'skipped' && !run.startedAt);
	type Fact = { text: string; href?: string; title?: string; live?: boolean };
	/** The first line's facts, in order; a missing one leaves no separator behind. */
	const facts = $derived.by((): Fact[] => {
		if (!run) return [];
		const out: (Fact | null)[] = [
			{ text: runKindLabel[run.kind].toLowerCase() },
			run.triggerId
				? { text: `${runOriginLabel[run.origin].toLowerCase()}${run.triggerName ? `: ${run.triggerName}` : ''}`, href: `/triggers/${run.triggerId}`, title: 'Open what started this run' }
				: run.rerunOfRunId
					? { text: `rerun of ${shortRunId(run.rerunOfRunId)}`, href: `/runs/${run.rerunOfRunId}` }
					: { text: runOriginLabel[run.origin].toLowerCase() },
			run.narrowing ? { text: `part: ${narrowingText(run.narrowing)}`, title: `Ran part of ${run.jobKey}` } : null,
			run.window ? { text: `window ${run.window.start} → ${run.window.end}` } : null,
			retried ? { text: `${retried} retried`, title: 'Member jobs the group retried inside this run' } : null,
			{ text: `by ${run.requestedBy.displayName}`, title: run.requestedBy.name },
			run.startedAt ? { text: formatAgo(run.startedAt, app.now), title: `Started ${formatStamp(run.startedAt)}` } : { text: `queued ${formatAgo(run.queuedAt, app.now)}`, title: formatStamp(run.queuedAt) },
			run.startedAt ? { text: `${view.active ? 'running for ' : ''}${formatDuration(run.startedAt, run.endedAt, app.now)}`, live: view.active, title: view.active ? 'Running for' : 'Took' } : null
		];
		return out.filter((f): f is Fact => !!f);
	});
</script>

<div class="head">
	<div class="left">
		<nav class="crumbs" aria-label="Breadcrumb">
			<a href={to(`/runs`)}>Runs</a>
			{#if run?.parentRunId}<span>/</span><a href={to(`/runs/${run.parentRunId}`)} title="Group run {run.parentRunId}">group {shortRunId(run.parentRunId)}</a>{/if}
			<span>/</span>
			<span class="mono" title={runId}>{shortRunId(runId)}</span>
			<CopyButton value={runId} label="Copy run ID" />
		</nav>
		{#if run}
			<!-- One line: what ran, how it ended, its facts and what it did; the actions on the right. On a
			     narrow page the facts wrap under the title and the actions stay right. The rest is under Details. -->
			<div class="title-row">
				<div class="title">
					<h2>
						{#if run.jobKey}<a href={to(`/jobs/${run.jobKey}`)}>{run.jobKey}</a>{:else}<span class="adhoc">ad hoc: <code>{what}</code></span>{/if}
					</h2>
					<RunStatusPill status={run.status} size="md" ignored={run.ignoredAuditFailures} notStarted={notStarted} />
					<span class="meta">
						{#each facts as f, i (i)}
							{#if i}<span class="sep">{' · '}</span>{/if}
							{#if f.href}<a href={to(f.href)} title={f.title}>{f.text}</a>{:else}<span class:live={f.live} title={f.title}>{f.text}</span>{/if}
						{/each}
						<span class="sep before-figures">{' · '}</span><RunSummary {view} />
					</span>
				</div>
				<div class="actions">
					{#if !view.active}<RerunMenu {view} {look} />{/if}
					{#if view.active}
						<span title={!app.can('run.cancel') ? "You don't have permission to cancel (run.cancel)" : cancelRequested ? 'Cancel already requested; see the progress below' : 'Stops the run: running queries are cancelled, then the run is stopped'}>
							<Button
								variant={view.cancelArmed ? 'destructive' : 'outline'}
								size="sm"
								disabled={!app.can('run.cancel') || cancelRequested || view.busy !== null}
								aria-busy={view.busy === 'cancel'}
								onclick={() => view.cancel()}
								onblur={() => view.disarmCancel()}
							>
								{cancelRequested ? 'Cancelling…' : view.cancelArmed ? 'Confirm cancel' : 'Cancel run'}
							</Button>
						</span>
					{/if}
					<Button variant={look === 'before' ? 'ghost' : 'outline'} size="sm" class={detailsClass[look]} onclick={() => view.toggleDetails()} aria-expanded={view.showDetails} title="Command, version, worker, cost and related runs">
						{#if look === '3' || look === '4'}<PanelRightIcon />{/if}Details{#if look === 'before'} {view.showDetails ? '▴' : '▾'}{:else if look !== '3' && look !== '4'}<ChevronDownIcon />{/if}
					</Button>
				</div>
			</div>
		{:else if view.detailRes.error}
			<p class="err">Couldn't load this run: {view.detailRes.error}</p>
		{:else if view.detailRes.data === null}
			<p class="faint">There's no run {runId} in this location. It may belong to another location.</p>
		{/if}
	</div>
</div>
{#if view.actionError}<div class="action-error" role="alert">{view.actionError}</div>{/if}

<style>
	.head {
		display: flex;
		align-items: flex-start;
		gap: 16px;
		padding: 16px 24px 14px;
		border-bottom: 1px solid var(--border-subtle);
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
		align-items: flex-start;
		gap: 16px;
		margin-top: 4px;
	}
	.title {
		display: flex;
		min-width: 0;
		flex: 1;
		flex-wrap: wrap;
		align-items: center;
		gap: 4px 10px;
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
	/* Stays beside the title while it has room, wrapping inside itself; below that it wraps under the title. */
	.meta {
		min-width: 0;
		flex: 1 1 360px;
		display: inline-flex;
		flex-wrap: wrap;
		align-items: center;
		gap: 0;
		white-space: pre-wrap;
		color: var(--muted-foreground);
		font-size: 12.5px;
	}
	.meta a:hover {
		color: var(--primary);
	}
	.sep {
		color: var(--sb-text-faint);
	}
	/* Clear of the first figure's reserved pill room, as between the figures. */
	.before-figures {
		margin-right: 6px;
	}
	.live {
		color: var(--sb-running);
		font-variant-numeric: tabular-nums;
	}
	.actions {
		display: flex;
		flex: none;
		align-items: center;
		gap: 6px;
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
