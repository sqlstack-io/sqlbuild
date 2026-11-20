<script lang="ts">
	import Button from '$lib/components/ui/button/button.svelte';
	import { plural } from '$lib/format/plural';
	import type { RunDetailState } from './state.svelte';
	import type { NodeGroupKey } from './types';

	// What the run did, in one line (CHI-1591, change 1): problems first, then what it built. Each figure
	// filters the timeline and the log to its nodes; picking it again shows everything. A figure that is
	// zero is left out, and with it its separator.
	let { view }: { view: RunDetailState } = $props();

	const s = $derived(view.summary);
	const rows = (n: number) => n.toLocaleString('en-GB');
	type Fig = { key: NodeGroupKey; text: string; tone?: 'err' | 'warn' | 'run'; note?: string };
	const figs = $derived.by((): Fig[] => {
		const out: (Fig | null)[] = [
			s.failed ? { key: 'failed', text: `${s.failed} failed`, tone: 'err' } : null,
			s.skipped ? { key: 'skipped', text: `${s.skipped} skipped` } : null,
			s.notRun ? { key: 'not_run', text: `${s.notRun} not run` } : null,
			s.running ? { key: 'running', text: `${s.running} running`, tone: 'run' } : null,
			s.waiting ? { key: 'waiting', text: `${s.waiting} waiting` } : null,
			s.built ? { key: 'succeeded', text: `${s.built} built`, note: s.incremental ? `(${plural(s.incremental, 'incremental')}, +${rows(s.incrementalRows)} rows)` : undefined } : null,
			s.warned ? { key: 'warned', text: plural(s.warned, 'warning'), tone: 'warn' } : null
		];
		return out.filter((f): f is Fig => !!f);
	});
</script>

<div class="summary" data-testid="run-summary">
	{#each figs as f, i (f.key)}
		{#if i}<span class="sep">·</span>{/if}
		<Button variant="link" size="inline" class="fig {f.tone ?? ''} {view.statusFilter === f.key ? 'on' : ''}" onclick={() => view.focusGroup(f.key, f.text.replace(/^\d+ /, ''))} title="Show only these on the timeline and in the log">{f.text}</Button>
		{#if f.note}<span class="faint">{f.note}</span>{/if}
	{:else}
		<span class="faint">{view.active ? 'Starting…' : 'No nodes ran.'}</span>
	{/each}
</div>

<style>
	.summary {
		display: flex;
		flex-wrap: wrap;
		align-items: baseline;
		gap: 0 5px;
		font-size: 12.5px;
	}
	.summary :global(.fig) {
		font-size: 12.5px;
	}
	.summary :global(.fig.on) {
		font-weight: 600;
		text-decoration: underline;
	}
	.summary :global(.fig.err) {
		color: var(--sb-error);
	}
	.summary :global(.fig.warn) {
		color: var(--sb-warning);
	}
	.summary :global(.fig.run) {
		color: var(--sb-running);
	}
	.sep,
	.faint {
		color: var(--sb-text-faint);
	}
</style>
