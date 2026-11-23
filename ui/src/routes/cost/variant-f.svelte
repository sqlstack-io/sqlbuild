<script lang="ts">
	import { goto } from '$app/navigation';
	import { page } from '$app/state';
	import XIcon from '@lucide/svelte/icons/x';
	import SegmentedFilter from '$lib/components/list/segmented-filter.svelte';
	import Button from '$lib/components/ui/button/button.svelte';
	import Crumbs from './drill/crumbs.svelte';
	import DrillTable from './drill/drill-table.svelte';
	import LevelStrip from './drill/level-strip.svelte';
	import ModelDetail from './drill/model-detail.svelte';
	import TrendBars from './drill/trend-bars.svelte';
	import type { CostPage } from './types';
	import { ROOT, childrenOf, rowId, costTitle, crumbsOf, dailyOf, factsIn, levelOf, scopeFor, scopeFromId, scopeOf, scopeParams, total, treeIds, treeRows, usd, type DrillRow, type GroupRoot, type Scope } from './utils';

	// F: one wide table. The level's totals and trend on top, its children ranked below; a chevron
	// opens a row in place, a name opens it as the level, and a model opens in a drawer.
	let { cost }: { cost: CostPage } = $props();
	const scope = $derived(scopeOf(page.url.searchParams));
	/** The level the table lists: the scope without the model a drawer shows. */
	const level = $derived<Scope>({ ...scope, model: null });
	const href = (s: Scope) => `?${scopeParams(page.url.searchParams, s)}`;
	const nav = (s: Scope) => goto(href(s), { noScroll: true, keepFocus: true });

	// Rows opened in place belong to the level; opening another level starts closed.
	let opened = $state<{ at: string; ids: ReadonlySet<string> }>({ at: '', ids: new Set() });
	const levelKey = $derived(JSON.stringify(level));
	/** Until rows are opened by hand: a level of three or fewer rows opens them, and a model in the
	 *  drawer opens the folders down to it. */
	const defaultOpen = $derived.by(() => {
		const ids = new Set<string>();
		const top = childrenOf(cost, level).filter((r) => r.deeper);
		if (top.length <= 3) for (const r of top) ids.add(`/${rowId(r)}`);
		const folder = scope.model ? cost.facts.find((f) => f.model === scope.model)?.folder : null;
		if (folder && (!level.folder || folder.startsWith(`${level.folder}/`))) {
			const parts = folder.split('/');
			let path = '';
			for (let i = level.folder ? level.folder.split('/').length : 0; i < parts.length; i++) ids.add((path += `/folder:${parts.slice(0, i + 1).join('/')}`));
		}
		return ids;
	});
	const open = $derived(opened.at === levelKey ? opened.ids : defaultOpen);
	function toggle(id: string) {
		const next = new Set(open);
		if (!next.delete(id)) next.add(id);
		opened = { at: levelKey, ids: next };
	}
	// A flat list shows every model; the table draws only the rows in view.
	const rows = $derived(treeRows(cost, level, open, { limit: level.flat ? Infinity : 25 }));
	const facts = $derived(factsIn(cost, level));
	const sum = $derived(total(facts));
	/** The scope of a row opened in place: the level, narrowed by the jobs and folders above the row. */
	const rowScope = (id: string): Scope => {
		const s = scopeFromId(level.by, id);
		return { ...level, job: s.job ?? level.job, folder: s.folder ?? level.folder, model: s.model };
	};
	const hrefOf = (r: DrillRow, id: string) => (r.kind === 'model' ? href({ ...level, model: r.key }) : href(scopeFor(rowScope(id), r)));
	const trend = (_r: DrillRow, id: string) => dailyOf(cost, factsIn(cost, rowScope(id)));
	const inJob = $derived(level.by === 'job' && !!level.job);
	const levelName = $derived(levelOf(level) === 'jobs' ? 'Job' : levelOf(level) === 'models' ? 'Model' : 'Name');
	const modelFacts = $derived(scope.model ? total(factsIn(cost, { ...scope, job: null, folder: null })) : null);

	// The drawer, resizable from its left edge like the run page's.
	let width = $state(560);
	let dragging = $state(false);
	const clamp = (w: number) => Math.round(Math.min(Math.max(420, w), Math.max(440, window.innerWidth * 0.6)));
	function onkey(e: KeyboardEvent) {
		if (e.key === 'Escape' && scope.model) void nav(level);
	}
</script>

<svelte:window onkeydown={onkey} />

<header class="level-head">
	<div class="top">
		<Crumbs crumbs={crumbsOf(level)} />
		<div class="controls">
			{#if inJob}<SegmentedFilter label="Models" value={level.flat ? 'flat' : 'tree'} options={[{ value: 'tree', label: 'By folder' }, { value: 'flat', label: 'All models' }]} onselect={(v) => nav({ ...level, flat: v === 'flat' })} />{/if}
			<SegmentedFilter label="Group by" value={level.by} options={[{ value: 'job', label: 'Job' }, { value: 'folder', label: 'Folder' }]} onselect={(v) => nav({ ...ROOT, by: v as GroupRoot })} />
		</div>
	</div>
	<div class="head">
		<div class="facts"><LevelStrip {cost} totals={sum} /></div>
		<div class="trend"><TrendBars {cost} values={dailyOf(cost, facts)} height={48} /></div>
	</div>
</header>
<div class="tree-bar">
	<span>{rows.filter((r) => !('more' in r) && r.depth === 0).length.toLocaleString('en')} rows</span>
	<span class="spacer"></span>
	<Button variant="ghost" size="sm" onclick={() => (opened = { at: levelKey, ids: new Set(treeIds(cost, level, 2)) })}>Expand all</Button>
	<Button variant="ghost" size="sm" disabled={!open.size} onclick={() => (opened = { at: levelKey, ids: new Set() })}>Collapse all</Button>
</div>
<div class="table-area">
	<DrillTable {cost} {rows} name={levelName} {hrefOf} moreHref={(s) => href({ ...s, flat: true })} tree ontoggle={toggle} selected={scope.model} {trend} />
	{#if scope.model && modelFacts}
		<aside class="drawer" style:width="{width}px" aria-label={scope.model}>
			<!-- svelte-ignore a11y_no_noninteractive_tabindex, a11y_no_noninteractive_element_interactions -->
			<div
				class="resize"
				role="separator"
				aria-orientation="vertical"
				aria-label="Resize the drawer"
				aria-valuenow={width}
				tabindex="0"
				onpointerdown={(e) => {
					dragging = true;
					(e.target as HTMLElement).setPointerCapture(e.pointerId);
				}}
				onpointermove={(e) => dragging && (width = clamp(window.innerWidth - e.clientX))}
				onpointerup={() => (dragging = false)}
				onkeydown={(e) => {
					if (e.key === 'ArrowLeft') width = clamp(width + 24);
					if (e.key === 'ArrowRight') width = clamp(width - 24);
				}}
			></div>
			<div class="drawer-head">
				<span class="name" title={scope.model}>{scope.model}</span>
				<strong title={costTitle(modelFacts.credits, cost.price)}>{usd(modelFacts.credits, cost.price)}</strong>
				<Button variant="ghost" size="icon-sm" onclick={() => nav(level)} aria-label="Close" title="Close (Esc)"><XIcon /></Button>
			</div>
			<div class="drawer-body sb-scroll"><ModelDetail {cost} {scope} narrow /></div>
		</aside>
	{/if}
</div>

<style>
	.level-head { padding: 14px 24px 16px; border-bottom: 1px solid var(--border); }
	.top { display: flex; flex-wrap: wrap; align-items: center; justify-content: space-between; gap: 8px 16px; }
	.controls { display: flex; align-items: center; gap: 10px; }
	.head { display: grid; grid-template-columns: minmax(0, auto) minmax(260px, 1fr); align-items: end; gap: 12px 40px; margin-top: 12px; }
	@media (max-width: 1500px) { .head { grid-template-columns: minmax(0, 1fr); } }
	.tree-bar { display: flex; align-items: center; gap: 6px 14px; padding: 6px 20px; border-bottom: 1px solid var(--border); color: var(--muted-foreground); font-size: 12.5px; }
	.spacer { flex: 1; }
	.table-area { position: relative; display: flex; min-height: 0; flex: 1; flex-direction: column; }
	.drawer { position: absolute; top: 0; right: 0; bottom: 0; z-index: 10; display: flex; flex-direction: column; border-left: 1px solid var(--border); background: var(--card); box-shadow: -12px 0 32px rgba(0, 0, 0, 0.25); }
	.resize { position: absolute; top: 0; bottom: 0; left: -3px; z-index: 2; width: 6px; cursor: col-resize; }
	.resize:hover, .resize:focus-visible { background: color-mix(in srgb, var(--primary) 35%, transparent); }
	.drawer-head { display: flex; align-items: center; gap: 12px; padding: 12px 12px 12px 18px; border-bottom: 1px solid var(--border); }
	.name { min-width: 0; flex: 1; overflow: hidden; font-family: var(--font-mono); font-size: 13px; font-weight: 600; text-overflow: ellipsis; white-space: nowrap; }
	.drawer-head strong { font-variant-numeric: tabular-nums; }
	.drawer-body { min-height: 0; flex: 1; overflow: auto; padding: 14px 18px 24px; }
</style>
