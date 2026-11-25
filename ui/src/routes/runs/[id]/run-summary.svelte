<script lang="ts">
	import { plural } from '$lib/format/plural';
	import type { RunDetailState } from './state.svelte';
	import type { NodeGroupKey } from './types';

	// What the run did, in one line: problems first, then what it built. Each figure
	// filters the timeline and the log to its nodes; picking it again shows everything. A figure that is
	// zero is left out, and with it its separator.
	let { view }: { view: RunDetailState } = $props();

	const s = $derived(view.summary);
	const rows = (n: number) => n.toLocaleString('en-GB');
	type Fig = { key: NodeGroupKey | 'checks_failed'; text: string; tone?: 'err' | 'warn' | 'run'; note?: string };
	const figs = $derived.by((): Fig[] => {
		const out: (Fig | null)[] = [
			s.failed ? { key: 'failed', text: `${s.failed} failed`, tone: 'err' } : null,
			s.skipped ? { key: 'skipped', text: `${s.skipped} skipped` } : null,
			s.notRun ? { key: 'not_run', text: `${s.notRun} not run` } : null,
			s.running ? { key: 'running', text: `${s.running} running`, tone: 'run' } : null,
			s.waiting ? { key: 'waiting', text: `${s.waiting} waiting` } : null,
			// The note sits only after its figure: no parenthesis without the count it explains.
			s.built ? { key: 'succeeded', text: `${s.built} built`, note: s.incremental ? `(${s.incremental} incremental, +${rows(s.incrementalRows)} rows)` : undefined } : null,
			s.warned ? { key: 'warned', text: plural(s.warned, 'warning'), tone: 'warn' } : null,
			// Failed checks count with the nodes: a run can fail on a check after every node built.
			s.checksFailed ? { key: 'checks_failed', text: `${plural(s.checksFailed, 'check')} failed`, tone: 'err' } : null
		];
		return out.filter((f): f is Fig => !!f);
	});
</script>

<div class="summary" data-testid="run-summary">
	{#each figs as f, i (f.key)}
		<!-- Separators and spaces are real text, so the line reads the same copied or read aloud. -->
		{#if i}<span class="sep">{' · '}</span>{/if}
		<!-- A real link: the figure reads as text anywhere, and filters the timeline and log. -->
		<span class="part"
			><a
				class="fig {f.tone ?? ''}"
				class:sb-pressed={view.figure === f.key}
				class:on={view.figure === f.key}
				aria-current={view.figure === f.key ? "true" : undefined}
				href="?show={f.key}"
				onclick={(ev) => {
					ev.preventDefault();
					if (f.key === 'checks_failed') view.toggleChecksFilter('failed checks');
					else view.focusGroup(f.key, f.text.replace(/^\d+ /, ''));
				}}
				title={view.figure === f.key ? 'Show everything again' : f.key === 'checks_failed' ? 'Show only the failed checks in the log' : 'Show only these on the timeline and in the log'}>{f.text}</a
			>{#if f.note}<span class="faint">{` ${f.note}`}</span>{/if}</span
		>
	{:else}
		<span class="faint">{view.active ? 'Starting…' : 'No nodes ran.'}</span>
	{/each}
</div>

<style>
	.summary {
		display: inline-flex;
		flex-wrap: wrap;
		align-items: baseline;
		gap: 0;
		white-space: pre-wrap;
		font-size: 12.5px;
	}
	/* Calm: a figure is plain text until hovered; only problems keep their status colour. */
	.fig {
		color: var(--foreground);
		font-size: 12.5px;
	}
	.fig:hover {
		text-decoration: underline;
	}
	.fig.on {
		padding: 0 5px;
		text-decoration: none;
	}
	.fig.err.on {
		--pressed-tone: var(--sb-error);
	}
	.fig.err {
		color: var(--sb-error);
	}
	.fig.warn {
		color: var(--sb-warning);
	}
	.fig.run {
		color: var(--sb-running);
	}
	.sep,
	.faint {
		color: var(--sb-text-faint);
	}
</style>
