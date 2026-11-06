<script lang="ts">
	import { goto } from '$app/navigation';
	import type { JobDetail } from '$lib/api';
	import Button from '$lib/components/ui/button/button.svelte';
	import { app } from '$lib/state/app.svelte';

	let { job, onclose }: { job: JobDetail; onclose: () => void } = $props();

	let fullRefresh = $state(false);
	let useWindow = $state(false);
	let start = $state('');
	let end = $state('');
	let busy = $state(false);
	let error = $state<string | null>(null);

	const canRewrite = $derived(app.can('run.rewrite_history'));
	const rewrites = $derived(fullRefresh || useWindow);
	const placeholder = $derived(job.cursorType === 'integer' ? 'e.g. 1200' : 'e.g. 2026-10-01T00:00:00Z');
	const windowInvalid = $derived(
		useWindow &&
			(!start && !end
				? 'Give a start or an end'
				: job.cursorType === 'integer'
					? [start, end].some((v) => v && !/^\d+$/.test(v)) && 'Use whole numbers'
					: [start, end].some((v) => v && Number.isNaN(Date.parse(v))) && 'Use a date and time like 2026-10-01T00:00:00Z')
	);

	async function launch() {
		const scope = app.scope;
		if (!scope) return;
		busy = true;
		error = null;
		try {
			const run = await app.client.launchJob(scope, job.key, {
				fullRefresh: fullRefresh || undefined,
				window: useWindow && job.cursorType ? { type: job.cursorType, start: start || null, end: end || null } : undefined
			});
			onclose();
			await goto(`/runs/${run.id}`);
		} catch (e) {
			error = e instanceof Error ? e.message : String(e);
		} finally {
			busy = false;
		}
	}
</script>

<div class="launch">
	<p class="title">Run {job.kind === 'group' ? 'group' : 'job'} <code>{job.key}</code> now</p>
	<p class="faint">
		{job.kind === 'group' ? `Starts a group run, with one run per job in it (${job.members.length}).` : `Builds ${job.nodeCount} nodes with the job's options.`} If another run is updating the same data, it waits for it to finish.
	</p>
	<div class="opts" title={canRewrite ? undefined : "You don't have permission to rebuild history (run.rewrite_history)"}>
		<label class:off={!canRewrite}>
			<input type="checkbox" class="sb-check" bind:checked={fullRefresh} disabled={!canRewrite} />
			Rebuild from scratch <code title="Same as sqb --full-refresh">--full-refresh</code>
		</label>
		{#if job.cursorType}
			<label class:off={!canRewrite}>
				<input type="checkbox" class="sb-check" bind:checked={useWindow} disabled={!canRewrite} />
				Only a range of {job.cursorType === 'integer' ? 'IDs' : 'time'} <span class="faint" title="A cursor window: incremental models and loaders process only this range">(cursor window)</span>
			</label>
			{#if useWindow}
				<div class="window">
					<input bind:value={start} {placeholder} aria-label="Window start" />
					<span class="faint">to</span>
					<input bind:value={end} {placeholder} aria-label="Window end" />
				</div>
				{#if windowInvalid}<p class="err">{windowInvalid}</p>{/if}
			{/if}
		{/if}
		{#if !canRewrite}<p class="faint">Rebuilding or reprocessing a range needs extra permission (<code>run.rewrite_history</code>).</p>{:else if rewrites}<p class="warn">This replaces data that was already built. It's recorded in the audit log.</p>{/if}
	</div>
	{#if error}<p class="err">{error}</p>{/if}
	<div class="row">
		<Button size="sm" onclick={launch} disabled={busy || !!windowInvalid}>▷ Run</Button>
		<Button size="sm" variant="ghost" onclick={onclose}>Cancel</Button>
	</div>
</div>

<style>
	.launch {
		position: absolute;
		top: calc(100% + 6px);
		right: 0;
		z-index: 60;
		width: 380px;
		padding: 12px 14px;
		border: 1px solid var(--border);
		border-radius: 2px;
		background: var(--popover);
		box-shadow: var(--sb-elev);
		font-size: 12.5px;
	}
	.title {
		margin-bottom: 4px;
		font-weight: 600;
	}
	.faint {
		color: var(--muted-foreground);
		font-size: 12px;
	}
	.opts {
		display: flex;
		flex-direction: column;
		gap: 6px;
		margin: 10px 0;
	}
	label {
		display: flex;
		align-items: center;
		gap: 8px;
	}
	label.off {
		opacity: 0.55;
	}
	code {
		font-family: var(--font-mono);
		font-size: 11.5px;
	}
	.window {
		display: flex;
		align-items: center;
		gap: 6px;
		padding-left: 22px;
	}
	.window input {
		width: 150px;
		padding: 3px 6px;
		border: 1px solid var(--border);
		border-radius: 2px;
		background: var(--background);
		font-family: var(--font-mono);
		font-size: 11.5px;
	}
	.err {
		color: var(--sb-error);
		font-size: 12px;
	}
	.warn {
		color: var(--sb-warning);
		font-size: 12px;
	}
	.row {
		display: flex;
		gap: 6px;
	}
</style>
