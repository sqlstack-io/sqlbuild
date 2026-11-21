<script lang="ts">
	import { untrack } from 'svelte';
	import SegmentedFilter from '$lib/components/list/segmented-filter.svelte';
	import Button from '$lib/components/ui/button/button.svelte';
	import Loading from '$lib/components/loading.svelte';
	import AppTopbar from '$lib/components/app-topbar.svelte';
	import { app } from '$lib/state/app.svelte';
	import FailuresPanel from './failures-panel.svelte';
	import IgnoredAuditsPanel from './ignored-audits-panel.svelte';
	import LiveStatements from './live-statements.svelte';
	import NodeDrawer from './node-drawer.svelte';
	import RunLineage from './run-lineage.svelte';
	import SelectionBar from '$lib/components/selection/selection-bar.svelte';
	import { to } from '$lib/paths';
	import { nodeLabel } from '$lib/operate/labels';
	import { skipCause, QUICK_PICKS } from './utils/proto';
	import RelatedRuns from './related-runs.svelte';
	import RunBanners from './run-banners.svelte';
	import RunGantt from './run-gantt.svelte';
	import RunHeader from './run-header.svelte';
	import RunTabs from './run-tabs.svelte';
	import { createRunDetailState } from './state.svelte';
	import { bottomTabs } from './utils';

	let { runId }: { runId: string } = $props();

	// svelte-ignore state_referenced_locally
	const view = createRunDetailState(runId);

	const detail = $derived(view.detail);
	const nodes = $derived(view.nodes);
	// Back to the page restores the view and the split as they were (CHI-1591).
	const mem = view.memory;
	let topView = $state<'timeline' | 'graph'>((mem.topView as 'timeline' | 'graph') ?? 'timeline');
	// A group run opens on its Graph: when groups get big, the graph is how you see them (CHI-1591).
	let viewChosen = 'topView' in mem;
	$effect(() => {
		if (viewChosen || !detail) return;
		viewChosen = true;
		if (detail.run.kind === 'group') topView = 'graph';
	});
	// A graph needs room: choosing it gives the top pane most of the height.
	$effect(() => {
		if (topView === 'graph') untrack(() => (topFrac = Math.max(topFrac, 0.68)));
	});
	/** The graph is always offered (CHI-1591, change 2): the run's nodes on their lineage, by folder. */
	const hasGraph = $derived(nodes.length > 0);
	const names = $derived([...new Map(nodes.map((n) => [nodeLabel(n.nodeKey), { value: nodeLabel(n.nodeKey), hint: n.jobKey ?? '' }])).values()]);
	const causeOf = (key: string) => {
		const n = nodes.find((x) => x.nodeKey === key);
		return n ? skipCause(n, nodes, detail?.edges ?? []) : null;
	};
	const planCount = $derived(view.selection.evaluation?.count ?? 0);
	const hasBottom = $derived(bottomTabs(app.client, (c) => app.can(c)).length > 0);

	/** The axis starts at the first node start, not at queue time. */
	const origin = $derived.by(() => {
		const starts = nodes.map((n) => (n.startedAt ? Date.parse(n.startedAt) : Infinity));
		const first = Math.min(...starts);
		if (Number.isFinite(first)) return first;
		return view.run?.startedAt ? Date.parse(view.run.startedAt) : app.now;
	});
	// A group run's nodes carry their member job as a quiet tag; the timeline is not split into sections.
	const memberOf = (runId: string) => detail?.children.find((c) => c.id === runId)?.jobKey ?? null;

	// ---- resizable split between the timeline and the bottom tabs ----
	let topFrac = $state((mem.topFrac as number) ?? 0.5);
	$effect(() => {
		Object.assign(mem, { topView, topFrac });
	});
	let dragging = $state(false);
	let splitEl = $state<HTMLDivElement>();
	// The timeline starts as tall as its rows, so a short run leaves the room to the log below.
	let sized = 'topFrac' in mem;
	$effect(() => {
		if (sized || !splitEl || !nodes.length || !hasBottom || !splitEl.clientHeight) return;
		sized = true;
		const rows = nodes.length;
		topFrac = untrack(() => topView) === 'graph' ? 0.68 : Math.min(0.6, Math.max(0.22, (125 + rows * 32) / splitEl.clientHeight));
	});
	function onDrag(e: PointerEvent) {
		if (!dragging || !splitEl) return;
		const r = splitEl.getBoundingClientRect();
		topFrac = Math.min(0.85, Math.max(0.15, (e.clientY - r.top) / r.height));
	}
	function onKey(e: KeyboardEvent) {
		if (e.key === 'ArrowUp') topFrac = Math.max(0.15, topFrac - 0.05);
		if (e.key === 'ArrowDown') topFrac = Math.min(0.85, topFrac + 0.05);
	}
</script>

<AppTopbar title="Runs" />

<!-- The drawer sits beside the page and pushes it aside (CHI-1591), so the toolbar and Plan these stay in view. -->
<div class="flex min-h-0 min-w-0 flex-1">
<div class="flex min-h-0 min-w-0 flex-1 flex-col">
	<RunHeader {view} {runId} />

	{#if detail}
		<RunBanners banners={view.banners} />
		<FailuresPanel {view} />
		<IgnoredAuditsPanel {view} />
		<RelatedRuns {view} />

		<div bind:this={splitEl} class="flex min-h-0 flex-1 flex-col" class:select-none={dragging}>
			<div class="flex min-h-0 flex-col" style:flex={hasBottom ? topFrac : 1}>
				{#if nodes.length}
					<!-- One toolbar row above the chart (CHI-1591): the view, the selection to plan with its quick picks, and Plan these. -->
					<div class="pick">
						<SelectionBar selection={view.selection} {names} total={nodes.length} build={false} copy={false}>
							{#snippet leading()}
								<SegmentedFilter label="View" bind:value={topView} options={[{ value: 'timeline', label: 'Timeline' }, { value: 'graph', label: 'Graph' }]} />
							{/snippet}
							{#snippet trailing()}
								<span class="picks">
									<span class="faint">Pick</span>
									{#each QUICK_PICKS as q (q.value)}
										<Button variant="link" size="inline" class="text-[12.5px]" onclick={() => view.pick(q.value)}>{q.label}</Button>
									{/each}
								</span>
								{#if view.planLink && planCount}
									<a class="plan" href={to(view.planLink)} data-testid="plan-these" title="Open the plan page for this selection{view.run?.jobKey && view.run.kind !== 'group' ? ` as ${view.run.jobKey} builds it` : ''}">Plan these {planCount} →</a>
								{:else}
									<span class="plan off" title="Select nodes first">Plan these →</span>
								{/if}
							{/snippet}
						</SelectionBar>
					</div>
					{#if view.statusFilter}
						{@const g = view.nodeGroups.find((x) => x.key === view.statusFilter)}
						<div class="filtered" data-testid="timeline-filter">
							Showing {g?.label.toLowerCase()} only · {g?.nodes.length} of {nodes.length}
							<Button variant="link" size="inline" class="text-[12px]" onclick={() => view.clearFocus()}>Show all</Button>
						</div>
					{/if}
				{/if}
				<div class="flex min-h-0 flex-1">
					<div class="relative flex min-h-0 flex-1 flex-col">
						{#if !nodes.length}
							<div class="empty">{detail.children.length ? 'No nodes yet.' : 'Not started.'}</div>
						{:else if topView === 'timeline' || !hasGraph}
							<RunGantt nodes={view.shownNodes} {origin} now={app.now} active={view.active} selected={view.selected} excluded={view.excluded} tag={detail.children.length ? (n) => memberOf(n.runId) : undefined} skipText={causeOf} onselect={(key) => view.selectNode(key)} onopen={(key) => view.clickNode(key)} />
						{:else}
							<RunLineage {view} />
						{/if}
					</div>
				</div>
			</div>

			{#if hasBottom}
			<!-- A focusable separator is a keyboard-resizable widget (WAI-ARIA window splitter). -->
			<!-- svelte-ignore a11y_no_noninteractive_tabindex, a11y_no_noninteractive_element_interactions -->
			<div
				class="splitter"
				onpointerdown={(e) => {
					dragging = true;
					(e.target as HTMLElement).setPointerCapture(e.pointerId);
				}}
				onpointermove={onDrag}
				onpointerup={() => (dragging = false)}
				onkeydown={onKey}
				role="separator"
				aria-orientation="horizontal"
				aria-label="Resize the timeline and the tabs below"
				aria-valuenow={Math.round(topFrac * 100)}
				tabindex="0"
			><span></span></div>

			<div class="flex min-h-0 flex-col" style:flex={1 - topFrac}>
				<LiveStatements statements={view.runningStatements} onpick={(key) => view.focusNode(key, 'statements')} />
				<RunTabs {view} />
			</div>
			{/if}
		</div>

	{:else if view.detailRes.loading}
		<div class="empty"><Loading lines={4} /></div>
	{:else if view.detailRes.error}
		<div class="empty">Couldn't load this run: {view.detailRes.error}</div>
	{:else}
		<div class="empty" title="Only the newest 1,000 runs per location are kept here">Run not found.</div>
	{/if}
</div>
<NodeDrawer {view} />
</div>

<style>
	.faint {
		color: var(--sb-text-faint);
	}
	.pick {
		padding: 4px 14px;
		border-bottom: 1px solid var(--border-subtle);
	}
	.pick :global(.selection-bar) {
		border-bottom: none;
		background: transparent;
	}
	/* With the drawer open the row is narrower: it wraps whole controls, never inside the view switch. */
	.pick :global(.sb-row) {
		flex-wrap: wrap;
	}
	.pick :global(.segmented-filter) {
		flex: none;
		flex-wrap: nowrap;
	}
	.pick :global(.sb-row > :nth-child(2)) {
		min-width: 260px;
		flex: 1;
	}
	.picks {
		display: inline-flex;
		flex: none;
		align-items: center;
		gap: 10px;
		margin: 0 6px 0 8px;
		font-size: 12.5px;
	}
	.plan {
		flex: none;
	}
	.filtered {
		padding: 4px 24px;
		border-bottom: 1px solid var(--border);
		color: var(--sb-text-faint);
		font-size: 12px;
	}
	.plan {
		padding: 3px 10px;
		border-radius: 2px;
		background: var(--primary);
		color: var(--primary-foreground);
		font-size: 12.5px;
		font-weight: 500;
	}
	.plan.off {
		background: var(--muted);
		color: var(--sb-text-faint);
		cursor: default;
	}
	.splitter {
		position: relative;
		height: 7px;
		flex-shrink: 0;
		border-block: 1px solid var(--border-subtle);
		background: var(--sb-surface-low);
		cursor: row-resize;
	}
	.splitter:focus-visible {
		outline: 2px solid var(--primary);
	}
	.splitter span {
		position: absolute;
		top: 50%;
		left: 50%;
		width: 36px;
		height: 3px;
		transform: translate(-50%, -50%);
		border-radius: 999px;
		background: var(--border-strong);
	}
	.empty {
		padding: 32px;
		color: var(--sb-text-faint);
		font-size: 12.5px;
		text-align: center;
	}
</style>
