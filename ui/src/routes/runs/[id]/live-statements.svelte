<script lang="ts">
	import LoaderIcon from '@lucide/svelte/icons/loader-circle';
	import type { RunStatement } from '$lib/api';
	import { app } from '$lib/state/app.svelte';
	import { formatSpan } from './utils';

	let { statements, onpick }: { statements: RunStatement[]; onpick: (nodeKey: string) => void } = $props();
</script>

{#if statements.length}
	<div class="live" role="status" aria-live="polite">
		{#each statements.slice(0, 3) as st (st.id)}
			<button class="card" onclick={() => onpick(st.nodeKey)} title="Show this statement below">
				<LoaderIcon size={14} class="spin" />
				<span class="what">Running statement {st.sequence} · <b>{st.nodeKey}</b></span>
				<span class="num">{formatSpan((app.now - Date.parse(st.startedAt)) / 1000)}</span>
				<span class="num faint">{st.rowsSoFar != null ? `${st.rowsSoFar.toLocaleString('en-GB')} rows so far` : 'no row count yet'}</span>
			</button>
		{/each}
		{#if statements.length > 3}<span class="faint more">+{statements.length - 3} more running</span>{/if}
	</div>
{/if}

<style>
	.live {
		display: flex;
		flex: 0 0 auto;
		flex-wrap: wrap;
		align-items: center;
		gap: 8px;
		padding: 6px 16px;
		border-bottom: 1px solid var(--border);
		background: color-mix(in srgb, var(--sb-running) 6%, var(--background));
	}
	.card {
		display: inline-flex;
		align-items: center;
		gap: 10px;
		padding: 4px 10px;
		border: 1px solid color-mix(in srgb, var(--sb-running) 35%, var(--border));
		border-radius: 3px;
		background: var(--background);
		font-size: 12.5px;
	}
	.card:hover {
		background: var(--sb-hover);
	}
	.card :global(.spin) {
		color: var(--sb-running);
		animation: spin 1s linear infinite;
	}
	@keyframes spin {
		to {
			transform: rotate(360deg);
		}
	}
	b {
		font-family: var(--font-mono);
		font-size: 12px;
		font-weight: 500;
	}
	.num {
		font-variant-numeric: tabular-nums;
	}
	.faint {
		color: var(--muted-foreground);
	}
	.more {
		font-size: 12px;
	}
</style>
