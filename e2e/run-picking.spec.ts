// 내 매크로 목록 — 끌어서 여러 개 고르기·한 번에 빼기(useRunPicking). 실제 배치가 있어야 하는 것만 여기서 봄.
// - 한 줄 목록(넓은 창): 위아래로 끈 범위, 실행 중 카드는 지나가도 안 골라짐, 머리 높이 그대로.
// - 끝에 가까이 끌면 저절로 스크롤하며 계속 고름.
// - 두 칸 격자(720): 끈 사각형에 닿은 덩어리만.
// - 클릭·키 동작은 vitest(RunList.picking.test.tsx)에서 봄.
import { expect, test, type Locator, type Page } from "@playwright/test";

const NO_MOTION = "*,*::before,*::after{animation:none!important;transition:none!important}";

async function open(page: Page) {
  await page.goto("/?demo=many", { waitUntil: "networkidle" });
  await page.addStyleTag({ content: NO_MOTION });
  await page.locator(".run-group.is-idle").waitFor();
  const more = page.getByRole("button", { name: /모두 보기/ });
  if (await more.count()) await more.click();
}

const idleRows = (page: Page) => page.locator(".run-group.is-idle .run-card");
const pickedIds = (page: Page) =>
  page
    .locator(".run-card.is-picked .run-card-select")
    .evaluateAll((els) => els.map((el) => (el as HTMLElement).dataset.runId));

/** 끌 줄을 화면 가운데로 — 위·아래 끝 가까이에서 끌면 자동 스크롤이 켜져 더 골라짐(그건 따로 봄). */
async function center(row: Locator) {
  await row.evaluate((el) => el.scrollIntoView({ block: "center" }));
}

/** 줄 안쪽 왼편(순서 손잡이·오른쪽 단추를 피한 자리). */
async function inside(row: Locator, dx = 80) {
  const box = (await row.boundingBox())!;
  return { x: box.x + Math.min(dx, box.width / 2), y: box.y + box.height / 2 };
}

async function drag(page: Page, from: { x: number; y: number }, to: { x: number; y: number }) {
  await page.mouse.move(from.x, from.y);
  await page.mouse.down();
  await page.mouse.move(to.x, to.y, { steps: 8 });
  await page.mouse.up();
}

test.describe("1100x760 — 한 줄 목록", () => {
  test.use({ viewport: { width: 1100, height: 760 } });

  test("대기 줄을 위아래로 끌면 지나간 줄만 골라지고 머리가 'N개 선택'으로 바뀜", async ({ page }) => {
    await open(page);
    const head = page.locator(".panel-head");
    const before = (await head.boundingBox())!;
    const selected = await page.locator(".run-card.is-selected .run-card-select").getAttribute("data-run-id");
    const rows = idleRows(page);
    const ids = await rows
      .locator(".run-card-select")
      .evaluateAll((els) => els.map((el) => (el as HTMLElement).dataset.runId));
    await center(rows.nth(2));
    await drag(page, await inside(rows.nth(1)), await inside(rows.nth(4)));
    expect(await pickedIds(page)).toEqual(ids.slice(1, 5));
    await expect(head.getByRole("heading")).toHaveText("4개 선택");
    // 끌어 고르기는 상세를 바꾸지 않음
    expect(await page.locator(".run-card.is-selected .run-card-select").getAttribute("data-run-id")).toBe(selected);
    // 새 줄 없이 같은 자리 — 머리 높이 그대로, 두 단추가 한 줄·패널 안
    const after = (await head.boundingBox())!;
    expect(Math.abs(after.height - before.height)).toBeLessThanOrEqual(1);
    const cancel = (await head.getByRole("button", { name: "취소" }).boundingBox())!;
    const remove = (await head.getByRole("button", { name: "목록에서 빼기" }).boundingBox())!;
    expect(Math.abs(cancel.y - remove.y)).toBeLessThanOrEqual(1);
    expect(remove.x + remove.width).toBeLessThanOrEqual(before.x + before.width);
  });

  test("실행 중 카드에서 시작해 대기까지 끌어도 실행 중은 안 골라짐", async ({ page }) => {
    await open(page);
    const running = page.locator(".run-group.is-running .run-card");
    const runningIds = await running
      .locator(".run-card-select")
      .evaluateAll((els) => els.map((el) => (el as HTMLElement).dataset.runId));
    await center(running.last());
    await drag(page, await inside(running.last()), await inside(idleRows(page).nth(1)));
    const picked = await pickedIds(page);
    expect(picked.length).toBe(2);
    expect(picked.some((id) => runningIds.includes(id))).toBe(false);
  });

  test("목록 아래 끝 가까이 끌고 있으면 저절로 스크롤하며 더 고름", async ({ page }) => {
    await open(page);
    const panel = page.locator(".run-list-panel");
    const rows = idleRows(page);
    await rows.first().scrollIntoViewIfNeeded();
    const top0 = await panel.evaluate((el) => el.scrollTop);
    const start = await inside(rows.first());
    const box = (await panel.boundingBox())!;
    await page.mouse.move(start.x, start.y);
    await page.mouse.down();
    await page.mouse.move(start.x, box.y + box.height - 6, { steps: 8 });
    const near = (await pickedIds(page)).length;
    // 프레임마다 조금씩 내려감 — 화면 없는 WebKit(CI)은 프레임이 드물어 정해진 시간 대신 늘어날 때까지 봄.
    await expect.poll(async () => (await pickedIds(page)).length, { timeout: 5000 }).toBeGreaterThan(near);
    await page.mouse.up();
    expect(await panel.evaluate((el) => el.scrollTop)).toBeGreaterThan(top0);
  });

  test("[목록에서 빼기] → 확인 → 고른 줄이 목록에서 빠짐", async ({ page }) => {
    await open(page);
    const count = page.locator(".run-group.is-idle .run-group-count");
    const total = Number(await count.textContent());
    const rows = idleRows(page);
    await center(rows.nth(1));
    await drag(page, await inside(rows.nth(0)), await inside(rows.nth(2)));
    await page.locator(".panel-head").getByRole("button", { name: "목록에서 빼기" }).click();
    const dialog = page.getByRole("dialog");
    await expect(dialog.getByRole("heading")).toHaveText("3개를 목록에서 뺄까요?");
    await dialog.getByRole("button", { name: "목록에서 빼기" }).click();
    await expect(count).toHaveText(String(total - 3));
    await expect(page.locator(".panel-head").getByRole("heading")).toHaveText("내 매크로");
  });
});

test.describe("720x600 — 두 칸 격자", () => {
  test.use({ viewport: { width: 720, height: 600 } });

  test("왼쪽 칸만 세로로 끌면 왼쪽 줄만 — 읽는 차례로 번지지 않음", async ({ page }) => {
    await open(page);
    const rows = idleRows(page);
    const ids = await rows
      .locator(".run-card-select")
      .evaluateAll((els) => els.map((el) => (el as HTMLElement).dataset.runId));
    // 격자 차례: 0 1 / 2 3 / 4 5 — 0에서 4로 왼쪽 칸만 끌면 0·2·4(차례 범위라면 0~4 다섯 줄)
    await center(rows.nth(2));
    await drag(page, await inside(rows.nth(0), 40), await inside(rows.nth(4), 40));
    expect(await pickedIds(page)).toEqual([ids[0], ids[2], ids[4]]);
  });

  test("2×2로 끌면 그 네 줄", async ({ page }) => {
    await open(page);
    const rows = idleRows(page);
    const ids = await rows
      .locator(".run-card-select")
      .evaluateAll((els) => els.map((el) => (el as HTMLElement).dataset.runId));
    // 2(왼쪽 둘째 줄)에서 5(오른쪽 셋째 줄)로
    await center(rows.nth(3));
    await drag(page, await inside(rows.nth(2), 40), await inside(rows.nth(5), 40));
    expect(await pickedIds(page)).toEqual(ids.slice(2, 6));
    const xs = await page
      .locator(".run-card.is-picked")
      .evaluateAll((els) => els.map((el) => Math.round(el.getBoundingClientRect().left)));
    expect(new Set(xs).size, "두 칸에 둘씩").toBe(2);
  });
});
