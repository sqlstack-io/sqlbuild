<script lang="ts">
	import type { HealthComponentDetails, HealthStatus, ServerHealth } from '$lib/api';
	import { formatAgo, formatClock, formatStamp } from '$lib/format/time';
	import { app } from '$lib/state/app.svelte';

	/** `details` are administrators' only: where the database is, each cause, and the requests that failed. */
	let { report, details = null }: { report: ServerHealth; details?: HealthComponentDetails[] | null } = $props();

	const tone: Record<HealthStatus, string> = { healthy: 'var(--sb-success)', degraded: 'var(--sb-warning)', unavailable: 'var(--sb-error)' };
	const label: Record<HealthStatus, string> = { healthy: 'Healthy', degraded: 'Degraded', unavailable: 'Unavailable' };
	const detailOf = (component: string, locationId: string | null) => details?.find((d) => d.component === component && d.locationId === locationId) ?? null;
</script>

<table class="health" data-testid="health-table">
	<thead>
		<tr><th>Part</th><th>State</th><th>Since</th><th>Reason</th><th>Last worked</th></tr>
	</thead>
	<tbody>
		{#each report.components as part (`${part.component}:${part.locationId ?? ''}`)}
			{@const detail = detailOf(part.component, part.locationId)}
			<tr data-component={part.component}>
				<td>
					{part.name}
					{#if part.component === 'server'}<span class="faint">v{report.server.version}</span>{/if}
				</td>
				<td><span class="state"><i style:background={tone[part.status]}></i>{label[part.status]}</span></td>
				<td title={formatStamp(part.since)}>{part.since ? `${formatClock(part.since, app.now)} · ${formatAgo(part.since, app.now)}` : '—'}</td>
				<td class="reason">
					{part.reason ?? '—'}
					{#if detail && (detail.host || detail.errorChain.length || detail.requestIds.length)}
						<div class="detail" data-testid="health-detail">
							{#if detail.host}<div>Host <code>{detail.host}</code></div>{/if}
							{#each detail.errorChain as cause, i (i)}<div><code>{cause}</code></div>{/each}
							{#if detail.requestIds.length}<div>Requests {#each detail.requestIds as id (id)}<code>{id}</code> {/each}</div>{/if}
						</div>
					{/if}
				</td>
				<td title={formatStamp(part.lastSuccess)}>{part.component === 'server' ? `up ${formatAgo(report.server.startedAt, app.now).replace(/ ago$/, '')}` : part.lastSuccess ? formatAgo(part.lastSuccess, app.now) : 'Not yet'}</td>
			</tr>
		{/each}
	</tbody>
</table>

<style>
	.health {
		width: 100%;
		border-collapse: collapse;
		font-size: 13px;
		text-align: left;
	}
	th {
		padding: 6px 10px;
		border-bottom: 1px solid var(--border);
		color: var(--muted-foreground);
		font-size: 12px;
		font-weight: 500;
	}
	td {
		padding: 8px 10px;
		border-bottom: 1px solid var(--border);
		vertical-align: top;
	}
	.state {
		display: inline-flex;
		align-items: center;
		gap: 6px;
		white-space: nowrap;
	}
	.state i {
		display: inline-block;
		width: 8px;
		height: 8px;
		border-radius: 50%;
	}
	.reason {
		max-width: 46ch;
		overflow-wrap: anywhere;
	}
	.detail {
		margin-top: 4px;
		color: var(--muted-foreground);
		font-size: 12px;
	}
	.faint {
		margin-left: 6px;
		color: var(--sb-text-faint);
		font-size: 12px;
	}
</style>
