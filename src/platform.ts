// 창 모양이 플랫폼마다 다름 — macOS는 신호등이 머리줄 안에, Windows는 직접 그린 창 단추.
export type Platform = "mac" | "windows" | "other";

export function detectPlatform(nav: Pick<Navigator, "userAgent" | "platform"> = navigator): Platform {
  const s = `${nav.platform || ""} ${nav.userAgent || ""}`;
  if (/Mac/i.test(s)) return "mac";
  if (/Win/i.test(s)) return "windows";
  return "other";
}

/** 기억한 키를 보관하는 OS 저장소 이름 — "키체인"은 macOS 말이라 Windows에서는 낯섦. */
export function keyStoreName(p: Platform = detectPlatform()): string {
  return p === "windows" ? "Windows 자격 증명 관리자" : "키체인";
}

/** <html data-platform="…"> — CSS가 신호등 자리·창 단추 자리를 이 값으로 정함. */
export function markPlatform(): Platform {
  const p = detectPlatform();
  document.documentElement.dataset.platform = p;
  return p;
}

/** 껄무새 웹 주소(Rust `shared::WEB_URL`과 같음). 앱에서는 Rust가 기본 브라우저로 엶. */
export const WEB_URL = "https://gg-parrot.vercel.app/";
