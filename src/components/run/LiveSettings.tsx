// 구동 중 설정 바꾸기 — 금액과 조건만, 돌고 있는 그대로.
//
// **칸 목록은 서버가 줌**(run.liveSettings.fields). 앱이 따로 목록을 들면 두 벌이 되어,
// 화면에는 보이는데 서버가 거절하는 칸이 생김 — 사용자는 그걸 고장으로 읽음.
//
// 칸마다 한 줄 설명과 단위를 붙임. 설명이 없으면 '지금 적용'을 고를 때 **팔고 다시 사는
// 건지** 멈칫하게 됨 — 팔지 않음. 들고 있는 수량과 평단은 그대로 두고 청산 조건만
// 갈아 끼움. 설명·단위 글자도 전부 서버가 줌(두 벌이 되지 않게).
//
// **앱이 혼자 바꾸지 않음.** 서버가 받아 전략을 다시 세우고 기록을 남긴 뒤, 다음
// heartbeat로 바뀐 값이 돌아옴. 여기서 바로 고치면 서버 전략과 실행기 설정이 갈라져
// 어느 쪽이 진짜인지 알 수 없게 됨.
import { useMemo, useState } from "react";
import { call } from "../../ipc";
import type { LiveSettingField, RunSlot } from "../../types";

type Edits = Record<string, string>;
type ApplyMode = "immediate" | "next_entry";

/** 입력 칸에 넣을 글자. 없는 값(꺼 둔 규칙)은 빈 칸. */
function text(field: LiveSettingField): string {
  if (field.value == null) return "";
  return field.unit === "money" ? field.value.toLocaleString() : String(field.value);
}

/** 사람이 친 숫자 — "1,000,000"처럼 쉼표가 있어도 받음. 숫자가 아니면 null. */
function parse(raw: string): number | null {
  const cleaned = raw.replace(/,/g, "").trim();
  if (!cleaned) return null;
  const n = Number(cleaned);
  return Number.isFinite(n) ? n : null;
}

/**
 * 바뀐 칸만 담음(부분 수정). 빈 칸의 뜻은 칸마다 다름 — 끌 수 있는 칸을 비우면 **끄는
 * 것**이고, 끌 수 없는 칸을 비운 것은 아직 덜 친 것이라 보내지 않음.
 */
export function buildChanges(fields: LiveSettingField[], edits: Edits) {
  const out: Record<string, Record<string, number | null>> = {};
  for (const field of fields) {
    const raw = edits[field.path];
    if (raw === undefined) continue;
    let next: number | null;
    if (raw.trim() === "") {
      if (!field.clearable || field.value == null) continue;
      next = null;
    } else {
      const n = parse(raw);
      if (n == null || n === field.value) continue;
      next = n;
    }
    (out[field.section] ||= {})[field.key] = next;
  }
  return out;
}

export function hasChanges(fields: LiveSettingField[], edits: Edits): boolean {
  return Object.values(buildChanges(fields, edits)).some((s) => Object.keys(s).length > 0);
}

/**
 * 바꾼 값이 **지금 들고 있는 포지션의 청산 조건**을 건드리는지.
 *
 * 건드리지 않으면(금액·비율·재진입 대기) 적용 시점을 어느 쪽으로 고르든 결과가 같음.
 * 무엇이 어느 쪽인지는 서버가 정함(field.affects) — 여기서 또 정하면 두 벌이 됨.
 */
export function touchesPosition(fields: LiveSettingField[], edits: Edits): boolean {
  const changes = buildChanges(fields, edits);
  return fields.some((field) => changes[field.section]?.[field.key] !== undefined && field.affects !== "next_entry");
}

/** 숫자로 읽히지 않는 칸 — 보내기 전에 화면이 먼저 말해 줌. 손댄 칸만 봄. */
export function badFields(fields: LiveSettingField[], edits: Edits): LiveSettingField[] {
  return fields.filter((field) => {
    const raw = edits[field.path];
    if (raw === undefined) return false;
    if (raw.trim() === "") return !field.clearable;
    return parse(raw) == null;
  });
}

export function LiveSettings({ slot }: { slot: RunSlot }) {
  const run = slot.run;
  const settings = run?.liveSettings ?? null;
  const fields = useMemo(() => settings?.fields ?? [], [settings]);
  const [open, setOpen] = useState(false);
  const [edits, setEdits] = useState<Edits>({});
  const [applyMode, setApplyMode] = useState<ApplyMode>("immediate");
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState("");
  const [done, setDone] = useState("");

  // `edits`는 **사람이 손댄 칸만** 담음. 안 담긴 칸은 그릴 때 지금 값으로 채움.
  //
  // 열 때 모든 칸을 미리 채워 두면 안 됨 — 그러려면 effect가 필요한데, 그 effect는
  // `fields`가 바뀔 때마다 돎. `fields`는 heartbeat(5초)마다 새 배열이라 **치고 있는
  // 중에 입력이 초기화됨.** 손댄 칸만 담으면 effect 자체가 사라짐.
  function openPanel() {
    setEdits({});
    setApplyMode("immediate");
    setError("");
    setDone("");
    setOpen(true);
  }

  if (!run || run.finished || !settings || fields.length === 0) return null;

  const bad = badFields(fields, edits);
  const ready = hasChanges(fields, edits) && bad.length === 0 && !busy;
  // 금액·비율만 바꿨으면 어느 쪽을 골라도 같음 — 묻기는 하되 같다는 사실을 말해 줌.
  const hitsPosition = touchesPosition(fields, edits);

  async function submit() {
    if (!ready) return;
    setBusy(true);
    setError("");
    try {
      const result = await call("change_run_settings", {
        runId: slot.id,
        changes: buildChanges(fields, edits),
        applyMode: run!.inPosition ? applyMode : "immediate",
      });
      setDone(result.applied ? `바꿨어요 · ${result.summary}` : result.note);
      setOpen(false);
    } catch (reason) {
      setError(reason instanceof Error ? reason.message : String(reason));
    } finally {
      setBusy(false);
    }
  }

  if (!open) {
    return (
      <div className="live-settings">
        <button type="button" className="live-settings-open" onClick={openPanel}>
          설정 바꾸기
          {settings.changed_count > 0 && <small> · {settings.changed_count}번 바뀜</small>}
        </button>
        {done && <p className="live-settings-done">{done}</p>}
      </div>
    );
  }

  return (
    <div className="live-settings is-open">
      <p className="live-settings-title">금액과 조건만 바꿔요 — 매매 방식·종목·방향은 못 바꿔요.</p>
      <div className="live-settings-fields">
        {fields.map((field) => (
          <label key={field.path} className="live-settings-field">
            <span className="live-settings-name">
              {field.label}
              {field.help && <small className="live-settings-help">{field.help}</small>}
            </span>
            {/* 단위를 칸 옆에 붙임 — 숫자만 있으면 2가 2%인지 2시간인지 2봉인지 모름. */}
            <span className="live-settings-box">
              <input
                type="text"
                inputMode="decimal"
                value={edits[field.path] ?? text(field)}
                disabled={busy}
                aria-invalid={bad.includes(field) || undefined}
                placeholder={field.clearable ? "비우면 끔" : ""}
                onChange={(e) => setEdits((prev) => ({ ...prev, [field.path]: e.target.value }))}
              />
              {field.unit_text && <span className="live-settings-unit">{field.unit_text}</span>}
            </span>
          </label>
        ))}
      </div>
      {bad.length > 0 && (
        <p className="live-settings-bad" role="alert">
          {bad.map((f) => f.label).join(" · ")} — 숫자로 입력해 주세요.
        </p>
      )}
      {run.inPosition && (
        <fieldset className="live-settings-apply">
          <legend>지금 포지션을 들고 있어요. 언제 적용할까요?</legend>
          <p className="live-settings-help">
            어느 쪽을 골라도 지금 들고 있는 건 팔지 않아요. 수량과 평단은 그대로예요.
            {!hitsPosition && " 이번에 바꾼 값은 다음에 살 때 쓰는 것이라 두 쪽이 같아요."}
          </p>
          <label>
            <input
              type="radio"
              name="applyMode"
              checked={applyMode === "immediate"}
              disabled={busy}
              onChange={() => setApplyMode("immediate")}
            />
            <span>
              지금 적용: 들고 있는 포지션의 청산 조건도 바뀌어요
              <small className="live-settings-help">
                새 손절·익절이 지금 포지션에 바로 걸려요. 이미 그 선을 넘었으면 곧 정리될 수 있어요.
              </small>
            </span>
          </label>
          <label>
            <input
              type="radio"
              name="applyMode"
              checked={applyMode === "next_entry"}
              disabled={busy}
              onChange={() => setApplyMode("next_entry")}
            />
            <span>
              이 포지션 정리된 뒤부터: 지금 포지션은 살 때 조건 그대로 끝나요
              <small className="live-settings-help">새 설정은 예약해 두고, 지금 포지션이 정리되면 그때부터 써요.</small>
            </span>
          </label>
        </fieldset>
      )}
      {/* 기록이 남는다는 사실을 숨기지 않음 — 나중에 알리면 그게 더 나쁨. */}
      <p className="live-settings-note">바꾼 내용은 기록으로 남아요.</p>
      {error && (
        <p className="live-settings-bad" role="alert">
          {error}
        </p>
      )}
      <div className="live-settings-actions">
        <button type="button" className="live-settings-save" disabled={!ready} onClick={submit}>
          {busy ? "바꾸는 중…" : "이대로 바꾸기"}
        </button>
        <button type="button" className="live-settings-cancel" disabled={busy} onClick={() => setOpen(false)}>
          취소
        </button>
      </div>
    </div>
  );
}
