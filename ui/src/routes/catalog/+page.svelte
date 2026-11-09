<script lang="ts">
	import { goto } from '$app/navigation';
	import { page } from '$app/state';
	import type { CatalogNode, NodeKind } from '$lib/api';
	import ListToolbar from '$lib/components/list/list-toolbar.svelte';
	import SegmentedFilter from '$lib/components/list/segmented-filter.svelte';
	import ChoiceFilter from '$lib/components/list/choice-filter.svelte';
	import StatusLabel from '$lib/components/list/status-label.svelte';
	import RelativeTime from '$lib/components/list/relative-time.svelte';
	import LoadRow from '$lib/components/list/load-row.svelte';
	import AppTopbar from '$lib/components/app-topbar.svelte';
	import Button from '$lib/components/ui/button/button.svelte';
	import GatedButton from '$lib/components/operate/gated-button.svelte';
	import StaleBadges from '$lib/components/build-area/stale-badges.svelte';
	import RunPanel from '$lib/run/run-panel.svelte';
	import { app } from '$lib/state/app.svelte';
	import { createResource } from '$lib/state/resource.svelte';
	import { formatAgo, formatStamp } from '$lib/format/time';
	import { nodeOutcomeLabel, nodeOutcomeTone } from '$lib/status';
	import { NODE_KINDS, kindLine, nodeIcon, nodeIconKey, nodeKindPlural, shortName, shortRunId } from '$lib/build-area/labels';

	type StatusFilter = 'stale' | 'changed' | 'behind' | 'failed' | 'warn';

	const catalog = createResource((client, scope) => client.listCatalogNodes(scope), ['runs', 'checks', 'snapshot', 'node_status']);
	const all = $derived(catalog.data ?? []);

	const params = page.url.searchParams;
	let query = $state(params.get('q') ?? '');
	let kindFilter = $state<'all' | NodeKind>((NODE_KINDS as string[]).includes(params.get('kind') ?? '') ? (params.get('kind') as NodeKind) : 'all');
	let ownerFilter = $state(params.get('owner') ?? '');
	let groupFilter = $state(params.get('group') ?? '');
	let statusFilter = $state<StatusFilter | ''>(params.get('stale') ? 'stale' : '');

	const matchesStatus = (n: CatalogNode, f: StatusFilter | '') =>
		!f ||
		(f === 'stale' && !!n.stale?.length) ||
		(f === 'changed' && !!n.stale?.some((s) => s.reason !== 'behind')) ||
		(f === 'behind' && !!n.stale?.some((s) => s.reason === 'behind')) ||
		(f === 'failed' && n.lastOutcome === 'failed') ||
		(f === 'warn' && n.lastOutcome === 'warn');

	const kinds = $derived(NODE_KINDS.filter((k) => all.some((n) => n.kind === k)));
	const owners = $derived([...new Set(all.flatMap((n) => n.owner))].sort());
	const groups = $derived([...new Set(all.flatMap((n) => (n.group === null ? [] : [n.group])))]);
	/** Outcome and staleness come together; a server that reports neither leaves them unknown. */
	const showBuildState = $derived(all.some((n) => n.stale !== null));
	const showChecks = $derived(all.some((n) => n.checks !== null));
	const linksNodes = $derived(app.client.supports('getCatalogNode'));
	const canRun = $derived(app.client.supports('launchAdhocRun'));

	const filtered = $derived(
		all.filter((n) => {
			if (kindFilter !== 'all' && n.kind !== kindFilter) return false;
			// A filter whose control is hidden, for example from a shared link, never narrows the list.
			if (owners.length && ownerFilter && !n.owner.includes(ownerFilter)) return false;
			if (groups.length && groupFilter && n.group !== groupFilter) return false;
			if (showBuildState && !matchesStatus(n, statusFilter)) return false;
			const q = query.trim().toLowerCase();
			return !q || [n.key, n.description, kindLine(n), ...n.tags, ...n.owner].some((s) => s.toLowerCase().includes(q));
		})
	);

	const columns = $derived(3 + (owners.length ? 1 : 0) + (showBuildState ? 2 : 0) + (showChecks ? 1 : 0));
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

	const filteredRunnable = $derived(filtered.filter((n) => n.kind !== 'source').map((n) => n.key));
	const narrowed = $derived(filtered.length !== all.length);
	let runOpen = $state(false);
</script>

<AppTopbar title="Catalog">
	<Button variant="outline" size="sm" onclick={() => catalog.refresh()}>↻ Refresh</Button>
	{#if canRun}<GatedButton requires="run.adhoc" variant="default" onclick={() => (runOpen = true)}>▷ Execute{narrowed ? ` ${filteredRunnable.length}` : ''}</GatedButton>{/if}
</AppTopbar>

{#if canRun}<RunPanel bind:open={runOpen} selection={narrowed ? filteredRunnable : []} />{/if}

<ListToolbar
	bind:query
	placeholder="Search catalog"
	total="{filtered.length} of {all.length} nodes"
	counts={showBuildState
		? [
				{ tone: 'var(--sb-stale)', count: counts.changed, label: 'changed' },
				{ tone: 'var(--sb-warning)', count: counts.behind, label: 'behind' },
				{ tone: 'var(--sb-error)', count: counts.failed, label: 'failed' }
			]
		: []}
>
	{#snippet filters()}
		<SegmentedFilter
			bind:value={kindFilter}
			options={[{ value: 'all' as const, label: 'All', count: all.length }, ...kinds.map((k) => ({ value: k, label: nodeKindPlural[k], count: all.filter((n) => n.kind === k).length }))]}
		/>
		{#if showBuildState}<ChoiceFilter label="Status" bind:value={statusFilter} options={statusOptions.map((o) => ({ ...o, count: all.filter((n) => matchesStatus(n, o.value)).length }))} />{/if}
		{#if owners.length}<ChoiceFilter label="Owner" bind:value={ownerFilter} options={owners.map((o) => ({ value: o, label: o, count: all.filter((n) => n.owner.includes(o)).length }))} />{/if}
		{#if groups.length}<ChoiceFilter label="Group" bind:value={groupFilter} options={groups.map((g) => ({ value: g, label: g, count: all.filter((n) => n.group === g).length }))} />{/if}
	{/snippet}
</ListToolbar>

<div class="sb-list-wrap">
	<table class="sb-list">
		<thead>
			<tr>
				<th>Name</th>
				<th>Kind</th>
				{#if owners.length}<th>Owner</th>{/if}
				{#if showBuildState}<th>Last outcome</th><th>Stale</th>{/if}
				{#if showChecks}<th class="num">Checks</th>{/if}
				<th>Last built</th>
			</tr>
		</thead>
		<tbody>
			{#each filtered as n (n.key)}
				{@const Glyph = nodeIcon[nodeIconKey(n)] ?? nodeIcon.table}
				<tr onclick={() => linksNodes && goto(`/catalog/${n.key}`)}>
					<td>
						<div class="cell-name">
							<Glyph size={15} strokeWidth={1.75} />
							<div class="min-w-0">
								{#if linksNodes}<a href="/catalog/{n.key}" title={n.key} onclick={(e) => e.stopPropagation()}>{shortName(n.key)}</a>{:else}<span class="mono" title={n.key}>{shortName(n.key)}</span>{/if}
								<div class="cell-desc">{n.description}</div>
							</div>
						</div>
					</td>
					<td class="muted">{kindLine(n)}{#if n.group}<div class="cell-desc">{n.group}</div>{/if}</td>
					{#if owners.length}
					<td>
						{#if n.owner.length}<div class="flex flex-wrap gap-1">{#each n.owner as o (o)}<span class="sb-tag">{o}</span>{/each}</div>
						{:else}<span class="muted">—</span>{/if}
					</td>
					{/if}
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
			{:else}
				<LoadRow colspan={columns} resource={catalog} empty={all.length ? 'No nodes match the filter.' : 'Nothing deployed yet. Deploy the project to see its models, sources and seeds here.'} />
			{/each}
		</tbody>
	</table>
</div>
