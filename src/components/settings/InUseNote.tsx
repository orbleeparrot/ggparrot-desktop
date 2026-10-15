import { Info } from "lucide-react";

/** 키는 언제든 바꿀 수 있음 — 실행 중인 매크로는 시작할 때 받은 키를 계속 씀. 여럿이면 잔고를 나눠 씀(시작 전 확인은 각자 혼자 쓴다고 보고 통과함). */
export function InUseNote({ count }: { count: number }) {
  if (count <= 0) return null;
  return (
    <p className="note note-muted in-use-note">
      <Info size={13} aria-hidden="true" />
      실행 중인 매크로 {count}개는 시작할 때의 키를 계속 써요. 바꾼 키는 다음 시작부터 적용돼요.
      {count > 1 && " 같은 키로 여러 매크로가 돌면 잔고를 나눠 써서, 나중 진입은 잔고가 모자라 거절될 수 있어요."}
    </p>
  );
}
