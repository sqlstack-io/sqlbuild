<script lang="ts">
	import { goto, pushState } from '$app/navigation';
	import { page } from '$app/state';
	import { untrack } from 'svelte';
	import XIcon from '@lucide/svelte/icons/x';
	import type { CostSelection, CostSeriesGroup } from '$lib/api';
	import SegmentedFilter from '$lib/components/list/segmented-filter.svelte';
	import SelectionBar from '$lib/components/selection/selection-bar.svelte';
	import Button from '$lib/components/ui/button/button.svelte';
	import { shortName } from '$lib/build-area/labels';
	import { folderOf } from '$lib/lineage/catalog-graph';
	import { readSelectionParams, writeSelectionParams } from '$lib/selection/command';
	import { app } from '$lib/state/app.svelte';
	import { useCatalogNodes } from '$lib/state/catalog.svelte';
	import type { SelectionState } from '$lib/state/selection.svelte';
	import DrillTable from './drill/drill-table.svelte';
	import LevelStrip from './drill/level-strip.svelte';
	import ModelDetail from './drill/model-detail.svelte';
	import TrendArea from './drill/trend-area.svelte';
	import { createSeries } from './series.svelte';
	import { costTitle, count, dayLabel, money } from './utils/format';
	import { pathOfFolder, placeCrumbs, placeOf, selectorOf, union, type Place } from './utils/selector';
	import { MANUAL, folderIds, indexCost, nameOf, type DrillRow, type Level } from './utils/tree';

	// The Cost page: the selection (in the URL) decides what it shows. Drilling into a job, folder or
	// model adds a part to the selection, breadcrumbs take parts away, and "Add" unions a row in;
	// what the tree can't break down (tags, `+`, unions, Exclude) shows by folder. The server resolves
	// the free-form part; the job, folder and model are looked up in the read's index, so drilling
	// and opening rows don't read again.
	let { read, selection, days }: { read: CostSelection; selection: SelectionState; days: number } = $props();

	const catalog = useCatalogNodes();
	const names = $derived((catalog.data ?? []).map((n) => ({ value: shortName(n.key), hint: folderOf(n) })));
	const place = $derived(placeOf(selection.select, selection.exclude));
	const index = $derived(indexCost(read));
	/** Node keys by model name, for the model in the selection. */
	const keyByName = $derived(new Map(read.facts.map((f) => [nameOf(f.nodeKey), f.nodeKey])));
	const jobDaily = $derived(new Map(read.jobDaily.map((s) => [s.key ?? MANUAL, s.credits])));
	const folderDaily = $derived(new Map(read.folderDaily.map((s) => [s.key ?? '', s.credits])));

	const byParam = $derived<'job' | 'folder'>(page.url.searchParams.get('by') === 'folder' ? 'folder' : 'job');
	const flat = $derived(page.url.searchParams.get('flat') === '1');
	/** The level the table lists; a model in the selection opens in the drawer beside it. */
	const level = $derived<Level>({
		by: place.job ? 'job' : place.base.length || place.folder ? 'folder' : byParam,
		job: place.job,
		folder: place.folder,
		flat: flat && !!place.job
	});
	const levelJob = $derived(level.by === 'job' ? level.job : null);

	const href = (select: string, extra: Record<string, string | null> = {}) => {
		const params = writeSelectionParams(page.url.searchParams, { select, exclude: selection.exclude });
		for (const [k, v] of Object.entries(extra)) {
			if (v === null) params.delete(k);
			else params.set(k, v);
		}
		return `?${params}`;
	};
	const nav = (url: string) => goto(url, { noScroll: true, keepFocus: true });
	/**
	 * A drill or breadcrumb link taken in place: the selection changes in the click, and the URL
	 * follows as a new history entry, so Back goes up again. A link opened in a new tab stays a link.
	 */
	function follow(event: MouseEvent, url: string) {
		if (event.button !== 0 || event.metaKey || event.ctrlKey || event.shiftKey || event.altKey) return;
		event.preventDefault();
		const next = new URL(url, page.url);
		pushState(next, {});
		const linked = readSelectionParams(next.searchParams);
		selection.select = linked.select;
		selection.exclude = linked.exclude;
	}

	let opened = $state.raw<{ at: string; ids: ReadonlySet<string> }>({ at: '', ids: new Set() });
	const levelKey = $derived(`${selectorOf({ ...place, model: null })}|${selection.exclude}|${level.by}|${level.flat}|${days}`);
	const modelKey = $derived(place.model ? (keyByName.get(place.model) ?? null) : null);
	const defaultOpen = $derived.by(() => {
		const ids = new Set<string>();
		const top = index.children(level).filter((r) => r.deeper);
		if (top.length <= 3) for (const r of top) ids.add(`/${r.kind}:${r.key}`);
		const folder = modelKey ? index.folderOf(modelKey) : undefined;
		if (folder && !level.flat) for (const id of folderIds(folder, level.folder)) ids.add(id);
		return ids;
	});
	const open = $derived(opened.at === levelKey ? opened.ids : defaultOpen);
	function toggle(id: string) {
		const next = new Set(open);
		if (!next.delete(id)) next.add(id);
		opened = { at: levelKey, ids: next };
	}

	/** A selection `sqb` refuses shows nothing until it's fixed; the bar says what's wrong. */
	const refused = $derived(!!selection.problem || !!read.error);
	const rows = $derived(refused ? [] : index.rows(level, open, level.flat ? Infinity : 25));
	const totals = $derived(index.totals(levelJob, level.folder));
	const topRows = $derived(rows.reduce((n, r) => n + ('row' in r && r.depth === 0 ? 1 : 0), 0));

	/** The selection a row stands for, or null for builds by hand, which have no job to select. */
	function rowPlace(r: DrillRow): Place | null {
		if (r.job === MANUAL || (r.kind !== 'model' && r.folder === 'other')) return null;
		const job = r.kind === 'job' ? r.key : r.job;
		if (r.kind === 'model') return { base: place.base, job, folder: r.folder && r.folder !== 'other' ? r.folder : null, model: nameOf(r.key) };
		return { base: place.base, job, folder: r.folder, model: null };
	}
	const hrefOf = (r: DrillRow) => {
		const at = rowPlace(r);
		return at ? href(selectorOf(at)) : null;
	};
	/** "Add" unions the row's own selector in: a job, a whole folder or a model, not narrowed to the level. */
	const add = (r: DrillRow) => {
		const one = r.kind === 'job' ? `job:${r.key}` : r.kind === 'folder' ? `path:${pathOfFolder(r.key)}` : nameOf(r.key);
		void nav(href(union(selection.select, one)));
	};

	// Jobs' and top-level folders' series come with the read; deeper rows' are read as they come into view.
	const series = createSeries();
	const query = $derived({ days, select: place.base.join(' '), exclude: selection.exclude });
	const groupOf = (r: Pick<DrillRow, 'kind' | 'key' | 'job' | 'folder'>): CostSeriesGroup => ({
		...(r.job === MANUAL ? { manual: true } : r.job !== null ? { job: r.job } : {}),
		...(r.kind === 'folder' ? { folder: r.key } : r.kind === 'model' ? { nodeKey: r.key } : {})
	});
	/** A series the read already has, or undefined for one to read. */
	function known(r: Pick<DrillRow, 'kind' | 'key' | 'job' | 'folder'>): readonly number[] | undefined {
		if (r.kind === 'job') return jobDaily.get(r.key) ?? read.dates.map(() => 0);
		if (r.kind === 'folder' && r.job === null && !r.key.includes('/')) return folderDaily.get(r.key) ?? read.dates.map(() => 0);
		return undefined;
	}
	const trend = (r: DrillRow) => known(r) ?? series.get(groupOf(r));
	let range = $state.raw({ first: 0, last: 40 });
	const levelGroup = $derived<DrillRow>({ kind: level.folder ? 'folder' : 'job', key: level.folder ?? levelJob ?? '', job: levelJob, folder: level.folder, label: '', deeper: false, credits: 0, previous: 0, seconds: 0, runs: 0, queries: 0, models: 0 });
	const levelTrend = $derived.by(() => {
		if (levelJob === null && level.folder === null) {
			const sum = read.dates.map(() => 0);
			for (const s of read.jobDaily) s.credits.forEach((v, i) => (sum[i] += v));
			return sum;
		}
		return known(levelGroup) ?? series.get(groupOf(levelGroup));
	});
	const drawerGroup = $derived<CostSeriesGroup | null>(modelKey ? { nodeKey: modelKey, ...(place.job ? { job: place.job } : {}) } : null);
	$effect(() => {
		const want: CostSeriesGroup[] = [];
		for (const t of rows.slice(Math.max(0, range.first - 10), range.last + 10)) if ('row' in t && !known(t.row)) want.push(groupOf(t.row));
		if (!(levelJob === null && level.folder === null) && !known(levelGroup)) want.push(groupOf(levelGroup));
		if (drawerGroup) want.push(drawerGroup);
		const [client, scope, q] = [app.client, app.scope, query];
		if (scope && want.length) untrack(() => series.want(client, scope, q, want));
	});

	const dayTitle = (i: number, v: number) => `${dayLabel(read.dates[i] ?? '', true)} · ${money(v, read.price)}`;
	const crumbs = $derived(placeCrumbs(refused ? { base: [], job: null, folder: null, model: null } : { ...place, model: null }));
	const inJob = $derived(level.by === 'job' && !!level.job);
	const levelName = $derived(level.by === 'job' && !level.job ? 'Job' : level.flat ? 'Model' : 'Name');
	const modelJobs = $derived(modelKey ? index.jobsOf(modelKey) : []);
	const modelTotal = $derived(modelKey ? index.totals(null, null, modelKey) : null);
	const closeDrawer = () => nav(href(selectorOf({ ...place, model: null })));
	const jobHref = (r: DrillRow) => (r.key === MANUAL || !place.model ? null : href(selectorOf({ base: place.base, job: r.key, folder: place.folder, model: place.model })));

	let width = $state(560);
	let dragging = $state(false);
	const clamp = (w: number) => Math.round(Math.min(Math.max(420, w), Math.max(440, window.innerWidth * 0.6)));
	function onkey(e: KeyboardEvent) {
		if (e.key === 'Escape' && place.model && !(e.target as Element)?.closest?.('input, textarea')) void closeDrawer();
	}
</script>

<svelte:window onkeydown={onkey} />

<SelectionBar {selection} {names} total={names.length} build={false} />
<header class="level-head">
	<nav class="crumbs" aria-label="Breadcrumb">
		{#each crumbs as c, i (i)}
			{#if i}<span class="sep">/</span>{/if}
			{#if i < crumbs.length - 1}<a href={href(c.select)} class:code={c.code} data-sveltekit-noscroll onclick={(e) => follow(e, href(c.select))}>{c.label}</a>{:else}<span class="here" class:code={c.code} title={c.label}>{c.label}</span>{/if}
		{/each}
	</nav>
	{#if !refused}
		<div class="head">
			<div class="facts"><LevelStrip {totals} {days} whole={read.credits} price={read.price} /></div>
			<div class="trend">{#if levelTrend}<TrendArea dates={read.dates} values={[...levelTrend]} price={read.price} height={52} />{/if}</div>
		</div>
	{/if}
</header>
<div class="tree-bar">
	{#if !place.job && !place.folder && !place.base.length}<SegmentedFilter label="Group by" value={byParam} options={[{ value: 'job', label: 'Job' }, { value: 'folder', label: 'Folder' }]} onselect={(v) => nav(href(selection.select, { by: v === 'folder' ? 'folder' : null }))} />{/if}
	{#if inJob}<SegmentedFilter label="Models" value={flat ? 'flat' : 'tree'} options={[{ value: 'tree', label: 'By folder' }, { value: 'flat', label: 'All models' }]} onselect={(v) => nav(href(selection.select, { flat: v === 'flat' ? '1' : null }))} />{/if}
	<span class="count">{count(topRows)} {topRows === 1 ? 'row' : 'rows'}</span>
	{#if series.failed}<span class="failed" role="alert">Daily cost failed to load: {series.failed}</span>{/if}
	<span class="spacer"></span>
	<Button variant="ghost" size="sm" onclick={() => (opened = { at: levelKey, ids: new Set(index.ids(level, 2)) })}>Expand all</Button>
	<Button variant="ghost" size="sm" disabled={!open.size} onclick={() => (opened = { at: levelKey, ids: new Set() })}>Collapse all</Button>
</div>
<div class="table-area">
	<DrillTable price={read.price} {rows} name={levelName} {hrefOf} moreHref={inJob ? () => href(selection.select, { flat: '1' }) : undefined} ontoggle={toggle} selected={modelKey} {trend} onadd={add} {dayTitle} onwindow={(r) => (range = r)} onfollow={follow} empty={refused ? 'No nodes selected.' : 'No cost in this period.'} />
	{#if modelKey && modelTotal && !refused}
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
				<span class="name" title={modelKey}>{place.model}</span>
				<strong title={costTitle(modelTotal.credits, read.price)}>{money(modelTotal.credits, read.price)}</strong>
				<Button variant="ghost" size="icon-sm" onclick={closeDrawer} aria-label="Close" title="Close (Esc)"><XIcon /></Button>
			</div>
			<div class="drawer-body sb-scroll">
				{#key modelKey}<ModelDetail nodeKey={modelKey} job={place.job} {days} dates={read.dates} price={read.price} daily={drawerGroup ? series.get(drawerGroup) : null} jobs={modelJobs} {jobHref} />{/key}
			</div>
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
	/* Lined up like the Catalog's toolbar: its segmented controls, then the count. */
	.tree-bar { display: flex; flex-wrap: wrap; align-items: center; gap: 6px 10px; min-height: 45px; padding: 6px 20px; border-bottom: 1px solid var(--border); color: var(--muted-foreground); font-size: 12.5px; }
	.count { margin-left: 4px; }
	.failed { color: var(--sb-error); }
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
