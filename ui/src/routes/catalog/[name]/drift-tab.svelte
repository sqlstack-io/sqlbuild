<script lang="ts">
	import type { Snippet } from 'svelte';
	import type { CatalogNodeDetail } from '$lib/api';
	import BuildSource from '$lib/components/build-area/build-source.svelte';
	import StaleBadges from '$lib/components/build-area/stale-badges.svelte';

	// Drift against what was last built in this target, from synced fingerprints.
	let { node, children }: { node: CatalogNodeDetail; children: Snippet } = $props();
</script>

<div class="drift">
	<div class="bar">
		<span class="label">As built in this location</span>
		<BuildSource build={node.lastBuild} />
		{#if node.lastBuild}<span class="fp" title="Fingerprint of the last build">last build <code>{node.lastBuild.fingerprint.slice(0, 12)}</code></span>{/if}
		<span class="reasons"><StaleBadges reasons={node.stale.filter((r) => r.reason !== 'behind')} empty="Matches the last build" /></span>
	</div>
	<div class="viewer">{@render children()}</div>
</div>

<style>
	.drift {
		display: flex;
		min-height: 0;
		flex-direction: column;
	}
	.bar {
		display: flex;
		align-items: center;
		gap: 14px;
		padding: 8px 20px;
		border-bottom: 1px solid var(--border);
		font-size: 12.5px;
	}
	.label {
		color: var(--sb-text-faint);
		font-weight: 500;
	}
	.fp {
		color: var(--muted-foreground);
	}
	code {
		font: 12px var(--font-mono);
	}
	.reasons {
		margin-left: auto;
	}
	.viewer {
		display: flex;
		min-height: 0;
		flex: 1;
	}
	.viewer > :global(*) {
		min-width: 0;
		flex: 1;
	}
</style>
