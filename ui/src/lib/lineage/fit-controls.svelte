<script lang="ts">
	import { ControlButton, Controls } from '@xyflow/svelte';
	import RotateCcwIcon from '@lucide/svelte/icons/rotate-ccw';

	// Svelte Flow's zoom controls, with a fit button that fits as the page's own fits do: no further out
	// than a readable zoom, so a large graph opens on its cards rather than as dots.
	let { onfit, showLock = false }: { onfit: () => void; showLock?: boolean } = $props();
</script>

<Controls {showLock} showFitView={false}>
	<!-- The view the graph opened at: the readable fit every page's first fit uses. -->
	<ControlButton onclick={onfit} title="Reset view" aria-label="Reset view" class="fit-control"><RotateCcwIcon /></ControlButton>
</Controls>

<style>
	:global(.svelte-flow__controls-button.fit-control svg) {
		fill: none !important;
		stroke: currentColor;
		stroke-width: 2.25;
	}
</style>
