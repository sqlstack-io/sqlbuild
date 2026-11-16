<script lang="ts">
	import { countBy } from '$lib/list/count-by';
	import { openRow, to } from '$lib/paths';
	import PlayIcon from '@lucide/svelte/icons/play';
	import XIcon from '@lucide/svelte/icons/x';
	import Button from '$lib/components/ui/button/button.svelte';
	import { NODES_PARAM } from '$lib/lineage/folder-selection';
	import { goto } from '$app/navigation';
	import { page } from '$app/state';
	import type { CatalogNode } from '$lib/api';
	import ListToolbar from '$lib/components/list/list-toolbar.svelte';
	import SegmentedFilter from '$lib/components/list/segmented-filter.svelte';
	import ChoiceFilter from '$lib/components/list/choice-filter.svelte';
	import StatusLabel from '$lib/components/list/status-label.svelte';
	import RelativeTime from '$lib/components/list/relative-time.svelte';
	import AppTopbar from '$lib/components/app-topbar.svelte';
	import RefreshButton from '$lib/components/refresh-button.svelte';
	import GatedButton from '$lib/components/operate/gated-button.svelte';
	import StaleBadges from '$lib/components/build-area/stale-badges.svelte';
	import { planHref } from '$lib/run/command';
	import { topFolder } from '$lib/lineage/catalog-graph';
	import { app } from '$lib/state/app.svelte';
	import { useCatalogNodes } from '$lib/state/catalog.svelte';
	import DataTable from '$lib/components/list/data-table.svelte';
	import type { TableColumn } from '$lib/components/list/table-columns';
	import { formatAgo, formatStamp } from '$lib/format/time';
	import { nodeOutcomeLabel, nodeOutcomeTone } from '$lib/status';
	import { KIND_TABS, kindLine, kindTabLabel, kindTabOf, nodeIcon, nodeIconKey, parseKindTab, shortName, shortRunId, type KindTab } from '$lib/build-area/labels';

	type StatusFilter = 'stale' | 'changed' | 'behind' | 'failed' | 'warn';

	const catalog = useCatalogNodes();
	const all = $derived(catalog.data ?? []);
	const byKindTab = $derived(countBy(all, (n) => kindTabOf(n.kind)));
	const byFolder = $derived(countBy(all, topFolder));

	const params = page.url.searchParams;
	let query = $state(params.get('q') ?? '');
	let kindFilter = $state<'all' | KindTab>(parseKindTab(params.get('kind')) ?? 'all');
	let groupFilter = $state(params.get('group') ?? '');
	let statusFilter = $state<StatusFilter | ''>(params.get('stale') ? 'stale' : '');
	// Nodes picked elsewhere, such as a provider's code from its Used by tab.
	let picked = $state<ReadonlySet<string>>(new Set((params.get(NODES_PARAM) ?? '').split(',').filter(Boolean)));
	function clearPicked() {
		picked = new Set();
		const url = new URL(page.url);
		url.searchParams.delete(NODES_PARAM);
		void goto(url, { replaceState: true, noScroll: true, keepFocus: true });
	}

	const matchesStatus = (n: CatalogNode, f: StatusFilter | '') =>
		!f ||
		(f === 'stale' && !!n.stale?.length) ||
		(f === 'changed' && !!n.stale?.some((s) => s.reason !== 'behind')) ||
		(f === 'behind' && !!n.stale?.some((s) => s.reason === 'behind')) ||
		(f === 'failed' && n.lastOutcome === 'failed') ||
		(f === 'warn' && n.lastOutcome === 'warn');

	const kinds = $derived(KIND_TABS.filter((k) => all.some((n) => kindTabOf(n.kind) === k)));
	const groups = $derived([...new Set(all.map(topFolder))].sort());
	/** Outcome and staleness come together; a server that reports neither leaves them unknown. */
	const showBuildState = $derived(all.some((n) => n.stale !== null));
	const showChecks = $derived(all.some((n) => n.checks !== null));
	const linksNodes = $derived(app.client.supports('getCatalogNode'));
	const canRun = $derived(app.client.supports('launchAdhocRun'));

	const filtered = $derived(
		all.filter((n) => {
			if (picked.size && !picked.has(n.key)) return false;
			if (kindFilter !== 'all' && kindTabOf(n.kind) !== kindFilter) return false;
			// A filter whose control is hidden, for example from a shared link, never narrows the list.
			if (groups.length > 1 && groupFilter && topFolder(n) !== groupFilter) return false;
			if (showBuildState && !matchesStatus(n, statusFilter)) return false;
			const q = query.trim().toLowerCase();
			return !q || [n.key, n.description, kindLine(n), ...n.tags].some((s) => s.toLowerCase().includes(q));
		})
	);

	const counts = $derived({
		failed: all.filter((n) => n.lastOutcome === 'failed').length,
		warn: all.filter((n) => n.lastOutcome === 'warn').length,
		changed: all.filter((n) => matchesStatus(n, 'changed')).length,
		behind: all.filter((n) => matchesStatus(n, 'behind')).length
	});
	const statusOptions: { value: StatusFilter; label: string }[] = [
		{ value: 'stale', label: 'Stale' },
		{ value: 'changed', label: 'Stale · changed' },
		{ value: 'behind', label: 'Stale · behind' },
		{ value: 'failed', label: 'Last build failed' },
		{ value: 'warn', label: 'Last build warned' }
	];

	const ROW_H = 52;
	const tableColumns = $derived<TableColumn[]>([
		{ id: 'name', label: 'Name', minWidth: 240 },
		{ id: 'kind', label: 'Kind', width: '170px' },
		...(showBuildState ? [{ id: 'outcome', label: 'Last outcome', width: '150px' }, { id: 'stale', label: 'Stale', width: '170px' }] : []),
		...(showChecks ? [{ id: 'checks', label: 'Checks', width: '90px', align: 'right' as const }] : []),
		{ id: 'built', label: 'Last built', width: '130px' }
	]);

	const filteredRunnable = $derived(filtered.filter((n) => n.kind !== 'source').map((n) => n.key));
	const narrowed = $derived(filtered.length !== all.length);
</script>

<AppTopbar title="Catalog">
	<RefreshButton onclick={() => catalog.refresh()} />
	{#if canRun}<GatedButton size="bar" requires="run.adhoc" variant="default" onclick={() => goto(to(planHref(narrowed ? filteredRunnable : [])))}><PlayIcon class="size-3.5 fill-current stroke-none" />Execute{narrowed ? ` ${filteredRunnable.length}` : ''}</GatedButton>{/if}
</AppTopbar>


<ListToolbar
	loading={catalog.data === undefined}
	bind:query
	placeholder="Search catalog"
	total="{filtered.length} of {all.length} nodes"
	counts={showBuildState
		? [
				{ tone: 'var(--sb-stale)', count: counts.changed, label: 'changed' },
				{ tone: 'var(--sb-behind)', count: counts.behind, label: 'behind' },
				{ tone: 'var(--sb-error)', count: counts.failed, label: 'failed' }
			]
		: []}
>
	{#snippet filters()}
		{#if picked.size}<Button variant="outline" size="sm" class="mr-1.5" onclick={clearPicked} title="Show every node">{picked.size} picked<XIcon /></Button>{/if}
		<SegmentedFilter loading={catalog.data === undefined}
			bind:value={kindFilter}
			options={[{ value: 'all' as const, label: 'All', count: all.length }, ...kinds.map((k) => ({ value: k, label: kindTabLabel[k], count: byKindTab.get(k) ?? 0 }))]}
		/>
		{#if showBuildState}<ChoiceFilter label="Status" bind:value={statusFilter} options={statusOptions.map((o) => ({ ...o, count: all.filter((n) => matchesStatus(n, o.value)).length }))} />{/if}
		{#if groups.length > 1}<ChoiceFilter label="Folder" bind:value={groupFilter} options={groups.map((g) => ({ value: g, label: g, count: byFolder.get(g) ?? 0 }))} />{/if}
	{/snippet}
</ListToolbar>

<DataTable class="catalog" columns={tableColumns} rows={filtered} key={(n) => n.key} rowHeight={ROW_H} resource={catalog} empty={all.length ? 'No nodes match the filter.' : 'Nothing deployed yet.'}>
	{#snippet row(n)}
				{@const Glyph = nodeIcon[nodeIconKey(n)] ?? nodeIcon.table}
				<tr onclick={(e) => linksNodes && openRow(e, `/catalog/${n.key}`)}>
					<td>
						<div class="cell-name">
							<Glyph size={15} strokeWidth={1.75} />
							<div class="min-w-0">
								{#if linksNodes}<a href={to(`/catalog/${n.key}`)} title={n.key}>{shortName(n.key)}</a>{:else}<span class="mono" title={n.key}>{shortName(n.key)}</span>{/if}
								<div class="cell-desc">{n.description}</div>
							</div>
						</div>
					</td>
					<td class="muted">{kindLine(n)}{#if n.group}<div class="cell-desc">{n.group}</div>{/if}</td>
					{#if showBuildState}
					<td>
						{#if n.kind === 'source'}
							{#if n.sourceFreshness}<StatusLabel tone={n.sourceFreshness.status === 'pass' ? 'var(--sb-success)' : n.sourceFreshness.status === 'warn' ? 'var(--sb-warning)' : 'var(--sb-error)'} label="Freshness {n.sourceFreshness.status}" />
							{:else}<span class="muted">External</span>{/if}
						{:else if n.lastOutcome}
							<StatusLabel tone={nodeOutcomeTone[n.lastOutcome]} label={nodeOutcomeLabel[n.lastOutcome]} />
						{:else}<span class="muted">Not built</span>{/if}
					</td>
					<td><StaleBadges reasons={n.stale ?? []} empty="—" /></td>
					{/if}
					{#if showChecks}
					<td class="num" style:color={n.checks?.failing ? 'var(--sb-error)' : n.checks?.warning ? 'var(--sb-warning)' : 'var(--muted-foreground)'}>
						{n.checks?.total ? `${n.checks.passing} / ${n.checks.total}` : '—'}
					</td>
					{/if}
					<td>
						{#if n.kind === 'source'}
							<RelativeTime value={n.sourceFreshness?.loadedAt ? `loaded ${formatAgo(n.sourceFreshness.loadedAt, app.now)}` : '—'} exact={formatStamp(n.sourceFreshness?.loadedAt)} />
						{:else if n.lastBuild}
							<RelativeTime value={formatAgo(n.lastBuild.builtAt, app.now)} exact="{formatStamp(n.lastBuild.builtAt)} · {n.lastBuild.runId ? `run ${shortRunId(n.lastBuild.runId)}` : `sqb run ${shortRunId(n.lastBuild.fingerprintRunId)}`}" />
						{:else}<span class="muted">Never</span>{/if}
					</td>
				</tr>
	{/snippet}
</DataTable>

<style>
	:global(.catalog) .cell-name a,
	:global(.catalog) .cell-name .mono {
		display: block;
		overflow: hidden;
		text-overflow: ellipsis;
	}
	:global(.catalog .stale-badges) {
		flex-wrap: nowrap;
	}
</style>
