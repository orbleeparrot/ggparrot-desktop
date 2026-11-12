// 복구 견본의 확인 동작·기존 세션 유지·좁은 창의 가로 넘침 확인.
import { expect, test } from "@playwright/test";

for (const viewport of [
  { width: 1440, height: 900 },
  { width: 390, height: 844 },
]) {
  test.describe(`복구 ${viewport.width}`, () => {
    test.use({ viewport });
    test("확인 뒤 같은 실행을 재개함", async ({ page }, info) => {
      await page.goto("/?demo=recovery", { waitUntil: "networkidle" });
      if (viewport.width < 900) await page.locator(".run-card-select").first().click();
      const resume = page.locator(".detail-start");
      await expect(resume).toHaveText("이어서 실행");
      await expect(page.getByRole("radiogroup", { name: "실행 모드" })).toHaveCount(0);
      await resume.click();
      const dialog = page.getByRole("dialog", { name: "중단된 실행을 이어서 실행할까요?" });
      await expect(dialog).toBeVisible();
      const screenshot = info.outputPath(`recovery-${viewport.width}.png`);
      await page.screenshot({ path: screenshot });
      await info.attach("복구 확인", { path: screenshot, contentType: "image/png" });
      const overflow = await page.evaluate(() => {
        const els = [document.documentElement, document.getElementById("root"), document.querySelector(".dialog")];
        return els.filter((el) => el && el.scrollWidth > el.clientWidth + 1).map((el) => el?.tagName);
      });
      expect(overflow).toEqual([]);
      await dialog.getByRole("button", { name: "이어서 실행", exact: true }).click();
      await expect(dialog).toHaveCount(0);
      if (viewport.width >= 900) await expect(page.locator(".run-card-select")).toHaveCount(1);
      await expect(page.locator(".detail-start")).toHaveCount(0);
    });
  });
}
