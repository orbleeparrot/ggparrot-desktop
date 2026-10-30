//! 장부가 서버와 **같은 숫자**를 내는지.
//!
//! 아래 기대값은 전부 서버 장부(파이썬 `backend/engine/candles.py`의 `CandleSim`)를
//! 실제로 돌려 받아 적은 것임. 손으로 계산하지 않음 — 손으로 하면 내 산수를 검증할 뿐
//! 서버와 같은지는 모름.
//!
//! 여섯 규칙이 전부 이 위에 올라탐. 여기가 1원이라도 다르면 전부 어긋남.

use super::*;

/// 부동소수점은 식의 **순서**까지 같아야 같은 값이 나옴. 상대오차 1e-12는
/// "같은 식을 같은 순서로 계산했다"는 뜻이고, 더 느슨하게 두면 식이 바뀐 것을 못 잡음.
#[track_caller]
fn near(got: f64, want: f64) {
    let scale = want.abs().max(1.0);
    assert!(
        (got - want).abs() <= 1e-12 * scale,
        "서버와 다르다: got {got:.17}, want {want:.17} (차이 {:.3e})",
        got - want
    );
}

/// 실제 AZIT 매크로와 같은 비용 — 빗썸 현물 0.04%, 슬리피지 0.05%.
fn azit() -> Ledger {
    Ledger::new(Side::Long, Costs { commission_pct: 0.04, slippage_pct: 0.05, ..Costs::default() }, 40_000.0)
}

// --- 진입 -------------------------------------------------------------------
#[test]
fn 진입이_서버와_같은_수량과_현금을_만든다() {
    // 40,000원으로 2.771원짜리를 삼. 슬리피지로 체결가가 밀리고, 수수료가 현금에서 빠짐.
    let mut l = azit();
    let f = l.open_long(40_000.0, 2.771, 2.771, "진입").expect("체결");
    near(f.price, 2.7723855);
    near(f.qty, 14428.007937568567);
    near(l.cash, -16.0); // 수수료 16원 — 40,000원을 다 넣었으니 현금이 음수로 감(서버와 같음)
    near(l.avg_entry(), 2.7723855);
    near(l.committed_margin(), 40_000.0);
    near(l.equity(2.771), 39964.009995002496);
    near(f.equity_after, 39964.009995002496);
    near(f.return_pct, -0.08997501249376);
    near(f.qty_before, 0.0);
    assert_eq!((f.side, f.reason.as_str()), ("buy", "진입"));
}

#[test]
fn 현금보다_많이_넣으려_하면_현금까지만_쓴다() {
    let mut l =
        Ledger::new(Side::Long, Costs { commission_pct: 0.04, slippage_pct: 0.05, ..Costs::default() }, 1_000.0);
    let f = l.open_long(5_000.0, 10.0, 10.0, "진입").expect("체결");
    near(f.qty, 99.95002498750625);
    near(l.cash, -0.39999999999997726);
    near(l.committed_margin(), 1_000.0);
}

#[test]
fn 돈이_없으면_아무_일도_없다() {
    let mut l = azit();
    l.cash = 0.0;
    assert!(l.open_long(1_000.0, 2.771, 2.771, "진입").is_none());
    assert!(!l.in_position());
    assert!(l.lots.is_empty());
}

// --- 청산 -------------------------------------------------------------------
#[test]
fn 전량_청산이_서버와_같은_실현손익을_낸다() {
    let mut l = azit();
    l.open_long(40_000.0, 2.771, 2.771, "진입").expect("체결");
    let g = l.close_all(3.012, 3.012, "청산").expect("체결");
    near(g.price, 3.010494);
    near(g.qty, 14428.007937568567);
    near(l.cash, 43402.057155471346);
    near(l.equity(3.012), 43402.057155471346);
    near(l.closed_trades[0], 3418.0571554713447);
    near(g.equity_after, 43402.057155471346);
    near(g.return_pct, 8.505142888678364);
    near(g.qty_before, 14428.007937568567);
    assert_eq!((g.side, g.reason.as_str()), ("sell", "청산"));
    assert!(!l.in_position());
}

#[test]
fn 보유가_없으면_청산이_없다() {
    let mut l = azit();
    assert!(l.close_all(3.0, 3.0, "청산").is_none());
    assert!(l.closed_trades.is_empty());
}

// --- 나눠 사기(그리드·마틴게일) ---------------------------------------------
#[test]
fn 여러_번_나눠_사면_평단이_서버와_같다() {
    let mut l = azit();
    for (margin, price) in [(10_000.0, 3.0), (10_000.0, 2.5), (10_000.0, 2.0)] {
        l.open_long(margin, price, price, "진입").expect("체결");
    }
    near(l.total_qty(), 12327.169748459104);
    near(l.avg_entry(), 2.4336486486486484);
    near(l.cash, 9_988.0);
    near(l.committed_margin(), 30_000.0);
    near(l.equity(2.2), 37107.77344661003);
    let want: [(f64, f64, f64); 3] = [
        (3331.6674995835415, 3.0015, 10_000.0),
        (3998.00099950025, 2.5012499999999998, 10_000.0),
        (4997.501249375313, 2.001, 10_000.0),
    ];
    assert_eq!(l.lots.len(), 3);
    for (lot, (qty, fill, margin)) in l.lots.iter().zip(want) {
        near(lot.qty, qty);
        near(lot.fill, fill);
        near(lot.margin, margin);
    }
}

#[test]
fn 절반_청산이_서버와_같다() {
    let mut l = azit();
    for (margin, price) in [(10_000.0, 3.0), (10_000.0, 2.5), (10_000.0, 2.0)] {
        l.open_long(margin, price, price, "진입").expect("체결");
    }
    let h = l.reduce_long(0.5, 2.4, 2.4, "부분 청산").expect("체결");
    near(h.price, 2.3988);
    near(h.qty, 6163.584874229552);
    near(l.cash, 24767.29331334333);
    near(l.total_qty(), 6163.584874229552);
    // 비율로 줄이므로 평단은 그대로임 — 이게 틀리면 다음 익절선이 어긋남.
    near(l.avg_entry(), 2.4336486486486484);
    near(l.closed_trades[0], -220.70668665667063);
    near(h.qty_before, 12327.169748459104);
    near(h.equity_after, 39559.89701149425);
    assert_eq!(h.reason, "부분 청산");
}

#[test]
fn 전량_비율로_줄이면_묶음이_비고_평단이_0_이_된다() {
    let mut l = azit();
    l.open_long(10_000.0, 3.0, 3.0, "진입").expect("체결");
    l.reduce_long(1.0, 2.4, 2.4, "부분 청산").expect("체결");
    assert!(!l.in_position());
    assert!(l.lots.is_empty(), "먼지 묶음이 남으면 다음 평단이 더러워진다");
    near(l.avg_entry(), 0.0);
}

#[test]
fn 숏과_빈_장부는_비율_청산을_하지_않는다() {
    let mut short = Ledger::new(Side::Short, Costs::default(), 1_000.0);
    short.open_short(1_000.0, 10.0, 10.0, "진입").expect("체결");
    assert!(short.reduce_long(0.5, 9.0, 9.0, "부분 청산").is_none());
    let mut flat = azit();
    assert!(flat.reduce_long(0.5, 2.4, 2.4, "부분 청산").is_none());
}

// --- 마찰 없는 경우 ---------------------------------------------------------
#[test]
fn 수수료와_슬리피지가_0_이면_식이_깨끗하다() {
    // 1,000원으로 10원짜리 100개를 사서 11원에 팖 — 정확히 +10%.
    let mut l = Ledger::new(Side::Long, Costs::default(), 1_000.0);
    let a = l.open_long(1_000.0, 10.0, 10.0, "진입").expect("체결");
    near(a.qty, 100.0);
    let b = l.close_all(11.0, 11.0, "청산").expect("체결");
    near(b.price, 11.0);
    near(l.cash, 1_100.0);
    near(l.closed_trades[0], 100.0);
    near(b.return_pct, 10.0);
}

// --- 체결 메타 ---------------------------------------------------------------
#[test]
fn 직전_수량을_바르게_되계산한다() {
    // 청산 비율(qty / qty_before)이 여기서 나옴 — 틀리면 실행기가 엉뚱한 수량을 팖.
    let mut l = azit();
    let a = l.open_long(10_000.0, 2.0, 2.0, "진입").expect("체결");
    near(a.qty_before, 0.0);
    let b = l.open_long(10_000.0, 2.0, 2.0, "추가 진입").expect("체결");
    near(b.qty_before, a.qty);
    let total = l.total_qty();
    let c = l.close_all(2.5, 2.5, "청산").expect("체결");
    near(c.qty_before, total);
    near(c.qty / c.qty_before, 1.0);
}

#[test]
fn 사유가_비면_기본_문구가_들어간다() {
    let mut l = azit();
    let a = l.open_long(10_000.0, 2.0, 2.0, "").expect("체결");
    assert_eq!(a.reason, "진입");
    let b = l.close_all(2.5, 2.5, "").expect("체결");
    assert_eq!(b.reason, "청산");
}

#[test]
fn 자산은_체결가가_아니라_기준가로_잰다() {
    // 서버와 같게 — 체결가로 재면 슬리피지가 수익률에 두 번 반영됨.
    let mut l = Ledger::new(Side::Long, Costs { slippage_pct: 1.0, ..Costs::default() }, 1_000.0);
    let f = l.open_long(1_000.0, 10.0, 10.0, "진입").expect("체결");
    near(f.equity_after, l.equity(10.0));
    assert!(f.price > 10.0, "체결가는 밀렸는데");
    assert!(f.equity_after < 1_000.0, "기준가로 잰 자산은 슬리피지만큼 줄어 있다");
}

// --- 손절선 ------------------------------------------------------------------
#[test]
fn 손절은_봉의_저가로_본다() {
    // 봉 안에서 스쳤다면 실제로는 체결됨 — 종가만 보면 "지났는데 안 팔린" 것이 됨.
    let mut l = Ledger::new(Side::Long, Costs::default(), 1_000.0);
    l.open_long(1_000.0, 100.0, 100.0, "진입").expect("체결");
    assert!(l.stop_hit(94.9, 101.0, 5.0), "저가가 -5% 아래로 내려갔다");
    assert!(!l.stop_hit(95.1, 101.0, 5.0));
    assert!(l.stop_hit(95.0, 101.0, 5.0), "정확히 닿아도 체결이다");
}

#[test]
fn 숏_손절은_고가로_본다() {
    let mut l = Ledger::new(Side::Short, Costs::default(), 1_000.0);
    l.open_short(1_000.0, 100.0, 100.0, "진입").expect("체결");
    assert!(l.stop_hit(99.0, 105.1, 5.0));
    assert!(!l.stop_hit(99.0, 104.9, 5.0));
}

#[test]
fn 손절선이_없거나_보유가_없으면_닿지_않는다() {
    let mut l = Ledger::new(Side::Long, Costs::default(), 1_000.0);
    assert!(!l.stop_hit(1.0, 1.0, 5.0), "보유가 없다");
    l.open_long(1_000.0, 100.0, 100.0, "진입").expect("체결");
    assert!(!l.stop_hit(1.0, 1.0, 0.0), "손절선을 안 걸었다");
}

// --- 숏 ------------------------------------------------------------------------
#[test]
fn 숏은_평가와_청산_식이_롱과_다르다() {
    let mut l = Ledger::new(Side::Short, Costs::default(), 1_000.0);
    l.open_short(1_000.0, 10.0, 10.0, "진입").expect("체결");
    // 내려가면 범.
    near(l.equity(9.0), 1_100.0);
    near(l.equity(11.0), 900.0);
    let g = l.close_all(9.0, 9.0, "청산").expect("체결");
    assert_eq!(g.side, "cover");
    near(l.closed_trades[0], 100.0);
    near(l.cash, 1_100.0);
}

#[test]
fn 현물은_펀딩이_붙지_않는다() {
    let mut l = azit();
    l.open_long(10_000.0, 2.0, 2.0, "진입").expect("체결");
    let before = l.cash;
    l.apply_funding(2.0);
    near(l.cash, before);
}
