// Self-check for `anchorMenu`'s placement arithmetic. Run it: `node src/lib/menu.check.ts` from ui/.
// ponytail: a script over a test runner — the UI has no test setup and this is the only piece of
// menu.ts that does arithmetic worth getting wrong. The sizes here are menu.ts's own guesses
// (224x280), which is what `fitMenu` re-runs this same code with once it has measured the popup.
import { anchorMenu } from './menu.ts';

// ponytail: two lines instead of node:assert, which would want @types/node in a DOM-only tsconfig.
const eq = (got: string, want: string) => {
	if (got !== want) throw new Error(`expected\n  ${want}\ngot\n  ${got}`);
};
const viewport = (w: number, h: number) => Object.assign(globalThis, { window: { innerWidth: w, innerHeight: h } });
const at = (x: number, y: number) =>
	anchorMenu({ type: 'contextmenu', clientX: x, clientY: y } as unknown as Event).style;
const from = (box: { left: number; right: number; top: number; bottom: number }, align?: 'left' | 'right') =>
	anchorMenu({ type: 'click', currentTarget: { getBoundingClientRect: () => box } } as unknown as Event, {
		align
	}).style;

viewport(1000, 800);

// Pointer, room everywhere: opens down and to the right of the cursor.
eq(at(100, 100), 'left:100px;right:auto;top:100px;bottom:auto;transform-origin:top left');
// Near the bottom it flips above the cursor; near the right it hangs off it leftwards.
eq(at(100, 700), 'left:100px;right:auto;top:auto;bottom:100px;transform-origin:bottom left');
eq(at(900, 100), 'left:auto;right:100px;top:100px;bottom:auto;transform-origin:top right');
eq(at(900, 700), 'left:auto;right:100px;top:auto;bottom:100px;transform-origin:bottom right');

// A trigger: 4px below its box, and `align: 'right'` lines the menu's right edge up with it.
const box = { left: 400, right: 440, top: 200, bottom: 240 };
eq(from(box), 'left:400px;right:auto;top:244px;bottom:auto;transform-origin:top left');
eq(from(box, 'right'), 'left:auto;right:560px;top:244px;bottom:auto;transform-origin:top right');

// The clamp, which is what stops a menu running off screen when the anchor asks for too much:
// a left-aligned trigger near the right edge flips, and a right-aligned one near the left edge
// stops 8px short instead of hanging off it.
eq(from({ left: 900, right: 940, top: 200, bottom: 240 }), 'left:auto;right:60px;top:244px;bottom:auto;transform-origin:top right');
eq(from({ left: 10, right: 50, top: 200, bottom: 240 }, 'right'), 'left:auto;right:768px;top:244px;bottom:auto;transform-origin:top right');

// Too little room either way (a short window): stays downwards rather than flipping into nothing,
// and the clamp pulls it back inside.
viewport(1000, 300);
eq(at(100, 150), 'left:100px;right:auto;top:12px;bottom:auto;transform-origin:top left');

console.log('menu.check: ok');
