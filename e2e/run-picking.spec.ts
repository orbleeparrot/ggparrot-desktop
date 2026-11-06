// 내 매크로 목록 — [선택]으로 선택 모드에 들어가 고르고 한 번에 빼기(useRunPicking). 실제 배치가 있어야 하는 것만 여기서 봄.
// - 머리: [선택]·[+ 추가]와 선택 모드의 '○ N개 선택 [취소] [목록에서 빼기]'가 한 줄·같은 높이·패널 안.
// - 선택 모드: 고를 수 있는 줄은 빈 원·노란 체크, 실행 중은 흐림, 손잡이·[▶]·[×] 감춤.
// - 순서 손잡이는 카드 왼쪽 여백 안 — 코인 표시를 덮지 않음.
// - 클릭·키 동작은 vitest(RunList.picking.test.tsx)에서 봄.
import { expect, test, type Locator, type Page } from "@playwright/test";

const NO_MOTION = "*,*::before,*::after{animation:none!important;transition:none!important}";

async function open(page: Page) {
  await page.goto("/?demo=many", { waitUntil: "networkidle" });
  await page.addStyleTag({ content: NO_MOTION });
  await page.locator(".run-group.is-idle").waitFor();
}

const head = (page: Page) => page.locator(".panel-head");
const idleRows = (page: Page) => page.locator(".run-group.is-idle .run-card");
const ids = (rows: Locator) =>
  rows.locator(".run-card-select").evaluateAll((els) => els.map((el) => (el as HTMLElement).dataset.runId));
const pickedIds = (page: Page) => ids(page.locator(".run-card.is-picked"));

async function center(row: Locator) {
  await row.evaluate((el) => el.scrollIntoView({ block: "center" }));
}

/** 머리의 단추들이 한 줄·패널 안에 있는지. */
async function oneLine(page: Page, names: string[]) {
  const box = (await head(page).boundingBox())!;
  const tops: number[] = [];
  for (const name of names) {
    const b = (await head(page).getByRole("button", { name }).boundingBox())!;
    tops.push(b.y);
    expect(b.x + b.width, `${name}: 패널 안`).toBeLessThanOrEqual(box.x + box.width);
  }
  expect(Math.max(...tops) - Math.min(...tops), "한 줄").toBeLessThanOrEqual(1);
}

for (const vp of [
  { width: 1100, height: 760 },
  { width: 720, height: 600 },
]) {
  test.describe(`${vp.width}x${vp.height} — 선택 모드`, () => {
    test.use({ viewport: vp });

    test("[선택] → 줄을 눌러 고르면 머리가 'N개 선택'으로 — 같은 높이 · 한 줄 · 상세 그대로", async ({ page }) => {
      await open(page);
      const before = (await head(page).boundingBox())!;
      await oneLine(page, ["선택", "매크로 추가"]);
      const viewing = await page.locator(".run-card.is-selected .run-card-select").getAttribute("data-run-id");
      await head(page).getByRole("button", { name: "선택" }).click();
      const rows = idleRows(page);
      const all = await ids(rows);
      expect(all.length, "대기 묶음을 다 펼침").toBeGreaterThan(3);
      await center(rows.nth(2));
      for (const i of [1, 2, 4]) await rows.nth(i).locator(".run-card-select").click();
      expect(await pickedIds(page)).toEqual([all[1], all[2], all[4]]);
      await expect(head(page).getByRole("heading")).toHaveText("3개 선택");
      await oneLine(page, ["취소", "목록에서 빼기"]);
      const after = (await head(page).boundingBox())!;
      expect(Math.abs(after.height - before.height), "머리 높이 그대로").toBeLessThanOrEqual(1);
      expect(await page.locator(".run-card.is-selected .run-card-select").getAttribute("data-run-id")).toBe(viewing);
    });

    test("선택 모드: 빈 원 · 노란 체크, 실행 중은 흐리고 안 골라짐, 손잡이 · [▶] · [×] 감춤", async ({ page }) => {
      await open(page);
      await head(page).getByRole("button", { name: "선택" }).click();
      const rows = idleRows(page);
      await center(rows.nth(1));
      await rows.nth(1).locator(".run-card-select").click();
      await expect(rows.nth(1).locator(".run-card-check")).toBeVisible();
      await expect(rows.nth(2).locator(".run-card-ring")).toBeVisible();
      await rows.nth(2).hover();
      for (const part of [".run-card-grip", ".run-card-action", ".run-card-remove-slot"]) {
        await expect(rows.nth(2).locator(part)).toHaveCSS("visibility", "hidden");
      }
      const running = page.locator(".run-group.is-running .run-card").first();
      await center(running);
      expect(Number(await running.evaluate((el) => getComputedStyle(el).opacity))).toBeLessThan(0.6);
      await running.locator(".run-card-select").click();
      expect((await pickedIds(page)).length).toBe(1);
    });

    test("[목록에서 빼기] → 확인 → 고른 줄이 빠지고 선택 모드가 끝남", async ({ page }) => {
      await open(page);
      const count = page.locator(".run-group.is-idle .run-group-count");
      const total = Number(await count.textContent());
      await head(page).getByRole("button", { name: "선택" }).click();
      const remove = head(page).getByRole("button", { name: "목록에서 빼기" });
      await expect(remove).toBeDisabled();
      const rows = idleRows(page);
      await center(rows.nth(1));
      for (const i of [0, 1, 2]) await rows.nth(i).locator(".run-card-select").click();
      await remove.click();
      const dialog = page.getByRole("dialog");
      await expect(dialog.getByRole("heading")).toHaveText("3개를 목록에서 뺄까요?");
      await dialog.getByRole("button", { name: "목록에서 빼기" }).click();
      await expect(count).toHaveText(String(total - 3));
      await expect(head(page).getByRole("heading")).toHaveText("내 매크로");
    });
  });
}

test.describe("1100x760 — 순서 손잡이", () => {
  test.use({ viewport: { width: 1100, height: 760 } });

  test("가리켜도 순서 손잡이가 코인 표시를 덮지 않음 — 한 줄 행 · 두 줄 카드", async ({ page }) => {
    await open(page);
    const more = page.getByRole("button", { name: /모두 보기/ });
    if (await more.count()) await more.click();
    for (const group of ["is-idle", "is-running"]) {
      const row = page.locator(`.run-group.${group} .run-card`).nth(2);
      await center(row);
      await row.hover();
      const grip = (await row.locator(".run-card-grip").boundingBox())!;
      const coin = (await row.locator(".coin-mark").boundingBox())!;
      expect(grip.x + grip.width, `${group}: 손잡이 오른쪽 끝 ≤ 코인 왼쪽`).toBeLessThanOrEqual(coin.x + 0.5);
      const dots = await row
        .locator(".run-card-grip svg circle")
        .evaluateAll((els) => Math.max(...els.map((el) => el.getBoundingClientRect().right)));
      expect(dots, `${group}: 점과 코인 사이 틈`).toBeLessThan(coin.x - 1);
    }
  });
});
