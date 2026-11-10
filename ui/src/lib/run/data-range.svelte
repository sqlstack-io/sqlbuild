<script lang="ts">
	// Which data a run processes: the latest, as a scheduled run does, or a date range that incremental models
	// reprocess (`--start-cursor-ts` / `--end-cursor-ts`). The window is the caller's; this only edits it.
	import type { CursorWindow } from '$lib/api';
	import { app } from '$lib/state/app.svelte';

	let { cursorType, window, onchange }: { cursorType: 'timestamp' | 'integer' | null; window: CursorWindow | null; onchange: (w: CursorWindow | null) => void } = $props();

	const canRewrite = $derived(app.can('run.rewrite_history'));
	const day = 86_400_000;
	const midnight = (ms: number) => new Date(Math.floor(ms / day) * day);
	const iso = (d: Date) => d.toISOString().replace('.000Z', 'Z');
	const dateOf = (ts: string | null) => (ts ? ts.slice(0, 10) : '');
	/** Whole UTC days ending with yesterday, so a range never covers a day still being written. */
	const PRESETS = [
		{ key: 'yesterday', label: 'Yesterday', days: 1 },
		{ key: 'week', label: 'Last 7 days', days: 7 },
		{ key: 'month', label: 'Last 30 days', days: 30 }
	];
	const presetWindow = (days: number): CursorWindow => {
		const today = midnight(app.now);
		return { type: 'timestamp', start: iso(new Date(today.getTime() - days * day)), end: iso(today) };
	};
	let custom = $state(false);
	const preset = $derived(
		cursorType === 'timestamp' && window && !custom ? (PRESETS.find((p) => { const w = presetWindow(p.days); return w.start === window.start && w.end === window.end; })?.key ?? null) : null
	);
	const ranged = $derived(!!window || custom);

	function setCustom(start: string, end: string) {
		if (cursorType === 'integer') return onchange({ type: 'integer', start: start || null, end: end || null });
		// Dates are whole UTC days: the end date is included, so the window ends at the next midnight.
		const last = Date.parse(`${end}T00:00:00Z`);
		const next = end && !Number.isNaN(last) ? iso(new Date(last + day)) : null;
		onchange({ type: 'timestamp', start: start ? `${start}T00:00:00Z` : null, end: next });
	}
	/** The last included day of a timestamp window; an end still being typed in the command shows as it is. */
	const customEnd = $derived.by(() => {
		const end = window?.end ?? '';
		const at = Date.parse(end);
		return window?.type === 'timestamp' && !Number.isNaN(at) ? dateOf(iso(new Date(at - day))) : end;
	});
</script>

{#if cursorType}
	<div class="range" title={canRewrite ? undefined : "Reprocessing a date range needs the rewrite-history permission (run.rewrite_history)"}>
		<div class="pills" role="group" aria-label="Data">
			<button aria-pressed={!ranged} onclick={() => { custom = false; onchange(null); }}>Latest data</button>
			<button aria-pressed={ranged} disabled={!canRewrite} onclick={() => {
				if (ranged) return;
				if (cursorType === 'timestamp') onchange(presetWindow(7));
				else custom = true;
			}}>{cursorType === 'timestamp' ? 'Date range' : 'Cursor range'}</button>
		</div>
		{#if ranged}
			{#if cursorType === 'timestamp'}
				<div class="pills" role="group" aria-label="Range">
					{#each PRESETS as p (p.key)}
						<button aria-pressed={preset === p.key} onclick={() => { custom = false; onchange(presetWindow(p.days)); }}>{p.label}</button>
					{/each}
					<button aria-pressed={!preset} onclick={() => (custom = true)}>Custom</button>
				</div>
			{/if}
			{#if !preset}
				<div class="custom">
					{#if cursorType === 'timestamp'}
						<input type="date" aria-label="From" value={dateOf(window?.start ?? null)} onchange={(e) => setCustom(e.currentTarget.value, customEnd)} />
						<span>to</span>
						<input type="date" aria-label="To, included" value={customEnd} onchange={(e) => setCustom(dateOf(window?.start ?? null), e.currentTarget.value)} />
					{:else}
						<input inputmode="numeric" aria-label="From" placeholder="start" value={window?.start ?? ''} onchange={(e) => setCustom(e.currentTarget.value, window?.end ?? '')} />
						<span>to</span>
						<input inputmode="numeric" aria-label="To" placeholder="end" value={window?.end ?? ''} onchange={(e) => setCustom(window?.start ?? '', e.currentTarget.value)} />
					{/if}
				</div>
			{/if}
			<p class="note">Incremental models reprocess this range, replacing what was built for it. Recorded in the audit log.</p>
		{/if}
	</div>
{/if}

<style>
	.range {
		display: flex;
		flex-direction: column;
		align-items: flex-start;
		gap: 8px;
	}
	.pills {
		display: flex;
		flex-wrap: wrap;
		gap: 4px;
	}
	.pills button {
		padding: 2px 10px;
		border: 1px solid var(--border);
		border-radius: 999px;
		color: var(--muted-foreground);
		font-size: 11.5px;
	}
	.pills button[aria-pressed='true'] {
		border-color: var(--border-strong);
		background: var(--sidebar-accent);
		color: var(--foreground);
	}
	.pills button:hover:not(:disabled) {
		color: var(--foreground);
	}
	.pills button:disabled {
		opacity: 0.5;
	}
	.custom {
		display: flex;
		align-items: center;
		gap: 6px;
		color: var(--sb-text-faint);
		font-size: 12px;
	}
	.custom input {
		padding: 2px 6px;
		border: 1px solid var(--border);
		border-radius: 2px;
		background: var(--background);
		color: var(--foreground);
		font-family: var(--font-mono);
		font-size: 11.5px;
	}
	.note {
		margin: 0;
		color: var(--sb-warning);
		font-size: 11.5px;
	}
</style>
