<script lang="ts">
	import { to } from '$lib/paths';
	import type { CatalogNodeDetail, NodeBuildCost } from '$lib/api';
	import { CREDIT_PRICE_HINT, formatQueryCost } from '$lib/operate/cost';
	import CostChart from './cost-chart.svelte';
	import StatusLabel from '$lib/components/list/status-label.svelte';
	import RelativeTime from '$lib/components/list/relative-time.svelte';
	import { app } from '$lib/state/app.svelte';
	import { formatAgo, formatStamp } from '$lib/format/time';
	import { nodeOutcomeLabel, nodeOutcomeTone } from '$lib/status';
	import { formatMs, formatNumber, shortRunId } from '$lib/build-area/labels';

	// Build attempts of this node: scheduler run nodes plus builds outside the
	// scheduler, which are known only by their fingerprint run and time.
	// Estimated query cost appears only where the adapter collects it.
	let { node, costs, costDays }: { node: CatalogNodeDetail; costs: NodeBuildCost[]; costDays: number } = $props();
	const costOf = (r: NonNullable<CatalogNodeDetail['runs']>[number]) => costs.find((c) => (r.runId !== null && c.runId === r.runId) || c.buildRunId === r.fingerprintRunId);
	const costText = (c: NodeBuildCost | undefined) => (!c ? '—' : c.status === 'pending' ? 'pending' : c.status === 'collection_failed' || c.status === 'unavailable' ? 'unavailable' : formatQueryCost(c));
</script>

{#if costs.length}
	<CostChart {costs} days={costDays} />
	{#if costs.some((c) => c.usd === null)}<p class="hint">{CREDIT_PRICE_HINT}</p>{/if}
{/if}

<table class="sb-list">
	<thead><tr><th>Outcome</th><th>Run</th><th>Job</th><th class="num">Rows</th><th class="num">Duration</th>{#if costs.length}<th class="num" title="Estimated query cost of this node in the build">Est. cost</th>{/if}<th>Started</th></tr></thead>
	<tbody>
		{#each node.runs ?? [] as r, i (`${r.runId ?? r.fingerprintRunId}-${i}`)}
			<tr>
				<td>
					<StatusLabel tone={nodeOutcomeTone[r.outcome]} label={nodeOutcomeLabel[r.outcome]} />
					{#if r.message}<div class="detail" style:color={r.outcome === 'failed' ? 'var(--sb-error)' : 'var(--muted-foreground)'}>{r.message}</div>{/if}
				</td>
				<td>
					{#if r.runId}<a class="mono link" href={to(`/runs/${r.runId}`)}>{r.runId}</a>
					{:else}<span class="muted" title="Built by sqb outside this server, e.g. from a laptop or CI (fingerprint run {r.fingerprintRunId})">from the command line · <code>{shortRunId(r.fingerprintRunId ?? '')}</code></span>{/if}
				</td>
				<td>{#if r.jobKey}<a class="link" href={to(`/jobs/${r.jobKey}`)}>{r.jobKey}</a>{:else}<span class="muted">{r.runId ? 'started by hand' : '—'}</span>{/if}</td>
				<td class="num muted">{formatNumber(r.rows)}</td>
				<td class="num muted">{formatMs(r.durationMs)}</td>
				{#if costs.length}{@const c = costOf(r)}<td class="num muted" title={c?.status === 'partial' ? 'Partial: the warehouse has not reported every statement of this build' : undefined}>{costText(c)}{c?.status === 'partial' ? '*' : ''}</td>{/if}
				<td><RelativeTime value={formatAgo(r.startedAt, app.now)} exact={formatStamp(r.startedAt)} /></td>
			</tr>
		{:else}
			<tr><td colspan={costs.length ? 7 : 6} class="empty">No builds recorded in this location.</td></tr>
		{/each}
	</tbody>
</table>

<style>
	.mono,
	code {
		font: 12px var(--font-mono);
	}
	.link {
		color: var(--primary);
	}
	.muted {
		color: var(--muted-foreground);
	}
	.hint {
		margin: 0 0 10px;
		color: var(--sb-text-faint);
		font-size: 12px;
	}
	.detail {
		margin-top: 3px;
		font-size: 12px;
	}
</style>
