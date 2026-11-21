<script lang="ts">
	import SegmentedFilter from '$lib/components/list/segmented-filter.svelte';
	import Button from '$lib/components/ui/button/button.svelte';
	import Loading from '$lib/components/loading.svelte';
	import AppTopbar from '$lib/components/app-topbar.svelte';
	import { app } from '$lib/state/app.svelte';
	import { runStatusTone } from '$lib/status';
	import { shortRunId } from '$lib/operate/labels';
	import FailuresPanel from './failures-panel.svelte';
	import IgnoredAuditsPanel from './ignored-audits-panel.svelte';
	import LiveStatements from './live-statements.svelte';
	import NodePanel from './node-panel.svelte';
	import RelatedRuns from './related-runs.svelte';
	import RunBanners from './run-banners.svelte';
	import RunGantt from './run-gantt.svelte';
	import RunGraph from './run-graph.svelte';
	import RunHeader from './run-header.svelte';
	import RunTabs from './run-tabs.svelte';
	import { createRunDetailState } from './state.svelte';
	import { bottomTabs } from './utils';

	let { runId }: { runId: string } = $props();

	// svelte-ignore state_referenced_locally
	const view = createRunDetailState(runId);

	const detail = $derived(view.detail);
	const nodes = $derived(view.nodes);
	let topView = $state<'timeline' | 'graph'>('timeline');
	/** The graph needs dependency edges and the bottom pane needs execution reads; servers without them show neither. */
	const hasGraph = $derived((detail?.edges.length ?? 0) > 0);
	const hasBottom = $derived(bottomTabs(app.client, (c) => app.can(c)).length > 0);

	/** The axis starts at the first node start, not at queue time. */
	const origin = $derived.by(() => {
		const starts = nodes.map((n) => (n.startedAt ? Date.parse(n.startedAt) : Infinity));
		const first = Math.min(...starts);
		if (Number.isFinite(first)) return first;
		return view.run?.startedAt ? Date.parse(view.run.startedAt) : app.now;
	});
	const groups = $derived(
		(detail?.children ?? []).map((c) => ({ runId: c.id, label: `${c.jobKey} · ${shortRunId(c.id)}`, tone: runStatusTone[c.status], href: `/runs/${c.id}` }))
	);

	// ---- resizable split between the timeline and the bottom tabs ----
	let topFrac = $state(0.5);
	let dragging = $state(false);
	let splitEl = $state<HTMLDivElement>();
	// The timeline starts as tall as its rows, so a short run leaves the room to the log below.
	let sized = false;
	$effect(() => {
		if (sized || !splitEl || !nodes.length || !hasBottom || !splitEl.clientHeight) return;
		sized = true;
		const rows = nodes.length + (groups.length > 1 ? groups.length : 0);
		topFrac = Math.min(0.6, Math.max(0.22, (125 + rows * 32) / splitEl.clientHeight));
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

<AppTopbar title="Runs" subject={`Run ${shortRunId(runId)} · ${view.run ? (view.run.jobKey ?? 'ad hoc') : ''}`.replace(/ · $/, '')} />

<div class="flex min-h-0 flex-1 flex-col">
	<RunHeader {view} {runId} />

	{#if detail}
		<RunBanners banners={view.banners} />
		<FailuresPanel {view} />
		<IgnoredAuditsPanel {view} />
		<RelatedRuns {view} />

		<div bind:this={splitEl} class="flex min-h-0 flex-1 flex-col" class:select-none={dragging}>
			<div class="flex min-h-0 flex-col" style:flex={hasBottom ? topFrac : 1}>
				<div class="exec-head">
					<span class="label">Nodes over time</span>
					{#if view.statusFilter}
						{@const g = view.nodeGroups.find((x) => x.key === view.statusFilter)}
						<span class="faint" data-testid="timeline-filter">{g?.label} only · {g?.nodes.length} of {nodes.length}</span>
						<Button variant="link" size="inline" class="text-[12px]" onclick={() => view.filterStatus(null)}>Show all</Button>
					{/if}
					{#if detail.children.length}<span class="faint">{detail.children.length} job runs · nodes from all of them</span>{/if}
					{#if hasGraph}
					<div class="seg"><SegmentedFilter label="View" bind:value={topView} options={[{ value: 'timeline', label: 'Timeline' }, { value: 'graph', label: 'Graph' }]} /></div>
					{/if}
				</div>
				<div class="flex min-h-0 flex-1">
					<div class="relative flex min-h-0 flex-1 flex-col">
						{#if !nodes.length}
							<div class="empty">{detail.children.length ? 'No nodes yet.' : 'Not started.'}</div>
						{:else if topView === 'timeline' || !hasGraph}
							<RunGantt nodes={view.shownNodes} {origin} now={app.now} active={view.active} selected={view.selected} {groups} onselect={(key, add) => view.selectNode(key, add)} />
						{:else}
							<RunGraph {nodes} edges={detail.edges} ownRunId={detail.run.id} />
						{/if}
					</div>
					<NodePanel {view} />
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

<style>
	.exec-head {
		display: flex;
		align-items: center;
		gap: 12px;
		padding: 6px 24px;
		border-bottom: 1px solid var(--border);
		font-size: 11.5px;
	}
	.exec-head .label {
		color: var(--sb-text-faint);
		font-size: 12px;
		font-weight: 600;
	}
	.faint {
		color: var(--sb-text-faint);
	}
	.seg {
		margin-left: auto;
	}
	.splitter {
		position: relative;
		height: 7px;
		flex-shrink: 0;
		border-block: 1px solid var(--border);
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
