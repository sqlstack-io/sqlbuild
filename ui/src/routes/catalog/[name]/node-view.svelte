<script lang="ts">
	import DataTable from '$lib/components/list/data-table.svelte';
	import type { TableColumn } from '$lib/components/list/table-columns';
	import PageTabs from '$lib/components/platform/page-tabs.svelte';
	import { to } from '$lib/paths';
	import PlayIcon from '@lucide/svelte/icons/play';
	import { page } from '$app/state';
	import { goto } from '$app/navigation';
	import AppTopbar from '$lib/components/app-topbar.svelte';
	import RefreshButton from '$lib/components/refresh-button.svelte';
	import GatedButton from '$lib/components/operate/gated-button.svelte';
	import CodePreview from '$lib/components/code/code-preview.svelte';
	import SqlViewer from '$lib/components/code/sql-viewer.svelte';
	import StatusLabel from '$lib/components/list/status-label.svelte';
	import StaleBadges from '$lib/components/build-area/stale-badges.svelte';
	import ScopedLineage from '$lib/lineage/scoped-lineage.svelte';
	import ColumnLineage from '$lib/lineage/column-lineage.svelte';
	import { planHref } from '$lib/run/command';
	import { app } from '$lib/state/app.svelte';
	import { createResource } from '$lib/state/resource.svelte';
	import { useCatalogNodes } from '$lib/state/catalog.svelte';
	import { useChecks } from '$lib/state/checks.svelte';
	import { untrack } from 'svelte';
	import { formatAgo } from '$lib/format/time';
	import { statusLabel } from '$lib/build-area/node-status';
	import { kindLine, kindTabLabel, kindTabOf, nodeIcon, nodeIconKey, shortName, sqlVersions } from '$lib/build-area/labels';
	import { cardData } from '$lib/lineage/catalog-graph';
	import OverviewTab from './overview-tab.svelte';
	import DriftTab from './drift-tab.svelte';
	import RunsTab from './runs-tab.svelte';
	import ChecksTab from './checks-tab.svelte';
	import WarehouseTab from './warehouse-tab.svelte';
	import HistoryTab from './history-tab.svelte';
	import SyncingNote from '$lib/components/platform/syncing-note.svelte';
	import { upstreamNote, type Tab } from './utils';

	let { name }: { name: string } = $props();
	const COST_DAYS = 30;

	const catalog = useCatalogNodes();
	const checks = useChecks();
	// The shared listing, when held, saves the detail read from listing every node again; it is not a dependency.
	// The detail reloads once for each read of the shared checks, which follow the signals that change it (runs,
	// node states, snapshots, warehouse syncs and reconnects), so a burst of signals reloads it once, with its
	// checks. A checks read in flight, such as of a list that went stale while no page showed it, is waited for
	// rather than read around; after a failure the detail reads the checks itself. Derived, so the store's
	// other changes do not reload the detail.
	const checkList = $derived(checks.data);
	const checksRead = $derived(checks.revision);
	const checksSettled = $derived(!checks.loading);
	const res = createResource((client, scope) => {
		void checksRead;
		if (!checksSettled) return new Promise<never>(() => {});
		return client.getCatalogNode(scope, name, untrack(() => catalog.data), checkList);
	});
	const n = $derived(res.data ?? null);
	// Checks and migrations link nodes by bare name; settle on the node key so the graph and links agree.
	$effect(() => {
		if (n && n.key !== name) goto(to(`/catalog/${n.key}${page.url.search}`), { replaceState: true });
	});
	const colLineage = createResource((client, scope) => client.getColumnLineage(scope, name, untrack(() => catalog.data)), ['snapshot']);
	const costs = createResource((client, scope) => (n && client.supports('listNodeCosts') ? client.listNodeCosts(scope, n.key, COST_DAYS) : Promise.resolve([])), ['runs', 'warehouse_sync']);

	const tabs = $derived.by<[Tab, string][]>(() => {
		if (!n) return [['overview', 'Overview']];
		const t: [Tab, string][] = [['overview', 'Overview']];
		if (n.columns.length) t.push(['columns', 'Columns']);
		if (n.sql) t.push(['sql', 'SQL']);
		else if (n.code) t.push(['sql', 'Code']);
		if (n.sql && n.builds.length) t.push(['drift', 'Drift']);
		t.push(['lineage', 'Lineage']);
		if (n.columns.length && app.client.supports('getColumnLineage')) t.push(['collineage', 'Column lineage']);
		if (n.kind !== 'source' && n.runs) t.push(['runs', 'Runs']);
		t.push(['checks', `Checks${n.checkResults.length ? ` ${n.checkResults.length}` : ''}`]);
		if (n.warehouse) t.push(['warehouse', 'Warehouse']);
		if (n.history) t.push(['history', 'History']);
		return t;
	});
	// The URL is the single source of truth for the tab, so back/forward and reloads restore it.
	const requested = $derived((page.url.searchParams.get('tab') ?? 'overview') as Tab);
	const tab = $derived<Tab>(tabs.some(([t]) => t === requested) ? requested : 'overview');

	function setTab(t: Tab) {
		const url = new URL(page.url);
		if (t === 'overview') url.searchParams.delete('tab');
		else url.searchParams.set('tab', t);
		goto(url, { replaceState: true, noScroll: true, keepFocus: true });
	}

	let viewerMode = $state<'raw' | 'compiled' | 'diff' | 'drift'>('raw');
	let driftMode = $state<'raw' | 'compiled' | 'diff' | 'drift'>('drift');
	const versions = $derived(n ? sqlVersions(n.builds, app.now) : []);
	const graph = $derived.by(() => {
		const nodes = catalog.data ?? [];
		return {
			nodes: nodes.map((c) => ({ id: c.key, name: c.key, data: cardData(c) })),
			edges: nodes.flatMap((c) => c.upstream.map((up) => [up, c.key] as [string, string]))
		};
	});

	const canRun = app.client.supports('launchAdhocRun');
	function execute(selection: string[]) {
		void goto(to(planHref(selection)));
	}
	const Glyph = $derived(n ? (nodeIcon[nodeIconKey(n)] ?? nodeIcon.table) : nodeIcon.table);
	const columnColumns: TableColumn[] = [
		{ id: 'column', label: 'Column', minWidth: 180 },
		{ id: 'type', label: 'Type', width: '160px' },
		{ id: 'description', label: 'Description', width: '40%' },
		{ id: 'audits', label: 'Audits', width: '22%', truncate: false }
	];
</script>

<AppTopbar title="Catalog" subject="{name} · Catalog">
	<RefreshButton onclick={() => res.refresh()} />
	{#if n && n.kind !== 'source' && canRun}
		<GatedButton size="bar" requires="run.adhoc" variant="default" onclick={() => execute([name])}><PlayIcon class="size-3.5 fill-current stroke-none" />Build</GatedButton>
	{/if}
</AppTopbar>

<div class="asset-page">
	<header class="asset-header">
		<nav class="crumbs"><a href={to(`/catalog`)}>Catalog</a><span>/</span>{#if n}<a href={to(`/catalog?kind=${kindTabOf(n.kind)}`)}>{kindTabLabel[kindTabOf(n.kind)]}</a>{/if}</nav>
		<div class="title-row">
			<Glyph size={18} strokeWidth={1.75} />
			<h2 title={name}>{shortName(n?.key ?? name)}</h2>
			{#if n}
				{#if n.kind === 'source'}
					<StatusLabel tone="var(--sb-text-faint)" label="External source" muted />
				{:else if statusLabel(n)}
					{@const s = statusLabel(n)!}
					<StatusLabel tone={s.tone} label={s.label} muted={n.status === 'not_built'} />
				{/if}
				<StaleBadges reasons={n.stale} />
			{/if}
			<div class="header-actions">
				<a class="btn-ghost" href={to(`/lineage?focus=${encodeURIComponent(name)}`)}>View in lineage</a>
			</div>
		</div>
		{#if n}
			<div class="meta-row">
				<span>{kindLine(n)}</span>{#if n.group}<span>{n.group}</span>{/if}
				{#if n.kind === 'source'}<span>{n.sourceFreshness?.loadedAt ? `Loaded ${formatAgo(n.sourceFreshness.loadedAt, app.now)}` : 'No freshness data'}</span>
				{:else}<span>{n.statusAt ? `Last built ${formatAgo(n.statusAt, app.now)}` : 'Never built'}</span>{/if}
			</div>
		{/if}
		<div class="mt-2.5"><PageTabs flush value={tab} tabs={tabs.map(([value, label]) => ({ value, label }))} onselect={setTab} /></div>
	</header>

	{#if res.error}<div class="msg err">Couldn't load {name}: {res.error}</div>{:else if res.data === null}<div class="msg">There's no node <code>{name}</code> in the version deployed to {app.location?.name}.</div>{/if}

	{#if n}
		<div class="asset-body" class:fill={tab === 'lineage' || tab === 'collineage' || (tab === 'sql' && !!n.sql) || tab === 'drift'}>
			{#if tab === 'overview'}
				<OverviewTab node={n} onTab={setTab} onSql={(mode) => { viewerMode = mode; setTab('sql'); }} />
			{:else if tab === 'columns'}
				<DataTable class="static" embedded columns={columnColumns} rows={n.columns} key={(c) => c.name} empty="No columns declared.">
					{#snippet row(c)}
						<tr>
							<td><code>{c.name}</code></td>
							<td><code class="muted">{c.type ?? '—'}</code></td>
							<td class="muted">{c.description}</td>
							<td><div class="flex flex-wrap gap-1">{#each c.audits as a (a)}<span class="sb-tag">{a}</span>{:else}<span class="muted">—</span>{/each}</div></td>
						</tr>
					{/snippet}
				</DataTable>
			{:else if tab === 'sql'}
				{#if n.sql}
					<SqlViewer name={shortName(n.key)} raw={n.sql.raw} compiled={n.sql.compiled} bind:mode={viewerMode} {versions} upstreamNote={upstreamNote(n)} onRef={(m) => goto(to(`/catalog/${m}?tab=sql`))} />
				{:else if n.code}
					<div class="p-5"><CodePreview file={n.code.file} code={n.code.source} language="text" maxLines={200} /></div>
				{/if}
			{:else if tab === 'drift' && n.sql}
				<DriftTab node={n}>
					<SqlViewer name={shortName(n.key)} raw={n.sql.raw} compiled={n.sql.compiled} bind:mode={driftMode} {versions} upstreamNote={upstreamNote(n)} onRef={(m) => goto(to(`/catalog/${m}?tab=drift`))} />
				</DriftTab>
			{:else if tab === 'lineage'}
				{#if graph.nodes.length}
					<ScopedLineage focal={name} graphNodes={graph.nodes} graphEdges={graph.edges} onExecute={canRun ? execute : undefined} />
				{/if}
			{:else if tab === 'collineage'}
				{#if colLineage.data}
					<ColumnLineage focal={name} models={colLineage.data.models} colEdges={colLineage.data.edges} modelSql={colLineage.data.sql} modelColExpr={colLineage.data.expressions} />
				{/if}
			{:else if tab === 'runs'}
				<SyncingNote what="Builds and costs" />
				<RunsTab node={n} costs={costs.data ?? []} costDays={COST_DAYS} />
			{:else if tab === 'checks'}
				<SyncingNote what="Check results" />
				<ChecksTab node={n} />
			{:else if tab === 'warehouse' && n.warehouse}
				<WarehouseTab warehouse={n.warehouse} microbatch={n.microbatch} />
			{:else if tab === 'history' && n.history}
				<SyncingNote what="History" />
				<HistoryTab node={n} history={n.history} />
			{/if}
		</div>
	{/if}
</div>

<style>
	.asset-page {
		display: flex;
		min-height: 0;
		flex: 1;
		flex-direction: column;
	}
	.asset-header {
		padding: 14px 24px 0;
		border-bottom: 1px solid var(--border);
	}
	.crumbs {
		display: flex;
		gap: 6px;
		color: var(--muted-foreground);
		font-size: 12.5px;
	}
	.crumbs a:hover {
		color: var(--foreground);
	}
	.title-row {
		display: flex;
		align-items: center;
		gap: 10px;
		margin-top: 8px;
	}
	.title-row :global(svg) {
		color: var(--muted-foreground);
	}
	.title-row h2 {
		margin: 0 6px 0 0;
		font-size: 20px;
		font-weight: 600;
		letter-spacing: -0.01em;
	}
	.header-actions {
		display: flex;
		gap: 6px;
		margin-left: auto;
	}
	.btn-ghost {
		height: 30px;
		padding: 0 12px;
		border: 1px solid var(--border);
		border-radius: 2px;
		color: var(--foreground);
		font-size: 12.5px;
		line-height: 28px;
	}
	.btn-ghost:hover {
		background: var(--sb-hover);
	}
	.meta-row {
		display: flex;
		gap: 12px;
		margin-top: 6px;
		color: var(--muted-foreground);
		font-size: 12.5px;
	}
	.meta-row span + span::before {
		content: '·';
		margin-right: 12px;
		color: var(--sb-text-faint);
	}
	.asset-body {
		min-height: 0;
		flex: 1;
		overflow: auto;
	}
	.asset-body.fill {
		display: flex;
		overflow: hidden;
	}
	.asset-body.fill > :global(*) {
		min-width: 0;
		flex: 1;
	}
	.msg {
		padding: 14px 24px;
		color: var(--muted-foreground);
		font-size: 13px;
	}
	.err {
		color: var(--sb-error);
	}
	.muted {
		color: var(--muted-foreground);
	}
	code {
		font: 12px var(--font-mono);
	}
</style>
