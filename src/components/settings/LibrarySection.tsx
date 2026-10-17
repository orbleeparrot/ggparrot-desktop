// 설정 › 매크로 폴더 — 경로와 [폴더 열기].
import { FolderOpen } from "lucide-react";
import { useRunnerContext } from "../../state/context";
import { Section } from "./Section";

/** 매크로 폴더 설명 — 설정 › 매크로 폴더 머리 아래(매크로 추가 시트에서는 줄글을 뺐음). */
const LIBRARY_HINT =
  "웹에서 받은 .ggm.json 파일을 창에 끌어다 놓거나 매크로 추가의 [파일에서 추가]로 넣으면 이 폴더에 복사되고 목록에 나타나요.";

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
