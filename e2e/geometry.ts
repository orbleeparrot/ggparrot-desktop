// 배치 검사용 도구 — 브라우저 안에서 요소 사각형을 재고 겹침·넘침을 찾음.
// - installGeometry는 page.evaluate로 페이지에 넣어 window.__geo로 씀(함수 본문만 넘어가므로 바깥 변수를 쓰지 않음).
// - 결과는 사람이 읽을 위반 문구 배열로 돌려줌 — 테스트는 빈 배열인지 확인하고, 실패하면 숫자가 그대로 보임.
import type { Page } from "@playwright/test";

export interface Box {
  l: number;
  t: number;
  r: number;
  b: number;
  w: number;
  h: number;
}

export interface Geo {
  box(el: Element): Box;
  /** 두 사각형이 가로·세로 모두 tol보다 많이 겹치면 참. */
  overlaps(a: Box, b: Box, tol?: number): boolean;
  /** a가 b 안에 있으면(tol만큼 봐줌) 참. */
  inside(a: Box, b: Box, tol?: number): boolean;
  /** 화면에 그려지는지 — display·visibility·opacity·크기 0을 봄. */
  shown(el: Element): boolean;
  /** root 안 글자 줄들의 사각형 — 넘침을 자르는 조상(overflow ≠ visible)으로 잘라 실제로 보이는 부분만. skip에 걸리는 조상 아래 글자는 뺌. */
  textBoxes(root: Element, skip?: string): { text: string; box: Box }[];
  fmt(b: Box): string;
}

declare global {
  interface Window {
    __geo: Geo;
  }
}

/** 페이지에 window.__geo를 심음. 화면을 다시 불러오면 다시 불러야 함. */
export async function installGeometry(page: Page): Promise<void> {
  await page.evaluate(() => {
    const round = (n: number) => Math.round(n * 100) / 100;
    const box = (el: Element): Box => {
      const r = el.getBoundingClientRect();
      return {
        l: round(r.left),
        t: round(r.top),
        r: round(r.right),
        b: round(r.bottom),
        w: round(r.width),
        h: round(r.height),
      };
    };
    const fromRect = (r: DOMRect): Box => ({
      l: round(r.left),
      t: round(r.top),
      r: round(r.right),
      b: round(r.bottom),
      w: round(r.width),
      h: round(r.height),
    });
    const overlaps = (a: Box, b: Box, tol = 0.5) =>
      Math.min(a.r, b.r) - Math.max(a.l, b.l) > tol && Math.min(a.b, b.b) - Math.max(a.t, b.t) > tol;
    const inside = (a: Box, b: Box, tol = 0.5) =>
      a.l >= b.l - tol && a.t >= b.t - tol && a.r <= b.r + tol && a.b <= b.b + tol;
    const shown = (el: Element) => {
      const r = el.getBoundingClientRect();
      if (r.width === 0 || r.height === 0) return false;
      for (let n: Element | null = el; n; n = n.parentElement) {
        const s = getComputedStyle(n);
        if (s.display === "none" || s.visibility === "hidden" || s.opacity === "0") return false;
      }
      return true;
    };
    const clip = (a: Box, c: Box): Box | null => {
      const l = Math.max(a.l, c.l);
      const t = Math.max(a.t, c.t);
      const r = Math.min(a.r, c.r);
      const b = Math.min(a.b, c.b);
      if (r - l < 0.5 || b - t < 0.5) return null;
      return { l, t, r, b, w: round(r - l), h: round(b - t) };
    };
    const textBoxes = (root: Element, skip = ".sr-only") => {
      const out: { text: string; box: Box }[] = [];
      const walker = document.createTreeWalker(root, NodeFilter.SHOW_TEXT);
      for (let node = walker.nextNode(); node; node = walker.nextNode()) {
        const text = (node.textContent ?? "").trim();
        const parent = node.parentElement;
        if (!text || !parent || parent.closest(skip) || !shown(parent)) continue;
        // 잘라 내는 조상들의 사각형 — 글자가 그 밖으로 나간 부분은 보이지 않음.
        const clips: Box[] = [];
        for (let n: Element | null = parent; n && n !== document.body; n = n.parentElement) {
          const s = getComputedStyle(n);
          if (s.overflowX !== "visible" || s.overflowY !== "visible") clips.push(box(n));
        }
        const range = document.createRange();
        range.selectNodeContents(node);
        for (const r of Array.from(range.getClientRects())) {
          let b: Box | null = fromRect(r);
          for (const c of clips) {
            b = b && clip(b, c);
          }
          if (b) out.push({ text, box: b });
        }
      }
      return out;
    };
    const fmt = (b: Box) => `[${b.l},${b.t} → ${b.r},${b.b} (${b.w}×${b.h})]`;
    window.__geo = { box, overlaps, inside, shown, textBoxes, fmt };
  });
}
