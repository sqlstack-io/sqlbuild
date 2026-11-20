<script lang="ts">
	// One provider whose missing secrets stop nodes, or every run through a sink: what it needs, what it stops and how to fix it.
	// The plan page says Can't run; a run that hit it says Couldn't run.
	import type { PlanCantRun } from '$lib/api';
	import { to } from '$lib/paths';
	import { fixText, needsText, variablesText } from './cant-run';

	let { entry, then = 'plan again', title = "Can't run:" }: { entry: PlanCantRun; then?: string; title?: string } = $props();
</script>

<div class="problem" data-testid="cant-run-problem">
	<div class="head">
		<span class="mark" aria-hidden="true">✗</span><b>{title}</b>
		<span>provider <a href={to(`/providers/${encodeURIComponent(entry.provider)}`)} class="provider">{entry.provider}</a> needs <code>{variablesText(entry)}</code></span>
		<span class="tag">missing secret</span>
	</div>
	<dl>
		{#if entry.nodes.length}
			<dt>needs</dt>
			<dd>{needsText(entry)}</dd>
		{/if}
		{#if entry.sinks?.length}
			<dt>sinks</dt>
			<dd data-testid="cant-run-sinks">{entry.sinks.join(', ')}: every run uses sinks, so nothing can run</dd>
		{/if}
		<dt>fix</dt>
		<dd>{fixText(entry, then)}</dd>
	</dl>
</div>

<style>
	.problem {
		border: 1px solid color-mix(in srgb, var(--sb-error) 40%, var(--border));
		border-left: 3px solid var(--sb-error);
		border-radius: 2px;
		padding: 10px 14px;
		background: color-mix(in srgb, var(--sb-error) 5%, transparent);
		font-size: 12.5px;
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
</style>
