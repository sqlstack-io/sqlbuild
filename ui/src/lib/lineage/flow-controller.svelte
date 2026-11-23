<script lang="ts">
	// Lives inside the graph canvas so it can access the flow context, and exposes
	// fitView() upward so the parent can reset pan+zoom after re-layout.
	// `fitFloor` keeps these fits readable on a large graph; zooming out by hand can still go below it.
	// With `readable`, a graph too large to show whole at `fitFloor` is shown at `fitFloor` instead,
	// centred on its largest box or from its top-left, rather than centred on empty space.
	// `fitted` turns true once a fit has been painted; graphs stay hidden until then, so the first
	// frame never shows cards at unlaid-out positions that then jump into place.
	import { useStore, useSvelteFlow, useViewport, type Viewport } from '@xyflow/svelte';
	import { LABEL_MIN_ZOOM, WHOLE_FIT_CARDS, firstCard, fitViewport, largestBox, type Rect } from './fit';

	let {
		fitView = $bindable(),
		viewport = $bindable(),
		setViewport = $bindable(),
		fitted = $bindable(false),
		fitFloor,
		readable = false,
		wholeFloor = LABEL_MIN_ZOOM
	}: {
		fitView?: (opts?: { duration?: number }) => void;
		viewport?: Viewport;
		setViewport?: (v: Viewport) => void;
		fitted?: boolean;
		fitFloor?: number;
		readable?: boolean;
		/** Mockup: the lowest zoom a modest graph is shown whole at; below it, it opens readable on its start. */
		wholeFloor?: number;
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
		const rects: (Rect & { box: boolean; id: string })[] = [];
		for (const n of top) {
			const internal = flow.getInternalNode(n.id);
			const w = internal?.measured.width ?? n.width;
			const h = internal?.measured.height ?? n.height;
			if (!internal || !w || !h) return false;
			rects.push({ id: n.id, x: internal.internals.positionAbsolute.x, y: internal.internals.positionAbsolute.y, width: w, height: h, box: boxes.has(n.id) });
		}
		const left = Math.min(...rects.map((r) => r.x));
		const topY = Math.min(...rects.map((r) => r.y));
		const bounds = { x: left, y: topY, width: Math.max(...rects.map((r) => r.x + r.width)) - left, height: Math.max(...rects.map((r) => r.y + r.height)) - topY };
		const focus = largestBox(rects);
		const start = firstCard(rects);
		// Named on the canvas, so the layout test can check the view starts with it wholly in view.
		const anchorId = focus ? rects.find((r) => r.box && r.x === focus.x && r.y === focus.y)?.id : start?.id;
		store.domNode?.setAttribute('data-fit-anchor', anchorId ?? '');
		// A modest graph is shown whole; a larger one opens readable on its start card.
		const minZoom = all.length <= WHOLE_FIT_CARDS ? wholeFloor : fitFloor;
		void flow.setViewport(fitViewport(bounds, width, height, { minZoom, focus, start }), { duration });
		return true;
	}

	/** The lowest zoom a fit goes to: low enough to show a modest graph whole, else the page's floor. */
	const floorFor = () => (fitFloor !== undefined && flow.getNodes().length <= WHOLE_FIT_CARDS ? Math.min(fitFloor, wholeFloor) : fitFloor);

	fitView = (opts) => {
		// Nothing to fit yet; a fit now would count the graph as shown before its cards exist.
		if (!flow.getNodes().length) return;
		if (fitted) {
			if (readable && readableFit(opts?.duration ?? 300)) return;
			void flow.fitView({ duration: 300, padding: 0.08, maxZoom: 1, minZoom: floorFor(), ...opts });
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
				const done = readable ? readableFit(0) : await Promise.race([flow.fitView({ padding: 0.08, maxZoom: 1, minZoom: floorFor(), ...opts, duration: 0 }), frame().then(() => false)]);
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
</script>
