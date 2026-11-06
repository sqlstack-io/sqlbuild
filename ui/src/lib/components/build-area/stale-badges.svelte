<script lang="ts">
	import type { StaleReason } from '$lib/api';
	import { staleReasonDetail, staleReasonLabel } from '$lib/status';

	// Stale reasons as small badges, separate from the last outcome.
	let { reasons, empty = '' }: { reasons: StaleReason[]; empty?: string } = $props();
</script>

{#if reasons.length}
	<span class="stale-badges">
		{#each reasons as r, i (i)}
			<span class="badge {r.reason}" title={staleReasonDetail(r)}>{staleReasonLabel(r)}</span>
		{/each}
	</span>
{:else if empty}
	<span class="none">{empty}</span>
{/if}

<style>
	.stale-badges {
		display: inline-flex;
		flex-wrap: wrap;
		gap: 4px;
	}
	.badge {
		padding: 0 6px;
		border: 1px solid transparent;
		border-radius: 2px;
		background: color-mix(in srgb, var(--sb-stale) 15%, transparent);
		color: var(--sb-stale);
		font-size: 11.5px;
		font-weight: 500;
		line-height: 18px;
		white-space: nowrap;
	}
	.badge.changed_upstream {
		border: 1px dashed color-mix(in srgb, var(--sb-stale) 55%, transparent);
		background: transparent;
	}
	.badge.behind {
		background: color-mix(in srgb, var(--sb-warning) 13%, transparent);
		color: var(--sb-warning);
	}
	.none {
		color: var(--muted-foreground);
	}
</style>
