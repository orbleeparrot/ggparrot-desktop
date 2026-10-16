// 바이낸스 화면 그림의 조각 — 색·글자 폭 어림·표식·어두운 화면 머리줄(데모·실거래).
import { T } from "./kit";
import { W } from "./types";

export const BN_YELLOW = "#FCD535";
export const BN_GOLD = "#F0B90B";
export const TN_BAR = "#12161C";
export const TN_BG = "#FAFAFA";
export const TN_LINK = "#2F8AE6";
export const TN_LINE = "#DADADA";
export const TN_HEAD = "#F2F2F2";
export const TN_CHECK = "#1A73E8";
export const TN_SELECT = "#B4D5FE";
export const BN_BG = "#181A20";
export const BN_PANEL = "#1E2329";
export const BN_LINE = "#2B3139";
export const BN_INK = "#EAECEF";
export const BN_MUTE = "#848E9C";
export const BN_GRAY = "#2B3139";
export const BN_NOTE = "#2A2718";
export const BN_CHECK_OFF = "#333B47";
export const BN_RED = "#F6465D";
/** 어두운 화면 위에 뜬 창 뒤를 덮는 막. */
export const BN_DIM = "rgba(11, 14, 17, 0.72)";
export const SMALL = 10;

export const BN_KEYS = ["0lOHb8TL" + "•".repeat(18), "•".repeat(30)] as const;
export const TN_KEYS = ["FdiXVmAJ" + "*".repeat(22), "iRvB4t51" + "*".repeat(22)] as const;

const WIDE = /[\u1100-\u11FF\u3000-\u9FFF\uAC00-\uD7AF\uFF00-\uFFEF]/;

const UPPER = /[A-Z]/;

/** 글자 폭 어림값(그림 배치용). 한글 등 넓은 글자는 글자 크기만큼, 영문 대문자는 0.7배, 그 밖은 반쯤. */
export function tw(text: string, size = 12, bold = false): number {
  let w = 0;
  for (const ch of text) w += WIDE.test(ch) ? size : UPPER.test(ch) ? size * 0.7 : size * (bold ? 0.55 : 0.5);
  return w;
}

/** limit 폭에 맞게 줄이고 '…'을 붙임. */
export function fit(text: string, limit: number, size = 12, bold = false): string {
  if (tw(text, size, bold) <= limit) return text;
  let t = text;
  while (t && tw(t + "…", size, bold) > limit) t = t.slice(0, -1);
  return t + "…";
}

/** 바이낸스 표식 — 마름모 하나와 BINANCE 글자(단순화). */
export function BnMark({ x, cy }: { x: number; cy: number }) {
  return (
    <g>
      <polygon points={`${x + 6},${cy - 6} ${x + 12},${cy} ${x + 6},${cy + 6} ${x},${cy}`} fill={BN_GOLD} />
      <T x={x + 18} y={cy} bold fill={BN_GOLD}>
        BINANCE
      </T>
    </g>
  );
}
export const MARK_END = 12 + 18 + tw("BINANCE", 12, true);

// 선물 데모·실거래 API Management(어두운 화면)의 머리줄.
export type Dark = "demo" | "live";
export const DARK_URL: Record<Dark, string> = {
  demo: "demo.binance.com/en/my/settings/api-management",
  live: "www.binance.com/en/my/settings/api-management",
};

export function BnChrome({ env, height }: { env: Dark; height: number }) {
  const items = env === "demo" ? ["Spot", "Futures"] : ["Buy Crypto", "Markets", "Trade", "Futures"];
  const badge = "DEMO TRADING";
  const bx = MARK_END + 8;
  let x = env === "demo" ? bx + tw(badge, SMALL) + 20 : MARK_END + 18;
  return (
    <g>
      <rect x={0} y={0} width={W} height={height} fill={BN_BG} />
      <BnMark x={12} cy={16} />
      {env === "demo" && (
        <g>
          <rect x={bx} y={10} width={tw(badge, SMALL) + 8} height={12} fill="#B88F09" />
          <T x={bx + 4} y={16} size={SMALL}>
            {badge}
          </T>
        </g>
      )}
      {items.map((item) => {
        const at = x;
        x += tw(item, 12, true) + 14;
        return (
          <T key={item} x={at} y={16} bold fill={BN_INK}>
            {item}
          </T>
        );
      })}
      <line x1={0} y1={32} x2={W} y2={32} stroke={BN_LINE} />
    </g>
  );
}
