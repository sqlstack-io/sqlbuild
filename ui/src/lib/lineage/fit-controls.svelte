<script lang="ts">
	import { ControlButton, Controls } from '@xyflow/svelte';
	import RotateCcwIcon from '@lucide/svelte/icons/rotate-ccw';
	import MaximizeIcon from '@lucide/svelte/icons/maximize';

	// Svelte Flow's zoom controls, with a fit button that fits as the page's own fits do: no further out
	// than a readable zoom, so a large graph opens on its cards rather than as dots.
	let { onfit, showLock = false, label = 'Fit view' }: { onfit: () => void; showLock?: boolean; label?: string } = $props();
</script>

<Controls {showLock} showFitView={false}>
	<!-- The view the graph opened at: the readable fit every page's first fit uses. A page with its own
	     Reset (Lineage's layout reset) keeps "Fit view", so two resets never sit side by side. -->
	<ControlButton onclick={onfit} title={label} aria-label={label} class="fit-control">{#if label === 'Reset view'}<RotateCcwIcon />{:else}<MaximizeIcon />{/if}</ControlButton>
</Controls>

<style>
	:global(.svelte-flow__controls-button.fit-control svg) {
		fill: none !important;
		stroke: currentColor;
		stroke-width: 2.25;
	}
</style>
