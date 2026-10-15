// 설정 › 기억한 키 — 이 기기에 기억한 키의 상태.
// 읽기 실패(키체인 접근 거부·잠김)는 [다시 읽기]만 — 항목은 멀쩡할 수 있으니 지우지 않음.
// 내용이 깨진 경우(복호화 실패·형식 오류)에만 [지우고 새로 만들기]로 지우고 새로 만듦.
import { useState } from "react";
import { keyStoreName } from "../../platform";
import { useRunnerContext } from "../../state/context";
import type { SettingsState } from "../../types";
import { Dialog } from "../Dialog";
import { Section } from "./Section";

const BROKEN: SettingsState["storage"]["status"][] = ["read_error", "decrypt_error", "invalid_data"];
const CORRUPT: SettingsState["storage"]["status"][] = ["decrypt_error", "invalid_data"];

const STORE = keyStoreName();
const STORAGE_TEXT: Record<SettingsState["storage"]["status"], string> = {
  missing: "이 기기에 기억한 키가 없어요.",
  loaded: "이 기기에 기억한 키를 불러왔어요.",
  read_error: `기억한 키를 읽지 못했어요. ${STORE} 접근을 거부했거나 잠겨 있을 수 있어요.`,
  decrypt_error: "기억한 키를 읽을 수 없어요(복호화 실패).",
  invalid_data: "기억한 키의 저장 항목이 손상됐거나 형식이 맞지 않아요.",
  unsupported: "이 OS에서는 키를 기억할 수 없어요.",
};

export function StorageSection() {
  const { state, act } = useRunnerContext();
  const [ask, setAsk] = useState(false);
  const storage = state.settings.storage;
  const broken = BROKEN.includes(storage.status);
  const corrupt = CORRUPT.includes(storage.status);
  return (
    <Section id="storage" title="기억한 키">
      <div className={`plain-card ${broken ? "is-warn" : ""}`}>
        <div className="note-with-action note">
          <span className="storage-text">
            <span className="strong">{STORAGE_TEXT[storage.status]}</span>
            {storage.note && <span className={broken ? "tone-warn" : "muted"}>{storage.note}</span>}
          </span>
          {storage.status === "read_error" && (
            <button type="button" className="btn btn-ghost btn-sm" onClick={() => act("reload_storage")}>
              다시 읽기
            </button>
          )}
          {corrupt && (
            <button type="button" className="btn btn-ghost btn-sm" onClick={() => setAsk(true)}>
              지우고 새로 만들기
            </button>
          )}
        </div>
      </div>
      {ask && (
        <Dialog
          title="기억한 키를 지우고 새로 만들까요?"
          tone="warn"
          onClose={() => setAsk(false)}
          actions={
            <>
              <button type="button" className="btn btn-ghost" data-autofocus onClick={() => setAsk(false)}>
                취소
              </button>
              <button
                type="button"
                className="btn btn-danger-solid"
                onClick={() => {
                  setAsk(false);
                  void act("recover_storage");
                }}
              >
                지우고 새로 만들기
              </button>
            </>
          }
        >
          <p>
            손상된 저장 항목을 지우고 새로 만들어요. 지운 키는 되살릴 수 없어요. 지금 화면에 입력해 두고 '이 기기에 키
            기억하기'를 켠 키만 다시 기억해요.
          </p>
        </Dialog>
      )}
    </Section>
  );
}
