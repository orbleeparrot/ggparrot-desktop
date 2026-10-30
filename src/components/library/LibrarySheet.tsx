// [+ 추가]를 누르면 오른쪽에서 나오는 시트 — 매크로 폴더의 파일을 골라 실행 목록에 넣음.
// - 추가하는 길은 둘: 위의 [파일에서 추가](다른 곳의 파일을 폴더로 가져옴 — 하나면 바로 실행 목록에도)·[웹에서 고르기].
//   예전 [폴더로 가져오기]·[골라서 실행 목록에 추가…]는 둘 다 파일 고르기 창이라 [파일에서 추가] 하나로 합침(10/18).
// - 위 단추·아래 폴더 줄은 고정, 가운데 목록만 안에서 굴러감(매크로가 많아도 단추가 밀려나지 않게).
// - 추가할 수 없는 파일은 목록에 섞지 않고 아래 접히는 회색 칸에 '이름 · 이유' 한 줄씩.
// - 회색 줄글 안내는 두지 않음 — 끌어다 놓기는 실제로 끌 때 겹친 화면(DropOverlay)이 말하고, 폴더 경로는 마우스를 올리면.
import { ChevronDown, ExternalLink, FolderOpen, Plus, RefreshCw, X } from "lucide-react";
import { SwapLabel } from "../SwapLabel";
import { useRef, useState, type KeyboardEvent } from "react";
import { useRunnerContext } from "../../state/context";
import { trapTab, useModalFocus } from "../focus";
import { isProblem, LibraryItem, LibraryProblem } from "./LibraryItem";

export function LibrarySheet({ onClose, onAdded }: { onClose(): void; onAdded(runId: string): void }) {
  const { state, act } = useRunnerContext();
  const panel = useRef<HTMLDivElement>(null);
  const [busy, setBusy] = useState(false);
  const [problemsOpen, setProblemsOpen] = useState(true);
  const { dir, files, skipped, truncated } = state.library;
  const good = files.filter((f) => !isProblem(f));
  const problems = files.filter(isProblem);
  const problemCount = problems.length + (skipped > 0 ? 1 : 0);
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

  async function fromFile() {
    const id = await run(() => act("add_macro_file"));
    if (id) onAdded(id);
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

        <div className="lib-actions">
          <button type="button" className="btn btn-ghost" onClick={fromFile} disabled={busy}>
            <Plus size={15} aria-hidden="true" />
            파일에서 추가
          </button>
          <button type="button" className="btn btn-ghost" onClick={() => act("open_web")}>
            <ExternalLink size={14} aria-hidden="true" />
            웹에서 고르기
          </button>
        </div>

        <div className="sheet-body lib-scroll">
          {good.length === 0 && problemCount === 0 ? (
            <div className="lib-empty">
              <p className="strong">매크로 폴더에 매크로가 없어요</p>
              <p className="muted">[파일에서 추가]로 넣거나 웹에서 골라 [실행기 열기]를 누르세요.</p>
            </div>
          ) : (
            <>
              <div className="lib-group">
                <h3 className="lib-group-name">매크로 폴더</h3>
                <span className="lib-group-count num">{good.length}</span>
                <span className="lib-group-line" aria-hidden="true" />
              </div>
              {good.length > 0 && (
                <ul className="lib-list" aria-label="매크로 파일">
                  {good.map((f) => (
                    <LibraryItem key={f.fileName} file={f} busy={busy} onPick={() => pick(f.fileName)} />
                  ))}
                </ul>
              )}
              {truncated > 0 && <p className="lib-more">최근 수정한 300개만 보여요 · {truncated}개 더 있어요</p>}
              {problemCount > 0 && (
                <section className="lib-problems" aria-labelledby="lib-problems-title">
                  <div className="lib-group">
                    <h3 id="lib-problems-title" className="lib-group-name">
                      추가할 수 없는 파일
                    </h3>
                    <span className="lib-group-count num">{problemCount}</span>
                    <span className="lib-group-line" aria-hidden="true" />
                    <button
                      type="button"
                      className="lib-group-toggle"
                      aria-expanded={problemsOpen}
                      aria-controls="lib-problems-list"
                      onClick={() => setProblemsOpen((v) => !v)}
                    >
                      <SwapLabel active={problemsOpen ? 0 : 1} items={["접기", "펼치기"]} />
                      <ChevronDown size={13} className={problemsOpen ? "is-open" : ""} aria-hidden="true" />
                    </button>
                  </div>
                  {problemsOpen && (
                    <ul id="lib-problems-list" className="lib-problem-list">
                      {problems.map((f) => (
                        <LibraryProblem key={f.fileName} file={f} />
                      ))}
                      {skipped > 0 && (
                        <li className="lib-problem">
                          <span className="lib-problem-name">.json 파일 {skipped}개</span>
                          <span className="lib-problem-why">
                            이름이 .ggm.json이 아니에요 — [파일에서 추가]로 넣으면 고쳐져요
                          </span>
                        </li>
                      )}
                    </ul>
                  )}
                </section>
              )}
            </>
          )}
        </div>

        <footer className="lib-foot">
          <FolderOpen size={14} aria-hidden="true" />
          <span className="lib-foot-name" title={dir}>
            매크로 폴더
          </span>
          <button type="button" className="btn btn-ghost btn-sm lib-open" onClick={() => act("library_open_folder")}>
            폴더 열기
          </button>
          <button
            type="button"
            className="icon-btn"
            aria-label="새로고침"
            title="새로고침"
            onClick={() => act("library_refresh")}
          >
            <RefreshCw size={14} />
          </button>
        </footer>
      </div>
    </div>
  );
}
