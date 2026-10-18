import type { Exchange } from "../types";

const SRC: Record<Exchange, string> = {
  binance: "/exchanges/binance.svg",
  upbit: "/exchanges/upbit.png",
  bithumb: "/exchanges/bithumb.png",
};

/** 거래소 로고. label이 있으면 그 이름으로 읽힘(옆에 거래소 이름 글자가 없을 때), 없으면 장식. */
export function ExchangeLogo({
  exchange,
  size = 24,
  label = "",
  className = "",
}: {
  exchange: Exchange | null;
  size?: number;
  label?: string;
  className?: string;
}) {
  if (!exchange) return null;
  return (
    <img
      className={`ex-logo ${className}`.trim()}
      src={SRC[exchange]}
      width={size}
      height={size}
      alt={label}
      aria-hidden={label ? undefined : "true"}
    />
  );
}
