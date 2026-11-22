// A box header that drags its box like a title bar and still toggles it on a click: a press that
// moves further than a few pixels is a drag, and the click that ends it is ignored.

/** How far, in screen pixels, a press may move and still count as a click. */
export const CLICK_SLOP = 4;

type Point = { clientX: number; clientY: number };

/** `down` records where a press starts; `still` says whether the click ending it stayed within the slop. */
export function clickGuard() {
  let start: Point | null = null;
  return {
    down(event: Point) {
      start = { clientX: event.clientX, clientY: event.clientY };
    },
    still(event: Point): boolean {
      const from = start;
      start = null;
      return (
        !from ||
        Math.hypot(
          event.clientX - from.clientX,
          event.clientY - from.clientY,
        ) <= CLICK_SLOP
      );
    },
  };
}
