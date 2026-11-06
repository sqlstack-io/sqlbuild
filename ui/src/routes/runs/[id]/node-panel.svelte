<script lang="ts">
	import type { RunNode } from '$lib/api';
	import { shortRunId } from '$lib/operate/labels';
	import type { RunDetailState } from './state.svelte';
	import type { NodeGroupKey } from './types';
	import { groupNodes, notRunReason } from './utils';

	let { view }: { view: RunDetailState } = $props();

	const groups = $derived(groupNodes(view.nodes));
	let closed = $state<Set<NodeGroupKey>>(new Set(['succeeded']));

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
			<button class="group" onclick={() => toggle(g.key)} aria-expanded={!closed.has(g.key)}>
				<i style:background={g.tone}></i>{g.label}<b>{g.nodes.length}</b><span class="caret">{closed.has(g.key) ? '▸' : '▾'}</span>
			</button>
			{#if !closed.has(g.key)}
				<ul>
					{#each g.nodes as n (`${n.runId}/${n.nodeKey}`)}
						<li class:sel={view.selected.includes(n.nodeKey)}>
							<button class="node" onclick={(e) => pick(e, n)} title="Click to filter below; Ctrl- or Cmd-click to add to the selection">
								<span class="mono">{n.nodeKey}</span>
								{#if n.status === 'warn'}<span class="tag warn">warnings</span>{/if}
							</button>
							{#if n.skippedBecauseOf}
								{@const why = n.skippedBecauseOf}
								<div class="why">
									skipped:
									{#if why.runId === ownRun || view.nodes.some((x) => x.nodeKey === why.nodeKey)}
										<button class="link mono" onclick={() => view.selectNode(why.nodeKey)}>{why.nodeKey}</button>
									{:else}
										<a class="link mono" href="/runs/{why.runId}">{why.nodeKey}</a>
									{/if}
									failed{#if why.runId !== n.runId}&nbsp;in {why.jobKey ?? shortRunId(why.runId)}{/if}
								</div>
							{:else if notRunReason(n)}
								<div class="why">{notRunReason(n)}</div>
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
	.caret {
		width: 10px;
		color: var(--sb-text-faint);
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
	.tag {
		padding: 0 4px;
		border-radius: 2px;
		font-size: 10.5px;
	}
	.tag.warn {
		background: color-mix(in srgb, var(--sb-warning) 14%, transparent);
		color: var(--sb-warning);
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
