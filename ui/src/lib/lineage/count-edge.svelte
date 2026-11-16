<script lang="ts">
	import { BaseEdge, getBezierPath, type EdgeProps } from '@xyflow/svelte';
	import { edgeTitle, type MemberEdgeData } from './member-graph';

	// A wait between two members, drawn at the level you can see; hovering it says how many node links it stands for.
	let { sourceX, sourceY, targetX, targetY, sourcePosition, targetPosition, style, data }: EdgeProps = $props();
	const path = $derived(getBezierPath({ sourceX, sourceY, targetX, targetY, sourcePosition, targetPosition })[0]);
</script>

<g>
	<title>{edgeTitle(data as MemberEdgeData)}</title>
	<BaseEdge {path} {style} />
</g>
