<script lang="ts">
	import Loading from '$lib/components/loading.svelte';
	import { to } from '$lib/paths';
	import DatabaseIcon from '@lucide/svelte/icons/database';
	import AppTopbar from '$lib/components/app-topbar.svelte';
	import Button from '$lib/components/ui/button/button.svelte';
	import ConnectionTest from '$lib/components/platform/connection-test.svelte';
	import Panel from '$lib/components/platform/panel.svelte';
	import StatusLabel from '$lib/components/list/status-label.svelte';
	import { formatMs, formatNumber } from '$lib/build-area/labels';
	import { formatAgo, formatStamp } from '$lib/format/time';
	import { app } from '$lib/state/app.svelte';
	import { createResource } from '$lib/state/resource.svelte';

	const connections = createResource((client) => client.listWarehouseConnections(), ['warehouse_sync']);
	const list = $derived(connections.data ?? []);

	const where = (s: { database: string | null; schema: string }) => (s.database ? `${s.database}.${s.schema}` : s.schema);
</script>

<AppTopbar title="Connections">
	<Button variant="outline" size="sm" onclick={() => connections.refresh()}>↻ Refresh</Button>
</AppTopbar>

<div class="min-h-0 flex-1 overflow-auto px-6 pt-5 pb-12">
	{#if connections.error}
		<p class="err">Couldn't load connections: {connections.error} <button class="retry" onclick={() => connections.refresh()}>Retry</button></p>
	{/if}

	{#each list as c (c.id)}
		<Panel title="{c.location} · {c.target}" note={c.stateLocation ? `State tables in ${where(c.stateLocation)}` : undefined}>
			<div class="connection">
				<div class="head">
					<DatabaseIcon size={15} strokeWidth={1.75} />
					<a href={to(`/locations/${encodeURIComponent(c.id)}`)} class="name">Warehouse</a>
					<div class="test"><ConnectionTest scope={{ locationId: c.id }} connection={c.target} last={c.lastTest} ontested={() => connections.refresh()} /></div>
				</div>
				{#if c.tables.length}
					<table class="sb-list">
						<thead><tr><th>Synced table</th><th>Newest sync</th><th>Records</th><th>Took</th><th>Last clean sync</th></tr></thead>
						<tbody>
							{#each c.tables as t (t.table)}
								<tr>
									<td><code>{t.table}</code></td>
									<td>
										<span title={formatStamp(t.at)}>
											{#if t.failure}<StatusLabel tone={t.waiting ? 'var(--sb-warning)' : 'var(--sb-error)'} label={t.failure} />{:else}<StatusLabel tone="var(--sb-success)" label="synced {formatAgo(t.at, app.now)}" />{/if}
										</span>
									</td>
									<td>{formatNumber(t.recordsSent)}</td>
									<td>{formatMs(t.elapsedMs)}</td>
									<td class="muted" title={t.lastSyncedAt ? formatStamp(t.lastSyncedAt) : undefined}>{t.lastSyncedAt ? formatAgo(t.lastSyncedAt, app.now) : 'never'}</td>
								</tr>
							{/each}
						</tbody>
					</table>
				{:else}
					<p class="muted">No warehouse sync reported yet.</p>
				{/if}
			</div>
		</Panel>
	{:else}
		{#if connections.data}<p class="muted">No location you can see in full has reported yet.</p>{:else if !connections.error}<Loading />{/if}
	{/each}
</div>

<style>
	.err {
		margin-bottom: 16px;
		color: var(--sb-error);
		font-size: 12.5px;
	}
	.retry {
		margin-left: 8px;
		color: var(--primary);
	}
	.muted {
		color: var(--muted-foreground);
		font-size: 12.5px;
	}
	.connection {
		display: flex;
		flex-direction: column;
		gap: 12px;
		margin-bottom: 18px;
	}
	.head {
		display: flex;
		align-items: center;
		gap: 9px;
	}
	.head :global(svg) {
		color: var(--muted-foreground);
	}
	.name {
		font-size: 13px;
		font-weight: 500;
	}
	.name:hover {
		color: var(--primary);
	}
	.test {
		flex: 1;
		min-width: 0;
		margin-left: 12px;
	}
	code {
		font: 12px var(--font-mono);
	}
</style>
