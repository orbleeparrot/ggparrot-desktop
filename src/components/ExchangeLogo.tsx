import type { Exchange } from "../types";

const SRC: Record<Exchange, string> = {
  binance: "/exchanges/binance.svg",
  upbit: "/exchanges/upbit.png",
  bithumb: "/exchanges/bithumb.png",
};

export function ExchangeLogo({ exchange, size = 24 }: { exchange: Exchange | null; size?: number }) {
  if (!exchange) return null;
  return <img className="ex-logo" src={SRC[exchange]} width={size} height={size} alt="" aria-hidden="true" />;
}
