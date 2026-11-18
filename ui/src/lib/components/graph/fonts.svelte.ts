// Whether the cards' fonts have loaded. Text fitted to a card (compact names, the status line) is
// measured with the font's real widths once it has, so it is fitted once more then; never per zoom.
// `version` counts font loads after the first, such as the compact names' font, which loads only once
// a compact name first shows.
export const fonts = $state({ ready: false, version: 0 });
if (typeof document !== 'undefined') {
	void document.fonts?.ready.then(() => (fonts.ready = true));
	document.fonts?.addEventListener?.('loadingdone', () => fonts.version++);
}
