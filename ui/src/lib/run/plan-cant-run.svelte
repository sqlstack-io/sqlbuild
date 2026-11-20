<script lang="ts">
	// The nodes a plan can't run because a provider they use is missing secrets: one problem block per
	// provider (what it needs, what it stops, how to fix it), then a row per node with why it can't run.
	import type { PlanCantRun } from '$lib/api';
	import { to } from '$lib/paths';
	import { cantRunRows, fixText, keyName, needsText, variablesText } from './cant-run';

	/** `kindOf` and `upstreamOf` read the catalog; `then` ends the fix, such as "plan again". */
	let {
		entries,
		kindOf = () => '',
		upstreamOf = () => [],
		then = 'plan again'
	}: {
		entries: PlanCantRun[];
		kindOf?: (key: string) => string;
		upstreamOf?: (key: string) => string[];
		then?: string;
	} = $props();

	const rows = $derived(cantRunRows(entries, upstreamOf));
	const href = (key: string) => to(`/catalog/${key}`);
</script>

<div class="cant-run" data-testid="plan-cant-run">
	{#each entries as e (e.provider)}
		<div class="problem" data-testid="cant-run-problem">
			<div class="head">
				<span class="mark" aria-hidden="true">✗</span><b>Can't run:</b>
				<span>provider <a href={to(`/providers/${encodeURIComponent(e.provider)}`)} class="provider">{e.provider}</a> needs <code>{variablesText(e)}</code></span>
				<span class="tag">missing secret</span>
			</div>
			<dl>
				<dt>needs</dt>
				<dd>{needsText(e)}</dd>
				<dt>fix</dt>
				<dd>{fixText(e, then)}</dd>
			</dl>
		</div>
	{/each}
	<table class="nodes">
		<thead><tr><th class="st"></th><th>Node</th><th>Kind</th><th>Why it can't run</th></tr></thead>
		<tbody>
			{#each rows as r (r.key)}
				<tr class:uses={r.uses}>
					<td class="st"><span class="state" class:hollow={!r.uses} aria-label={r.uses ? 'uses the provider' : 'reads a node that can’t run'}>{r.uses ? '✕' : '!'}</span></td>
					<td><a href={href(r.key)} class="mono">{keyName(r.key)}</a></td>
					<td class="faint">{kindOf(r.key)}</td>
					<td>{r.why}</td>
				</tr>
			{/each}
		</tbody>
	</table>
</div>

<style>
	.cant-run {
		display: flex;
		flex-direction: column;
		gap: 12px;
	}
	.problem {
		border: 1px solid color-mix(in srgb, var(--sb-error) 40%, var(--border));
		border-left: 3px solid var(--sb-error);
		border-radius: 2px;
		padding: 10px 14px;
		background: color-mix(in srgb, var(--sb-error) 5%, transparent);
	}
	.head {
		display: flex;
		flex-wrap: wrap;
		align-items: baseline;
		gap: 6px;
	}
	.mark {
		color: var(--sb-error);
		font-weight: 600;
	}
	.provider {
		font-weight: 600;
	}
	.tag {
		margin-left: auto;
		padding: 0 6px;
		border: 1px solid color-mix(in srgb, var(--sb-error) 50%, var(--border));
		border-radius: 2px;
		color: var(--sb-error);
		font-size: 11px;
	}
	dl {
		display: grid;
		grid-template-columns: max-content minmax(0, 1fr);
		gap: 3px 14px;
		margin: 8px 0 0;
	}
	dt {
		color: var(--muted-foreground);
	}
	dd {
		margin: 0;
	}
	code {
		font-family: var(--font-mono);
		font-size: 11.5px;
	}
	.nodes {
		width: 100%;
		border-collapse: collapse;
	}
	.nodes th {
		padding: 4px 8px;
		border-bottom: 1px solid var(--border);
		color: var(--muted-foreground);
		font-weight: 500;
		text-align: left;
	}
	.nodes td {
		padding: 5px 8px;
		border-bottom: 1px solid var(--border-subtle, var(--border));
	}
	.nodes .st {
		width: 34px;
	}
	.state {
		display: inline-grid;
		width: 16px;
		height: 16px;
		place-items: center;
		border-radius: 2px;
		background: var(--sb-error);
		color: var(--background);
		font-size: 10px;
		font-weight: 700;
	}
	.state.hollow {
		border: 1px solid var(--sb-error);
		background: transparent;
		color: var(--sb-error);
	}
	.mono {
		font-family: var(--font-mono);
	}
	.faint {
		color: var(--muted-foreground);
	}
</style>
