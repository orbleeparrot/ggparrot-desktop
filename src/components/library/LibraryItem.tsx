// 매크로 폴더의 파일 한 줄. 읽히지 않는 파일·이 앱이 실행할 수 없는 매크로는 이유와 함께 흐리게(누를 수 없음).
import { BadgeCheck, FileWarning } from "lucide-react";
import { formatWhen } from "../../state/format";
import { SIDE_LABELS } from "../../state/runs";
import type { LibraryFile } from "../../types";
import { ExchangeLogo } from "../ExchangeLogo";

export function LibraryItem({ file, busy, onPick }: { file: LibraryFile; busy: boolean; onPick(): void }) {
  if (!file.valid) {
    return (
      <li>
        <button type="button" className="lib-item is-invalid" disabled>
          <span className="lib-icon" aria-hidden="true">
            <FileWarning size={18} />
          </span>
          <span className="lib-main">
            <span className="lib-title">{file.fileName}</span>
            <span className="lib-error">{file.error || "매크로 파일로 읽지 못했어요."}</span>
          </span>
        </button>
      </li>
    );
  }
  const blocked = !!file.unsupported;
  return (
    <li>
      <button
        type="button"
        className={`lib-item ${blocked ? "is-blocked" : ""}`}
        onClick={onPick}
        disabled={busy || blocked}
        title={blocked ? file.unsupported : undefined}
      >
        <span className="lib-icon" aria-hidden="true">
          <ExchangeLogo exchange={file.exchange} size={24} />
        </span>
        <span className="lib-main">
          <span className="lib-title">{file.title || file.fileName}</span>
          <span className="lib-meta">
            <span className="sr-only">{file.exchangeLabel} </span>
            <span className="num strong">{file.symbol}</span>
            <span className={`side side-${file.side}`}>{SIDE_LABELS[file.side]}</span>
            {file.strategy && <span className="lib-strategy">{file.strategy}</span>}
            {file.signed && (
              <span className="lib-signed">
                <BadgeCheck size={12} aria-hidden="true" />
                서명됨
              </span>
            )}
            <span className="lib-when">{file.modifiedAt > 0 ? `수정 ${formatWhen(file.modifiedAt)}` : ""}</span>
          </span>
          {blocked && <span className="lib-error">{file.unsupported}</span>}
        </span>
        {file.inUse > 0 && <span className="lib-inuse">실행 목록에 {file.inUse}개</span>}
      </button>
    </li>
  );
}
