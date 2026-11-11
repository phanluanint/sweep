// Squarified treemap layout (Bruls, Huizing & van Wijk).

export interface Rect {
  x: number;
  y: number;
  w: number;
  h: number;
}

function worst(row: number[], side: number): number {
  const sum = row.reduce((a, b) => a + b, 0);
  const max = Math.max(...row);
  const min = Math.min(...row);
  const s2 = sum * sum;
  const w2 = side * side;
  return Math.max((w2 * max) / s2, s2 / (w2 * min));
}

/** Lays out `weights` (largest first works best) inside `rect`; returns one rect per weight. */
export function squarify(weights: number[], rect: Rect): Rect[] {
  const total = weights.reduce((a, b) => a + Math.max(0, b), 0);
  const out: Rect[] = weights.map(() => ({ x: rect.x, y: rect.y, w: 0, h: 0 }));
  if (total <= 0 || rect.w <= 0 || rect.h <= 0) return out;

  const scale = (rect.w * rect.h) / total;
  const areas = weights.map((w) => Math.max(0, w) * scale);
  let { x, y, w, h } = rect;
  let row: number[] = [];
  let rowStart = 0;

  const place = (start: number, items: number[]) => {
    const sum = items.reduce((a, b) => a + b, 0);
    if (sum <= 0) return;
    if (w >= h) {
      const colW = sum / h;
      let cy = y;
      items.forEach((a, k) => {
        const ih = a / colW;
        out[start + k] = { x, y: cy, w: colW, h: ih };
        cy += ih;
      });
      x += colW;
      w -= colW;
    } else {
      const rowH = sum / w;
      let cx = x;
      items.forEach((a, k) => {
        const iw = a / rowH;
        out[start + k] = { x: cx, y, w: iw, h: rowH };
        cx += iw;
      });
      y += rowH;
      h -= rowH;
    }
  };

  let i = 0;
  while (i < areas.length) {
    const a = areas[i];
    if (a <= 0) {
      i++;
      continue;
    }
    const side = Math.min(w, h);
    if (row.length === 0 || worst([...row, a], side) <= worst(row, side)) {
      if (row.length === 0) rowStart = i;
      row.push(a);
      i++;
    } else {
      place(rowStart, row);
      row = [];
    }
  }
  if (row.length) place(rowStart, row);
  return out;
}
