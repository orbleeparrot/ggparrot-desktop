//! 거래소의 종목 화면 주소(시세·차트) — 상세 머리줄의 '○○에서 보기'가 기본 브라우저로 엶.
//! 주소는 거래소·종목·시장·모드로 여기서만 만듦. 웹뷰가 넘긴 주소는 열지 않음.

use crate::types::{base_asset, Exchange, Market, RunMode};

/// 종목 화면 — 단추 글자와 주소.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MarketPage {
    /// "업비트에서 보기"·"바이낸스 데모에서 보기" 등
    pub label: String,
    pub url: String,
}

/// 종목 이름 조각은 영문 대문자·숫자만 받음 — 주소에 다른 글자가 끼지 않게.
fn plain(s: &str) -> bool {
    !s.is_empty() && s.len() <= 20 && s.bytes().all(|b| b.is_ascii_uppercase() || b.is_ascii_digit())
}

/// 바이낸스 종목(`BTCUSDT`)의 기초·견적 자산. 모르는 모양이면 None.
fn binance_pair(symbol: &str) -> Option<(String, String)> {
    let s = symbol.trim().to_uppercase();
    let base = base_asset(&s);
    let quote = s.strip_prefix(&base)?.to_string();
    (plain(&base) && plain(&quote)).then_some((base, quote))
}

/// 매크로 종목의 거래소 화면. 모르는 거래소·종목 모양이면 None.
/// - 국내(업비트·빗썸): 원화 시장 화면
/// - 바이낸스: 현물·선물 화면. 테스트넷 모드의 선물은 데모 사이트(주문·포지션이 거기 보임).
///   현물 테스트넷(testnet.binance.vision)은 종목 화면이 없어 실제 현물 화면으로 엶.
pub fn market_page(exchange: Exchange, symbol: &str, market: Market, mode: RunMode) -> Option<MarketPage> {
    let page = |label: &str, url: String| Some(MarketPage { label: format!("{label}에서 보기"), url });
    match exchange {
        Exchange::Upbit | Exchange::Bithumb => {
            let s = symbol.trim().to_uppercase();
            let base = s.strip_prefix("KRW-")?;
            if !plain(base) {
                return None;
            }
            if exchange == Exchange::Upbit {
                page("업비트", format!("https://upbit.com/exchange?code=CRIX.UPBIT.KRW-{base}"))
            } else {
                page("빗썸", format!("https://www.bithumb.com/react/trade/order/{base}-KRW"))
            }
        }
        Exchange::Binance => {
            let (base, quote) = binance_pair(symbol)?;
            match (market, mode) {
                (Market::Futures, RunMode::Testnet) => {
                    page("바이낸스 데모", format!("https://demo.binance.com/futures/{base}{quote}"))
                }
                (Market::Futures, _) => page("바이낸스", format!("https://www.binance.com/futures/{base}{quote}")),
                (Market::Spot, _) => {
                    page("바이낸스", format!("https://www.binance.com/trade/{base}_{quote}?type=spot"))
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn url(e: Exchange, s: &str, m: Market, mode: RunMode) -> Option<String> {
        market_page(e, s, m, mode).map(|p| p.url)
    }

    #[test]
    fn domestic_pages() {
        let p = market_page(Exchange::Upbit, "KRW-BTC", Market::Spot, RunMode::Live).unwrap();
        assert_eq!(p.label, "업비트에서 보기");
        assert_eq!(p.url, "https://upbit.com/exchange?code=CRIX.UPBIT.KRW-BTC");
        assert_eq!(
            url(Exchange::Bithumb, "krw-xrp", Market::Spot, RunMode::Mock).as_deref(),
            Some("https://www.bithumb.com/react/trade/order/XRP-KRW")
        );
    }

    #[test]
    fn binance_pages_follow_market_and_mode() {
        assert_eq!(
            url(Exchange::Binance, "BTCUSDT", Market::Spot, RunMode::Live).as_deref(),
            Some("https://www.binance.com/trade/BTC_USDT?type=spot")
        );
        // 현물 테스트넷은 종목 화면이 없어 실제 현물 화면
        assert_eq!(
            url(Exchange::Binance, "ETHUSDC", Market::Spot, RunMode::Testnet).as_deref(),
            Some("https://www.binance.com/trade/ETH_USDC?type=spot")
        );
        assert_eq!(
            url(Exchange::Binance, "BTCUSDT", Market::Futures, RunMode::Mock).as_deref(),
            Some("https://www.binance.com/futures/BTCUSDT")
        );
        let demo = market_page(Exchange::Binance, "BTCUSDT", Market::Futures, RunMode::Testnet).unwrap();
        assert_eq!(demo.label, "바이낸스 데모에서 보기");
        assert_eq!(demo.url, "https://demo.binance.com/futures/BTCUSDT");
    }

    #[test]
    fn odd_symbols_get_no_page() {
        for s in ["", "KRW-", "BTC", "KRW-BTC/../x", "KRW-BT C", "KRW-한글"] {
            assert_eq!(url(Exchange::Upbit, s, Market::Spot, RunMode::Live), None, "{s:?}");
        }
        for s in ["", "USDT", "BTC-USDT", "BTCUSDT?x=1", "BTCKRW"] {
            assert_eq!(url(Exchange::Binance, s, Market::Spot, RunMode::Live), None, "{s:?}");
        }
    }
}
