<script lang="ts">
	import type { CatalogNodeDetail } from '$lib/api';
	import StatusLabel from '$lib/components/list/status-label.svelte';
	import RelativeTime from '$lib/components/list/relative-time.svelte';
	import { app } from '$lib/state/app.svelte';
	import { formatAgo, formatStamp } from '$lib/format/time';
	import { nodeOutcomeLabel, nodeOutcomeTone } from '$lib/status';
	import { formatMs, formatNumber, shortRunId } from '$lib/build-area/labels';

	// Build attempts of this node: scheduler run nodes plus builds outside the
	// scheduler, which are known only by their fingerprint run and time.
	let { node }: { node: CatalogNodeDetail } = $props();
</script>

<table class="sb-list">
	<thead><tr><th>Outcome</th><th>Run</th><th>Job</th><th class="num">Rows</th><th class="num">Duration</th><th>Started</th></tr></thead>
	<tbody>
		{#each node.runs ?? [] as r, i (`${r.runId ?? r.fingerprintRunId}-${i}`)}
			<tr>
				<td>
					<StatusLabel tone={nodeOutcomeTone[r.outcome]} label={nodeOutcomeLabel[r.outcome]} />
					{#if r.message}<div class="detail" style:color={r.outcome === 'failed' ? 'var(--sb-error)' : 'var(--muted-foreground)'}>{r.message}</div>{/if}
				</td>
				<td>
					{#if r.runId}<a class="mono link" href="/runs/{r.runId}">{r.runId}</a>
					{:else}<span class="muted" title="Built by sqb outside this server, e.g. from a laptop or CI (fingerprint run {r.fingerprintRunId})">from the command line · <code>{shortRunId(r.fingerprintRunId ?? '')}</code></span>{/if}
				</td>
				<td>{#if r.jobKey}<a class="link" href="/jobs/{r.jobKey}">{r.jobKey}</a>{:else}<span class="muted">{r.runId ? 'started by hand' : '—'}</span>{/if}</td>
				<td class="num muted">{formatNumber(r.rows)}</td>
				<td class="num muted">{formatMs(r.durationMs)}</td>
				<td><RelativeTime value={formatAgo(r.startedAt, app.now)} exact={formatStamp(r.startedAt)} /></td>
			</tr>
		{:else}
			<tr><td colspan="6" class="empty">No builds recorded in this location.</td></tr>
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
	.detail {
		margin-top: 3px;
		font-size: 12px;
	}
</style>
