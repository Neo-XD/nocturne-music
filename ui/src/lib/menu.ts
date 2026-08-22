/**
 * Reparent a `fixed` popup to <body>.
 *
 * `position: fixed` is only viewport-relative while no ancestor is a containing block for it, and
 * `contain` / `content-visibility` / `transform` / `filter` on any ancestor all make one. Shelf uses
 * content-visibility, so an in-place popup anchored by `anchorMenu` lands at the shelf's offset
 * instead of the viewport's and is clipped away by paint containment: the further down the feed, the
 * further off it goes. At <body> there is nothing above it to contain it.
 */
export function toBody(el: HTMLElement) {
	document.body.appendChild(el);
	return () => el.remove();
}

/** What a popup hangs off: a trigger's box, or a zero-size box at the pointer. */
type Box = { left: number; right: number; top: number; bottom: number };

/** A placement, plus what `fitMenu` needs to redo it once the popup's real size is known. */
export type Anchor = { style: string; box: Box; gap: number; align: 'left' | 'right' };

/** Placeholder for a menu that has never been opened; nothing renders until `anchorMenu` runs. */
export const NO_ANCHOR: Anchor = {
	style: '',
	box: { left: 0, right: 0, top: 0, bottom: 0 },
	gap: 0,
	align: 'left'
};

/** Closest the popup may come to the window edge. */
const EDGE = 8;
// First guess only, for the frame before `fitMenu` measures. Roughly a mid-sized menu.
const GUESS_W = 224;
const GUESS_H = 280;

/**
 * Inline style placing a `w` x `h` popup against `box`: below it, or above when there isn't room
 * below and there is above; hanging off its left or right edge; and never closer to the window
 * edge than EDGE, whatever the anchor asked for. All four offsets are named so the result can
 * replace an earlier one wholesale.
 */
function layout(box: Box, gap: number, align: 'left' | 'right', w: number, h: number) {
	const { innerWidth: vw, innerHeight: vh } = window;
	const up = box.bottom + gap + h > vh && box.top - gap - h >= 0;
	const right = align === 'right' || box.left + w > vw;
	// Both offsets are distances from an edge, so one clamp does either side.
	const clamp = (v: number, size: number, limit: number) =>
		Math.max(EDGE, Math.min(v, limit - size - EDGE));
	const x = clamp(right ? vw - box.right : box.left, w, vw);
	const y = clamp(up ? vh - box.top + gap : box.bottom + gap, h, vh);
	return (
		`left:${right ? 'auto' : x + 'px'};right:${right ? x + 'px' : 'auto'};` +
		`top:${up ? 'auto' : y + 'px'};bottom:${up ? y + 'px' : 'auto'};` +
		`transform-origin:${up ? 'bottom' : 'top'} ${right ? 'right' : 'left'}`
	);
}

/**
 * Place a `fixed` menu for whatever opened it: a click on a ⋯ trigger anchors to the trigger's box
 * (`e.currentTarget`), a right-click anchors to the pointer. `align: 'right'` lines the menu's
 * right edge up with the trigger's, which is what a ⋯ button wants.
 *
 * The size here is a guess. Pair it with `fitMenu` on the popup, which measures the real thing and
 * places it again before the frame is painted.
 */
export function anchorMenu(e: Event, { align = 'left' }: { align?: 'left' | 'right' } = {}): Anchor {
	const atPointer = e.type === 'contextmenu';
	const { clientX: px, clientY: py } = e as MouseEvent;
	const box = atPointer
		? { left: px, right: px, top: py, bottom: py }
		: (e.currentTarget as HTMLElement).getBoundingClientRect();
	const gap = atPointer ? 0 : 4;
	return { style: layout(box, gap, align, GUESS_W, GUESS_H), box, gap, align };
}

/**
 * Attachment for the popup itself: measure it, then place it for real. An estimate can't know that
 * a ten-item track menu is 380px tall or that "Remove from Liked Songs" makes it 260 wide, which is
 * how menus near a window edge ended up half off screen.
 *
 * Measuring happens at left:0 first: a `fixed` box shrink-wraps to the room left of the window
 * edge, so a menu sitting where it doesn't fit reports the squeezed width rather than its own.
 */
export function fitMenu(a: Anchor) {
	return (el: HTMLElement) => {
		el.style.cssText = 'left:0;top:0;right:auto;bottom:auto';
		// offset*, not getBoundingClientRect: the popup opens under a `zoom-in-95` animation, and a
		// client rect is the *transformed* box, so it would report 95% of the real size.
		const { offsetWidth, offsetHeight } = el;
		el.style.cssText = layout(a.box, a.gap, a.align, offsetWidth, offsetHeight);
	};
}

/**
 * Where WebKit's own menu still earns its place: a text field, a selection sitting under the
 * pointer, and a held Shift. The first two carry cut/copy/paste; everywhere else the menu is
 * back / reload / inspect, which is nothing you can do to a song.
 */
function wantsNative(e: MouseEvent) {
	// Shift+right-click is the browser convention for "give me the real menu anyway", and it is how
	// you still reach Inspect Element in a dev build.
	if (e.shiftKey) return true;
	const t = e.target;
	if (!(t instanceof Element)) return false;
	if (t.closest('input, textarea, [contenteditable]')) return true;
	const sel = window.getSelection();
	return !!sel && !sel.isCollapsed && sel.containsNode(t, true);
}

/**
 * Window-level handler that takes the native menu away everywhere it is useless. Our own menus
 * (`ctxHost` below) stop the event before it reaches here, so this only ever sees the leftovers.
 */
export function suppressNative(e: MouseEvent) {
	if (!wantsNative(e)) e.preventDefault();
}

/**
 * Attachment for a ⋯ trigger: right-clicking anywhere inside its nearest `[data-ctx]` ancestor
 * opens the same menu at the pointer. The host marks the region; the menu that lives inside it does
 * the wiring, so a host only ever grows one attribute.
 *
 * A host is *an item* — a track row, a card, a library row, the now-playing block. Page headers,
 * toolbars and hero artwork are not: they carry a ⋯ button of their own in plain sight, and making
 * half a page right-clickable only makes it unclear what the menu is even about.
 *
 * Buttons inside an item are their own thing too, so right-clicking Like does nothing rather than
 * opening the row's menu. The ⋯ trigger is the exception, since this menu is what it is for.
 *
 * Nested hosts are fine: the innermost one wins, because `open` stops the event.
 */
export function ctxHost(open: (e: MouseEvent) => void) {
	return (trigger: HTMLElement) => {
		const host = trigger.closest('[data-ctx]');
		if (!host) return;
		const onContextMenu = (e: Event) => {
			if (wantsNative(e as MouseEvent)) return; // let the field keep its paste menu
			const t = e.target;
			const button = t instanceof Element ? t.closest('button') : null;
			if (button && button !== trigger) return;
			open(e as MouseEvent);
		};
		host.addEventListener('contextmenu', onContextMenu);
		return () => host.removeEventListener('contextmenu', onContextMenu);
	};
}
