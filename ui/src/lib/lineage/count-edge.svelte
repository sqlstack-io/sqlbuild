<script lang="ts">
	import { BaseEdge, getBezierPath, type EdgeProps } from '@xyflow/svelte';
	import type { VisibleEdgeData } from './visible-edges';

	// An edge drawn at the level you can see, between folders, members or nodes; hovering it says how many node links it stands for.
	let { sourceX, sourceY, targetX, targetY, sourcePosition, targetPosition, style, data }: EdgeProps = $props();
	const path = $derived(getBezierPath({ sourceX, sourceY, targetX, targetY, sourcePosition, targetPosition })[0]);
</script>

<g>
	<title>{(data as VisibleEdgeData | undefined)?.title ?? ''}</title>
	<BaseEdge {path} {style} />
</g>
