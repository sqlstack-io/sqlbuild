<script lang="ts">
	import { to } from '$lib/paths';
	import type { CatalogNodeDetail, NodeHistory } from '$lib/api';
	import Panel from '$lib/components/platform/panel.svelte';
	import StatusLabel from '$lib/components/list/status-label.svelte';
	import { app } from '$lib/state/app.svelte';
	import { formatAgo, formatStamp } from '$lib/format/time';
	import { formatRelation } from '$lib/platform/labels';
	import { decisionLabel, migrationKind, oldNameStatus, oldNameStatusLabel, oldNameStatusTone, oldNameStepLabel } from '$lib/platform/maintenance';

	// Renames and moves of this node, from synced `_sqlbuild_migrations` and
	// `_sqlbuild_old_name_views`; the Maintenance page shows them for every node.
	let { node, history: h }: { node: CatalogNodeDetail; history: NodeHistory } = $props();
</script>

<div class="wrap">
	{#if h.formerNames.length}
		<p class="formerly">Formerly known as {#each h.formerNames as f, i (f)}{i ? ', ' : ''}<code>{f}</code>{/each}</p>
	{/if}

	<Panel title="Renames and moves" note={h.migrations.length ? `${h.migrations.length} recorded in this location` : undefined}>
		{#if h.migrations.length}
			<table class="sb-list">
				<thead><tr><th>When</th><th>Change</th><th>From</th><th>To</th><th>What happened</th><th>Run</th></tr></thead>
				<tbody>
					{#each h.migrations as m (m.eventId)}
						<tr>
							<td><span title={formatStamp(m.createdAt)}>{formatAgo(m.createdAt, app.now)}</span></td>
							<td class="capitalize">{migrationKind(m)} <span class="muted">· {m.discovery}</span></td>
							<td>{#if m.originModel}<code>{m.originModel}</code><div class="cell-desc">{formatRelation(m.origin)}</div>{:else}<code>{formatRelation(m.origin)}</code><div class="cell-desc">found in the warehouse</div>{/if}</td>
							<td><code>{m.destinationModel}</code><div class="cell-desc">{formatRelation(m.destination)}</div></td>
							<td>{decisionLabel[m.decision]}</td>
							<td><a class="mono link" href={to(`/runs/${m.runId}`)}>{m.runId}</a></td>
						</tr>
					{/each}
				</tbody>
			</table>
		{:else}
			<p class="muted">No renames or moves recorded for {node.key} in this location.</p>
		{/if}
	</Panel>

	{#if h.oldNameViews.length}
		<Panel title="Views under the old name" note="Kept so queries that use the old name keep working">
			<table class="sb-list">
				<thead><tr><th>Old name</th><th>Status</th><th>Kept for</th><th>Renamed columns</th><th>History</th></tr></thead>
				<tbody>
					{#each h.oldNameViews as v (v.migrationEventId)}
						{@const status = oldNameStatus(v, app.now)}
						<tr>
							<td><code>{formatRelation(v.old)}</code></td>
							<td>
								<StatusLabel tone={oldNameStatusTone[status]} label={oldNameStatusLabel[status]} />
								{#if v.expiresAt}<div class="cell-desc">{Date.parse(v.expiresAt) > app.now ? 'expires' : 'expired'} {formatStamp(v.expiresAt)}</div>{/if}
								{#if v.dropReason}<div class="cell-desc">{v.dropReason}</div>{/if}
							</td>
							<td class="muted">{v.retention === 'false' ? 'not kept' : v.retention}</td>
							<td class="muted">{v.columnAliases.length ? v.columnAliases.map((a) => `${a.old} → ${a.new}`).join(', ') : '—'}</td>
							<td class="muted">{v.steps.map((s) => oldNameStepLabel[s.eventType]).join(' → ')}</td>
						</tr>
					{/each}
				</tbody>
			</table>
		</Panel>
	{/if}
	<p class="muted">All renames, moves and archived relations are on the <a class="link" href={to(`/maintenance`)}>Maintenance</a> page.</p>
</div>

<style>
	.wrap {
		padding: 20px 24px 48px;
	}
	.formerly {
		margin: 0 0 18px;
		font-size: 13px;
	}
	code,
	.mono {
		font: 12px var(--font-mono);
	}
	.muted {
		color: var(--muted-foreground);
		font-size: 12.5px;
	}
	.link {
		color: var(--primary);
	}
</style>
