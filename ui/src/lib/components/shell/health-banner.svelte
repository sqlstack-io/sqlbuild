<script lang="ts">
	import { to } from '$lib/paths';
	import TriangleAlertIcon from '@lucide/svelte/icons/triangle-alert';
	import { degradedLine } from '$lib/platform/health';
	import { health } from '$lib/state/health.svelte';

	const line = $derived(degradedLine(health.degraded));
</script>

{#if line}
	<div class="banner" role="status" aria-live="polite" data-testid="health-banner">
		<span class="icon"><TriangleAlertIcon size={14} /></span>
		<span>{line}</span>
		<a href={to('/status')}>View status →</a>
	</div>
{/if}

<style>
	.banner {
		--tone: var(--sb-warning);
		display: flex;
		align-items: center;
		gap: 10px;
		padding: 5px 20px;
		border-bottom: 1px solid color-mix(in srgb, var(--tone) 35%, var(--border));
		background: color-mix(in srgb, var(--tone) 9%, var(--background));
		font-size: 12.5px;
	}
	.icon {
		display: inline-flex;
		color: var(--tone);
	}
	a {
		margin-left: auto;
		color: var(--primary);
		font-weight: 500;
		white-space: nowrap;
	}
</style>
