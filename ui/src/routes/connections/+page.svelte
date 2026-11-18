<script lang="ts">
	import PageBody from '$lib/components/page-body.svelte';
	import DataTable from '$lib/components/list/data-table.svelte';
	import type { TableColumn } from '$lib/components/list/table-columns';
	import Button from '$lib/components/ui/button/button.svelte';
	import Loading from '$lib/components/loading.svelte';
	import { to } from '$lib/paths';
	import DatabaseIcon from '@lucide/svelte/icons/database';
	import DisclosureButton from '$lib/components/disclosure-button.svelte';
	import AppTopbar from '$lib/components/app-topbar.svelte';
	import RefreshButton from '$lib/components/refresh-button.svelte';
	import ConnectionTest from '$lib/components/platform/connection-test.svelte';
	import Panel from '$lib/components/platform/panel.svelte';
	import StatusLabel from '$lib/components/list/status-label.svelte';
	import { formatMs, formatNumber } from '$lib/build-area/labels';
	import { formatAgo, formatStamp } from '$lib/format/time';
	import { testTone } from '$lib/platform/labels';
	import { detailRows, failingTables, groupConnections, lastCheckLine, syncSummary } from '$lib/platform/connections';
	import { app } from '$lib/state/app.svelte';
	import { createResource } from '$lib/state/resource.svelte';

	const connections = createResource((client) => client.listWarehouseConnections(), ['warehouse_sync']);
	const groups = $derived(groupConnections(connections.data ?? []));
	// Each target's per-table sync list starts collapsed under its summary.
	let openSyncs = $state<Record<string, boolean>>({});

	const where = (s: { database: string | null; schema: string }) => (s.database ? `${s.database}.${s.schema}` : s.schema);
	const userColumns: TableColumn[] = [
		{ id: 'by', label: 'Used by', minWidth: 180 },
		{ id: 'state', label: 'State tables', width: '30%' },
		{ id: 'check', label: 'Last check', width: '200px' },
		{ id: 'test', label: '', width: '150px', truncate: false }
	];
	const syncColumns: TableColumn[] = [
		{ id: 'table', label: 'Table', minWidth: 180 },
		{ id: 'newest', label: 'Newest sync', width: '180px' },
		{ id: 'total', label: 'Mirrored total', width: '120px', align: 'right' },
		{ id: 'added', label: 'Last sync added', width: '130px', align: 'right' },
		{ id: 'took', label: 'Took', width: '80px', align: 'right' },
		{ id: 'clean', label: 'Last clean sync', width: '130px' }
	];
</script>

<AppTopbar title="Connections">
	<RefreshButton onclick={() => connections.refresh()} />
</AppTopbar>

<PageBody padding="20px 24px 48px">
	{#if connections.error}
		<p class="err">Couldn't load connections: {connections.error} <Button variant="link" size="inline" class="ml-2" onclick={() => connections.refresh()}>Retry</Button></p>
	{/if}

	{#each groups as g (g.key)}
		<Panel title="Connection" note={g.details?.adapter}>
			<div class="card">
				<div class="card-head">
					<DatabaseIcon size={15} strokeWidth={1.75} />
					<span class="name">{g.users.length === 1 ? `${g.users[0].location} · ${g.users[0].target}` : `${g.users.length} targets`}</span>
				</div>
				<dl class="details" class:pending={!g.details}>
					{#each detailRows(g.details) as row (row.label)}
						<div><dt>{row.label}</dt><dd class:faint={row.value === null}>{row.value ?? '—'}</dd></div>
					{/each}
				</dl>
				{#if !g.details}<p class="note" title="Secrets are never shown">Settings not published</p>{/if}
				<DataTable class="users static" embedded columns={userColumns} rows={g.users} key={(c) => c.id} empty="No location uses this connection.">
					{#snippet row(c)}
							<tr>
								<td><a href={to(`/locations/${encodeURIComponent(c.id)}`)} class="link">{c.location} · {c.target}</a></td>
								<td>{#if c.stateLocation}<code>{where(c.stateLocation)}</code>{:else}<span class="faint">Not reported yet</span>{/if}</td>
								<td>
									{#if c.lastTest}
										<span title="{formatStamp(c.lastTest.at)} · {c.lastTest.message}"><StatusLabel tone={testTone[c.lastTest.outcome]} label={lastCheckLine(c.lastTest)} /></span>
									{:else}
										<span class="faint">Not checked yet</span>
									{/if}
								</td>
								<td class="test"><ConnectionTest scope={{ locationId: c.id }} connection={c.target} last={c.lastTest} showLast={false} ontested={() => connections.refresh()} /></td>
							</tr>
					{/snippet}
				</DataTable>
			</div>
		</Panel>

		{#each g.users as c (c.id)}
			{@const failing = failingTables(c.tables)}
			<Panel title="State history sync" note={g.users.length > 1 ? `${c.location} · ${c.target}` : undefined}>
				<div class="card">
					{#if c.tables.length}
						<DisclosureButton tone="default" open={!!openSyncs[c.id]} onclick={() => (openSyncs[c.id] = !openSyncs[c.id])} class="w-full gap-2 text-[12.5px]">
							<span>{syncSummary(c, app.now)}</span>
							{#if failing}<StatusLabel tone="var(--sb-error)" label="{failing} failing" />{/if}
						</DisclosureButton>
						{#if openSyncs[c.id]}
							<DataTable class="static" embedded columns={syncColumns} rows={c.tables} key={(t) => t.table} empty="No state tables reported.">
								{#snippet row(t)}
										<tr>
											<td><code>{t.table}</code></td>
											<td>
												<span title={formatStamp(t.at)}>
													{#if t.failure}<StatusLabel tone={t.waiting ? 'var(--sb-warning)' : 'var(--sb-error)'} label={t.failure} />{:else}<StatusLabel tone="var(--sb-success)" label="synced {formatAgo(t.at, app.now)}" />{/if}
												</span>
												{#if t.error}<p class="sync-error">{t.error}</p>{/if}
											</td>
											<td class="num" class:faint={t.mirroredTotal === null} title={t.mirroredTotal === null ? 'Not reported yet' : undefined}>{formatNumber(t.mirroredTotal)}</td>
											<td class="num">{formatNumber(t.recordsSent)}</td>
											<td class="num">{formatMs(t.elapsedMs)}</td>
											<td class="faint" title={t.lastSyncedAt ? formatStamp(t.lastSyncedAt) : undefined}>{t.lastSyncedAt ? formatAgo(t.lastSyncedAt, app.now) : 'never'}</td>
										</tr>
								{/snippet}
							</DataTable>
						{/if}
					{:else}
						<p class="muted">No warehouse sync reported yet.</p>
					{/if}
				</div>
			</Panel>
		{/each}
	{:else}
		{#if connections.data}<p class="muted" title="Only locations you can see in full are listed">Nothing reported yet.</p>{:else if !connections.error}<Loading />{/if}
	{/each}
</PageBody>

<style>
	.err {
		margin-bottom: 16px;
		color: var(--sb-error);
		font-size: 12.5px;
	}
	.muted {
		color: var(--muted-foreground);
		font-size: 12.5px;
	}
	.sync-error {
		max-width: 60ch;
		margin: 4px 0 0 15px;
		color: var(--muted-foreground);
		font-size: 12px;
		white-space: normal;
		overflow-wrap: anywhere;
	}
	.faint {
		color: var(--sb-text-faint);
	}
	.card {
		display: flex;
		flex-direction: column;
		gap: 12px;
		padding: 14px 16px;
		border: 1px solid var(--panel-border, var(--border));
		border-radius: 2px;
		background: var(--card);
		box-shadow: var(--panel-shadow);
	}
	.card-head {
		display: flex;
		align-items: center;
		gap: 9px;
	}
	.card-head :global(svg) {
		color: var(--muted-foreground);
	}
	.name {
		font-size: 13px;
		font-weight: 500;
	}
	.details {
		display: grid;
		grid-template-columns: repeat(auto-fill, minmax(170px, 1fr));
		gap: 10px 20px;
		margin: 0;
	}
	.details dt {
		color: var(--muted-foreground);
		font-size: 11.5px;
	}
	.details dd {
		margin: 2px 0 0;
		overflow: hidden;
		font: 12.5px var(--font-mono);
		text-overflow: ellipsis;
		white-space: nowrap;
	}
	.details.pending dd {
		font-family: var(--font-sans);
	}
	.note {
		margin: -4px 0 0;
		color: var(--sb-text-faint);
		font-size: 12px;
	}
	.link:hover {
		color: var(--primary);
	}
	.num {
		text-align: right;
		font-variant-numeric: tabular-nums;
	}
	code {
		font: 12px var(--font-mono);
	}
</style>
