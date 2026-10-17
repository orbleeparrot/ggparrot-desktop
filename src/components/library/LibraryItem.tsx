// 매크로 폴더의 파일 한 줄.
// - 추가할 수 있는 매크로: 두 줄 카드(제목 + 서명 표시 / 종목 · 방향 · 전략 · 수정 시각). 누르면 실행 목록에 추가.
//   마우스를 올리면 오른쪽에 노란 [+ 추가], 이미 실행 목록에 있으면 그 자리에 초록 '실행 목록 N개'.
// - 추가할 수 없는 파일(읽지 못함 · 이 앱이 아직 실행 못 하는 유형): 시트 아래 회색 칸에 '이름 · 이유' 한 줄(LibraryProblem).
import { BadgeCheck, Plus } from "lucide-react";
import { formatWhen } from "../../state/format";
import { SIDE_LABELS } from "../../state/runs";
import type { LibraryFile } from "../../types";
import { ExchangeLogo } from "../ExchangeLogo";

/** 목록에 넣을 수 없는 파일인지 — 읽지 못했거나 이 앱이 실행할 수 없는 유형. */
export function isProblem(file: LibraryFile): boolean {
  return !file.valid || !!file.unsupported;
}

export function LibraryItem({ file, busy, onPick }: { file: LibraryFile; busy: boolean; onPick(): void }) {
  return (
    <li>
      <button type="button" className="lib-item" onClick={onPick} disabled={busy}>
        <span className="lib-logo" aria-hidden="true">
          <ExchangeLogo exchange={file.exchange} size={22} />
        </span>
        <span className="lib-title">
          <span className="lib-title-text">{file.title || file.fileName}</span>
          {file.signed && <BadgeCheck size={14} className="lib-signed" aria-label="서명됨" />}
        </span>
        {file.inUse > 0 && <span className="lib-inuse">실행 목록 {file.inUse}개</span>}
        <span className="lib-add" aria-hidden="true">
          <Plus size={15} strokeWidth={2.6} />
          추가
        </span>
        <span className="lib-meta">
          <span className="sr-only">{file.exchangeLabel} </span>
          <span className="num lib-symbol">{file.symbol}</span>
          <span className={`side side-${file.side}`}>{SIDE_LABELS[file.side]}</span>
          {file.strategy && <span className="lib-dot">{file.strategy}</span>}
          {file.modifiedAt > 0 && <span className="lib-dot">{formatWhen(file.modifiedAt)}</span>}
        </span>
      </button>
    </li>
  );
}

export function LibraryProblem({ file }: { file: LibraryFile }) {
  const why = !file.valid ? file.error || "매크로 파일로 읽지 못했어요." : file.unsupported;
  return (
    <li className="lib-problem" title={why}>
      <span className="lib-problem-name">{file.title || file.fileName}</span>
      <span className="lib-problem-why">{why}</span>
    </li>
  );
}
