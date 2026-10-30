// 코인 표시 — 동그란 코인 로고 오른쪽 아래 모서리에 거래소 작은 로고를 겹침. 어느 거래소의 어느 코인인지 한눈에 보이게.
// - 로고 출처는 웹과 같음: 바이낸스 공개 로고 CDN → 껄무새 서버 사본(/api/coin-logo) → 글자 배지 순서로 되돌아감.
//   - CDN은 리퍼러가 붙으면 핫링크로 보고 막음 — referrerPolicy no-referrer.
//   - 상장 직후처럼 로고가 없는 종목은 BASE 앞 세 글자를 회색 동그라미에 보임(깨진 이미지 아이콘 대신).
// - 앱 CSP(tauri.conf.json의 img-src)는 이 두 곳만 허용함. 서버 주소가 기본 서버가 아니면(개발 빌드) 사본이 막혀 글자 배지로 감.
// - 몇 번째 출처까지 실패했는지는 BASE와 함께 기억함 — 같은 자리에서 종목이 바뀌면 CDN부터 다시 시도함.
// - 코인 로고는 장식(alt "") — 옆에 종목 글자가 늘 있음. 작은 로고는 거래소 이름으로 읽힘(exchangeLabel이 ""면 장식).
// - 작은 로고의 테두리는 둘레 바탕색(--coin-ring) — 카드·고른 카드·대기 행·다크 모드마다 놓인 곳에서 정함(cards.css).
import { useState, type CSSProperties } from "react";
import type { Exchange } from "../types";
import { ExchangeLogo } from "./ExchangeLogo";

const LOGO_CDN = "https://bin.bnbstatic.com/static/assets/logos";
/** 스냅샷에 서버 주소가 없을 때 — Rust DEFAULT_SERVER_BASE와 같은 값. */
export const DEFAULT_SERVER_BASE = "https://gg-parrot.onrender.com";

/** 로고 주소 차례 — 앞의 것이 실패하면 다음. 모두 실패하면 글자 배지. */
export function coinLogoSources(base: string, serverBase: string): string[] {
  const key = encodeURIComponent(base);
  const server = (serverBase.trim() || DEFAULT_SERVER_BASE).replace(/\/+$/, "");
  return [`${LOGO_CDN}/${key}.png`, `${server}/api/coin-logo/${key}.png`];
}

interface Props {
  /** 기초 자산(BTC) — 대문자로 바꿔 씀. */
  base: string;
  exchange: Exchange | null;
  /** 작은 로고의 대체 글자("업비트"). 같은 이름이 옆 글자에 이미 있으면 ""(장식). */
  exchangeLabel: string;
  /** 코인 로고 지름(px). */
  size: number;
  /** 거래소 작은 로고 지름(px). */
  badge: number;
  /** 껄무새 서버 주소(설정 스냅샷의 serverBase). */
  serverBase: string;
}

export function CoinMark({ base, exchange, exchangeLabel, size, badge, serverBase }: Props) {
  const key = base.trim().toUpperCase();
  const [failed, setFailed] = useState({ key, stage: 0 });
  const stage = failed.key === key ? failed.stage : 0;
  const src = key ? coinLogoSources(key, serverBase)[stage] : undefined;
  const style = { "--coin": `${size}px`, "--coin-badge": `${badge}px` } as CSSProperties;

  return (
    <span className="coin-mark" style={style}>
      {src ? (
        <img
          // 주소가 바뀌면 새 이미지로 — 지난 주소의 늦은 실패가 새 종목에 붙지 않게.
          key={src}
          className="coin-mark-logo"
          src={src}
          width={size}
          height={size}
          alt=""
          referrerPolicy="no-referrer"
          draggable={false}
          onError={() => setFailed({ key, stage: stage + 1 })}
        />
      ) : (
        <span className="coin-mark-letters" aria-hidden="true">
          {key.slice(0, 3) || "?"}
        </span>
      )}
      <ExchangeLogo exchange={exchange} size={badge} label={exchangeLabel} className="coin-mark-ex" />
    </span>
  );
}
