<script lang="ts">
	import { plural } from '$lib/format/plural';
	import type { RunDetailState } from './state.svelte';
	import { QUICK, type QuickFilter } from './utils/log-query';

	// What the run did, in one line: problems first, then what it built. Each figure writes its words into
	// the log's filter (`outcome:built`, `kind:audit outcome:failed`…) and takes them out again; it reads as
	// pressed whenever the filter holds them, typed or clicked. A figure that is zero is left out.
	let { view }: { view: RunDetailState } = $props();

	const s = $derived(view.summary);
	/** A job group's members: how many of its jobs succeeded, as a plain fact before the node counts. */
	const members = $derived(view.detail?.children ?? []);
	const jobsLine = $derived(members.length && members.every((c) => !['queued', 'launched', 'running', 'unresponsive'].includes(c.status)) ? `${members.filter((c) => c.status === 'success' || c.status === 'warn').length} of ${members.length} jobs succeeded` : null);
	const rows = (n: number) => n.toLocaleString('en-GB');
	type Fig = { key: string; text: string; filter: QuickFilter; tone?: 'err' | 'warn' | 'run' | 'muted'; note?: string };
	const figs = $derived.by((): Fig[] => {
		// Problems first, then what happened: "1 failed · 1 warning · 1 skipped · 7 built". Only failures
		// are red and warnings amber; skipped and not run are muted; built is plain text, never green.
		const out: (Fig | null)[] = [
			s.failed ? { key: 'failed', text: `${s.failed} failed`, filter: QUICK.failed, tone: 'err' } : null,
			s.checksFailed ? { key: 'checks_failed', text: `${plural(s.checksFailed, 'audit')} failed`, filter: QUICK.auditsFailed, tone: 'err' } : null,
			s.warned ? { key: 'warned', text: plural(s.warned, 'warning'), filter: QUICK.warnings, tone: 'warn' } : null,
			s.skipped ? { key: 'skipped', text: `${s.skipped} skipped`, filter: QUICK.skipped, tone: 'muted' } : null,
			s.notRun ? { key: 'not_run', text: `${s.notRun} not run`, filter: QUICK.notRun, tone: 'muted' } : null,
			s.checksNotRun ? { key: 'checks_not_run', text: `${plural(s.checksNotRun, 'audit')} not run`, filter: QUICK.auditsNotRun, tone: 'muted' } : null,
			s.running ? { key: 'running', text: `${s.running} running`, filter: QUICK.running, tone: 'run' } : null,
			s.waiting ? { key: 'waiting', text: `${s.waiting} waiting`, filter: QUICK.waiting, tone: 'muted' } : null,
			// The note sits only after its figure: no parenthesis without the count it explains.
			s.built ? { key: 'succeeded', text: `${s.built} built`, filter: QUICK.built, note: s.incremental ? `(${s.incremental} incremental, +${rows(s.incrementalRows)} rows)` : undefined } : null
		];

		return out.filter((f): f is Fig => !!f);
	});
</script>

<div class="summary" data-testid="run-summary">
	{#if jobsLine && figs.length}<span class="jobs">{jobsLine}</span><span class="sep">{' · '}</span>{/if}
	{#each figs as f, i (f.key)}
		<!-- Separators and spaces are real text, so the line reads the same copied or read aloud. -->
		{#if i}<span class="sep">{' · '}</span>{/if}
		<!-- A real link: the figure reads as text anywhere, and filters the timeline and log. -->
		<span class="part"
			><a
				class="fig {f.tone ?? ''}"
				class:on={view.logHolds(f.filter.text)}
				aria-current={view.logHolds(f.filter.text) ? 'true' : undefined}
				href="?show={f.key}"
				onclick={(ev) => {
					ev.preventDefault();
					view.toggleLog(f.filter.text);
				}}
				title={view.logHolds(f.filter.text) ? `Take ${f.filter.text} out of the log filter` : `Filter the log: ${f.filter.text}`}>{f.text}</a
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
	/*
	 * A figure is plain text in its meaning's colour, underlined on hover. While its filter is on it is a
	 * soft pill like the run's status pill (tint fill, no border, 24px tall, fully round). The pill's room
	 * is always reserved: padding with an equal negative margin, on an inline box whose vertical padding
	 * doesn't change the line, so pressing it moves nothing.
	 */
	.fig {
		--tone: var(--foreground);
		margin: 0 -8px;
		padding: 2px 8px;
		border-radius: 999px;
		color: var(--foreground);
		cursor: pointer;
		font-size: 12.5px;
		line-height: 20px;
	}
	.fig:hover {
		text-decoration: underline;
	}
	.fig.on {
		background: color-mix(in srgb, var(--tone) 13%, transparent);
		text-decoration: none;
	}
	.fig.err {
		--tone: var(--sb-error);
		color: var(--sb-error);
	}
	.fig.warn {
		--tone: var(--sb-warning);
		color: var(--sb-warning);
	}
	.fig.muted {
		--tone: var(--sb-text-faint);
		color: var(--muted-foreground);
	}
	.fig.run {
		--tone: var(--sb-running);
		color: var(--sb-running);
	}
	.jobs {
		color: var(--foreground);
	}
	/* The separators keep clear of a pill's reserved room either side. */
	.sep {
		margin: 0 6px;
	}
	.sep,
	.faint {
		color: var(--sb-text-faint);
	}
</style>
