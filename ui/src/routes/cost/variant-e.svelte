<script lang="ts">
	import { goto } from '$app/navigation';
	import { page } from '$app/state';
	import ChevronRightIcon from '@lucide/svelte/icons/chevron-right';
	import FolderIcon from '@lucide/svelte/icons/folder';
	import FolderOpenIcon from '@lucide/svelte/icons/folder-open';
	import WorkflowIcon from '@lucide/svelte/icons/workflow';
	import LayoutGridIcon from '@lucide/svelte/icons/layout-grid';
	import DataTable from '$lib/components/list/data-table.svelte';
	import type { TableColumn } from '$lib/components/list/table-columns';
	import SegmentedFilter from '$lib/components/list/segmented-filter.svelte';
	import Panel from '$lib/components/platform/panel.svelte';
	import Button from '$lib/components/ui/button/button.svelte';
	import { MANUAL } from '$lib/api/mock/cost-page';
	import Crumbs from './drill/crumbs.svelte';
	import DrillTable from './drill/drill-table.svelte';
	import LevelStrip from './drill/level-strip.svelte';
	import MiniRank from './drill/mini-rank.svelte';
	import ModelDetail from './drill/model-detail.svelte';
	import TrendBars from './drill/trend-bars.svelte';
	import RunsPanel from './runs-panel.svelte';
	import type { CostPage } from './types';
	import { ROOT, childrenOf, costTitle, crumbsOf, dailyOf, factsIn, levelOf, pathIds, runsIn, scopeFor, scopeFromId, scopeOf, scopeParams, sharePct, sizesIn, total, treeIds, treeRows, usd, type DrillRow, type GroupRoot, type Scope } from './utils';

	// E: the Catalog's two panes. Left, the jobs or folders as a tree with their cost; right, the
	// selected one: its totals and trend, its children ranked, and its top models, sizes and runs.
	let { cost }: { cost: CostPage } = $props();
	const scope = $derived(scopeOf(page.url.searchParams));
	const level = $derived(levelOf(scope));
	const href = (s: Scope) => `?${scopeParams(page.url.searchParams, s)}`;

	// The tree: jobs and their folders, or folders; models only in the table.
	const root = $derived<Scope>({ ...ROOT, by: scope.by });
	let open = $state<ReadonlySet<string>>(new Set());
	$effect(() => {
		const ids = pathIds(scope);
		if (ids.some((id) => !open.has(id))) open = new Set([...open, ...ids]);
	});
	const tree = $derived(treeRows(cost, root, open, { foldersOnly: true, limit: 0 }));
	const selectedId = $derived(pathIds(scope).at(-1) ?? '');
	const totalCredits = $derived(total(cost.facts).credits);
	function toggle(id: string) {
		const next = new Set(open);
		if (!next.delete(id)) next.add(id);
		open = next;
	}
	const treeColumns: TableColumn[] = [
		{ id: 'name', label: '', minWidth: 150 },
		{ id: 'cost', label: '', width: '78px', align: 'right' },
		{ id: 'share', label: '', width: '50px', align: 'right' }
	];
	const INDENT = 16;

	// Resizable from its right edge, like the run page's drawer.
	let width = $state(340);
	let dragging = $state(false);
	let pane: HTMLElement | undefined = $state();
	const clamp = (w: number) => Math.round(Math.min(Math.max(240, w), 560));

	// The selected level.
	// A model's totals cover every job that built it; the Jobs table marks the one on the way down.
	const facts = $derived(factsIn(cost, level === 'model' ? { ...scope, job: null, folder: null } : scope));
	const sum = $derived(total(facts));
	const children = $derived(childrenOf(cost, scope).map((row) => ({ row, depth: 0, parent: sum.credits, id: row.key, open: false })));
	const topModels = $derived(level === 'models' ? [] : childrenOf(cost, { ...scope, by: 'folder', flat: true, model: null }).slice(0, 8));
	const sizes = $derived(sizesIn(cost, facts));
	const runs = $derived(runsIn(cost, facts, MANUAL));
	const crumbs = $derived(crumbsOf(scope));
	const title = $derived(level === 'jobs' ? 'Jobs' : level === 'models' ? 'Models' : scope.folder || scope.by === 'job' ? 'Folders and models' : 'Folders');
	const inJob = $derived(scope.by === 'job' && !!scope.job && !scope.model);
	const setRoot = (by: GroupRoot) => goto(href({ ...ROOT, by }), { noScroll: true, keepFocus: true });
</script>

<div class="explorer">
	<aside class="tree-pane" style:width="{width}px" bind:this={pane}>
		<div class="pane-bar">
			<SegmentedFilter label="Group by" value={scope.by} options={[{ value: 'job', label: 'Job' }, { value: 'folder', label: 'Folder' }]} onselect={(v) => setRoot(v)} />
			<span class="spacer"></span>
			<Button variant="ghost" size="sm" onclick={() => (open = new Set(treeIds(cost, root, 3, true)))}>Expand all</Button>
			<Button variant="ghost" size="sm" disabled={!open.size} onclick={() => (open = new Set())}>Collapse all</Button>
		</div>
		<div class="tree-scroll sb-scroll">
			<a class="all" class:selected={!selectedId} href={href(root)} data-sveltekit-noscroll><LayoutGridIcon size={14} />{scope.by === 'job' ? 'All jobs' : 'All folders'}<span title={costTitle(totalCredits, cost.price)}>{usd(totalCredits, cost.price)}</span></a>
			<DataTable class="compact static tree" embedded columns={treeColumns} rows={tree} key={(t) => t.id} rowHeight={32} label="Cost tree" empty="No cost in this period.">
				{#snippet row(t)}
					{#if !('more' in t)}
						{@const r = t.row}
						{@const Icon = r.kind === 'job' ? WorkflowIcon : t.open ? FolderOpenIcon : FolderIcon}
						<tr class:selected={t.id === selectedId}>
							<td>
								<div class="inner" style:padding-left="{t.depth * INDENT}px">
									{#if r.deeper}
										<Button variant="ghost" size="icon-xs" class="chev {t.open ? 'open' : ''}" aria-expanded={t.open} aria-label="{t.open ? 'Collapse' : 'Expand'} {r.label}" onclick={() => toggle(t.id)}><ChevronRightIcon /></Button>
									{:else}<span class="chev-space"></span>{/if}
									<Icon size={14} strokeWidth={1.75} class="ficon" />
									<a href={href(scopeFromId(scope.by, t.id))} title={r.key} data-sveltekit-noscroll>{r.label}</a>
								</div>
							</td>
							<td title={costTitle(r.credits, cost.price)}>{usd(r.credits, cost.price)}</td>
							<td class="muted">{sharePct(r.credits, t.parent)}</td>
						</tr>
					{/if}
				{/snippet}
			</DataTable>
		</div>
		<!-- svelte-ignore a11y_no_noninteractive_tabindex, a11y_no_noninteractive_element_interactions -->
		<div
			class="resize"
			role="separator"
			aria-orientation="vertical"
			aria-label="Resize the tree"
			aria-valuenow={width}
			tabindex="0"
			onpointerdown={(e) => {
				dragging = true;
				(e.target as HTMLElement).setPointerCapture(e.pointerId);
			}}
			onpointermove={(e) => dragging && pane && (width = clamp(e.clientX - pane.getBoundingClientRect().left))}
			onpointerup={() => (dragging = false)}
			onkeydown={(e) => {
				if (e.key === 'ArrowLeft') width = clamp(width - 24);
				if (e.key === 'ArrowRight') width = clamp(width + 24);
			}}
		></div>
	</aside>

	<section class="main-pane sb-scroll">
		<header>
			<Crumbs {crumbs} />
			<div class="head">
				<div class="facts"><LevelStrip {cost} totals={sum} showModels={level !== 'model'} /></div>
				<div class="trend"><TrendBars {cost} values={dailyOf(cost, facts)} height={48} /></div>
			</div>
		</header>
		{#if level === 'model'}
			<ModelDetail {cost} {scope} />
		{:else}
			<div class="body">
				<div class="main-col">
					<Panel {title} note="{children.length.toLocaleString('en')} rows">
						{#snippet actions()}
							{#if inJob}<SegmentedFilter label="Models" value={scope.flat ? 'flat' : 'tree'} options={[{ value: 'tree', label: 'By folder' }, { value: 'flat', label: 'All models' }]} onselect={(v) => goto(href({ ...scope, flat: v === 'flat' }), { noScroll: true, keepFocus: true })} />{/if}
						{/snippet}
						<DrillTable {cost} rows={children} name={level === 'jobs' ? 'Job' : level === 'models' ? 'Model' : 'Name'} hrefOf={(r: DrillRow) => href(scopeFor(scope, r))} embedded />
					</Panel>
				</div>
				<div class="side-col">
					{#if topModels.length}
						<Panel title="Most expensive models">
							<MiniRank {cost} rows={topModels} total={sum.credits} name="Model" hrefOf={(r) => href({ ...scope, model: r.key })} />
						</Panel>
					{/if}
					<Panel title="Warehouse sizes">
						<MiniRank {cost} rows={sizes} total={sum.credits} name="Size" count={{ label: 'Runs', value: (r) => r.runs }} />
					</Panel>
					<RunsPanel runs={runs} price={cost.price} limit={6} />
				</div>
			</div>
		{/if}
	</section>
</div>

<style>
	.explorer { display: flex; min-height: 0; flex: 1; }
	.tree-pane { position: relative; display: flex; min-height: 0; flex: none; flex-direction: column; border-right: 1px solid var(--border); background: var(--sb-surface-low); }
	.pane-bar { display: flex; align-items: center; gap: 4px; padding: 8px 10px 8px 14px; border-bottom: 1px solid var(--border); }
	.spacer { flex: 1; }
	.tree-scroll { min-height: 0; flex: 1; overflow: auto; }
	.all { display: flex; align-items: center; gap: 8px; padding: 9px 20px 9px 20px; border-bottom: 1px solid var(--border); color: var(--muted-foreground); font-size: 12.5px; font-weight: 500; }
	.all span { margin-left: auto; color: var(--foreground); font-variant-numeric: tabular-nums; }
	.all:hover, .all.selected { background: var(--sb-hover); color: var(--foreground); }
	.all.selected { box-shadow: inset 2px 0 0 var(--primary); }
	:global(.tree thead) { display: none; }
	:global(.tree td) { height: 32px; padding-block: 0 !important; }
	:global(.tree td:first-child) { padding-left: 14px !important; }
	:global(.tree td:last-child) { padding-right: 14px !important; }
	:global(.tree td:not(:first-child)) { padding-left: 4px !important; }
	.inner { display: flex; min-width: 0; align-items: center; gap: 5px; }
	.inner a { overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
	.inner a:hover { color: var(--primary); text-decoration: underline; }
	.inner :global(.chev svg) { transition: transform 0.12s; }
	.inner :global(.chev.open svg) { transform: rotate(90deg); }
	.chev-space { width: 20px; flex-shrink: 0; }
	.inner :global(.ficon) { flex-shrink: 0; color: var(--sb-text-faint); }
	tr.selected { background: color-mix(in srgb, var(--primary) 12%, transparent); }
	tr.selected td:first-child { box-shadow: inset 2px 0 0 var(--primary); }
	.resize { position: absolute; top: 0; right: -3px; bottom: 0; z-index: 2; width: 6px; cursor: col-resize; }
	.resize:hover, .resize:focus-visible { background: color-mix(in srgb, var(--primary) 35%, transparent); }
	.main-pane { min-width: 0; min-height: 0; flex: 1; overflow: auto; padding: 16px 24px 40px; container-type: inline-size; }
	header { margin-bottom: 20px; }
	.head { display: grid; grid-template-columns: minmax(0, auto) minmax(260px, 1fr); align-items: end; gap: 12px 36px; margin-top: 12px; }
	.body { display: grid; grid-template-columns: minmax(0, 1fr) 530px; gap: 0 28px; }
	.main-col, .side-col { min-width: 0; }
	@container (max-width: 1150px) {
		.head { grid-template-columns: minmax(0, 1fr); }
		.body { grid-template-columns: minmax(0, 1fr); }
		.side-col { display: grid; grid-template-columns: repeat(auto-fill, minmax(min(100%, 420px), 1fr)); gap: 0 28px; }
	}
	@media (prefers-reduced-motion: reduce) { .inner :global(.chev svg) { transition: none; } }
</style>
