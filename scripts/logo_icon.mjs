// The launcher icon's art, shared by scripts/gen_logo.mjs (the launcher PNGs) and scripts/gen_brand_images.mjs
// (the avatar and GitHub's social preview): the website's mark with the icon's edits, and the lattice behind it.
import { SPRITES } from '../website/src/lib/pixel.js';

/** The adaptive icon uses the same angular V as the app and site. */
export const iconLogo = { pal: SPRITES.logo.pal, rows: SPRITES.logo.rows };

/** The icon's grid: CELLS cells across, the flower on the middle 32 (cells 20-51), the middle at the corner of
 * cells 35 / 36. */
export const CELLS = 72;

/** The background's cell (x, y) on that grid: diagonal 1-cell lines every LATTICE cells, '/' where x + y and '\'
 * where x - y hit a line; both pass through the middle, so the lattice mirrors exactly both ways round the flower
 * (an even width can't: it would sit half a cell off). The upper half of each diamond is lit, a triangle from its
 * left corner to its right one. */
export const LATTICE = 14;
export const LATTICE_COLOURS = { line: '#3A0A12', lit: '#B31231', fill: '#0B0B0D' };
export function lattice(x, y) {
  const m = (v) => ((v % LATTICE) + LATTICE) % LATTICE;
  const s = m(x + y - (CELLS - 1)), t = m(x - y);
  if (s === 0 || t === 0) return LATTICE_COLOURS.line;
  return s < t ? LATTICE_COLOURS.lit : LATTICE_COLOURS.fill;
}
