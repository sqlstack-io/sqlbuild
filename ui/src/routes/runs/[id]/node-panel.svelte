<script lang="ts">
	import ChevronRightIcon from '@lucide/svelte/icons/chevron-right';
	import Button from '$lib/components/ui/button/button.svelte';
	import { to } from '$lib/paths';
	import type { RunNode } from '$lib/api';
	import { shortRunId } from '$lib/operate/labels';
	import type { RunDetailState } from './state.svelte';
	import type { NodeGroupKey } from './types';
	import { formatSpan, groupNodes, notRunReason } from './utils';

	let { view }: { view: RunDetailState } = $props();

	const groups = $derived(groupNodes(view.nodes));
	// Small runs show every node; large ones start with the successes folded away.
	let closed = $state<Set<NodeGroupKey>>(new Set());
	let folded = false;
	$effect(() => {
		if (folded || !view.nodes.length) return;
		folded = true;
		if (view.nodes.length > 40) closed = new Set(['succeeded']);
	});
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
			<!-- raw-button: the sticky header of an outcome group, clickable as a whole to fold it -->
			<button class="group" onclick={() => toggle(g.key)} aria-expanded={!closed.has(g.key)}>
				<i style:background={g.tone}></i>{g.label}<b>{g.nodes.length}</b><ChevronRightIcon size={13} class="caret {closed.has(g.key) ? '' : 'open'}" />
			</button>
			{#if !closed.has(g.key)}
				<ul>
					{#each g.nodes as n (`${n.runId}/${n.nodeKey}`)}
						<li class:sel={view.selected.includes(n.nodeKey)}>
							<!-- raw-button: a row of the node list; it filters the tabs below to that node -->
							<button class="node" onclick={(e) => pick(e, n)} title="Click to filter below; Ctrl- or Cmd-click to add to the selection">
								<span class="mono">{n.nodeKey}</span>
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
							{:else if notRunReason(n)}
								<div class="why" title={n.status === 'not_run' ? 'The run ended before this node started' : undefined}>{notRunReason(n)}</div>
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
	.sb-scroll {
		scrollbar-width: thin;
		scrollbar-color: var(--border-strong) transparent;
	}
</style>
