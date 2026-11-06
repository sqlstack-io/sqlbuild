<script lang="ts">
	import AppTopbar from '$lib/components/app-topbar.svelte';
	import { app } from '$lib/state/app.svelte';
	import { runStatusTone } from '$lib/status';
	import { shortRunId } from '$lib/operate/labels';
	import FailuresPanel from './failures-panel.svelte';
	import LiveStatements from './live-statements.svelte';
	import NodePanel from './node-panel.svelte';
	import RelatedRuns from './related-runs.svelte';
	import RunBanners from './run-banners.svelte';
	import RunGantt from './run-gantt.svelte';
	import RunGraph from './run-graph.svelte';
	import RunHeader from './run-header.svelte';
	import RunTabs from './run-tabs.svelte';
	import { createRunDetailState } from './state.svelte';

	let { runId }: { runId: string } = $props();

	// svelte-ignore state_referenced_locally
	const view = createRunDetailState(runId);

	const detail = $derived(view.detail);
	const nodes = $derived(view.nodes);
	let topView = $state<'timeline' | 'graph'>('timeline');

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

<div class="flex min-h-0 flex-1 flex-col">
	<RunHeader {view} {runId} />

	{#if detail}
		<RunBanners banners={view.banners} />
		<FailuresPanel {view} />
		<RelatedRuns {view} />

		<div bind:this={splitEl} class="flex min-h-0 flex-1 flex-col" class:select-none={dragging}>
			<div class="flex min-h-0 flex-col" style:flex={topFrac}>
				<div class="exec-head">
					<span class="label">Nodes over time</span>
					{#if detail.children.length}<span class="faint">{detail.children.length} job runs · nodes from all of them</span>{/if}
					<div class="seg" role="tablist">
						<button role="tab" aria-selected={topView === 'timeline'} class:on={topView === 'timeline'} onclick={() => (topView = 'timeline')}>Timeline</button>
						<button role="tab" aria-selected={topView === 'graph'} class:on={topView === 'graph'} onclick={() => (topView = 'graph')}>Graph</button>
					</div>
				</div>
				<div class="flex min-h-0 flex-1">
					<div class="relative flex min-h-0 flex-1 flex-col">
						{#if !nodes.length}
							<div class="empty">{detail.children.length ? 'The job runs in this group have no nodes yet.' : 'No nodes yet: the run hasn’t started.'}</div>
						{:else if topView === 'timeline'}
							<RunGantt {nodes} {origin} now={app.now} selected={view.selected} {groups} onselect={(key, add) => view.selectNode(key, add)} />
						{:else}
							<RunGraph {nodes} edges={detail.edges} ownRunId={detail.run.id} />
						{/if}
					</div>
					<NodePanel {view} />
				</div>
			</div>

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
		</div>
	{:else if view.detailRes.loading}
		<div class="empty">Loading…</div>
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
		display: flex;
		overflow: hidden;
		margin-left: auto;
		border: 1px solid var(--border);
		border-radius: 2px;
	}
	.seg button {
		padding: 3px 12px;
		color: var(--muted-foreground);
		font-size: 11px;
		font-weight: 500;
	}
	.seg button.on {
		background: var(--sidebar-accent);
		color: var(--foreground);
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
