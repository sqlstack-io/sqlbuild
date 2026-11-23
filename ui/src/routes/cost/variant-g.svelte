<script lang="ts">
	import { goto } from '$app/navigation';
	import { page } from '$app/state';
	import XIcon from '@lucide/svelte/icons/x';
	import SegmentedFilter from '$lib/components/list/segmented-filter.svelte';
	import SelectionBar from '$lib/components/selection/selection-bar.svelte';
	import Button from '$lib/components/ui/button/button.svelte';
	import { costNames } from '$lib/api/mock/cost-selection';
	import { writeSelectionParams } from '$lib/selection/command';
	import { createCostSelection } from './cost-selection.svelte';
	import DrillTable from './drill/drill-table.svelte';
	import LevelStrip from './drill/level-strip.svelte';
	import ModelDetail from './drill/model-detail.svelte';
	import TrendArea from './drill/trend-area.svelte';
	import type { CostPage } from './types';
	import { childrenOf, costTitle, dailyOf, dayLabel, factsIn, levelOf, placeCrumbs, placeOf, rowId, scopeFromId, selectorOf, total, treeIds, treeRows, union, usd, type DrillRow, type GroupRoot, type Place, type Scope } from './utils';

	// G: F driven by the selection bar. The selection is the one source of what the page shows and
	// lives in the URL; drilling into a job, folder or model adds a part to it, breadcrumbs take
	// parts away, and "Add" unions a row in. What the tree can't break down shows by folder.
	let { cost }: { cost: CostPage } = $props();
	const selection = createCostSelection();
	const names = costNames();
	const place = $derived(placeOf(selection.select, selection.exclude));
	/** The models of the free-form part of the selection (and any except), or null when the tree covers it. */
	const baseModels = $derived.by(() => {
		if (!place.base.length && !selection.exclude.trim()) return null;
		const keys = selection.keys;
		return keys ? new Set([...keys].map((k) => k.slice(k.indexOf(':') + 1))) : new Set<string>();
	});
	const byParam = $derived<GroupRoot>(page.url.searchParams.get('by') === 'folder' ? 'folder' : 'job');
	const flat = $derived(page.url.searchParams.get('flat') === '1');
	/** The level the table lists; a model in the selection opens in the drawer beside it. */
	const level = $derived<Scope>({
		by: place.job ? 'job' : place.base.length || place.folder ? 'folder' : byParam,
		job: place.job,
		folder: place.folder,
		model: null,
		flat: flat && !!place.job,
		models: baseModels
	});
	const href = (select: string, extra: Record<string, string | null> = {}) => {
		const params = writeSelectionParams(page.url.searchParams, { select, exclude: selection.exclude });
		for (const [k, v] of Object.entries(extra)) {
			if (v === null) params.delete(k);
			else params.set(k, v);
		}
		return `?${params}`;
	};
	const nav = (url: string) => goto(url, { noScroll: true, keepFocus: true });
	const placeWith = (s: Scope): Place => ({ base: place.base, job: s.job, folder: s.folder, model: s.model });

	let opened = $state<{ at: string; ids: ReadonlySet<string> }>({ at: '', ids: new Set() });
	const levelKey = $derived(`${selectorOf({ ...place, model: null })}|${selection.exclude}|${level.by}|${level.flat}`);
	const defaultOpen = $derived.by(() => {
		const ids = new Set<string>();
		const top = childrenOf(cost, level).filter((r) => r.deeper);
		if (top.length <= 3) for (const r of top) ids.add(`/${rowId(r)}`);
		const folder = place.model ? cost.facts.find((f) => f.model === place.model)?.folder : null;
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
	/** A selection `sqb` refuses shows nothing until it's fixed; the bar says what's wrong. */
	const refused = $derived(!!selection.problem);
	const rows = $derived(refused ? [] : treeRows(cost, level, open, { limit: level.flat ? Infinity : 25 }));
	const facts = $derived(refused ? [] : factsIn(cost, level));
	const sum = $derived(total(facts));
	const rowScope = (id: string): Scope => {
		const s = scopeFromId(level.by, id);
		return { ...level, job: s.job ?? level.job, folder: s.folder ?? level.folder, model: s.model };
	};
	/** The selection a row stands for: the level's, narrowed to the row. */
	const rowSelect = (r: DrillRow, id: string) => {
		const s = rowScope(id);
		return selectorOf(placeWith(r.kind === 'job' ? { ...s, job: r.key, folder: null } : r.kind === 'folder' ? { ...s, folder: r.key } : { ...s, model: r.key }));
	};
	const hrefOf = (r: DrillRow, id: string) => href(rowSelect(r, id));
	/** "Add" unions the row's own selector in: a job, a whole folder or a model, not narrowed to the level. */
	const add = (r: DrillRow) => {
		const one = r.kind === 'job' ? `job:${r.key}` : r.kind === 'folder' ? selectorOf({ base: [], job: null, folder: r.key, model: null }) : r.key;
		void nav(href(union(selection.select, one)));
	};
	const trend = (_r: DrillRow, id: string) => dailyOf(cost, factsIn(cost, rowScope(id)));
	const dayTitle = (i: number, v: number) => `${dayLabel(cost.daily[i]?.date ?? '', true)} · ${usd(v, cost.price)}`;
	const crumbs = $derived(refused ? placeCrumbs({ base: [], job: null, folder: null, model: null }) : placeCrumbs({ ...place, model: null }));
	const inJob = $derived(level.by === 'job' && !!level.job);
	const levelName = $derived(levelOf(level) === 'jobs' ? 'Job' : levelOf(level) === 'models' ? 'Model' : 'Name');
	const modelScope = $derived<Scope | null>(place.model ? { by: 'folder', job: null, folder: null, model: place.model, flat: false } : null);
	const modelTotal = $derived(modelScope ? total(factsIn(cost, modelScope)) : null);
	const closeDrawer = () => nav(href(selectorOf({ ...place, model: null })));

	let width = $state(560);
	let dragging = $state(false);
	const clamp = (w: number) => Math.round(Math.min(Math.max(420, w), Math.max(440, window.innerWidth * 0.6)));
	function onkey(e: KeyboardEvent) {
		if (e.key === 'Escape' && place.model && !(e.target as Element)?.closest?.('input, textarea')) void closeDrawer();
	}
</script>

<svelte:window onkeydown={onkey} />

<SelectionBar {selection} {names} total={names.length} build={false}>
	{#snippet view()}
		{#if !place.job && !place.folder && !place.base.length}<SegmentedFilter label="Group by" value={byParam} options={[{ value: 'job', label: 'Job' }, { value: 'folder', label: 'Folder' }]} onselect={(v) => nav(href(selection.select, { by: v === 'folder' ? 'folder' : null }))} />{/if}
		{#if inJob}<SegmentedFilter label="Models" value={flat ? 'flat' : 'tree'} options={[{ value: 'tree', label: 'By folder' }, { value: 'flat', label: 'All models' }]} onselect={(v) => nav(href(selection.select, { flat: v === 'flat' ? '1' : null }))} />{/if}
	{/snippet}
</SelectionBar>
<header class="level-head">
	<nav class="crumbs" aria-label="Breadcrumb">
		{#each crumbs as c, i (i)}
			{#if i}<span class="sep">/</span>{/if}
			{#if i < crumbs.length - 1}<a href={href(c.select)} class:code={c.code} data-sveltekit-noscroll>{c.label}</a>{:else}<span class="here" class:code={c.code} title={c.label}>{c.label}</span>{/if}
		{/each}
	</nav>
	{#if !refused}<div class="head">
		<div class="facts"><LevelStrip {cost} totals={sum} /></div>
		<div class="trend"><TrendArea {cost} values={dailyOf(cost, facts)} height={52} /></div>
	</div>{/if}
</header>
<div class="tree-bar">
	<span>{rows.filter((r) => !('more' in r) && r.depth === 0).length.toLocaleString('en')} rows</span>
	<span class="spacer"></span>
	<Button variant="ghost" size="sm" onclick={() => (opened = { at: levelKey, ids: new Set(treeIds(cost, level, 2)) })}>Expand all</Button>
	<Button variant="ghost" size="sm" disabled={!open.size} onclick={() => (opened = { at: levelKey, ids: new Set() })}>Collapse all</Button>
</div>
<div class="table-area">
	<DrillTable {cost} {rows} name={levelName} {hrefOf} moreHref={() => href(selection.select, { flat: '1' })} tree ontoggle={toggle} selected={place.model} {trend} rowToggle peakBars smooth fillTrend onadd={add} {dayTitle} empty={refused ? 'No nodes selected.' : 'No cost in this period.'} />
	{#if modelScope && modelTotal && !refused}
		<aside class="drawer" style:width="{width}px" aria-label={place.model}>
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
				<span class="name" title={place.model}>{place.model}</span>
				<strong title={costTitle(modelTotal.credits, cost.price)}>{usd(modelTotal.credits, cost.price)}</strong>
				<Button variant="ghost" size="icon-sm" onclick={closeDrawer} aria-label="Close" title="Close (Esc)"><XIcon /></Button>
			</div>
			<div class="drawer-body sb-scroll"><ModelDetail {cost} scope={{ ...modelScope, job: place.job }} narrow smooth jobHref={(job) => href(selectorOf({ base: place.base, job, folder: cost.facts.find((f) => f.model === place.model)?.folder ?? null, model: place.model }))} /></div>
		</aside>
	{/if}
</div>

<style>
	.level-head { padding: 12px 24px 14px; border-bottom: 1px solid var(--border); }
	.crumbs { display: flex; min-width: 0; flex-wrap: wrap; align-items: baseline; gap: 4px 6px; color: var(--muted-foreground); font-size: 12.5px; }
	.crumbs a:hover { color: var(--foreground); text-decoration: underline; }
	.crumbs .code { font-family: var(--font-mono); font-size: 12px; }
	.sep { color: var(--sb-text-faint); }
	.here { color: var(--foreground); font-weight: 600; }
	.head { display: grid; grid-template-columns: minmax(0, auto) minmax(260px, 1fr); align-items: end; gap: 12px 40px; margin-top: 10px; }
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
