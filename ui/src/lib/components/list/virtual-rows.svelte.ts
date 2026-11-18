// Windowed rendering for long lists with fixed-height rows: only the rows in view, plus a
// few either side, are in the DOM. Spacer rows above and below keep the scrollbar honest.

import { windowRange } from './window-range';

export interface VirtualRows {
	readonly first: number;
	readonly last: number;
	/** Height of the spacer above the drawn rows, in pixels. */
	readonly before: number;
	/** Height of the spacer below the drawn rows, in pixels. */
	readonly after: number;
	/** Attach to the scrolling element: `{@attach rows.attach}`. */
	attach: (el: HTMLElement) => () => void;
	/** Attach to a list that scrolls with its page: the window follows the nearest scrolling element around it. */
	attachWithin: (el: HTMLElement) => () => void;
}

/** The nearest element around `el` that scrolls vertically, or the document's. */
function scrollParent(el: HTMLElement): HTMLElement {
	for (let at = el.parentElement; at; at = at.parentElement) {
		const y = getComputedStyle(at).overflowY;
		if (y === 'auto' || y === 'scroll') return at;
	}
	return document.scrollingElement as HTMLElement;
}

/** `rowHeight` is fixed, or read each time for lists whose rows vary, from an average measured as they draw. */
export function createVirtualRows(rowHeight: number | (() => number), count: () => number, overscan = 8): VirtualRows {
	let scrollTop = $state(0);
	let viewHeight = $state(600);
	const height = () => (typeof rowHeight === 'number' ? rowHeight : rowHeight());
	const range = $derived(windowRange(scrollTop, viewHeight, height(), overscan, count()));

	function attach(el: HTMLElement) {
		const onScroll = () => (scrollTop = el.scrollTop);
		const ro = new ResizeObserver(() => (viewHeight = el.clientHeight));
		ro.observe(el);
		viewHeight = el.clientHeight;
		scrollTop = el.scrollTop;
		el.addEventListener('scroll', onScroll, { passive: true });
		return () => {
			ro.disconnect();
			el.removeEventListener('scroll', onScroll);
		};
	}

	/** The list's own scroll position is how far its top has scrolled above the scroller's top. */
	function attachWithin(el: HTMLElement) {
		const scroller = scrollParent(el);
		const measure = () => {
			scrollTop = Math.max(0, scroller.getBoundingClientRect().top - el.getBoundingClientRect().top);
			viewHeight = scroller.clientHeight;
		};
		const ro = new ResizeObserver(measure);
		ro.observe(scroller);
		measure();
		scroller.addEventListener('scroll', measure, { passive: true });
		return () => {
			ro.disconnect();
			scroller.removeEventListener('scroll', measure);
		};
	}

	return {
		get first() {
			return range.first;
		},
		get last() {
			return range.last;
		},
		get before() {
			return range.first * height();
		},
		get after() {
			return (count() - range.last) * height();
		},
		attach,
		attachWithin
	};
}
