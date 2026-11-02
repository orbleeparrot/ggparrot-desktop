// 묶음 판 — 한 레그 상세 맨 위에서 **묶음 전체**를 보여 줌 (2026-11-02).
//
// 묶음은 레그마다 줄 하나·세션 하나로 돎. 그래서 목록에서는 따로 보이는데, 그러면 "지금
// 묶음 전체가 얼마인지"를 볼 자리가 어디에도 없음. 레그를 하나씩 눌러 보며 머릿속으로
// 더해야 했음.
//
// 어느 레그에서 열어도 같은 판이 보임 — 묶음 전용 화면을 따로 두면 거기까지 가는 길을
// 새로 만들어야 하고, 사용자는 이미 레그 하나를 보고 있음.
import { useState } from "react";
import { Sparkline } from "../Sparkline";
import { bundleLegs, bundleTotals, changeSeries } from "../../state/bundlePanel";
import { coinTicker } from "../../state/runs";
import { call } from "../../ipc";
import type { RunSlot, StopMode } from "../../types";

interface Trail {
  points: number[];
  times?: number[];
}

interface Props {
  slot: RunSlot;
  runs: RunSlot[];
  prices: Record<string, Trail>;
  /** 표의 줄을 누르면 그 레그 상세로 — 탭은 차트만 바꿈. */
  onOpen(runId: string): void;
}

/** 손익 퍼센트 한 조각. 모르면 `—` — 0%로 적으면 '본전'으로 읽힘. */
function Pct({ value }: { value: number | null }) {
  if (value == null) return <span className="bundle-pct is-none">—</span>;
  const tone = value > 0 ? " is-up" : value < 0 ? " is-down" : "";
  return (
    <span className={`bundle-pct${tone}`}>
      {value > 0 ? "+" : ""}
      {value.toFixed(2)}%
    </span>
  );
}

/** 그 레그가 지금 들고 있는 손익률. 안 들고 있으면 null. */
function legPct(leg: RunSlot): number | null {
  return leg.run?.inPosition ? leg.run.unrealizedPct : null;
}

export function BundlePanel({ slot, runs, prices, onOpen }: Props) {
  const bundle = slot.bundle;
  const [picked, setPicked] = useState(slot.id);
  const [busy, setBusy] = useState(false);
  const [done, setDone] = useState("");
  if (!bundle) return null;

  const legs = bundleLegs(runs, bundle.key);
  const totals = bundleTotals(legs);
  const open = legs.find((l) => l.id === picked) ?? legs.find((l) => l.id === slot.id) ?? legs[0];
  const change = changeSeries(prices[open?.id ?? ""]?.points ?? []);
  const live = legs.some((l) => l.run && !l.run.finished);
  const holding = legs.some((l) => l.run?.inPosition);
  const mode: StopMode = holding ? "close_and_stop" : "stop_only";

  async function stopAll() {
    setBusy(true);
    try {
      const n = await call("stop_bundle", { bundleKey: bundle!.key, mode });
      setDone(`${n}개에 종료를 걸었어요.`);
    } catch (reason) {
      setDone(reason instanceof Error ? reason.message : String(reason));
    } finally {
      setBusy(false);
    }
  }

  return (
    <section className="bundle-panel" aria-label="여러 종목 묶음">
      <header className="bundle-panel-head">
        <h3 className="bundle-panel-title">여러 종목 묶음 · {bundle.size}종목</h3>
        {live && (
          <button type="button" className="bundle-bar-stop" disabled={busy} onClick={() => void stopAll()}>
            {busy ? "거는 중…" : holding ? "묶음 전체 청산하고 종료" : "묶음 전체 종료"}
          </button>
        )}
      </header>

      {/* 머리 숫자는 표의 '평단 대비'를 투입금으로 가중한 값임 — 둘이 같은 뜻이어야 함.
          실현손익은 분모가 달라 섞지 않고 옆에 따로 적음(bundlePanel.ts 머리글). */}
      <div className="bundle-total">
        <span className="bundle-total-label">전체 평단 대비</span>
        <span className="bundle-total-pct" data-testid="bundle-total-pct">
          <Pct value={totals.pct} />
        </span>
        <span className="bundle-total-note">
          투입금 {Math.round(totals.invested).toLocaleString("ko-KR")}
          {Math.round(totals.realized) !== 0 &&
            ` · 실현 ${totals.realized > 0 ? "+" : ""}${Math.round(totals.realized).toLocaleString("ko-KR")}`}
          {totals.feeAware ? "" : " · 수수료 전"}
        </span>
      </div>

      {/* 탭은 차트만 바꿈 — 레그를 옮겨 다니면 보던 자리를 잃음. */}
      <div className="bundle-tabs" role="tablist" aria-label="종목 고르기">
        {legs.map((leg) => (
          <button
            key={leg.id}
            type="button"
            role="tab"
            aria-selected={leg.id === open?.id}
            className={"bundle-tab" + (leg.id === open?.id ? " is-on" : "")}
            onClick={() => setPicked(leg.id)}
          >
            <b>{coinTicker(leg.macro)}</b>
            <Pct value={changePctOf(prices[leg.id]?.points ?? [])} />
          </button>
        ))}
      </div>

      <div className="bundle-chart" data-testid="bundle-chart" data-symbol={String(open?.macro.symbol ?? "")}>
        {change.length >= 2 ? (
          <Sparkline points={change} times={prices[open!.id]?.times} height={72} />
        ) : (
          <p className="bundle-chart-none">아직 시세를 모아는 중이에요.</p>
        )}
      </div>

      <table className="bundle-table">
        <thead>
          <tr>
            <th scope="col">종목</th>
            <th scope="col">상태</th>
            <th scope="col">평단 대비</th>
            <th scope="col">투입금</th>
          </tr>
        </thead>
        <tbody>
          {legs.map((leg) => (
            <tr
              key={leg.id}
              data-testid={`bundle-row-${leg.id}`}
              className={"bundle-row" + (leg.id === slot.id ? " is-current" : "")}
              onClick={() => onOpen(leg.id)}
            >
              <th scope="row">{coinTicker(leg.macro)}</th>
              <td>{leg.status.text}</td>
              <td>
                <Pct value={legPct(leg)} />
              </td>
              <td className="num">{Math.round(leg.run?.invested ?? 0).toLocaleString("ko-KR")}</td>
            </tr>
          ))}
        </tbody>
      </table>

      {done && (
        <p className="bundle-bar-done" role="status">
          {done}
        </p>
      )}
    </section>
  );
}

/** 받아 둔 시세의 첫 점 대비 지금 등락률. 모르면 null. */
function changePctOf(points: number[]): number | null {
  const series = changeSeries(points);
  return series.length ? series[series.length - 1] : null;
}
