<script lang="ts">
	// Runs the job without leaving it: what runs (all of the job, or the nodes picked in the graph), which data,
	// and the plan of exactly that, then Run. Anything more, such as a rebuild from scratch, is a command on the
	// plan page, which Edit command opens with this run filled in.
	import { goto } from '$app/navigation';
	import { onMount } from 'svelte';
	import XIcon from '@lucide/svelte/icons/x';
	import RotateCwIcon from '@lucide/svelte/icons/rotate-cw';
	import type { CursorWindow, JobDetail, PlanDocument } from '$lib/api';
	import GatedButton from '$lib/components/operate/gated-button.svelte';
	import { to } from '$lib/paths';
	import { jobBuildTokens, jobRunProblem, parseCommand, selectorOf, withWindow } from '$lib/run/command';
	import DataRange from '$lib/run/data-range.svelte';
	import PlanReport from '$lib/run/plan-report.svelte';
	import { readySummary } from '$lib/run/plan-text';
	import { app } from '$lib/state/app.svelte';

	let { job, picked, onclose }: { job: JobDetail; picked: string[]; onclose: () => void } = $props();

	let window = $state<CursorWindow | null>(null);
	const command = $derived(withWindow(`sqb build${picked.length ? ` --select ${picked.map(selectorOf).join(' ')}` : ''}`, window));
	const parts = $derived(parseCommand(command));
	const tokens = $derived(jobBuildTokens(job.key, parts));
	const planPage = $derived(`/plan?cmd=${encodeURIComponent(command)}&job=${encodeURIComponent(job.key)}`);

	// The location plans exactly this run. The server runs one plan per person and target at a time, so a change
	// made while a plan runs is planned once that one returns.
	let plan = $state<PlanDocument | null>(null);
	let planError = $state<string | null>(null);
	let planning = $state(false);
	let plannedKey = $state('');
	const requestKey = $derived(JSON.stringify(tokens));
	$effect(() => {
		const key = requestKey;
		const request = tokens;
		const scope = app.scope;
		if (!scope || planning || plannedKey === key) return;
		const timer = setTimeout(async () => {
			planning = true;
			try {
				const result = await app.client.planBuild(scope, request);
				plan = result.plan;
				planError = result.error;
			} catch (e) {
				planError = e instanceof Error ? e.message : String(e);
			} finally {
				plannedKey = key;
				planning = false;
			}
		}, 300);
		return () => clearTimeout(timer);
	});
	const current = $derived(plannedKey === requestKey && !planning);

	/** The same check the plan page makes, so a range that would plan but not launch is stopped here too. */
	const rangeProblem = $derived(jobRunProblem(parts));
	const blocked = $derived(rangeProblem ?? (window && !app.can('run.rewrite_history') ? "You don't have permission to reprocess data (run.rewrite_history)" : !current ? 'Waiting for the plan' : planError));
	let launching = $state(false);
	let launchError = $state<string | null>(null);
	async function launch() {
		const scope = app.scope;
		if (!scope || blocked) return;
		launching = true;
		launchError = null;
		try {
			const run = await app.client.launchJob(scope, job.key, { select: parts.select, window: window ?? undefined });
			onclose();
			await goto(to(`/runs/${run.id}`));
		} catch (e) {
			launchError = e instanceof Error ? e.message : String(e);
		} finally {
			launching = false;
		}
	}

	onMount(() => {
		const onKey = (e: KeyboardEvent) => e.key === 'Escape' && onclose();
		document.addEventListener('keydown', onKey);
		return () => document.removeEventListener('keydown', onKey);
	});
</script>

<aside class="panel" aria-label="Run {job.key}">
	<header>
		<span class="title">Run <code>{job.key}</code></span>
		<button class="close" onclick={onclose} aria-label="Close"><XIcon size={14} /></button>
	</header>
	<div class="body sb-scroll">
		<p class="scope">{picked.length ? `${picked.length} selected node${picked.length === 1 ? '' : 's'}` : `All ${job.nodeCount} nodes`}</p>
		{#if job.cursorType}<DataRange cursorType={job.cursorType} {window} onchange={(w) => (window = w)} />{/if}
		<div class="plan">
			{#if !plan && !current}
				<p class="wait"><RotateCwIcon size={13} class="animate-spin" /> Planning on the location…</p>
			{:else if planError && current}
				<p class="err">{planError}</p>
			{:else if plan}
				<p class="summary" class:stale={!current}>{readySummary(plan)}{#if !current}<span class="faint"> · planning again…</span>{/if}</p>
				<div class:stale={!current}><PlanReport {plan} compact /></div>
			{/if}
		</div>
	</div>
	<footer>
		{#if launchError}<p class="err">{launchError}</p>{:else if rangeProblem}<p class="faint">{rangeProblem}</p>{/if}
		<div class="row">
			<GatedButton requires="job.run" variant={blocked ? 'outline' : 'default'} disabled={!!blocked} reason={blocked ?? undefined} busy={launching} onclick={launch}>▷ Run</GatedButton>
			<button class="ghost" onclick={onclose}>Cancel</button>
			<a class="edit" href={to(planPage)}>Edit command</a>
		</div>
	</footer>
</aside>

<style>
	.panel {
		position: fixed;
		top: 52px;
		right: 0;
		bottom: 0;
		z-index: 40;
		display: flex;
		width: 420px;
		max-width: 100%;
		flex-direction: column;
		border-left: 1px solid var(--border);
		background: var(--popover);
		box-shadow: var(--sb-elev);
	}
	header {
		display: flex;
		align-items: center;
		justify-content: space-between;
		padding: 12px 16px;
		border-bottom: 1px solid var(--border);
	}
	.title {
		font-size: 13px;
		font-weight: 600;
	}
	.close {
		color: var(--muted-foreground);
	}
	.close:hover {
		color: var(--foreground);
	}
	.body {
		display: flex;
		min-height: 0;
		flex: 1;
		flex-direction: column;
		gap: 12px;
		overflow: auto;
		padding: 14px 16px;
	}
	.scope {
		margin: 0;
		color: var(--muted-foreground);
		font-size: 12.5px;
	}
	.plan {
		padding-top: 12px;
		border-top: 1px solid var(--border);
	}
	.summary {
		margin: 0 0 10px;
		font-size: 12.5px;
	}
	.wait {
		display: flex;
		align-items: center;
		gap: 8px;
		margin: 6px 0;
		color: var(--muted-foreground);
		font-size: 12.5px;
	}
	.stale {
		opacity: 0.55;
	}
	footer {
		display: flex;
		flex-direction: column;
		gap: 8px;
		padding: 12px 16px;
		border-top: 1px solid var(--border);
	}
	.row {
		display: flex;
		align-items: center;
		gap: 10px;
	}
	.ghost {
		color: var(--muted-foreground);
		font-size: 12.5px;
	}
	.ghost:hover {
		color: var(--foreground);
	}
	.edit {
		margin-left: auto;
		color: var(--primary);
		font-size: 12px;
	}
	.err {
		margin: 0;
		color: var(--sb-error);
		font-size: 12px;
	}
	.faint {
		margin: 0;
		color: var(--sb-text-faint);
		font-size: 12px;
	}
	code {
		font-family: var(--font-mono);
		font-size: 12px;
	}
</style>
