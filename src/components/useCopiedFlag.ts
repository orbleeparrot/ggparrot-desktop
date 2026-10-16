// '복사했어요' 표시 — 복사에 성공하면 켜고 ms 뒤에 저절로 끔.
import { useCallback, useEffect, useState } from "react";

/** [켜져 있는지, 켜기]. 켜져 있는 동안 다시 켜도 타이머는 처음 켠 때부터 셈. */
export function useCopiedFlag(ms: number): [boolean, () => void] {
  const [copied, setCopied] = useState(false);
  useEffect(() => {
    if (!copied) return;
    const t = setTimeout(() => setCopied(false), ms);
    return () => clearTimeout(t);
  }, [copied, ms]);
  const markCopied = useCallback(() => setCopied(true), []);
  return [copied, markCopied];
}
