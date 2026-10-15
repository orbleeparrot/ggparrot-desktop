// 업데이트의 '바뀐 점' — 릴리스 본문(마크다운)을 제목·목록으로 그림. 기호(###, -)를 그대로 보이지 않게.
import { Fragment } from "react";
import { parseReleaseNotes, splitCode } from "../../state/releaseNotes";

function Inline({ text }: { text: string }) {
  return (
    <>
      {splitCode(text).map((part, i) =>
        i % 2 === 1 ? <code key={i}>{part}</code> : <Fragment key={i}>{part}</Fragment>,
      )}
    </>
  );
}

export function ReleaseNotes({ body }: { body: string }) {
  const lines = parseReleaseNotes(body);
  return (
    <div className="release-notes">
      {lines.map((l, i) =>
        l.kind === "heading" ? (
          <p key={i} className="release-notes-heading">
            <Inline text={l.text} />
          </p>
        ) : l.kind === "item" ? (
          <p key={i} className={`release-notes-item level-${l.level}`}>
            <Inline text={l.text} />
          </p>
        ) : (
          <p key={i} className="release-notes-text">
            <Inline text={l.text} />
          </p>
        ),
      )}
    </div>
  );
}
