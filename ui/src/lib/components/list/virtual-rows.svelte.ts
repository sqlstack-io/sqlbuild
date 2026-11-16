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
}

export function createVirtualRows(rowHeight: number, count: () => number, overscan = 8): VirtualRows {
	let scrollTop = $state(0);
	let viewHeight = $state(600);
	const range = $derived(windowRange(scrollTop, viewHeight, rowHeight, overscan, count()));

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

	return {
		get first() {
			return range.first;
		},
		get last() {
			return range.last;
		},
		get before() {
			return range.first * rowHeight;
		},
		get after() {
			return (count() - range.last) * rowHeight;
		},
		attach
	};
}
