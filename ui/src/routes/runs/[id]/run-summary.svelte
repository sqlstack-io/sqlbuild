<script lang="ts">
	import Button from '$lib/components/ui/button/button.svelte';
	import { plural } from '$lib/format/plural';
	import { to } from '$lib/paths';
	import { runStatusLabel, runStatusTone } from '$lib/status';
	import type { RunDetailState } from './state.svelte';
	import type { NodeGroupKey } from './types';
	import { costSummary } from './utils';

	// The run in one line (CHI-1591, change 1): what it built, with the rows incrementals added, what
	// failed and was skipped, its checks and cost. Each figure narrows the timeline and the log to its
	// nodes; picking it again shows everything.
	let { view }: { view: RunDetailState } = $props();

	const s = $derived(view.summary);
	const picked = (key: NodeGroupKey) => view.statusFilter === key;
	const rows = (n: number) => n.toLocaleString('en-GB');
	const members = $derived(view.detail?.children ?? []);
	const memberLabel = (status: string) => (status === 'skipped' ? 'not started' : runStatusLabel[status as keyof typeof runStatusLabel]?.toLowerCase() ?? status);
</script>

<div class="summary" data-testid="run-summary">
	{#if s.running}<span class="fig"><i style:background="var(--sb-running)"></i>{s.running} running</span><span class="sep">·</span>{/if}
	{#if s.waiting}<span class="fig faint">{s.waiting} waiting</span><span class="sep">·</span>{/if}
	<Button variant="link" size="inline" class="fig {picked('succeeded') ? 'on' : ''}" onclick={() => view.focusGroup('succeeded', 'built')} disabled={!s.built}>{s.built} built</Button>
	{#if s.incremental}<span class="faint">&nbsp;({plural(s.incremental, 'incremental')}, +{rows(s.incrementalRows)} rows)</span>{/if}
	{#if s.warned}<span class="faint">&nbsp;· {s.warned} with warnings</span>{/if}
	{#if s.failed}<span class="sep">·</span><Button variant="link" size="inline" class="fig err {picked('failed') ? 'on' : ''}" onclick={() => view.focusGroup('failed', 'failed')}>{s.failed} failed</Button>{/if}
	{#if s.skipped}<span class="sep">·</span><Button variant="link" size="inline" class="fig {picked('skipped') ? 'on' : ''}" onclick={() => view.focusGroup('skipped', 'skipped')}>{s.skipped} skipped</Button>{/if}
	{#if s.notRun}<span class="sep">·</span><Button variant="link" size="inline" class="fig {picked('not_run') ? 'on' : ''}" onclick={() => view.focusGroup('not_run', 'not run')}>{s.notRun} not run</Button>{/if}
	<span class="sep">·</span>
	<Button variant="link" size="inline" class="fig" onclick={() => (view.tab = 'log')} title="Checks of the nodes this run built; the log lists each result">
		{plural(s.checksPassed, 'check')} passed{#if s.checksWarned}, <span class="warn">{s.checksWarned} warned</span>{/if}{#if s.checksFailed}, <span class="err">{s.checksFailed} failed</span>{/if}
	</Button>
	<span class="sep">·</span>
	{#if view.cost && view.cost.status !== 'none'}
		<Button variant="link" size="inline" class="fig" onclick={() => (view.tab = 'cost')}>{costSummary(view.cost).text.toLowerCase()}</Button>
	{:else}
		<span class="faint">{view.active ? 'cost when it finishes' : 'cost pending'}</span>
	{/if}
</div>
{#if members.length}
	<!-- A group run's members, in their order, each linking to its own run. -->
	<div class="members" data-testid="run-members">
		<span class="faint">Member jobs</span>
		{#each members as m, i (m.id)}
			{#if i}<span class="arrow">→</span>{/if}
			<a href={to(`/runs/${m.id}`)} title="{m.jobKey}: {memberLabel(m.status)}{m.statusReason ? ` · ${m.statusReason}` : ''}"><i style:background={m.status === 'skipped' ? 'transparent' : runStatusTone[m.status]} class:hollow={m.status === 'skipped'}></i>{m.jobKey}</a>
			<span class="faint state">{memberLabel(m.status)}</span>
		{/each}
	</div>
{/if}

<style>
	.summary,
	.members {
		display: flex;
		flex-wrap: wrap;
		align-items: baseline;
		gap: 0 4px;
		margin-top: 6px;
		font-size: 12.5px;
	}
	.members {
		align-items: center;
		gap: 4px 6px;
	}
	.summary :global(.fig) {
		font-size: 12.5px;
	}
	.summary :global(.fig.on) {
		font-weight: 600;
		text-decoration: underline;
	}
	.summary :global(.err),
	.err {
		color: var(--sb-error);
	}
	.warn {
		color: var(--sb-warning);
	}
	.sep {
		color: var(--sb-text-faint);
	}
	.faint {
		color: var(--sb-text-faint);
	}
	.fig i,
	.members i {
		display: inline-block;
		width: 7px;
		height: 7px;
		margin-right: 5px;
		border-radius: 50%;
		vertical-align: 1px;
	}
	.members i.hollow {
		border: 1px solid var(--sb-text-faint);
	}
	.members a {
		font-family: var(--font-mono);
		font-size: 12px;
	}
	.members .state {
		font-size: 11.5px;
	}
	.arrow {
		color: var(--sb-text-faint);
	}
</style>
