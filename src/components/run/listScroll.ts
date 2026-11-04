// '내 매크로' 목록을 끄는 동안의 스크롤 계산 — 순서 바꾸기와 여러 개 고르기가 함께 씀.
// - 넓은 창은 목록 패널이, 좁은 창(한 열)은 문서가 스크롤함 — 둘 다 같은 계산(화면상 위치)으로 다룸.

/** 끄는 중 자동 스크롤 속도(px/프레임). 보이는 영역 [top, bottom]의 가장자리 zone 안으로 들어갈수록 빨라짐. 밖이면 0. */
export function edgeScrollSpeed(y: number, top: number, bottom: number, zone = 48, max = 14): number {
  if (y < top + zone) return -Math.ceil(max * Math.min(1, (top + zone - y) / zone));
  if (y > bottom - zone) return Math.ceil(max * Math.min(1, (y - (bottom - zone)) / zone));
  return 0;
}

/** 목록이 실제로 보이는 세로 범위 — 붙어 있는 머리 아래 ~ 패널(또는 창) 아래 끝. */
export function visibleBand(head: HTMLElement | null, panel: HTMLElement | null): [number, number] {
  const bottom = panel ? Math.min(panel.getBoundingClientRect().bottom, window.innerHeight) : window.innerHeight;
  return [head?.getBoundingClientRect().bottom ?? 0, bottom];
}

/** 패널이 스크롤하는 배치인지 — 아니면 문서가 스크롤함. */
function panelScrolls(panel: HTMLElement | null): panel is HTMLElement {
  return !!panel && panel.scrollHeight > panel.clientHeight + 1;
}

/** 넓은 창은 패널이 스크롤하고, 좁은 창은 문서가 스크롤함. */
export function scrollBy(panel: HTMLElement | null, dy: number) {
  if (panelScrolls(panel)) panel.scrollTop += dy;
  else window.scrollBy(0, dy);
}

/** 지금 스크롤한 양 — 끄는 동안 스크롤해도 누른 자리를 화면 좌표로 다시 셈. */
export function scrollTopOf(panel: HTMLElement | null): number {
  return panelScrolls(panel) ? panel.scrollTop : window.scrollY;
}
