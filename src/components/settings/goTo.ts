// 설정 안에서 데려가기 — 그 칸으로 굴리고 첫 입력칸(없으면 칸 자체)에 포커스를 두고 잠깐 테두리를 밝힘(.is-target).
import { settingsElementId, type SettingsTarget } from "../../state/runs";
import { reducedMotion } from "../focus";

/** 그 칸이 화면에 없으면 아무것도 하지 않고 거짓. */
export function goToSettings(target: SettingsTarget): boolean {
  const el = document.getElementById(settingsElementId(target));
  if (!el) return false;
  el.scrollIntoView?.({
    behavior: reducedMotion() ? "auto" : "smooth",
    block: target.kind === "section" ? "start" : "center",
  });
  const input = target.kind === "section" ? null : el.querySelector<HTMLElement>("input:not([disabled])");
  (input ?? el).focus({ preventScroll: true });
  el.classList.remove("is-target");
  void el.offsetWidth;
  el.classList.add("is-target");
  return true;
}
