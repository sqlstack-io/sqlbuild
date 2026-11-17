<script lang="ts">
	import ChevronRightIcon from '@lucide/svelte/icons/chevron-right';
	import Button from '$lib/components/ui/button/button.svelte';
	import { to } from '$lib/paths';
	import type { RunNode } from '$lib/api';
	import { nodeLabel, shortRunId } from '$lib/operate/labels';
	import type { RunDetailState } from './state.svelte';
	import type { NodeGroupKey } from './types';
	import { formatSpan, notRunReason } from './utils';

	let { view }: { view: RunDetailState } = $props();

	/** The most nodes a run may have for this panel to list them all. */
	const LIST_MAX = 40;

	const groups = $derived(view.nodeGroups);
	/**
	 * A small run lists every node here. A large one's timeline already lists them, so here it is a summary:
	 * each outcome's count, which narrows the timeline to it, with only the failed and skipped nodes listed,
	 * as each says why.
	 */
	const summary = $derived(view.nodes.length > LIST_MAX);
	const LISTED_IN_SUMMARY: NodeGroupKey[] = ['failed', 'skipped'];
	let closed = $state<Set<NodeGroupKey>>(new Set());
	const open = (key: NodeGroupKey) => (summary ? LISTED_IN_SUMMARY.includes(key) : !closed.has(key));
	const took = (n: RunNode) => (n.startedAt && n.endedAt ? formatSpan((Date.parse(n.endedAt) - Date.parse(n.startedAt)) / 1000) : null);

	function toggle(key: NodeGroupKey) {
		const next = new Set(closed);
		if (next.has(key)) next.delete(key);
		else next.add(key);
		closed = next;
	}

	function pick(e: MouseEvent, n: RunNode) {
		view.selectNode(n.nodeKey, e.metaKey || e.ctrlKey || e.shiftKey);
	}

	const ownRun = $derived(view.run?.id);
</script>

<div class="panel sb-scroll" aria-label="Nodes by outcome">
	{#each groups as g (g.key)}
		<section>
			{#if summary}
				<!-- raw-button: an outcome's count in the summary; it narrows the timeline to that outcome's nodes -->
				<button class="group" class:picked={view.statusFilter === g.key} data-group={g.key} onclick={() => view.filterStatus(g.key)} aria-pressed={view.statusFilter === g.key} title="Show only these on the timeline">
					<i style:background={g.tone}></i>{g.label}<b>{g.nodes.length}</b>
				</button>
			{:else}
				<!-- raw-button: the sticky header of an outcome group, clickable as a whole to fold it -->
				<button class="group" data-group={g.key} onclick={() => toggle(g.key)} aria-expanded={!closed.has(g.key)}>
					<i style:background={g.tone}></i>{g.label}<b>{g.nodes.length}</b><ChevronRightIcon size={13} class="caret {closed.has(g.key) ? '' : 'open'}" />
				</button>
			{/if}
			{#if open(g.key)}
				<ul>
					{#each g.nodes as n (`${n.runId}/${n.nodeKey}`)}
						<li class:sel={view.selected.includes(n.nodeKey)}>
							<!-- raw-button: a row of the node list; it filters the tabs below to that node -->
							<button class="node" onclick={(e) => pick(e, n)} title="Click to filter below; Ctrl- or Cmd-click to add to the selection">
								<span class="mono" title={n.nodeKey}>{nodeLabel(n.nodeKey)}</span>
								{#if n.status === 'warn'}<span class="sb-tag warn">warnings</span>{/if}
								{#if took(n)}<span class="took">{took(n)}</span>{/if}
							</button>
							{#if n.skippedBecauseOf}
								{@const why = n.skippedBecauseOf}
								<div class="why">
									skipped:
									{#if why.runId === ownRun || view.nodes.some((x) => x.nodeKey === why.nodeKey)}
										<Button variant="link" size="inline" class="font-mono text-[11.5px]" onclick={() => view.selectNode(why.nodeKey)}>{why.nodeKey}</Button>
									{:else}
										<a class="link mono" href={to(`/runs/${why.runId}`)}>{why.nodeKey}</a>
									{/if}
									failed{#if why.runId !== n.runId}&nbsp;in {why.jobKey ?? shortRunId(why.runId)}{/if}
								</div>
							{:else if n.status !== 'pending' && notRunReason(n, view.active)}
								<div class="why" title={n.status === 'not_run' ? 'The run ended before this node started' : undefined}>{notRunReason(n, view.active)}</div>
							{/if}
						</li>
					{/each}
				</ul>
			{/if}
		</section>
	{:else}
		<p class="empty">No nodes yet.</p>
	{/each}
</div>

<style>
	.took {
		margin-left: auto;
		color: var(--sb-text-faint);
		font-size: 11px;
		white-space: nowrap;
	}
	.panel {
		width: 260px;
		flex-shrink: 0;
		overflow-y: auto;
		border-left: 1px solid var(--border);
		font-size: 12.5px;
	}
	.group {
		position: sticky;
		top: 0;
		z-index: 1;
		display: flex;
		width: 100%;
		align-items: center;
		gap: 8px;
		padding: 8px 14px;
		border-bottom: 1px solid var(--border-subtle);
		background: var(--background);
		font-weight: 500;
		text-align: left;
	}
	.group.picked {
		background: color-mix(in srgb, var(--primary) 12%, var(--background));
		box-shadow: inset 2px 0 0 var(--primary);
	}
	.group i {
		width: 8px;
		height: 8px;
		border-radius: 2px;
	}
	.group b {
		margin-left: auto;
		color: var(--muted-foreground);
		font-weight: 500;
	}
	.group :global(.caret) {
		flex: 0 0 auto;
		color: var(--sb-text-faint);
		transition: transform 120ms;
	}
	.group :global(.caret.open) {
		transform: rotate(90deg);
	}
	li {
		padding: 4px 14px 4px 30px;
		border-bottom: 1px solid var(--border-subtle);
	}
	li.sel {
		background: color-mix(in srgb, var(--primary) 10%, transparent);
		box-shadow: inset 2px 0 0 var(--primary);
	}
	li:hover {
		background: var(--sb-hover);
	}
	.node {
		display: flex;
		width: 100%;
		align-items: center;
		gap: 6px;
		text-align: left;
	}
	.mono {
		overflow: hidden;
		font-family: var(--font-mono);
		font-size: 12px;
		text-overflow: ellipsis;
		white-space: nowrap;
	}
	.why {
		color: var(--muted-foreground);
		font-size: 11.5px;
	}
	.link {
		color: var(--primary);
		font-size: 11.5px;
	}
	.link:hover {
		text-decoration: underline;
	}
	.empty {
		padding: 16px;
		color: var(--muted-foreground);
	}
</style>
