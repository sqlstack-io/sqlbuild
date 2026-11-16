// Whether the cards' font has loaded. Text fitted to a card (compact titles, the status line) is
// measured with the font's real widths once it has, so it is fitted once more then; never per zoom.
export const fonts = $state({ ready: false });
if (typeof document !== 'undefined') void document.fonts?.ready.then(() => (fonts.ready = true));
