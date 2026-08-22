// Self-check for `anchorMenu`'s flipping. Run it: `node src/lib/menu.check.ts` from ui/.
// ponytail: a script over a test runner — the UI has no test setup and this is the only piece of
// menu.ts that does arithmetic worth getting wrong.
import { anchorMenu, type Anchor } from './menu.ts';

// ponytail: two lines instead of node:assert, which would want @types/node in a DOM-only tsconfig.
const eq = (got: string, want: string) => {
	if (got !== want) throw new Error(`expected ${want}, got ${got}`);
};
const anchorEq = (got: Anchor, style: string, origin: string) => (eq(got.style, style), eq(got.origin, origin));

Object.assign(globalThis, { window: { innerWidth: 1000, innerHeight: 800 } });

const at = (x: number, y: number) => anchorMenu({ type: 'contextmenu', clientX: x, clientY: y } as unknown as Event);
const from = (box: { left: number; right: number; top: number; bottom: number }, align?: 'left' | 'right') =>
	anchorMenu({ type: 'click', currentTarget: { getBoundingClientRect: () => box } } as unknown as Event, {
		align
	});

// Pointer, room everywhere: opens down and to the right of the cursor.
anchorEq(at(100, 100), 'left:100px;top:100px', 'origin-top-left');
// Near the bottom: flips above the cursor. Near the right: hangs off it leftwards.
eq(at(100, 700).style, 'left:100px;bottom:100px');
eq(at(900, 100).style, 'right:100px;top:100px');
eq(at(900, 700).origin, 'origin-bottom-right');
// Too little room either way (a short window) stays downwards rather than flipping into nothing.
Object.assign(globalThis, { window: { innerWidth: 1000, innerHeight: 300 } });
eq(at(100, 150).style, 'left:100px;top:150px');
Object.assign(globalThis, { window: { innerWidth: 1000, innerHeight: 800 } });

// A trigger: 4px below its box, and `align: 'right'` lines the menu's right edge up with it.
const box = { left: 400, right: 440, top: 200, bottom: 240 };
eq(from(box).style, 'left:400px;top:244px');
eq(from(box, 'right').style, 'right:560px;top:244px');
// A left-aligned trigger near the right edge still flips rather than running off screen.
eq(from({ left: 900, right: 940, top: 200, bottom: 240 }).style, 'right:60px;top:244px');

console.log('menu.check: ok');
