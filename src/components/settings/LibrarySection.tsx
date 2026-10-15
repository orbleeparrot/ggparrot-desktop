// 설정 › 매크로 폴더 — 경로와 [폴더 열기].
import { FolderOpen } from "lucide-react";
import { useRunnerContext } from "../../state/context";
import { LIBRARY_HINT } from "../library/LibrarySheet";
import { Section } from "./Section";

export function LibrarySection() {
  const { state, act } = useRunnerContext();
  const { dir, files } = state.library;
  return (
    <Section id="library" title="매크로 폴더" lead={LIBRARY_HINT}>
      <div className="plain-card">
        <div className="lib-folder-row">
          <code className="path" title={dir}>
            {dir}
          </code>
          <button type="button" className="btn btn-ghost btn-sm" onClick={() => act("library_open_folder")}>
            <FolderOpen size={14} aria-hidden="true" />
            폴더 열기
          </button>
        </div>
        <p className="muted">매크로 파일 {files.length}개 · 껄무새는 이 폴더의 파일을 지우거나 고치지 않아요.</p>
      </div>
    </Section>
  );
}
