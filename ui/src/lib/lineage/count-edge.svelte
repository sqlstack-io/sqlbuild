<script lang="ts">
	import { BaseEdge, getBezierPath, type EdgeProps } from '@xyflow/svelte';
	import { trunkPath } from './trunk-path';
	import type { VisibleEdgeData } from './visible-edges';

	// Every graph's edge: edges into one card merge into one trunk just before it (trunk-path.ts); a
	// visible-level edge's tooltip says how many node links it stands for.
	let { sourceX, sourceY, targetX, targetY, sourcePosition, targetPosition, style, data }: EdgeProps = $props();
	const path = $derived(trunkPath(sourceX, sourceY, targetX, targetY) ?? getBezierPath({ sourceX, sourceY, targetX, targetY, sourcePosition, targetPosition })[0]);
</script>

<g>
	<title>{(data as VisibleEdgeData | undefined)?.title ?? ''}</title>
	<BaseEdge {path} {style} />
</g>
