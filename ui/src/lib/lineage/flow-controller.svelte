<script lang="ts">
	// Lives inside <SvelteFlow> so it can access the flow context, and exposes
	// fitView() upward so the parent can reset pan+zoom after re-layout.
	import { useSvelteFlow, useViewport, type Viewport } from '@xyflow/svelte';

	let {
		fitView = $bindable(),
		viewport = $bindable(),
		setViewport = $bindable()
	}: {
		fitView?: (opts?: { duration?: number }) => void;
		viewport?: Viewport;
		setViewport?: (v: Viewport) => void;
	} = $props();

	const flow = useSvelteFlow();
	const vp = useViewport();
	fitView = (opts) => flow.fitView({ duration: 300, padding: 0.08, maxZoom: 1, ...opts });
	setViewport = (v) => flow.setViewport(v);
	$effect(() => {
		viewport = vp.current;
	});
</script>
