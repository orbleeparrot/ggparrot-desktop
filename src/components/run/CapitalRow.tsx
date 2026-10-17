// 시작 전 돈 · 안전 설정 칸 — 투자 금액(params.initial_capital 보기·바꾸기·원본으로) · 한 번에 최대 · 안전 설정.
// 줄글 대신 칸(MoneyCells). 파일에서 연 매크로만 바꿀 수 있음 — 웹에서 연결한 매크로는 서버가 웹 저장본과 다르면 시작을 거절함.
import { Pencil, RotateCcw, ShieldCheck } from "lucide-react";
import { useState, type FormEvent, type KeyboardEvent } from "react";
import { useRunnerContext } from "../../state/context";
import { isIdle } from "../../state/runs";
import type { RunSlot } from "../../types";
import { MoneyCells } from "./MoneyCells";

/** 금액을 바꿀 때의 안내 — 서명 없는 파일은 '수정됨'이 아니고, 모의는 연결 확인을 하지 않음. */
function editHint(signature: RunSlot["macro"]["signature"], mode: RunSlot["mode"]): string {
  const web = signature === "unsigned" ? "" : " 바꾸면 웹의 [내 에이전트]에 '수정된 파일'로 표시돼요.";
  const check = mode === "mock" ? "" : " 시작할 때 연결을 새로 확인해요.";
  return web + check;
}

export function CapitalRow({ slot }: { slot: RunSlot }) {
  const { ok } = useRunnerContext();
  const m = slot.macro;
  const [editing, setEditing] = useState(false);
  const [draft, setDraft] = useState("");
  const [error, setError] = useState("");
  const [busy, setBusy] = useState(false);
  const idle = isIdle(slot.phase);

  // 다른 매크로를 고르면 편집을 접음 — effect로 다시 그리지 않고 렌더 중에 바로 맞춤.
  const [forId, setForId] = useState(slot.id);
  if (forId !== slot.id) {
    setForId(slot.id);
    setEditing(false);
    setError("");
  }

  const capital = m.capital;
  const lockedWhy = m.capitalLocked || (idle ? "" : "실행 중에는 바꿀 수 없어요. 종료한 뒤 바꾸세요.");

  function begin() {
    if (capital === null) return;
    setDraft(String(capital));
    setError("");
    setEditing(true);
  }

  async function save(e?: FormEvent) {
    e?.preventDefault();
    const amount = Number(draft.replace(/,/g, "").trim());
    if (!Number.isFinite(amount) || amount <= 0) {
      setError("0보다 큰 금액을 숫자로 적어 주세요.");
      return;
    }
    if (busy) return;
    setBusy(true);
    try {
      if (await ok("set_run_capital", { runId: slot.id, amount })) setEditing(false);
    } finally {
      setBusy(false);
    }
  }

  async function restore() {
    if (busy) return;
    setBusy(true);
    try {
      await ok("set_run_capital", { runId: slot.id, amount: null });
    } finally {
      setBusy(false);
    }
  }

  function onKey(e: KeyboardEvent<HTMLInputElement>) {
    if (e.key === "Escape") {
      e.preventDefault();
      e.stopPropagation();
      setEditing(false);
    }
  }

  const editor = editing ? (
    <form className="capital-form" onSubmit={save}>
      <input
        className="input capital-input num"
        inputMode="decimal"
        aria-label={`투자 금액(${m.quote})`}
        value={draft}
        onChange={(e) => setDraft(e.target.value)}
        onKeyDown={onKey}
        autoFocus
      />
      <span className="capital-unit">{m.quote}</span>
      <button type="submit" className="btn btn-primary btn-sm" disabled={busy}>
        저장
      </button>
      <button type="button" className="btn btn-quiet btn-sm" onClick={() => setEditing(false)}>
        취소
      </button>
    </form>
  ) : undefined;

  const actions = (
    <span className="capital-actions">
      <button
        type="button"
        className="btn btn-quiet btn-sm"
        onClick={begin}
        disabled={!!lockedWhy}
        title={lockedWhy || undefined}
      >
        <Pencil size={13} aria-hidden="true" />
        바꾸기
      </button>
      {m.capitalOriginal !== null && (
        <button type="button" className="btn btn-quiet btn-sm" onClick={restore} disabled={!idle || busy}>
          <RotateCcw size={13} aria-hidden="true" />
          원본으로
        </button>
      )}
    </span>
  );

  return (
    <div className="capital-row">
      <MoneyCells macro={m} capitalExtra={actions} capitalEditor={editor} />
      {editing && editHint(m.signature, slot.mode) && (
        <p className="capital-note">{editHint(m.signature, slot.mode).trim()}</p>
      )}
      {m.riskItems.length > 0 && (
        <p className="capital-guard">
          <ShieldCheck size={13} aria-hidden="true" />
          서버 연결이 끊겨도 이 기기에서 지켜요.
        </p>
      )}
      {error && (
        <p className="note note-warn" role="alert">
          {error}
        </p>
      )}
    </div>
  );
}
