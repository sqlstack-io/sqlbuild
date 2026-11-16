<script lang="ts">
	// Lives inside the graph canvas so it can access the flow context, and exposes
	// fitView() upward so the parent can reset pan+zoom after re-layout.
	// `fitFloor` keeps these fits readable on a large graph; zooming out by hand can still go below it.
	// With `readable`, a graph too large to show whole at `fitFloor` is shown at `fitFloor` instead,
	// centred on its largest box or from its top-left, rather than centred on empty space.
	// `fitted` turns true once a fit has been painted; graphs stay hidden until then, so the first
	// frame never shows cards at unlaid-out positions that then jump into place.
	import { useStore, useSvelteFlow, useViewport, type Viewport } from '@xyflow/svelte';
	import { firstCard, fitViewport, largestBox, type Rect } from './fit';
	import { unzoom } from './hit-area';

	let {
		fitView = $bindable(),
		viewport = $bindable(),
		setViewport = $bindable(),
		fitted = $bindable(false),
		fitFloor,
		readable = false
	}: {
		fitView?: (opts?: { duration?: number }) => void;
		viewport?: Viewport;
		setViewport?: (v: Viewport) => void;
		fitted?: boolean;
		fitFloor?: number;
		readable?: boolean;
	} = $props();

	const flow = useSvelteFlow();
	const store = useStore();
	const vp = useViewport();
	const frame = () => new Promise((resolve) => requestAnimationFrame(resolve));

	/** The readable fit, once the pane and every top-level card have a size; false until then. */
	function readableFit(duration: number): boolean {
		const { width, height } = store;
		const all = flow.getNodes();
		const top = all.filter((n) => !n.parentId && !n.hidden);
		if (!width || !height || !top.length) return false;
		const boxes = new Set(all.flatMap((n) => (n.parentId ? [n.parentId] : [])));
		const rects: (Rect & { box: boolean })[] = [];
		for (const n of top) {
			const internal = flow.getInternalNode(n.id);
			const w = internal?.measured.width ?? n.width;
			const h = internal?.measured.height ?? n.height;
			if (!internal || !w || !h) return false;
			rects.push({ x: internal.internals.positionAbsolute.x, y: internal.internals.positionAbsolute.y, width: w, height: h, box: boxes.has(n.id) });
		}
		const left = Math.min(...rects.map((r) => r.x));
		const topY = Math.min(...rects.map((r) => r.y));
		const bounds = { x: left, y: topY, width: Math.max(...rects.map((r) => r.x + r.width)) - left, height: Math.max(...rects.map((r) => r.y + r.height)) - topY };
		void flow.setViewport(fitViewport(bounds, width, height, { minZoom: fitFloor, focus: largestBox(rects), start: firstCard(rects) }), { duration });
		return true;
	}

	fitView = (opts) => {
		// Nothing to fit yet; a fit now would count the graph as shown before its cards exist.
		if (!flow.getNodes().length) return;
		if (fitted) {
			if (readable && readableFit(opts?.duration ?? 300)) return;
			void flow.fitView({ duration: 300, padding: 0.08, maxZoom: 1, minZoom: fitFloor, ...opts });
			return;
		}
		// Until the graph has been shown, fit without animating, so it appears already in place. A fit waits
		// until the flow can run it, so each try is bounded by a frame; after 20 the graph shows regardless,
		// and the fit keeps trying for a while (the pane may not have its size yet) unless the view moves.
		void (async () => {
			const start = { ...vp.current };
			for (let attempt = 0; attempt < 600; attempt++) {
				const moved = fitted && (vp.current.x !== start.x || vp.current.y !== start.y || vp.current.zoom !== start.zoom);
				if (moved) return;
				const done = readable ? readableFit(0) : await Promise.race([flow.fitView({ padding: 0.08, maxZoom: 1, minZoom: fitFloor, ...opts, duration: 0 }), frame().then(() => false)]);
				if (done) {
					await frame();
					await frame();
					fitted = true;
					return;
				}
				if (attempt === 19) fitted = true;
				await frame();
			}
		})();
	};
	setViewport = (v) => flow.setViewport(v);
	$effect(() => {
		viewport = vp.current;
	});
	// Card controls such as a folder's chevron and an open box's header keep at least their on-screen
	// size when the graph is zoomed out, by growing with `--sb-unzoom`.
	$effect(() => {
		store.domNode?.style.setProperty('--sb-unzoom', String(unzoom(vp.current.zoom)));
	});
</script>
