// 화면 배치 검사 — 가짜 백엔드 견본(?demo=…)을 창 크기별로 띄워 겹침·넘침·높이·정렬을 잼.
// - 픽셀 비교 없음. 위반은 "무엇이 · 어디서 · 몇 px"로 모아 빈 배열인지만 확인함.
// - 창 크기: 넓은 두 칸(1100), 좁은 격자(800·720 — 640~899px). 앱의 최소 창 너비가 720이라 그보다 좁게는 보지 않음.
// - 화면마다 스크린샷을 결과(test-results·HTML 보고서)에 붙임 — 사람이 훑어보는 용도.
import { expect, test, type Page, type TestInfo } from "@playwright/test";
import { installGeometry } from "./geometry";

const VIEWPORTS = [
  { width: 1100, height: 760 },
  { width: 800, height: 700 },
  { width: 720, height: 600 },
];

/** 움직임을 꺼 위치를 재는 순간마다 값이 같게 함. */
const NO_MOTION =
  "*,*::before,*::after{animation:none!important;transition:none!important;caret-color:transparent!important}";

async function open(page: Page, query: string) {
  await page.goto(`/${query}`, { waitUntil: "networkidle" });
  await page.addStyleTag({ content: NO_MOTION });
  await page.locator(".app-header").waitFor();
  await page.evaluate(() => document.fonts.ready.then(() => undefined));
  await installGeometry(page);
}

async function snap(page: Page, info: TestInfo, name: string) {
  const path = info.outputPath(`${name}.png`);
  await page.screenshot({ path });
  await info.attach(name, { path, contentType: "image/png" });
}

/** 가로로 넘치는 곳 — 문서, #root(넓은 창은 html이 넘침을 잘라 문서 값만으로는 못 봄), 각자 스크롤하는 패널. */
async function horizontalOverflow(page: Page, where: string): Promise<string[]> {
  const found = await page.evaluate(() => {
    const out: string[] = [];
    const iw = window.innerWidth;
    const doc = document.scrollingElement?.scrollWidth ?? 0;
    if (doc > iw) out.push(`문서 scrollWidth ${doc} > innerWidth ${iw}`);
    const root = document.getElementById("root")?.scrollWidth ?? 0;
    if (root > iw) out.push(`#root scrollWidth ${root} > innerWidth ${iw}`);
    for (const sel of [".run-list-panel", ".run-detail", ".settings-body", ".applog-scroll", ".sheet-body"])
      for (const el of Array.from(document.querySelectorAll(sel)))
        if (el.scrollWidth > el.clientWidth + 1) out.push(`${sel} scrollWidth ${el.scrollWidth} > ${el.clientWidth}`);
    return out;
  });
  return found.map((f) => `${where}: 가로 넘침 ${f}`);
}

for (const vp of VIEWPORTS) {
  const wide = vp.width >= 900;

  test.describe(`${vp.width}x${vp.height}`, () => {
    test.use({ viewport: vp });

    test("실행 목록 카드 — 같은 높이·글자 넘침·겹침 없음·긴 종목", async ({ page }, info) => {
      await open(page, "?demo=many");
      await expect(page.locator(".run-card")).toHaveCount(24);
      await snap(page, info, "run-list");

      const problems = await page.evaluate(() => {
        const g = window.__geo;
        const out: string[] = [];
        const cards = Array.from(document.querySelectorAll<HTMLElement>(".run-card"));
        const heights = cards.map((c) => g.box(c).h);
        const min = Math.min(...heights);
        const max = Math.max(...heights);
        if (max - min > 1) out.push(`카드 높이가 다름: ${min}~${max}px (${heights.join(",")})`);

        cards.forEach((card, i) => {
          const name = `카드 ${i + 1}(${card.querySelector(".run-card-symbol")?.textContent})`;
          const cb = g.box(card);
          const blockers = [".run-card-action", ".run-card-remove-slot"]
            .map((s) => card.querySelector(s))
            .filter((el): el is Element => !!el && g.shown(el))
            .map((el) => ({ cls: el.className, box: g.box(el) }));
          // 보이는 글자는 카드 안에 있고 주 버튼·× 자리와 겹치지 않음.
          for (const t of g.textBoxes(card.querySelector(".run-card-select")!)) {
            if (!g.inside(t.box, cb, 1))
              out.push(`${name}: "${t.text}" ${g.fmt(t.box)}가 카드 ${g.fmt(cb)} 밖으로 나감`);
            for (const bl of blockers)
              if (g.overlaps(t.box, bl.box))
                out.push(`${name}: "${t.text}" ${g.fmt(t.box)}가 ${bl.cls} ${g.fmt(bl.box)}와 겹침`);
          }
          // 같은 줄 칸끼리 겹치지 않음.
          const rows = [
            [".run-card-head > *:not(.sr-only)", ".run-card-top > :last-child:not(.run-card-head)"],
            [".run-card-status", ".run-card-status-rest", ".run-card-elapsed", ".run-card-strategy"],
          ];
          for (const sels of rows) {
            const els = sels
              .flatMap((s) => Array.from(card.querySelectorAll(s)))
              .filter((el) => g.shown(el))
              .map((el) => ({ cls: el.className, box: g.box(el) }));
            for (let a = 0; a < els.length; a++)
              for (let b = a + 1; b < els.length; b++)
                if (g.overlaps(els[a].box, els[b].box))
                  out.push(`${name}: ${els[a].cls} ${g.fmt(els[a].box)} ↔ ${els[b].cls} ${g.fmt(els[b].box)} 겹침`);
          }
          // 가로로 잘리면서 일부가 보이는 글자는 줄임표(…)로 끝남.
          // - 자리가 모자라면 둘째 줄로 밀어 통째로 가리는 칸(상태 뒷부분·전략·모드 배지)은 보이는 글자가 없어 빠짐.
          for (const el of Array.from(card.querySelectorAll<HTMLElement>(".run-card-select *"))) {
            if (!g.shown(el) || el.scrollWidth <= el.clientWidth) continue;
            if (getComputedStyle(el).overflowX === "visible") continue;
            if (g.textBoxes(el).length === 0) continue;
            if (getComputedStyle(el).textOverflow !== "ellipsis")
              out.push(`${name}: ${el.className} 잘리는데 줄임표 없음 (${el.scrollWidth} > ${el.clientWidth})`);
          }
        });
        return out;
      });
      expect(problems).toEqual([]);
    });

    test("긴 종목은 보이고, 모자랄 때만 줄임표·모드 점으로 줄어듦", async ({ page }) => {
      await open(page, "?demo=many");
      const rows = await page.evaluate(() => {
        const g = window.__geo;
        return Array.from(document.querySelectorAll<HTMLElement>(".run-card")).map((card) => {
          const sym = card.querySelector<HTMLElement>(".run-card-symbol")!;
          const head = card.querySelector<HTMLElement>(".run-card-head")!;
          const top = card.querySelector<HTMLElement>(".run-card-top")!;
          const end = top.lastElementChild !== head ? top.lastElementChild : null;
          const mode = card.querySelector<HTMLElement>(".run-card-mode")!;
          const badge = mode.querySelector<HTMLElement>(".mode-badge")!;
          const visibleText = g.textBoxes(sym).reduce((w, t) => w + t.box.w, 0);
          const gap = parseFloat(getComputedStyle(top).columnGap) || 0;
          const room = (end && g.shown(end) ? g.box(end).l : g.box(top).r) - gap;
          return {
            symbol: sym.textContent ?? "",
            visibleText,
            truncated: sym.scrollWidth > sym.clientWidth,
            ellipsis: getComputedStyle(sym).textOverflow === "ellipsis",
            // 배지가 둘째 줄로 밀려(가려져) 점만 남았는지.
            collapsed: g.box(badge).t - g.box(mode).t > 4 || g.box(mode).w < g.box(badge).w - 0.5,
            headRight: g.box(head).r,
            room: Math.round(room * 100) / 100,
            badgeInside: g.inside(g.box(badge), g.box(mode)),
          };
        });
      });
      const problems: string[] = [];
      for (const s of ["1000SHIBUSDT", "BTCDOMUSDT", "KRW-SHIB"]) {
        const r = rows.find((x) => x.symbol === s);
        if (!r) problems.push(`${s} 카드가 없음`);
        else if (r.visibleText < 40) problems.push(`${s}: 보이는 글자 폭 ${r.visibleText}px — 거의 잘림`);
      }
      for (const r of rows) {
        if (r.truncated && !r.ellipsis) problems.push(`${r.symbol}: 잘리는데 줄임표 없음`);
        // 점으로 줄었다면 머리가 오른쪽 칸까지 꽉 찼어야 함(자리가 남는데 줄면 안 됨).
        if (r.collapsed && r.headRight < r.room - 1)
          problems.push(`${r.symbol}: 모드 배지가 점으로 줄었는데 ${Math.round(r.room - r.headRight)}px가 남음`);
        // 종목이 잘렸다면 모드는 먼저 점으로 줄었어야 함.
        if (r.truncated && !r.collapsed) problems.push(`${r.symbol}: 종목이 잘렸는데 모드 배지는 그대로`);
        if (!r.collapsed && !r.badgeInside) problems.push(`${r.symbol}: 모드 배지가 제자리 밖으로 삐져나감`);
      }
      expect(problems).toEqual([]);
    });

    test("× 단추 — 가리킬 때·포커스일 때만 보이고 자리는 늘 비워 둠", async ({ page }) => {
      await open(page, "?demo=many");
      const card = page.locator(".run-card.has-remove").first();
      await card.scrollIntoViewIfNeeded();
      await page.mouse.move(0, 0);
      const measure = () =>
        card.evaluate((c) => {
          const g = window.__geo;
          const btn = c.querySelector(".run-card-remove")!;
          const slot = c.querySelector(".run-card-x-slot");
          return {
            opacity: getComputedStyle(btn).opacity,
            btn: g.box(btn),
            slot: slot ? g.box(slot) : null,
            texts: g.textBoxes(c.querySelector(".run-card-select")!).map((t) => `${t.text}@${g.fmt(t.box)}`),
          };
        });
      const before = await measure();
      expect(before.opacity, "가리키기 전 × 투명").toBe("0");
      expect(before.slot, "× 자리(.run-card-x-slot)가 있음").not.toBeNull();
      expect(before.slot!.w).toBeGreaterThanOrEqual(before.btn.w - 0.5);
      expect(Math.abs(before.slot!.l - before.btn.l), "× 자리와 × 단추 가로 위치").toBeLessThanOrEqual(1);
      expect(Math.abs(before.slot!.t - before.btn.t), "× 자리와 × 단추 세로 위치").toBeLessThanOrEqual(1);

      await card.locator(".run-card-symbol").hover();
      const hovered = await measure();
      expect(hovered.opacity, "가리키면 × 보임").toBe("1");
      expect(hovered.texts, "가리켜도 글자 자리가 그대로").toEqual(before.texts);

      await page.mouse.move(0, 0);
      await page.keyboard.press("Shift");
      await card.locator(".run-card-remove").focus();
      const focused = await measure();
      expect(focused.opacity, "키보드 포커스면 × 보임").toBe("1");
      expect(focused.texts, "포커스여도 글자 자리가 그대로").toEqual(before.texts);
    });

    for (const [label, query] of [
      ["대기(시작 단추)", "?open=1"],
      ["실행 중(종료 단추)", "?demo=running&open=1"],
    ] as const) {
      test(`상세 조작 줄 — ${label}: 세그먼트·주 단추 같은 높이·가운데·모드 설명 줄 없음`, async ({ page }, info) => {
        await open(page, query);
        await page.locator(".detail-controls").waitFor();
        await snap(page, info, "detail");
        const r = await page.evaluate(() => {
          const g = window.__geo;
          const row = document.querySelector(".detail-controls")!;
          const seg = row.querySelector(".segmented")!;
          const btn = row.querySelector(".detail-start, .detail-stop .btn, .btn")!;
          const hints = Array.from(
            document.querySelectorAll(".run-detail [class*='mode-hint'], .run-detail [class*='mode-note']"),
          )
            .filter((el) => g.shown(el))
            .map((el) => el.className);
          // 조작 줄 안에 세그먼트·단추 말고 보이는 글자가 있는지(설명 줄).
          const owners = [seg, ...Array.from(row.querySelectorAll(".btn"))].map((el) => g.box(el));
          const stray = g
            .textBoxes(row)
            .filter((t) => !owners.some((o) => g.inside(t.box, o, 1)))
            .map((t) => t.text);
          return { seg: g.box(seg), btn: g.box(btn), hints, stray, children: row.children.length };
        });
        expect(Math.abs(r.seg.h - r.btn.h), `높이 세그먼트 ${r.seg.h} · 단추 ${r.btn.h}`).toBeLessThanOrEqual(1);
        const segMid = (r.seg.t + r.seg.b) / 2;
        const btnMid = (r.btn.t + r.btn.b) / 2;
        expect(Math.abs(segMid - btnMid), `세로 가운데 세그먼트 ${segMid} · 단추 ${btnMid}`).toBeLessThanOrEqual(1);
        expect(r.hints, "모드 설명 줄").toEqual([]);
        expect(r.stray, "조작 줄 안 다른 글자").toEqual([]);
        expect(r.children, "조작 줄 칸 수(모드·단추)").toBe(2);
      });
    }

    test("상태 카드(?demo=pnl) — 막대·차트 꼬리표 안 겹침·표 정렬·가로 넘침 없음", async ({ page }, info) => {
      await open(page, "?demo=pnl&open=1");
      const problems: string[] = [];
      // 좁은 창은 상세가 열린 채 시작함 — 목록으로 돌아가 카드를 고름.
      if (!wide) await page.locator(".detail-back").click();
      await page.locator(".run-card").first().waitFor();
      const count = await page.locator(".run-card.is-live").count();
      expect(count, "실행 중 카드 수").toBeGreaterThanOrEqual(5);
      for (let i = 0; i < count; i++) {
        if (!wide && i > 0) await page.locator(".detail-back").click();
        await page.locator(".run-card.is-live .run-card-select").nth(i).click();
        await page.locator(".status-card").waitFor();
        // 그래프는 시세 점이 둘 이상 쌓여야 꼬리표가 생김.
        await page.locator(".price-chart:not(.is-empty)").waitFor({ timeout: 10_000 });
        const where = (await page.locator(".detail-title").innerText()).replace(/\s+/g, " ");
        problems.push(
          ...(await page.evaluate((where) => {
            const g = window.__geo;
            const out: string[] = [];
            const card = document.querySelector(".status-card")!;
            const pairwise = (sel: string, what: string) => {
              const els = Array.from(card.querySelectorAll(sel))
                .filter((el) => g.shown(el))
                .map((el) => ({ text: el.textContent?.trim(), box: g.box(el) }));
              for (let a = 0; a < els.length; a++)
                for (let b = a + 1; b < els.length; b++)
                  if (g.overlaps(els[a].box, els[b].box, 0))
                    out.push(
                      `${where} ${what}: "${els[a].text}" ${g.fmt(els[a].box)} ↔ "${els[b].text}" ${g.fmt(els[b].box)} 겹침`,
                    );
              return els;
            };
            const bar = card.querySelector(".exit-bar");
            if (bar) {
              const bb = g.box(bar);
              for (const t of [
                ...pairwise(".exit-tag", "청산 막대 위 꼬리표"),
                ...pairwise(".exit-end", "청산 막대 양끝"),
              ])
                if (!g.inside(t.box, bb, 1))
                  out.push(`${where} 청산 막대: "${t.text}" ${g.fmt(t.box)}가 막대 ${g.fmt(bb)} 밖`);
            }
            const chart = card.querySelector(".price-chart");
            if (chart) {
              const cb = g.box(card);
              for (const t of pairwise(".chart-extreme, .chart-tag, .chart-span", "그래프 꼬리표"))
                if (!g.inside(t.box, cb, 1)) out.push(`${where} 그래프: "${t.text}" ${g.fmt(t.box)}가 카드 밖`);
            }
            // 표 — 같은 줄의 두 칸은 위·높이가 같고, 열마다 이름 왼쪽·값 오른쪽 끝이 맞음.
            const rows = Array.from(card.querySelectorAll(".run-table > div"));
            const cols = new Map<number, { dt: number[]; dd: number[] }>();
            const lines = new Map<number, number[]>();
            for (const row of rows) {
              const rb = g.box(row);
              const col = Math.round(rb.l);
              const c = cols.get(col) ?? { dt: [], dd: [] };
              c.dt.push(g.box(row.querySelector("dt")!).l);
              c.dd.push(g.box(row.querySelector("dd")!).r);
              cols.set(col, c);
              const key = Math.round(rb.t);
              lines.set(key, [...(lines.get(key) ?? []), rb.h]);
              const dd = row.querySelector<HTMLElement>("dd")!;
              if (dd.scrollWidth > dd.clientWidth + 1)
                out.push(`${where} 표: "${dd.textContent}" 값이 잘림 (${dd.scrollWidth} > ${dd.clientWidth})`);
            }
            const spread = (xs: number[]) => Math.max(...xs) - Math.min(...xs);
            for (const [col, c] of cols) {
              if (spread(c.dt) > 1) out.push(`${where} 표 열 ${col}: 이름 왼쪽 끝이 어긋남 ${c.dt.join(",")}`);
              if (spread(c.dd) > 1) out.push(`${where} 표 열 ${col}: 값 오른쪽 끝이 어긋남 ${c.dd.join(",")}`);
            }
            if (cols.size > 1)
              for (const [top, hs] of lines)
                if (hs.length > 1 && spread(hs) > 1)
                  out.push(`${where} 표 줄 y=${top}: 두 칸 높이가 다름 ${hs.join(",")}`);
            if (lines.size * cols.size !== rows.length)
              out.push(`${where} 표: 줄이 어긋남 — 칸 ${rows.length}개가 ${lines.size}줄 × ${cols.size}열에 맞지 않음`);
            return out;
          }, where)),
        );
        problems.push(...(await horizontalOverflow(page, where)));
        if (i === 0) await snap(page, info, "status-card");
      }
      expect(problems).toEqual([]);
    });

    test("활동 기록 — › 펼치기 단추가 제목 줄 안에 있고 aria-expanded가 바뀜", async ({ page }) => {
      await open(page, wide ? "?demo=many" : "?demo=many&open=1");
      const toggle = page.locator(".act-toggle").first();
      await toggle.waitFor();
      await toggle.scrollIntoViewIfNeeded();
      const r = await toggle.evaluate((btn) => {
        const g = window.__geo;
        const title = btn.closest(".act-title");
        if (!title) return { inTitle: false, problems: ["펼치기 단추가 .act-title 밖에 있음"] };
        const lines = g.textBoxes(title).map((t) => t.box);
        const last = lines.reduce((a, b) => (b.b > a.b || (b.b === a.b && b.r > a.r) ? b : a));
        const bb = g.box(btn);
        const mid = (bb.t + bb.b) / 2;
        const problems: string[] = [];
        if (mid < last.t - 1 || mid > last.b + 1)
          problems.push(`› 세로 가운데 ${mid}가 제목 마지막 줄 ${g.fmt(last)} 밖`);
        if (bb.l < last.r - 1) problems.push(`› 왼쪽 ${bb.l}가 제목 글자 끝 ${last.r}보다 앞`);
        return { inTitle: true, problems };
      });
      expect(r.problems).toEqual([]);
      await expect(toggle).toHaveAttribute("aria-expanded", "false");
      const raw = page.locator(`#${await toggle.getAttribute("aria-controls")}`);
      await toggle.click();
      await expect(toggle).toHaveAttribute("aria-expanded", "true");
      await expect(raw).toBeVisible();
      await toggle.click();
      await expect(toggle).toHaveAttribute("aria-expanded", "false");
    });

    test("설정 — 넓은 창은 문서가 스크롤하지 않고 본문만 굴러감·앱 기록 높이 제한", async ({ page }, info) => {
      await open(page, "?demo=settings");
      await page.locator(".settings-body").waitFor();
      await snap(page, info, "settings");
      expect(await horizontalOverflow(page, "설정")).toEqual([]);
      if (wide) {
        const doc = await page.evaluate(() => ({ sh: document.documentElement.scrollHeight, ih: window.innerHeight }));
        expect(doc.sh, `html scrollHeight ${doc.sh} = innerHeight ${doc.ih}`).toBe(doc.ih);
        await page.getByRole("navigation", { name: "설정 목차" }).getByRole("button", { name: "앱 기록" }).click();
        await expect.poll(() => page.locator(".settings-body").evaluate((el) => el.scrollTop)).toBeGreaterThan(0);
        const after = await page.evaluate(() => ({
          docTop: document.scrollingElement?.scrollTop ?? 0,
          winY: window.scrollY,
          headerTop: document.querySelector(".app-header")!.getBoundingClientRect().top,
        }));
        expect(after, "목차를 눌러도 문서·머리줄은 그대로").toEqual({ docTop: 0, winY: 0, headerTop: 0 });
      } else {
        await expect(page.locator(".settings-nav")).toBeHidden();
      }
      const log = page.locator(".applog-scroll");
      await log.scrollIntoViewIfNeeded();
      const box = await log.evaluate((el) => ({
        h: el.getBoundingClientRect().height,
        maxH: getComputedStyle(el).maxHeight,
        overflowY: getComputedStyle(el).overflowY,
      }));
      expect(box.maxH, "앱 기록 max-height").not.toBe("none");
      expect(box.h).toBeLessThanOrEqual(parseFloat(box.maxH) + 0.5);
      expect(["auto", "scroll"]).toContain(box.overflowY);
    });

    test("알림 토스트 — 창 안에 뜨고 열린 시트와 겹쳐 그려지지 않음", async ({ page }, info) => {
      await open(page, "?demo=applog");
      const toast = page.locator(".toast").first();
      await toast.waitFor();
      await snap(page, info, "applog-toast");
      const inView = await toast.evaluate((el) => {
        const g = window.__geo;
        const b = g.box(el);
        const header = g.box(document.querySelector(".app-header")!);
        const out: string[] = [];
        if (!g.inside(b, { l: 0, t: 0, r: innerWidth, b: innerHeight, w: innerWidth, h: innerHeight }))
          out.push(`토스트 ${g.fmt(b)}가 창 ${innerWidth}×${innerHeight} 밖`);
        if (b.t < header.b) out.push(`토스트 위 ${b.t}가 머리줄 아래 ${header.b}보다 위`);
        return out;
      });
      expect(inView).toEqual([]);
      expect(await horizontalOverflow(page, "앱 기록")).toEqual([]);

      // 매크로 추가 시트를 연 채 알림을 띄움([폴더 열기] → 미리보기 알림).
      await open(page, "?demo=library");
      const sheet = page.locator(".side-sheet");
      await sheet.waitFor();
      await sheet.getByRole("button", { name: "폴더 열기" }).click();
      await page.locator(".toast").first().waitFor();
      await snap(page, info, "library-toast");
      const r = await page.evaluate(() => {
        const g = window.__geo;
        const sb = g.box(document.querySelector(".side-sheet")!);
        return Array.from(document.querySelectorAll(".toast")).flatMap((t) => {
          const tb = g.box(t);
          const out: string[] = [];
          if (!g.inside(tb, { l: 0, t: 0, r: innerWidth, b: innerHeight, w: innerWidth, h: innerHeight }))
            out.push(`토스트 ${g.fmt(tb)}가 창 밖`);
          if (!g.overlaps(tb, sb)) return out;
          // 겹치는 곳에서는 시트가 위에 그려져야 함(토스트가 시트를 가리면 안 됨).
          const x = (Math.max(tb.l, sb.l) + Math.min(tb.r, sb.r)) / 2;
          const y = (Math.max(tb.t, sb.t) + Math.min(tb.b, sb.b)) / 2;
          const hit = document.elementFromPoint(x, y);
          if (hit && t.contains(hit)) out.push(`토스트 ${g.fmt(tb)}가 시트 ${g.fmt(sb)} 위에 겹쳐 그려짐`);
          return out;
        });
      });
      expect(r).toEqual([]);
    });

    test("가로 넘침 없음 — 주요 화면", async ({ page }) => {
      const problems: string[] = [];
      for (const q of [
        "?demo=many",
        "?demo=many&open=1",
        "?demo=pnl&open=1",
        "?demo=running&open=1",
        "?demo=library",
      ]) {
        await open(page, q);
        await page.locator(".run-card, .run-detail, .side-sheet").first().waitFor();
        problems.push(...(await horizontalOverflow(page, q)));
      }
      expect(problems).toEqual([]);
    });
  });
}
