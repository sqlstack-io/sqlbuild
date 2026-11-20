<script lang="ts">
	import XIcon from '@lucide/svelte/icons/x';
	import Button from '$lib/components/ui/button/button.svelte';
	import { to } from '$lib/paths';
	import { formatDuration } from '$lib/format/time';
	import { nodeLabel, nodeStatusTone, shortRunId } from '$lib/operate/labels';
	import type { RunDetailState } from './state.svelte';
	import { fullStamp, skipCause } from './utils/proto';

	// A node's details in a drawer from the right (CHI-1591, change 4), in place of the side panel: what
	// happened to it in this run, why, its checks, and what to do next.
	let { view }: { view: RunDetailState } = $props();

	const key = $derived(view.drawer);
	const n = $derived(key ? (view.nodes.find((x) => x.nodeKey === key) ?? null) : null);
	const name = $derived(key ? nodeLabel(key) : '');
	const cause = $derived(n ? skipCause(n, view.nodes, view.detail?.edges ?? []) : null);
	const checks = $derived(key ? (view.eventsRes.data ?? []).filter((e) => e.type === 'audit_completed' && e.nodeKey === key) : []);
	const selected = $derived(!!key && view.selected.includes(key));
	const STATUS: Record<string, string> = { success: 'Built', warn: 'Built, with warnings', failed: 'Failed', skipped: 'Skipped', running: 'Running', pending: 'Waiting', not_run: 'Not run', cancelled: 'Cancelled' };
	const rows = $derived(n?.rowsAffected == null ? null : `${n.kind === 'incremental' ? '+' : ''}${n.rowsAffected.toLocaleString('en-GB')} rows${n.kind === 'incremental' ? ' inserted' : ''}`);

	function onkey(e: KeyboardEvent) {
		if (e.key === 'Escape' && key) view.openDrawer(null);
	}
</script>

<svelte:window onkeydown={onkey} />

{#if n && key}
	<aside class="drawer" aria-label="{name} in this run" data-testid="node-drawer">
		<header>
			<div class="title">
				<a class="mono" href={to(`/catalog/${key}`)} title="Open {key} in the catalog">{name}</a>
				<span class="state"><i style:background={nodeStatusTone[n.status]}></i>{STATUS[n.status] ?? n.status}</span>
			</div>
			<Button variant="ghost" size="icon-sm" onclick={() => view.openDrawer(null)} aria-label="Close" title="Close (Esc)"><XIcon /></Button>
		</header>
		<dl>
			<dt>Kind</dt><dd>{n.kind}</dd>
			{#if n.jobKey && view.detail?.children.length}<dt>Job run</dt><dd><a href={to(`/runs/${n.runId}`)}>{n.jobKey} · {shortRunId(n.runId)}</a></dd>{/if}
			{#if n.startedAt}<dt>Started</dt><dd class="mono">{fullStamp(n.startedAt)}</dd>{/if}
			{#if n.startedAt}<dt>Took</dt><dd>{formatDuration(n.startedAt, n.endedAt, Date.now())}{n.status === 'running' ? ' so far' : ''}</dd>{/if}
			{#if rows}<dt>Rows</dt><dd>{rows}</dd>{/if}
			{#if n.attempt > 1}<dt>Attempt</dt><dd>{n.attempt}</dd>{/if}
		</dl>
		{#if cause}<p class="why">{cause}</p>{/if}
		{#if n.status === 'failed' && n.errorMessage}
			<div class="error">
				<b>{n.errorMessage}</b>
				{#if n.errorLocation}<div class="faint mono">{n.errorLocation}</div>{/if}
				{#if n.errorDetail}<pre>{n.errorDetail}</pre>{/if}
			</div>
		{/if}
		{#if checks.length}
			<h3>Checks</h3>
			<ul class="checks">
				{#each checks as c (c.id)}
					<li><i style:background={c.level === 'warn' ? 'var(--sb-warning)' : c.level === 'error' ? 'var(--sb-error)' : 'var(--sb-success)'}></i><span class="mono">{c.message}</span></li>
				{/each}
			</ul>
		{/if}
		<div class="actions">
			<Button size="sm" variant="outline" onclick={() => view.focusNode(key, 'log')}>Filter the log to {name}</Button>
			<Button size="sm" variant="outline" onclick={() => view.selectNode(key)}>{selected ? 'Take out of the selection' : 'Add to the selection'}</Button>
			<a href={to(`/lineage?focus=${encodeURIComponent(key)}`)}>Open in Lineage</a>
		</div>
	</aside>
{/if}

<style>
	.drawer {
		position: fixed;
		top: 0;
		right: 0;
		bottom: 0;
		z-index: 40;
		display: flex;
		width: min(460px, 92vw);
		flex-direction: column;
		gap: 12px;
		overflow-y: auto;
		padding: 14px 18px;
		border-left: 1px solid var(--border);
		background: var(--card);
		box-shadow: -8px 0 24px rgb(0 0 0 / 0.18);
		font-size: 12.5px;
	}
	header {
		display: flex;
		align-items: flex-start;
		justify-content: space-between;
		gap: 8px;
	}
	.title {
		display: flex;
		flex-direction: column;
		gap: 4px;
		min-width: 0;
	}
	.title a {
		font-size: 14px;
		font-weight: 600;
		overflow-wrap: anywhere;
	}
	.state i,
	.checks i {
		display: inline-block;
		width: 7px;
		height: 7px;
		margin-right: 6px;
		border-radius: 50%;
	}
	dl {
		display: grid;
		grid-template-columns: max-content 1fr;
		gap: 4px 14px;
		margin: 0;
	}
	dt {
		color: var(--sb-text-faint);
	}
	dd {
		margin: 0;
	}
	.why {
		margin: 0;
		color: var(--muted-foreground);
	}
	.error {
		padding: 8px 10px;
		border-left: 2px solid var(--sb-error);
		background: color-mix(in srgb, var(--sb-error) 7%, transparent);
	}
	.error pre {
		margin: 6px 0 0;
		font-size: 11.5px;
		white-space: pre-wrap;
	}
	h3 {
		margin: 4px 0 0;
		color: var(--sb-text-faint);
		font-size: 12px;
		font-weight: 600;
	}
	.checks {
		display: flex;
		flex-direction: column;
		gap: 3px;
		margin: 0;
		padding: 0;
		list-style: none;
	}
	.actions {
		display: flex;
		flex-wrap: wrap;
		align-items: center;
		gap: 8px;
		margin-top: 4px;
	}
	.faint {
		color: var(--sb-text-faint);
	}
	.mono {
		font-family: var(--font-mono);
	}
</style>
