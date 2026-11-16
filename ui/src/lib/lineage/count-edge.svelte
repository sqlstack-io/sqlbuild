<script lang="ts">
	import { BaseEdge, getBezierPath, type EdgeProps } from '@xyflow/svelte';
	import { trunkPath } from './trunk-path';
	import { routedPath, sharedObstacles } from './edge-routes';
	import type { VisibleEdgeData } from './visible-edges';

	// Every graph's edge: routed through the gaps between the cards in its way (edge-routes.ts), and
	// merging into one trunk just before the card it enters (trunk-path.ts); a visible-level edge's
	// tooltip says how many node links it stands for.
	let { source, target, sourceX, sourceY, targetX, targetY, sourcePosition, targetPosition, style, data }: EdgeProps = $props();
	const cards = sharedObstacles();
	const path = $derived(
		(cards && routedPath(cards(), source, target, sourceX, sourceY, targetX, targetY)) ??
			trunkPath(sourceX, sourceY, targetX, targetY) ??
			getBezierPath({ sourceX, sourceY, targetX, targetY, sourcePosition, targetPosition })[0]
	);
</script>

<g>
	<title>{(data as VisibleEdgeData | undefined)?.title ?? ''}</title>
	<BaseEdge {path} {style} />
</g>
