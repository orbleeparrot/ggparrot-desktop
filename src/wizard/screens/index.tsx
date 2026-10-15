// 연결 도우미의 거래소 화면 그림 — 사이트(업비트·빗썸·바이낸스 테스트넷·데모·실거래) × 단계.
// 그림이 없는 단계(바이낸스 연습 환경의 허용 IP 등)는 그리지 않음.
import type { WizardState } from "../../types";
import {
  binanceDemoExisting,
  binanceDemoKeys,
  binanceLiveExisting,
  binanceLiveKeys,
  binanceLiveRestrict,
} from "./binanceCard";
import { binanceDemoPerm, binanceLivePerm } from "./binanceDark";
import { binanceTestnetExisting, binanceTestnetKeys, binanceTestnetPerm } from "./binanceTestnet";
import { bithumbExisting, bithumbFix, bithumbIp, bithumbKeys, bithumbPerm } from "./bithumb";
import { ScenePlayer } from "./ScenePlayer";
import type { Scene } from "./types";
import { upbitExisting, upbitFix, upbitIp, upbitKeys, upbitPerm } from "./upbit";

/** restrict: 바이낸스 실거래 키 카드의 Edit restrictions(허용 IP·주문 권한). */
export type ScreenKind = "ip" | "perm" | "keys" | "fix" | "existing" | "restrict";
export type Site = "upbit" | "bithumb" | "binance:testnet" | "binance:demo" | "binance:live";

export const SCENES = {
  upbit: { ip: upbitIp, perm: upbitPerm, keys: upbitKeys, fix: upbitFix, existing: upbitExisting },
  bithumb: { ip: bithumbIp, perm: bithumbPerm, keys: bithumbKeys, fix: bithumbFix, existing: bithumbExisting },
  "binance:testnet": { perm: binanceTestnetPerm, keys: binanceTestnetKeys, existing: binanceTestnetExisting },
  "binance:demo": { perm: binanceDemoPerm, keys: binanceDemoKeys, existing: binanceDemoExisting },
  "binance:live": {
    perm: binanceLivePerm,
    keys: binanceLiveKeys,
    restrict: binanceLiveRestrict,
    existing: binanceLiveExisting,
  },
} satisfies Record<Site, Partial<Record<ScreenKind, Scene>>>;

/** 도우미가 안내하는 사이트 — 바이낸스는 실행 환경(env)마다 다름. */
export function siteOf(w: Pick<WizardState, "exchange" | "env">): Site {
  if (w.exchange === "binance") return `binance:${w.env || "live"}` as Site;
  return w.exchange;
}

/** label: 사이트 이름(WizardState.label) — 그림의 접근성 이름에 씀. */
export function ExchangeScreen({ site, kind, ip, label }: { site: Site; kind: ScreenKind; ip: string; label: string }) {
  const scenes: Partial<Record<ScreenKind, Scene>> = SCENES[site];
  const scene = scenes[kind];
  if (!scene) return null;
  return <ScenePlayer scene={scene} ip={ip} label={label} />;
}
