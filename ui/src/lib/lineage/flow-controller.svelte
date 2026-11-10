<script lang="ts">
	// Lives inside <SvelteFlow> so it can access the flow context, and exposes
	// fitView() upward so the parent can reset pan+zoom after re-layout.
	// `fitted` turns true once a fit has been painted; graphs stay hidden until then, so the first
	// frame never shows cards at unlaid-out positions that then jump into place.
	import { useSvelteFlow, useViewport, type Viewport } from '@xyflow/svelte';

	let {
		fitView = $bindable(),
		viewport = $bindable(),
		setViewport = $bindable(),
		fitted = $bindable(false)
	}: {
		fitView?: (opts?: { duration?: number }) => void;
		viewport?: Viewport;
		setViewport?: (v: Viewport) => void;
		fitted?: boolean;
	} = $props();

	const flow = useSvelteFlow();
	const vp = useViewport();
	const frame = () => new Promise((resolve) => requestAnimationFrame(resolve));
	fitView = (opts) => {
		if (fitted) {
			void flow.fitView({ duration: 300, padding: 0.08, maxZoom: 1, ...opts });
			return;
		}
		// Until the graph has been shown, fit without animating, so it appears already in place. A fit fails
		// while cards are still being measured, so it is retried, and the graph shows only after a fit has painted.
		void (async () => {
			for (let attempt = 0; attempt < 20 && !fitted; attempt++) {
				if (await flow.fitView({ padding: 0.08, maxZoom: 1, ...opts, duration: 0 })) {
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
