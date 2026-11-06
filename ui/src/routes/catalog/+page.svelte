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
	import { NODE_KINDS, kindLine, nodeIcon, nodeIconKey, nodeKindPlural, shortRunId } from '$lib/build-area/labels';

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
		(f === 'stale' && n.stale.length > 0) ||
		(f === 'changed' && n.stale.some((s) => s.reason !== 'behind')) ||
		(f === 'behind' && n.stale.some((s) => s.reason === 'behind')) ||
		(f === 'failed' && n.lastOutcome === 'failed') ||
		(f === 'warn' && n.lastOutcome === 'warn');

	const filtered = $derived(
		all.filter((n) => {
			if (kindFilter !== 'all' && n.kind !== kindFilter) return false;
			if (ownerFilter && !n.owner.includes(ownerFilter)) return false;
			if (groupFilter && n.group !== groupFilter) return false;
			if (!matchesStatus(n, statusFilter)) return false;
			const q = query.trim().toLowerCase();
			return !q || [n.key, n.description, kindLine(n), ...n.tags, ...n.owner].some((s) => s.toLowerCase().includes(q));
		})
	);

	const kinds = $derived(NODE_KINDS.filter((k) => all.some((n) => n.kind === k)));
	const owners = $derived([...new Set(all.flatMap((n) => n.owner))].sort());
	const groups = $derived([...new Set(all.map((n) => n.group))]);
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
	<GatedButton requires="run.adhoc" variant="default" onclick={() => (runOpen = true)}>▷ Execute{narrowed ? ` ${filteredRunnable.length}` : ''}</GatedButton>
</AppTopbar>

<RunPanel bind:open={runOpen} selection={narrowed ? filteredRunnable : []} />

<ListToolbar
	bind:query
	placeholder="Search catalog"
	total="{filtered.length} of {all.length} nodes"
	counts={[
		{ tone: 'var(--sb-stale)', count: counts.changed, label: 'changed' },
		{ tone: 'var(--sb-warning)', count: counts.behind, label: 'behind' },
		{ tone: 'var(--sb-error)', count: counts.failed, label: 'failed' }
	]}
>
	{#snippet filters()}
		<SegmentedFilter
			bind:value={kindFilter}
			options={[{ value: 'all' as const, label: 'All', count: all.length }, ...kinds.map((k) => ({ value: k, label: nodeKindPlural[k], count: all.filter((n) => n.kind === k).length }))]}
		/>
		<ChoiceFilter label="Status" bind:value={statusFilter} options={statusOptions.map((o) => ({ ...o, count: all.filter((n) => matchesStatus(n, o.value)).length }))} />
		<ChoiceFilter label="Owner" bind:value={ownerFilter} options={owners.map((o) => ({ value: o, label: o, count: all.filter((n) => n.owner.includes(o)).length }))} />
		<ChoiceFilter label="Group" bind:value={groupFilter} options={groups.map((g) => ({ value: g, label: g, count: all.filter((n) => n.group === g).length }))} />
	{/snippet}
</ListToolbar>

<div class="sb-list-wrap">
	<table class="sb-list">
		<thead>
			<tr>
				<th>Name</th>
				<th>Kind</th>
				<th>Owner</th>
				<th>Last outcome</th>
				<th>Stale</th>
				<th class="num">Checks</th>
				<th>Last built</th>
			</tr>
		</thead>
		<tbody>
			{#each filtered as n (n.key)}
				{@const Glyph = nodeIcon[nodeIconKey(n)] ?? nodeIcon.table}
				<tr onclick={() => goto(`/catalog/${n.key}`)}>
					<td>
						<div class="cell-name">
							<Glyph size={15} strokeWidth={1.75} />
							<div class="min-w-0">
								<a href="/catalog/{n.key}" onclick={(e) => e.stopPropagation()}>{n.key}</a>
								<div class="cell-desc">{n.description}</div>
							</div>
						</div>
					</td>
					<td class="muted">{kindLine(n)}<div class="cell-desc">{n.group}</div></td>
					<td>
						{#if n.owner.length}<div class="flex flex-wrap gap-1">{#each n.owner as o (o)}<span class="sb-tag">{o}</span>{/each}</div>
						{:else}<span class="muted">—</span>{/if}
					</td>
					<td>
						{#if n.kind === 'source'}
							{#if n.sourceFreshness}<StatusLabel tone={n.sourceFreshness.status === 'pass' ? 'var(--sb-success)' : n.sourceFreshness.status === 'warn' ? 'var(--sb-warning)' : 'var(--sb-error)'} label="Freshness {n.sourceFreshness.status}" />
							{:else}<span class="muted">External</span>{/if}
						{:else if n.lastOutcome}
							<StatusLabel tone={nodeOutcomeTone[n.lastOutcome]} label={nodeOutcomeLabel[n.lastOutcome]} />
						{:else}<span class="muted">Not built</span>{/if}
					</td>
					<td><StaleBadges reasons={n.stale} empty="—" /></td>
					<td class="num" style:color={n.checks.failing ? 'var(--sb-error)' : n.checks.warning ? 'var(--sb-warning)' : 'var(--muted-foreground)'}>
						{n.checks.total ? `${n.checks.passing} / ${n.checks.total}` : '—'}
					</td>
					<td>
						{#if n.kind === 'source'}
							<RelativeTime value={n.sourceFreshness?.loadedAt ? `loaded ${formatAgo(n.sourceFreshness.loadedAt, app.now)}` : '—'} exact={formatStamp(n.sourceFreshness?.loadedAt)} />
						{:else if n.lastBuild}
							<RelativeTime value={formatAgo(n.lastBuild.builtAt, app.now)} exact="{formatStamp(n.lastBuild.builtAt)} · {n.lastBuild.runId ? `run ${shortRunId(n.lastBuild.runId)}` : `from the command line · ${shortRunId(n.lastBuild.fingerprintRunId)}`}" />
						{:else}<span class="muted">Never</span>{/if}
					</td>
				</tr>
			{:else}
				<LoadRow colspan={7} resource={catalog} empty={all.length ? 'No nodes match the filter.' : 'Nothing deployed yet. Deploy the project to see its models, sources and seeds here.'} />
			{/each}
		</tbody>
	</table>
</div>
