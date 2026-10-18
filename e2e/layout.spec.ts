// 화면 배치 검사 — 가짜 백엔드 견본(?demo=…)을 창 크기별로 띄워 겹침·넘침·높이·정렬을 잼.
// - 픽셀 비교 없음. 위반은 "무엇이 · 어디서 · 몇 px"로 모아 빈 배열인지만 확인함.
// - 창 크기: 넓은 두 칸(1100), 좁은 격자(800·720 — 640~899px). 앱의 최소 창 너비가 720이라 그보다 좁게는 보지 않음.
//   상세 두 칸(상태 | 활동 기록, 1360px~)은 맨 아래 1440×900에서 따로 봄.
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

async function open(page: Page, query: string, { details = "open" }: { details?: "open" | "closed" } = {}) {
  // 상태 카드의 세부 표는 기본으로 접혀 있다 — 표 정렬을 재는 검사는 펼친 상태로, 높이 예산은 접힌 기본 상태로 본다.
  await page.addInitScript((value) => window.localStorage.setItem("ggp.runner.status.details", value), details);
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
      // 대기 묶음은 처음엔 세 줄만 — 나머지는 [N개 모두 보기]로 펼침(시안 2절).
      const idleTotal = Number(await page.locator("#run-group-idle .run-group-count").innerText());
      await expect(page.locator("#run-group-idle .run-card")).toHaveCount(3);
      await expect(page.locator(".run-card")).toHaveCount(24 - (idleTotal - 3));
      await page.getByRole("button", { name: `${idleTotal}개 모두 보기` }).click();
      await expect(page.locator(".run-card")).toHaveCount(24);
      await snap(page, info, "run-list");

      const problems = await page.evaluate(() => {
        const g = window.__geo;
        const out: string[] = [];
        const cards = Array.from(document.querySelectorAll<HTMLElement>(".run-card"));
        // 두 줄 카드(확인 필요 · 실행 중)끼리, 한 줄 행(대기)끼리 높이가 같음.
        for (const [what, sel] of [
          ["두 줄 카드", ".run-card:not(.is-compact)"],
          ["한 줄 행", ".run-card.is-compact"],
        ] as const) {
          const heights = Array.from(document.querySelectorAll<HTMLElement>(sel)).map((c) => g.box(c).h);
          if (!heights.length) continue;
          const min = Math.min(...heights);
          const max = Math.max(...heights);
          if (max - min > 1) out.push(`${what} 높이가 다름: ${min}~${max}px (${heights.join(",")})`);
        }

        cards.forEach((card, i) => {
          const name = `카드 ${i + 1}(${card.querySelector(".run-card-symbol")?.textContent})`;
          const cb = g.box(card);
          // 한 줄 행의 [▶] · [×]는 가리킬 때 글자 끝 위에 겹쳐 뜨는 게 설계 — 두 줄 카드만 비켜 감을 잼.
          const blockers = (card.classList.contains("is-compact") ? [] : [".run-card-action", ".run-card-remove-slot"])
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
          // 코인 표시는 줄지 않음(두 줄 카드 32px · 한 줄 행 20px). 거래소 작은 로고는 오른쪽 아래 모서리에 3px 걸치고 카드 안.
          const size = card.classList.contains("is-compact") ? 20 : 32;
          const mark = card.querySelector(".coin-mark");
          if (!mark) out.push(`${name}: 코인 표시 없음`);
          else {
            const mb = g.box(mark);
            if (Math.abs(mb.w - size) > 0.5 || Math.abs(mb.h - size) > 0.5)
              out.push(`${name}: 코인 표시 ${mb.w}×${mb.h} — ${size}px이어야 함`);
            const ex = mark.querySelector(".coin-mark-ex");
            if (ex) {
              const eb = g.box(ex);
              if (Math.abs(eb.r - (mb.r + 3)) > 1 || Math.abs(eb.b - (mb.b + 3)) > 1)
                out.push(`${name}: 거래소 작은 로고 ${g.fmt(eb)}가 코인 표시 ${g.fmt(mb)} 모서리에 있지 않음`);
              if (!g.inside(eb, cb, 0.5)) out.push(`${name}: 거래소 작은 로고 ${g.fmt(eb)}가 카드 밖`);
            }
          }
          // 종목 글자는 기초 자산만(시장 쌍 KRW-BTC · BTCUSDT 없이), 바이낸스 선물의 '선물 N배'는 잘리지 않음.
          const ticker = card.querySelector(".run-card-symbol")?.textContent ?? "";
          if (/USDT$|^KRW|[-_]/.test(ticker)) out.push(`${name}: 종목 글자에 시장 쌍`);
          const tag = card.querySelector<HTMLElement>(".run-card-tag");
          if (tag && (!g.shown(tag) || tag.scrollWidth > tag.clientWidth + 0.5))
            out.push(`${name}: '${tag.textContent}'가 잘리거나 가려짐`);
          // 1줄 오른쪽 칸(손익 % · 진입 대기 거리)은 한 줄 — 둘째 줄로 떨어지지 않음.
          const end = card.querySelector(".run-card-top > :last-child:not(.run-card-head)");
          if (end && g.shown(end)) {
            const lines = new Set(g.textBoxes(end).map((t) => Math.round(t.box.t)));
            if (lines.size > 1) out.push(`${name}: 오른쪽 칸 "${end.textContent}"이 ${lines.size}줄`);
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
        // 모드 배지는 기본(모의)이 아닌 두 줄 카드에만 — 점으로 줄어드는 규칙은 그 카드들로 잼.
        return Array.from(document.querySelectorAll<HTMLElement>(".run-card:not(.is-compact):has(.run-card-mode)")).map(
          (card) => {
            const sym = card.querySelector<HTMLElement>(".run-card-symbol")!;
            const head = card.querySelector<HTMLElement>(".run-card-head")!;
            const top = card.querySelector<HTMLElement>(".run-card-top")!;
            const end = top.lastElementChild !== head ? top.lastElementChild : null;
            const mode = card.querySelector<HTMLElement>(".run-card-mode")!;
            const badge = mode.querySelector<HTMLElement>(".mode-badge")!;
            const visibleText = g.textBoxes(sym).reduce((w, t) => w + t.box.w, 0);
            const fullText = sym.scrollWidth;
            const gap = parseFloat(getComputedStyle(top).columnGap) || 0;
            const room = (end && g.shown(end) ? g.box(end).l : g.box(top).r) - gap;
            return {
              symbol: sym.textContent ?? "",
              visibleText,
              fullText,
              truncated: sym.scrollWidth > sym.clientWidth,
              ellipsis: getComputedStyle(sym).textOverflow === "ellipsis",
              // 배지가 둘째 줄로 밀려(가려져) 점만 남았는지.
              collapsed: g.box(badge).t - g.box(mode).t > 4 || g.box(mode).w < g.box(badge).w - 0.5,
              headRight: g.box(head).r,
              room: Math.round(room * 100) / 100,
              badgeInside: g.inside(g.box(badge), g.box(mode)),
            };
          },
        );
      });
      const problems: string[] = [];
      // 종목은 기초 자산(1000SHIBUSDT → 1000SHIB) — 짧은 종목은 40px보다 좁을 수 있어 글자 전체 폭과 견줌.
      const nearlyCut = (visible: number, full: number) => visible < Math.min(40, full - 1);
      for (const s of ["1000SHIB"]) {
        const r = rows.find((x) => x.symbol === s);
        if (!r) problems.push(`${s} 카드가 없음`);
        else if (nearlyCut(r.visibleText, r.fullText))
          problems.push(`${s}: 보이는 글자 폭 ${r.visibleText}px — 거의 잘림`);
      }
      // 대기 묶음의 한 줄 행도 긴 종목이 거의 다 보임(넓은 창은 목록이 따로 굴러가니 행마다 보이게 굴린 뒤 잼).
      const compact = await page.evaluate(() =>
        Array.from(document.querySelectorAll<HTMLElement>(".run-card.is-compact .run-card-symbol")).map((sym) => {
          sym.scrollIntoView({ block: "nearest" });
          return {
            symbol: sym.textContent ?? "",
            visible: window.__geo.textBoxes(sym).reduce((w, t) => w + t.box.w, 0),
            full: sym.scrollWidth,
          };
        }),
      );
      // BTCDOMUSDT → BTCDOM, KRW-SHIB → SHIB
      for (const s of ["BTCDOM", "SHIB"]) {
        const r = compact.find((x) => x.symbol === s);
        if (!r) problems.push(`${s} 행이 없음`);
        else if (nearlyCut(r.visible, r.full)) problems.push(`${s}: 보이는 글자 폭 ${r.visible}px — 거의 잘림`);
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

    test("× 단추 — 가리킬 때·포커스일 때만 보이고, 숨어 있을 때는 눌리지 않음", async ({ page }) => {
      await open(page, "?demo=many");
      const card = page.locator(".run-card.has-remove").first();
      await card.scrollIntoViewIfNeeded();
      await page.mouse.move(0, 0);
      const measure = () =>
        card.evaluate((c) => {
          const g = window.__geo;
          const btn = c.querySelector(".run-card-remove")!;
          const slot = c.querySelector(".run-card-x-slot");
          // 글자 자리는 카드 기준 — 가리키거나 포커스를 줄 때 브라우저가 행을 화면 안으로 굴려도(WebKit) 같은 값.
          const cb = g.box(c);
          const at = (n: number) => Math.round(n * 100) / 100;
          const inCard = (b: typeof cb) => ({
            l: at(b.l - cb.l),
            t: at(b.t - cb.t),
            r: at(b.r - cb.l),
            b: at(b.b - cb.t),
            w: b.w,
            h: b.h,
          });
          return {
            compact: c.classList.contains("is-compact"),
            opacity: getComputedStyle(btn).opacity,
            events: getComputedStyle(btn).pointerEvents,
            btn: g.box(btn),
            slot: slot ? g.box(slot) : null,
            texts: g.textBoxes(c.querySelector(".run-card-select")!).map((t) => `${t.text}@${g.fmt(inCard(t.box))}`),
          };
        });
      const before = await measure();
      expect(before.opacity, "가리키기 전 × 투명").toBe("0");
      if (before.compact) {
        // 한 줄 행 — 자리를 비우지 않고 겹쳐 뜸. 숨어 있을 때는 눌리지 않음(행을 고르는 클릭을 가로채지 않게).
        expect(before.events, "숨은 × 는 눌리지 않음").toBe("none");
      } else {
        expect(before.slot, "× 자리(.run-card-x-slot)가 있음").not.toBeNull();
        expect(before.slot!.w).toBeGreaterThanOrEqual(before.btn.w - 0.5);
        expect(Math.abs(before.slot!.l - before.btn.l), "× 자리와 × 단추 가로 위치").toBeLessThanOrEqual(1);
        expect(Math.abs(before.slot!.t - before.btn.t), "× 자리와 × 단추 세로 위치").toBeLessThanOrEqual(1);
      }

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

    test("상세 조작 줄 — 대기(시작 단추): 세그먼트·주 단추 같은 높이·가운데, 모드 설명은 그 아래 한 줄", async ({
      page,
    }, info) => {
      await open(page, "?open=1");
      await page.locator(".detail-controls").waitFor();
      await snap(page, info, "detail");
      const r = await page.evaluate(() => {
        const g = window.__geo;
        const row = document.querySelector(".detail-controls")!;
        const seg = row.querySelector(".segmented")!;
        const btn = row.querySelector(".detail-start, .detail-stop .btn, .btn")!;
        // 모드 설명(MODE_NOTES)은 조작 줄 밖, 같은 카드 안 아래 한 줄.
        const notes = Array.from(document.querySelectorAll(".run-detail .mode-note")).filter((el) => g.shown(el));
        const note = notes[0];
        const noteLines = note ? new Set(g.textBoxes(note).map((t) => Math.round(t.box.t))).size : 0;
        const hints = {
          count: notes.length,
          below: note ? g.box(note).t >= g.box(row).b - 1 : false,
          inCard: note ? note.parentElement === row.parentElement : false,
          lines: noteLines,
        };
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
      expect(r.hints, "모드 설명 줄").toEqual({ count: 1, below: true, inCard: true, lines: 1 });
      expect(r.stray, "조작 줄 안 다른 글자").toEqual([]);
      expect(r.children, "조작 줄 칸 수(모드·단추)").toBe(2);
    });

    test("상세 조작 줄 — 실행 중: 세그먼트 없이 상태 한 줄(모드 배지 · 경과 · 서버) + 종료 단추, 큰 빈 자리 없음", async ({
      page,
    }, info) => {
      await open(page, "?demo=running&open=1");
      await page.locator(".detail-controls.is-running").waitFor();
      await snap(page, info, "detail-running");
      const r = await page.evaluate(() => {
        const g = window.__geo;
        const row = document.querySelector(".detail-controls")!;
        const left = row.querySelector(".run-bar")!;
        const badge = left.querySelector(".detail-mode-badge")!;
        const btns = Array.from(row.querySelectorAll(".btn"));
        // 상태 한 줄의 칸(배지 · 저하 칩 · 경과 · 봉 마감 · 서버)은 각자 한 줄 — 칸 밖 글자가 없어야 함.
        const owners = [...Array.from(left.children), ...btns].map((el) => g.box(el));
        const wrapped = Array.from(left.children)
          .filter(
            (el) => g.textBoxes(el).length > 1 && new Set(g.textBoxes(el).map((t) => Math.round(t.box.t))).size > 1,
          )
          .map((el) => el.className);
        const stray = g
          .textBoxes(row)
          .filter((t) => !owners.some((o) => g.inside(t.box, o, 1)))
          .map((t) => t.text);
        const pad = parseFloat(getComputedStyle(row).paddingLeft);
        const firstBtn = g.box(btns[0]);
        return {
          segmented: row.querySelectorAll(".segmented, [role='radiogroup']").length,
          badge: badge.textContent,
          children: row.children.length,
          stray,
          left: g.box(left),
          btn: firstBtn,
          row: g.box(row),
          pad,
          wrapped,
        };
      });
      expect(r.segmented, "실행 중 세그먼트").toBe(0);
      expect(r.badge).toMatch(/^(모의로|테스트넷으로|실전으로) (실행 중|시작하는 중)$/);
      expect(r.children, "조작 줄 칸 수(배지 칸·단추)").toBe(2);
      expect(r.stray, "조작 줄 안 다른 글자").toEqual([]);
      expect(r.wrapped, "두 줄로 꺾인 칸").toEqual([]);
      // 왼쪽 칸이 단추 앞까지 채움 — 큰 빈 자리 없음(같은 줄일 때).
      if (Math.abs(r.left.t - r.btn.t) < r.btn.h)
        expect(r.btn.l - r.left.r, `왼쪽 칸 끝 ${r.left.r} · 단추 ${r.btn.l}`).toBeLessThanOrEqual(24);
      const leftMid = (r.left.t + r.left.b) / 2;
      const btnMid = (r.btn.t + r.btn.b) / 2;
      if (Math.abs(r.left.t - r.btn.t) < r.btn.h)
        expect(Math.abs(leftMid - btnMid), "배지 칸과 단추 세로 가운데").toBeLessThanOrEqual(2);
    });

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
              // 막대의 모든 글자(머리·거리·현재가 꼬리표·평단 이름·양 끝)는 서로 겹치지 않고 막대 칸 안.
              for (const t of pairwise(".exit-head, .exit-dist, .exit-tag, .exit-entry, .exit-end", "청산 막대 글자"))
                if (!g.inside(t.box, bb, 1))
                  out.push(`${where} 청산 막대: "${t.text}" ${g.fmt(t.box)}가 막대 ${g.fmt(bb)} 밖`);
              // 꼬리표·평단 이름은 막대 칸(.exit-scale) 안에서 잘리지 않음.
              const scale = g.box(bar.querySelector(".exit-scale")!);
              for (const el of Array.from(bar.querySelectorAll<HTMLElement>(".exit-tag, .exit-entry"))) {
                if (!g.inside(g.box(el), scale, 1))
                  out.push(`${where} 청산 막대: "${el.textContent}" ${g.fmt(g.box(el))}가 막대 칸 ${g.fmt(scale)} 밖`);
                if (el.scrollWidth > el.clientWidth + 1) out.push(`${where} 청산 막대: "${el.textContent}" 잘림`);
              }
              // 평단 눈금 이름은 눈금 아래, 현재가 꼬리표는 점 위에 걸침.
              const tick = g.box(bar.querySelector(".exit-tick")!);
              const entry = g.box(bar.querySelector(".exit-entry")!);
              const tickX = (tick.l + tick.r) / 2;
              if (entry.t < tick.b - 1 || tickX < entry.l - 1 || tickX > entry.r + 1)
                out.push(`${where} 청산 막대: 평단 이름 ${g.fmt(entry)}가 눈금 ${g.fmt(tick)} 아래에 없음`);
              const dot = g.box(bar.querySelector(".exit-dot")!);
              const tag = g.box(bar.querySelector(".exit-tag")!);
              const dotX = (dot.l + dot.r) / 2;
              if (tag.b > dot.t + 1 || dotX < tag.l - 1 || dotX > tag.r + 1)
                out.push(`${where} 청산 막대: 현재가 꼬리표 ${g.fmt(tag)}가 점 ${g.fmt(dot)} 위에 걸치지 않음`);
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

    test("활동 기록 — 한 건 한 줄, › 펼치기 단추가 그 줄 끝에 있고 aria-expanded가 바뀜", async ({ page }) => {
      await open(page, wide ? "?demo=many" : "?demo=many&open=1");
      const toggle = page.locator(".act-toggle").first();
      await toggle.waitFor();
      await toggle.scrollIntoViewIfNeeded();
      const r = await toggle.evaluate((btn) => {
        const g = window.__geo;
        const title = btn.closest(".act-line");
        if (!title) return { inTitle: false, problems: ["펼치기 단추가 .act-line 밖에 있음"] };
        const lines = g.textBoxes(title).map((t) => t.box);
        // 접힌 줄은 한 줄 — 제목 · 둘째 말이 다음 줄로 꺾이지 않음.
        // 줄은 세로 가운데로 셈: 제목(13px) · 둘째 말(12px) 칸은 높이가 달라 위 끝이 1px 안쪽으로 어긋나고,
        // 위 끝을 반올림하면 창 안 위치에 따라 같은 줄이 두 값으로 갈림(CI 글꼴 · WebKit). 반 줄(6px) 넘게 벌어질 때만 다른 줄.
        const mids = lines.map((b) => (b.t + b.b) / 2).sort((a, b) => a - b);
        const rows = mids.filter((m, i) => i === 0 || m - mids[i - 1] > 6).length;
        const last = lines.reduce((a, b) => (b.b > a.b || (b.b === a.b && b.r > a.r) ? b : a));
        const bb = g.box(btn);
        const mid = (bb.t + bb.b) / 2;
        const problems: string[] = [];
        if (mid < last.t - 1 || mid > last.b + 1)
          problems.push(`› 세로 가운데 ${mid}가 제목 마지막 줄 ${g.fmt(last)} 밖`);
        if (bb.l < last.r - 1) problems.push(`› 왼쪽 ${bb.l}가 제목 글자 끝 ${last.r}보다 앞`);
        if (rows > 1) problems.push(`접힌 줄이 ${rows}줄로 꺾임`);
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
        // 목차는 보이는 묶음을 밝힘 — 누른 곳, 그리고 맨 위로 굴리면 다시 거래소 연결.
        const nav = page.getByRole("navigation", { name: "설정 목차" });
        await expect(nav.getByRole("button", { name: "앱 기록" })).toHaveAttribute("aria-current", "true");
        // 사용자가 직접 굴리면(휠) 다시 보이는 묶음을 따라감.
        const bodyBox = (await page.locator(".settings-content").boundingBox())!;
        await page.mouse.move(bodyBox.x + bodyBox.width / 2, 300);
        for (let k = 0; k < 40; k++) await page.mouse.wheel(0, -600);
        await expect(nav.getByRole("button", { name: "거래소 연결" })).toHaveAttribute("aria-current", "true");
        await expect(nav.locator("[aria-current='true']")).toHaveCount(1);
      } else {
        await expect(page.locator(".settings-nav")).toBeHidden();
      }
      // 거래소마다 같은 폭의 한 줄 — 키 칸은 처음엔 모두 닫힘. 쓰지 않는 연습 칸은 뒤 '연습 칸'에 낮게, 머리는 한 줄.
      const slots = await page.evaluate(() =>
        Array.from(document.querySelectorAll<HTMLElement>(".slot-card")).map((el) => {
          const list = el.closest(".slot-list")!.getBoundingClientRect();
          const r = el.getBoundingClientRect();
          const mids = Array.from(el.querySelectorAll(".slot-head > *")).map((c) => {
            const b = c.getBoundingClientRect();
            return (b.top + b.bottom) / 2;
          });
          const side = el.querySelector(".slot-side")!.getBoundingClientRect();
          const toggle = el.querySelector(".slot-toggle")!.getBoundingClientRect();
          return {
            id: el.id,
            practice: !!el.closest(".slot-list.is-practice"),
            open: el.querySelector(".slot-toggle")!.getAttribute("aria-expanded") === "true",
            h: r.height,
            fullWidth: Math.abs(r.width - list.width) <= 2,
            midSpread: Math.max(...mids) - Math.min(...mids),
            // 잔고 · [연결 도우미] 칸은 ▾ 바로 왼쪽(오른쪽 끝)에 붙음.
            sideGap: toggle.left - side.right,
          };
        }),
      );
      const firstPractice = slots.findIndex((x) => x.practice);
      expect(firstPractice, "연습 칸이 있음(?demo=settings 테스트넷 · 데모)").toBeGreaterThan(0);
      expect(
        slots.slice(firstPractice).every((x) => x.practice),
        "쓰는 칸이 앞, 연습 칸이 뒤",
      ).toBe(true);
      for (const x of slots) {
        expect(x.open, `${x.id} 키 칸은 처음엔 닫힘`).toBe(false);
        expect(x.fullWidth, `${x.id} 목록 폭을 다 씀`).toBe(true);
        expect(x.midSpread, `${x.id} 머리 칸이 한 줄(세로 가운데 차이)`).toBeLessThanOrEqual(4);
        expect(x.sideGap, `${x.id} 오른쪽 칸과 ▾ 사이`).toBeLessThanOrEqual(24);
      }
      for (const x of slots.filter((y) => y.practice)) expect(x.h, `${x.id} 연습 칸 높이`).toBeLessThanOrEqual(56);
      const firstId = slots[0].id;
      const toggle = page.locator(`#${firstId} .slot-toggle`);
      await expect(toggle).toHaveAttribute("aria-expanded", "false");
      await toggle.click();
      await expect(toggle).toHaveAttribute("aria-expanded", "true");
      await expect(page.locator(`#${await toggle.getAttribute("aria-controls")}`)).toBeVisible();
      // 연결됨이어도 고쳐야 할 안내(업비트 잔고 부족)는 주의 색 줄, 잔고는 오른쪽 끝에 주의 색.
      await expect(page.locator("#settings-slot-upbit .slot-why")).toContainText("잔고가 모자라요");
      await expect(page.locator("#settings-slot-upbit .slot-balance.is-short")).toContainText("KRW");

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

    test("알림 토스트 — 연결 도우미가 열려 있어도 어두운 막 위에 보임", async ({ page }, info) => {
      // '이미 발급받았어요'를 고르면 API 관리 페이지를 열었다는 알림이 뜸(도우미는 키 넣기 단계로 길어짐).
      await open(page, "?demo=wizard");
      await page.locator(".wizard").waitFor();
      await page.getByRole("button", { name: /이미 발급받았어요/ }).click();
      const toast = page.locator(".toast").first();
      await toast.waitFor();
      await snap(page, info, "wizard-toast");
      const r = await toast.evaluate((el) => {
        const g = window.__geo;
        const tb = g.box(el);
        const out: string[] = [];
        if (!g.inside(tb, { l: 0, t: 0, r: innerWidth, b: innerHeight, w: innerWidth, h: innerHeight }))
          out.push(`토스트 ${g.fmt(tb)}가 창 밖`);
        // 토스트 가운데에서 토스트가 잡혀야 함 — 막 뒤에 깔려 있으면 막이 잡힘.
        const hit = document.elementFromPoint((tb.l + tb.r) / 2, (tb.t + tb.b) / 2);
        if (!hit || !el.contains(hit)) out.push(`토스트 ${g.fmt(tb)}가 막 뒤에 깔림(잡힌 것: ${hit?.className})`);
        return out;
      });
      expect(r).toEqual([]);
    });

    test("매크로 추가 시트 — 위 단추 · 아래 폴더 줄은 고정, 목록만 안에서 굴러감", async ({ page }, info) => {
      await open(page, "?demo=library");
      await page.locator(".side-sheet").waitFor();
      await snap(page, info, "library");
      const before = await page.evaluate(() => {
        const g = window.__geo;
        const sheet = g.box(document.querySelector(".side-sheet")!);
        const body = document.querySelector<HTMLElement>(".lib-scroll")!;
        return {
          sheet,
          ih: innerHeight,
          actions: g.box(document.querySelector(".lib-actions")!),
          foot: g.box(document.querySelector(".lib-foot")!),
          overflow: getComputedStyle(body).overflowY,
          scrollable: body.scrollHeight > body.clientHeight,
        };
      });
      expect(before.sheet.b, "시트가 창 아래로 넘치지 않음").toBeLessThanOrEqual(before.ih + 0.5);
      expect(before.foot.b, "아래 폴더 줄이 창 안").toBeLessThanOrEqual(before.ih + 0.5);
      expect(["auto", "scroll"]).toContain(before.overflow);
      if (before.scrollable) {
        await page.locator(".lib-scroll").evaluate((el) => el.scrollBy(0, 400));
        const after = await page.evaluate(() => ({
          actions: window.__geo.box(document.querySelector(".lib-actions")!),
          foot: window.__geo.box(document.querySelector(".lib-foot")!),
          top: document.querySelector<HTMLElement>(".lib-scroll")!.scrollTop,
        }));
        expect(after.top, "목록이 굴러감").toBeGreaterThan(0);
        expect(after.actions.t, "위 단추는 그대로").toBe(before.actions.t);
        expect(after.foot.t, "아래 폴더 줄은 그대로").toBe(before.foot.t);
      }
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

// 사용자가 실제로 쓰는 창 크기 — 실행 중 상태 카드가 활동 기록을 화면 밖으로 밀지 않는지.
test.describe("1040x720 — 상태 카드 높이", () => {
  test.use({ viewport: { width: 1040, height: 720 } });

  test("상태 카드 높이 예산·활동 기록 세 줄·보유 수량 값이 잘리지 않음", async ({ page }, info) => {
    await open(page, "?demo=pnl&open=1", { details: "closed" });
    const problems: string[] = [];
    let measured = 0;
    const count = await page.locator(".run-card.is-live").count();
    for (let i = 0; i < count; i++) {
      await page.locator(".run-card.is-live .run-card-select").nth(i).click();
      await page.locator(".status-card .exit-bar").waitFor();
      await page.locator(".feed .act").first().waitFor();
      const where = (await page.locator(".detail-title").innerText()).replace(/\s+/g, " ");
      // 높이 · 활동 기록은 기본(세부 표 접힘) 상태로 잰다.
      const fold = await page.evaluate(() => {
        const g = window.__geo;
        const acts = document.querySelectorAll(".feed .act");
        return {
          cardH: g.box(document.querySelector(".status-card")!).h,
          thirdBottom: acts.length >= 3 ? g.box(acts[2]).b : null,
        };
      });
      // 보유 수량 값은 표를 펼쳐서 보고, 다음 실행을 위해 다시 접는다.
      await page.getByRole("button", { name: "현재가 · 평단 · 수량 자세히" }).click();
      const r = await page.evaluate(() => {
        const qty = Array.from(document.querySelectorAll<HTMLElement>(".run-table > div")).find(
          (d) => d.querySelector("dt")?.textContent === "보유 수량",
        )!;
        const dd = qty.querySelector<HTMLElement>("dd")!;
        return {
          bottom: window.innerHeight,
          qty: dd.textContent,
          title: dd.title,
          cut: dd.scrollWidth > dd.clientWidth + 1,
        };
      });
      await page.getByRole("button", { name: "자세히 접기" }).click();
      // 예전 높이(약 400px)의 70% 남짓.
      if (fold.cardH > 292) problems.push(`${where}: 상태 카드 높이 ${fold.cardH}px > 292px`);
      // 견본 중 활동이 세 줄 이상인 실행(선물 3배 — 증거금 줄까지 있어 가장 높은 카드)으로 잼.
      if (fold.thirdBottom !== null) {
        measured++;
        if (fold.thirdBottom > r.bottom)
          problems.push(`${where}: 활동 기록 셋째 줄 아래 ${fold.thirdBottom}px가 창 아래 ${r.bottom}px 밖`);
      }
      if (r.cut) problems.push(`${where}: 보유 수량 "${r.qty}"이 잘림`);
      if (r.title !== r.qty) problems.push(`${where}: 보유 수량 풍선 도움말 "${r.title}" ≠ "${r.qty}"`);
      // 제목은 "거래소(가려진 글자) 종목 모드" — 시장 쌍 없이 거래소로 업비트 BTC를 가림.
      if (where.startsWith("업비트 BTC") && r.qty !== "0.00127 BTC") problems.push(`${where}: 보유 수량 "${r.qty}"`);
      if (where.startsWith("업비트 ZAMA") && r.qty !== "499.75012494 ZAMA")
        problems.push(`${where}: 보유 수량 "${r.qty}"`);
      if (i === 0) await snap(page, info, "status-card-1040");
    }
    // 카드의 풍선 도움말은 거래소 + 종목("업비트 BTC").
    const names = await page
      .locator(".run-card.is-live .run-card-name")
      .evaluateAll((els) => els.map((el) => el.getAttribute("title") ?? ""));
    expect(names, "수량 견본(KRW-BTC 0.00127 · KRW-ZAMA 499.75012494)").toEqual(
      expect.arrayContaining(["업비트 BTC", "업비트 ZAMA"]),
    );
    expect(measured, "활동 기록 세 줄 이상인 실행을 잰 수").toBeGreaterThan(0);
    expect(problems).toEqual([]);
  });
});

// 넓은 창(1360px~) — 상세가 '상태 | 활동 기록' 두 칸. 활동 기록이 오른쪽 칸을 위아래로 다 차지하고,
// 활동 기록 머리줄은 일부러 두 줄(제목 · [기록 복사] / 거르기 칩), 돈 칸 넷은 한 줄에 글자가 잘리지 않음.
test.describe("1440x900 — 넓은 창 두 칸", () => {
  test.use({ viewport: { width: 1440, height: 900 } });

  test("상태 | 활동 기록 두 칸 · 머리줄 두 줄 · 돈 칸 한 줄", async ({ page }, info) => {
    const problems: string[] = [];
    for (const q of ["?demo=running&open=1", "?demo=wait&open=1", "?demo=pnl&open=1"]) {
      await open(page, q);
      await page.locator(".run-detail > .feed").waitFor();
      await snap(page, info, `wide${q.replace(/[^a-z]+/g, "-")}`);
      problems.push(...(await horizontalOverflow(page, q)));
      const found = await page.evaluate((where) => {
        const g = window.__geo;
        const out: string[] = [];
        const one = (sel: string) => g.box(document.querySelector(sel)!);
        const main = one(".detail-main");
        const feed = one(".run-detail > .feed");
        const detail = one(".run-detail");
        if (feed.l < main.r) out.push(`${where}: 활동 기록 ${g.fmt(feed)}가 왼쪽 칸 ${g.fmt(main)} 오른쪽에 있지 않음`);
        if (feed.t - detail.t > 20 || detail.b - feed.b > 20)
          out.push(`${where}: 활동 기록 ${g.fmt(feed)}가 상세 ${g.fmt(detail)}의 위아래를 다 차지하지 않음`);
        const title = one(".feed-title");
        const copy = one(".feed-copy");
        if (Math.abs((title.t + title.b) / 2 - (copy.t + copy.b) / 2) > 4)
          out.push(`${where}: 제목 ${g.fmt(title)}과 [기록 복사] ${g.fmt(copy)}가 다른 줄`);
        const chips = Array.from(document.querySelectorAll(".feed-chip")).map((c) => g.box(c));
        if (new Set(chips.map((c) => Math.round(c.t))).size !== 1) out.push(`${where}: 거르기 칩이 두 줄 이상`);
        if (chips.some((c) => c.t < title.b)) out.push(`${where}: 거르기 칩이 제목 줄에 걸침`);
        const cells = Array.from(document.querySelectorAll(".money-cells:not(.is-compact) > .money-cell"));
        if (cells.length && new Set(cells.map((c) => Math.round(g.box(c).t))).size !== 1)
          out.push(`${where}: 돈 칸 ${cells.length}개가 한 줄에 안 들어감`);
        for (const c of cells)
          for (const e of Array.from(c.querySelectorAll("dt, dd")))
            if (g.box(e).r > g.box(c).r + 0.5)
              out.push(`${where}: 돈 칸 '${c.querySelector("dt")?.textContent}' 글자가 칸 밖`);
        return out;
      }, q);
      problems.push(...found);
    }
    expect(problems).toEqual([]);
  });

  test("알림 토스트 — 연결 도우미 아래에 틈이 있으면 패널을 피해 아래에 뜸", async ({ page }, info) => {
    await open(page, "?demo=wizard");
    await page.locator(".wizard").waitFor();
    await page.getByRole("button", { name: /이미 발급받았어요/ }).click();
    await page.locator(".toasts.at-bottom .toast").first().waitFor();
    await snap(page, info, "wizard-toast-wide");
    const r = await page.evaluate(() => {
      const g = window.__geo;
      const tb = g.box(document.querySelector(".toast")!);
      const wb = g.box(document.querySelector(".wizard")!);
      return g.overlaps(tb, wb) ? [`토스트 ${g.fmt(tb)}가 도우미 ${g.fmt(wb)}와 겹침`] : [];
    });
    expect(r).toEqual([]);
  });
});
