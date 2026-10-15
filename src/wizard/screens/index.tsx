// 연결 도우미의 거래소 화면 그림 — 업비트·빗썸 × ip·perm·keys·fix·existing.
import { bithumbExisting, bithumbFix, bithumbIp, bithumbKeys, bithumbPerm } from "./bithumb";
import { ScenePlayer } from "./ScenePlayer";
import type { Scene } from "./types";
import { upbitExisting, upbitFix, upbitIp, upbitKeys, upbitPerm } from "./upbit";

export type ScreenKind = "ip" | "perm" | "keys" | "fix" | "existing";

export const SCENES: Record<"upbit" | "bithumb", Record<ScreenKind, Scene>> = {
  upbit: { ip: upbitIp, perm: upbitPerm, keys: upbitKeys, fix: upbitFix, existing: upbitExisting },
  bithumb: { ip: bithumbIp, perm: bithumbPerm, keys: bithumbKeys, fix: bithumbFix, existing: bithumbExisting },
};

const LABEL = { upbit: "업비트", bithumb: "빗썸" } as const;

export function ExchangeScreen({
  exchange,
  kind,
  ip,
}: {
  exchange: "upbit" | "bithumb";
  kind: ScreenKind;
  ip: string;
}) {
  return <ScenePlayer scene={SCENES[exchange][kind]} ip={ip} label={LABEL[exchange]} />;
}
