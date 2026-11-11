<script lang="ts">
	// Lives inside <SvelteFlow> so it can access the flow context, and exposes
	// fitView() upward so the parent can reset pan+zoom after re-layout.
	// `fitFloor` keeps these fits readable on a large graph; zooming out by hand can still go below it.
	// `fitted` turns true once a fit has been painted; graphs stay hidden until then, so the first
	// frame never shows cards at unlaid-out positions that then jump into place.
	import { useSvelteFlow, useViewport, type Viewport } from '@xyflow/svelte';

	let {
		fitView = $bindable(),
		viewport = $bindable(),
		setViewport = $bindable(),
		fitted = $bindable(false),
		fitFloor
	}: {
		fitView?: (opts?: { duration?: number }) => void;
		viewport?: Viewport;
		setViewport?: (v: Viewport) => void;
		fitted?: boolean;
		fitFloor?: number;
	} = $props();

	const flow = useSvelteFlow();
	const vp = useViewport();
	const frame = () => new Promise((resolve) => requestAnimationFrame(resolve));
	fitView = (opts) => {
		// Nothing to fit yet; a fit now would count the graph as shown before its cards exist.
		if (!flow.getNodes().length) return;
		if (fitted) {
			void flow.fitView({ duration: 300, padding: 0.08, maxZoom: 1, minZoom: fitFloor, ...opts });
			return;
		}
		// Until the graph has been shown, fit without animating, so it appears already in place. A fit waits
		// until the flow can run it, so each try is bounded by a frame; after 20 the graph shows regardless.
		void (async () => {
			for (let attempt = 0; attempt < 20 && !fitted; attempt++) {
				const fit = flow.fitView({ padding: 0.08, maxZoom: 1, minZoom: fitFloor, ...opts, duration: 0 });
				if (await Promise.race([fit, frame().then(() => false)])) {
					await frame();
					await frame();
					fitted = true;
					return;
				}
				await frame();
			}
			fitted = true;
		})();
	};
	setViewport = (v) => flow.setViewport(v);
	$effect(() => {
		viewport = vp.current;
	});
</script>
