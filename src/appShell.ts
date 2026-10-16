// 앱 창에서 브라우저 흔적 막기 — 웹뷰 기본 우클릭 메뉴(새로고침·뒤로)와 새로고침 단축키.
// - 새로고침하면 화면 상태(고른 매크로·열린 설정)만 초기화돼 혼란스러움. 앱 기능은 모두 화면 단추에 있음.
// - 입력 칸·편집 영역, 글자를 골라 둔 상태에서는 기본 메뉴(붙여넣기·복사)를 그대로 둠.
// - 개발 빌드(tauri dev·브라우저 미리보기)에서는 막지 않음 — 새로고침·검사 도구가 필요함.

/** 기본 메뉴를 그대로 둘 곳 — 입력 칸·편집 영역 안이거나 골라 둔 글자가 있을 때. */
export function keepNativeMenu(target: EventTarget | null, selection: string): boolean {
  if (selection.trim() !== "") return true;
  if (!(target instanceof Element)) return false;
  return !!target.closest('input, textarea, select, [contenteditable=""], [contenteditable="true"]');
}

/** 새로고침 단축키 — ⌘R·Ctrl+R(+Shift)·F5. */
export function isReloadKey(e: Pick<KeyboardEvent, "key" | "metaKey" | "ctrlKey">): boolean {
  if (e.key === "F5") return true;
  return (e.metaKey || e.ctrlKey) && (e.key === "r" || e.key === "R");
}

export function lockAppShell(): void {
  window.addEventListener("contextmenu", (e) => {
    if (!keepNativeMenu(e.target, window.getSelection()?.toString() ?? "")) e.preventDefault();
  });
  window.addEventListener(
    "keydown",
    (e) => {
      if (isReloadKey(e)) e.preventDefault();
    },
    { capture: true },
  );
}
