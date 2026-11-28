<script lang="ts">
	import type { ConnectionDetails, WarehouseConnection } from '$lib/api';
	import DataTable from '$lib/components/list/data-table.svelte';
	import type { TableColumn } from '$lib/components/list/table-columns';
	import AppTopbar from '$lib/components/app-topbar.svelte';
	import RefreshButton from '$lib/components/refresh-button.svelte';
	import ListToolbar from '$lib/components/list/list-toolbar.svelte';
	import DisclosureButton from '$lib/components/disclosure-button.svelte';
	import StatusLabel from '$lib/components/list/status-label.svelte';
	import ConnectionTest from '$lib/components/platform/connection-test.svelte';
	import KeyValues from '$lib/components/platform/key-values.svelte';
	import { to } from '$lib/paths';
	import { plural } from '$lib/format/plural';
	import { formatAgo } from '$lib/format/time';
	import { detailRows, lastFullSync } from '$lib/platform/connections';
	import { app } from '$lib/state/app.svelte';
	import CheckStatus from './check-status.svelte';
	import SyncTables from './sync-tables.svelte';
	import { createConnectionsState } from './state.svelte';
	import { failing, syncFacts, targetName, where } from './utils/connections';

	// List first, like Locations. One row per target: its connection, last check, sync state and Test
	// connection. A row opens its settings and state history sync under it; rows with a problem start open.
	const conn = createConnectionsState();
	type Row = WarehouseConnection & { group: ConnectionDetails | null };
	let query = $state('');
	const all = $derived<Row[]>(conn.groups.flatMap((g) => g.users.map((c) => ({ ...c, group: g.details }))));
	const shown = $derived(all.filter((c) => !query.trim() || targetName(c).toLowerCase().includes(query.trim().toLowerCase())));
	const problem = (c: Row) => c.lastTest?.outcome === 'failed' || failing(c.tables) > 0;
	const attention = $derived(all.filter(problem).length);

	let open = $state<Record<string, boolean>>({});
	const isOpen = (c: Row) => open[c.id] ?? problem(c);
	const toggle = (c: Row) => (open[c.id] = !isOpen(c));

	const columns: TableColumn[] = [
		{ id: 'connection', label: 'Connection', minWidth: 260 },
		{ id: 'check', label: 'Last check', width: '240px' },
		{ id: 'sync', label: 'State history sync', width: '220px' },
		{ id: 'test', label: '', width: '170px', align: 'right' }
	];
	const setting = (r: { label: string; value: string | null }) => ({ label: r.label, value: r.value ?? '—', mono: r.value !== null && r.label !== 'Authentication', tone: r.value === null ? 'var(--sb-text-faint)' : undefined });
	const subtitle = (d: ConnectionDetails | null) => (d ? [d.adapter, d.account ?? d.host].filter(Boolean).join(' · ') : 'Settings not published');
</script>

<AppTopbar title="Connections">
	<RefreshButton onclick={conn.refresh} />
</AppTopbar>

<ListToolbar loading={conn.resource.data === undefined} bind:query placeholder="Search connections" total={plural(shown.length, 'target')} counts={[{ tone: 'var(--sb-error)', count: attention, label: 'need attention' }]} />

<DataTable {columns} rows={shown} key={(c) => c.id} resource={conn.resource} empty={all.length ? 'No connections match the search.' : 'Nothing reported yet.'}>
	{#snippet row(c)}
		{@const bad = failing(c.tables)}
		{@const fullAt = lastFullSync(c.tables)}
		<tr class="expandable" class:opened={isOpen(c)} onclick={() => toggle(c)}>
			<td>
				<div class="cell-name">
					<DisclosureButton open={isOpen(c)} class="min-w-0">
						<div class="min-w-0">
							<strong>{targetName(c)}</strong>
							<div class="cell-desc">{subtitle(c.group)}</div>
						</div>
					</DisclosureButton>
				</div>
			</td>
			<td><CheckStatus test={c.lastTest} /></td>
			<td>
				{#if !c.tables.length}
					<StatusLabel tone="var(--sb-text-faint)" label="Not reported yet" muted />
				{:else if bad && c.lastTest?.outcome === 'failed'}
					<!-- One cause, one red: the failed connection check carries it, the sync only says it waits on it. -->
					<!-- copy-allow: the product owner's wording for a sync waiting on a failed connection check -->
					<span title="The sync can't run until the connection check passes; {bad} of {c.tables.length} tables failed"><StatusLabel tone="var(--sb-text-faint)" label="blocked: can’t connect" muted /></span>
				{:else if bad}
					<StatusLabel tone="var(--sb-error)" label="{bad} of {c.tables.length} failing" />
				{:else}
					<StatusLabel tone="var(--sb-success)" label="{c.tables.length} synced" />
				{/if}
				{#if fullAt}<div class="cell-desc">full sync {formatAgo(fullAt, app.now)}</div>{/if}
			</td>
			<td onclick={(e) => e.stopPropagation()}>
				<div class="test"><ConnectionTest scope={{ locationId: c.id }} connection={c.target} last={c.lastTest} showLast={false} ontested={conn.refresh} /></div>
			</td>
		</tr>
		{#if isOpen(c)}
			<tr class="expansion">
				<td colspan={columns.length}>
					<div class="expanded">
						<div class="facts">
							<KeyValues rows={detailRows(c.group).map(setting)} />
							{#snippet location()}<a class="link" href={to(`/locations/${encodeURIComponent(c.id)}`)}>{targetName(c)}</a>{/snippet}
							<KeyValues rows={[{ label: 'Location', content: location }, { label: 'State tables', value: c.stateLocation ? where(c.stateLocation) : 'Not reported yet', mono: !!c.stateLocation }, ...syncFacts(c, app.now)]} />
						</div>
						<div class="sync">
							<h3>State history sync</h3>
							<SyncTables tables={c.tables} blocked={c.lastTest?.outcome === 'failed'} />
						</div>
					</div>
				</td>
			</tr>
		{/if}
	{/snippet}
</DataTable>

<style>
	tr.opened td {
		border-bottom-color: transparent;
	}
	tr.expansion,
	tr.expansion:hover {
		background: transparent;
		cursor: default;
	}
	tr.expansion > td {
		padding: 4px 20px 20px 40px !important;
	}
	.expanded {
		display: grid;
		grid-template-columns: minmax(280px, 420px) minmax(0, 780px);
		gap: 20px 48px;
	}
	@media (max-width: 1500px) {
		.expanded {
			grid-template-columns: minmax(0, 1fr);
		}
	}
	.facts {
		display: grid;
		grid-template-columns: minmax(0, 1fr);
		align-items: start;
		gap: 14px;
		align-content: start;
	}
	@media (max-width: 1500px) {
		.facts {
			grid-template-columns: repeat(2, minmax(0, 360px));
			gap: 0 40px;
		}
	}
	.facts :global(.key-values) {
		grid-template-columns: 116px minmax(0, 1fr);
	}
	.sync {
		display: flex;
		min-width: 0;
		flex-direction: column;
		gap: 6px;
	}
	h3 {
		font-size: 12.5px;
		font-weight: 600;
	}
	.test {
		display: flex;
		justify-content: flex-end;
	}
	.link {
		color: var(--primary);
	}
	.link:hover {
		text-decoration: underline;
	}
</style>
