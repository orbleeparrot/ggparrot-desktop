// [+ 매크로 추가] 시트 — 매크로 폴더의 파일을 골라 실행 목록에 넣음. 폴더 열기·폴더로 가져오기·새로고침·골라서 실행 목록에 추가(둘 다 매크로 폴더로 복사함).
import { FolderOpen, FileInput, FilePlus2, Globe, RefreshCw, X } from "lucide-react";
import { useRef, useState, type KeyboardEvent } from "react";
import { useRunnerContext } from "../../state/context";
import { trapTab, useModalFocus } from "../focus";
import { LibraryItem } from "./LibraryItem";

export const LIBRARY_HINT =
  "웹에서 받은 .ggm.json 파일을 창에 끌어다 놓거나 [폴더로 가져오기]로 넣으면 이 폴더에 복사되고 목록에 나타나요.";

export function LibrarySheet({ onClose, onAdded }: { onClose(): void; onAdded(runId: string): void }) {
  const { state, act, toast } = useRunnerContext();
  const panel = useRef<HTMLDivElement>(null);
  const [busy, setBusy] = useState(false);
  const { dir, files, skipped, truncated } = state.library;
  const unreadable = files.filter((f) => !f.valid).length;
  const unsupported = files.filter((f) => f.valid && f.unsupported).length;
  useModalFocus(panel);

  async function run<T>(fn: () => Promise<T>): Promise<T | undefined> {
    setBusy(true);
    try {
      return await fn();
    } finally {
      setBusy(false);
    }
  }

  async function pick(fileName: string) {
    const id = await run(() => act("add_run_from_library", { fileName }));
    if (id) onAdded(id);
  }

  async function other() {
    const id = await run(() => act("add_macro_file"));
    if (id) onAdded(id);
  }

  async function importFiles() {
    const n = await run(() => act("library_import"));
    if (n) toast(`매크로 파일 ${n}개를 매크로 폴더로 가져왔어요.`);
  }

  function onKeyDown(e: KeyboardEvent<HTMLDivElement>) {
    if (e.key === "Escape") {
      e.preventDefault();
      e.stopPropagation();
      onClose();
      return;
    }
    trapTab(e, panel.current);
  }

  return (
    <div className="scrim scrim-side" onMouseDown={(e) => e.target === e.currentTarget && onClose()}>
      <div
        ref={panel}
        className="side-sheet library"
        role="dialog"
        aria-modal="true"
        aria-labelledby="library-title"
        tabIndex={-1}
        onKeyDown={onKeyDown}
      >
        <header className="sheet-head">
          <h2 id="library-title" className="sheet-title" tabIndex={-1} data-autofocus>
            매크로 추가
          </h2>
          <button type="button" className="icon-btn" aria-label="닫기" onClick={onClose}>
            <X size={18} />
          </button>
        </header>

        <div className="lib-folder">
          <span className="eyebrow">매크로 폴더</span>
          <div className="lib-folder-row">
            <code className="path" title={dir}>
              {dir}
            </code>
            <button type="button" className="btn btn-ghost btn-sm" onClick={() => act("library_open_folder")}>
              <FolderOpen size={14} aria-hidden="true" />
              폴더 열기
            </button>
            <button
              type="button"
              className="icon-btn"
              aria-label="새로고침"
              title="새로고침"
              onClick={() => act("library_refresh")}
            >
              <RefreshCw size={15} />
            </button>
          </div>
          <p className="lib-hint">
            {LIBRARY_HINT}{" "}
            <button type="button" className="link-btn" onClick={() => act("open_web")}>
              껄무새 웹 열기
            </button>
          </p>
        </div>

        <div className="sheet-body">
          {(skipped > 0 || truncated > 0) && (
            <p className="lib-note">
              {skipped > 0 &&
                `이름이 .ggm.json이 아닌 .json 파일 ${skipped}개는 목록에서 뺐어요. 매크로라면 [폴더로 가져오기]로 넣으세요. 가져올 때 이름을 .ggm.json으로 바꿔 저장해 줘요. `}
              {truncated > 0 && `파일이 많아 최근 수정한 300개만 보여요(${truncated}개 더 있어요).`}
            </p>
          )}
          {files.length === 0 ? (
            <div className="lib-empty">
              <p className="strong">목록에 보일 매크로 파일이 없어요</p>
              <p className="muted">[폴더로 가져오기]로 웹에서 받은 매크로 파일을 넣어 보세요.</p>
              <button type="button" className="btn btn-ghost btn-sm" onClick={() => act("open_web")}>
                <Globe size={14} aria-hidden="true" />
                껄무새 웹에서 매크로 만들기
              </button>
            </div>
          ) : (
            <>
              <p className="lib-count">
                매크로 파일 {files.length}개{unreadable > 0 ? ` · 읽지 못한 파일 ${unreadable}개` : ""}
                {unsupported > 0 ? ` · 실행할 수 없는 매크로 ${unsupported}개` : ""}
              </p>
              <ul className="lib-list" aria-label="매크로 파일">
                {files.map((f) => (
                  <LibraryItem key={f.fileName} file={f} busy={busy} onPick={() => pick(f.fileName)} />
                ))}
              </ul>
            </>
          )}
        </div>

        <footer className="sheet-foot">
          <button type="button" className="btn btn-ghost" onClick={importFiles} disabled={busy}>
            <FileInput size={15} aria-hidden="true" />
            폴더로 가져오기
          </button>
          <button type="button" className="btn btn-quiet" onClick={other} disabled={busy}>
            <FilePlus2 size={15} aria-hidden="true" />
            골라서 실행 목록에 추가…
          </button>
        </footer>
      </div>
    </div>
  );
}
