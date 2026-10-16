// 바이낸스 키 카드(데모·실거래) — 키 복사(Copy)·기존 키, 실거래 키 카드의 Edit restrictions(허용 IP·주문 권한).
import {
  BN_BG,
  BN_CHECK_OFF,
  BN_GOLD,
  BN_INK,
  BN_KEYS,
  BN_LINE,
  BN_MUTE,
  BN_PANEL,
  BN_RED,
  BN_YELLOW,
  BnChrome,
  DARK_URL,
  fit,
  SMALL,
  tw,
  type Dark,
} from "./binanceKit";
import { CheckMark, InputBox, T } from "./kit";
import type { Market } from "../../types";
import { W, type Scene, type SceneState } from "./types";

// 실거래 키 카드의 API restrictions(3열). 선물 계정을 열기 전에 만든 키에는 Enable Futures가 없음
// — 현물 매크로 그림은 바이낸스 화면 그대로, 선물 매크로 그림은 Margin Loan 뒤에 Enable Futures가 있는 계정으로 그림.
const SPOT_TRADE = "Enable Spot & Margin & Stock Trading";
const FUTURES_TRADE = "Enable Futures";
const LIVE_SPOT_RESTRICTIONS = [
  "Enable Reading",
  SPOT_TRADE,
  "Enable Margin Loan, Repay & Transfer",
  "Permits Universal Transfer",
  "Enable Withdrawals",
  "Enable Alpha Withdrawals",
  "Enable Prediction Trading",
  "Enable Symbol Whitelist",
] as const;
const LIVE_FUTURES_RESTRICTIONS = [
  ...LIVE_SPOT_RESTRICTIONS.slice(0, 3),
  FUTURES_TRADE,
  ...LIVE_SPOT_RESTRICTIONS.slice(3),
];
const DEMO_RESTRICTIONS = ["Enable Reading", SPOT_TRADE, FUTURES_TRADE] as const;

/** 그 시장의 권한 칸 목록과 켤 주문 권한. */
function restrictionsOf(env: Dark, market: Market): { labels: readonly string[]; trade: string } {
  if (env === "demo") return { labels: DEMO_RESTRICTIONS, trade: market === "futures" ? FUTURES_TRADE : SPOT_TRADE };
  return market === "futures"
    ? { labels: LIVE_FUTURES_RESTRICTIONS, trade: FUTURES_TRADE }
    : { labels: LIVE_SPOT_RESTRICTIONS, trade: SPOT_TRADE };
}
const CX0 = 14;
const CX1 = W - 14;
const COL = (CX1 - CX0 - 28) / 3;

function editTarget(y: number): readonly [number, number] {
  const right = CX1 - 14 - tw("Delete", 12, true) - 16;
  return [right - tw("Edit restrictions", 12, true) / 2, y + 20];
}

function CardHead({ env, y }: { env: Dark; y: number }) {
  const bw = tw("HMAC", SMALL) + 10;
  const right = CX1 - 14 - tw("Delete", 12, true) - 16;
  return (
    <g>
      <rect x={CX0 + 14} y={y + 12} width={bw} height={16} rx={3} fill="#12352B" />
      <T x={CX0 + 19} y={y + 20} size={SMALL} fill="#2EBD85">
        HMAC
      </T>
      <T x={CX0 + 22 + bw} y={y + 20} bold fill={BN_INK}>
        gg_parrot
      </T>
      <T x={CX1 - 14} y={y + 20} bold fill={BN_GOLD} anchor="end">
        Delete
      </T>
      {env === "live" && (
        <T x={right} y={y + 20} bold fill={BN_GOLD} anchor="end">
          Edit restrictions
        </T>
      )}
    </g>
  );
}

/** 권한 체크 — 켜짐은 회색 칸에 체크, 꺼짐은 어두운 칸. */
function BnCheck({ x, y, label, on }: { x: number; y: number; label: string; on: boolean }) {
  return (
    <g>
      <rect x={x} y={y} width={13} height={13} rx={2} fill={on ? "#9AA0A8" : BN_CHECK_OFF} className="ex-fade" />
      {on && <CheckMark x={x} y={y} size={13} stroke={BN_PANEL} width={1.6} />}
      <T x={x + 19} y={y + 7} size={SMALL} fill={BN_INK}>
        {fit(label, COL - 26, SMALL)}
      </T>
    </g>
  );
}

function restrictionAt(i: number, y: number): readonly [number, number] {
  return [CX0 + 14 + (i % 3) * COL, y + 18 + Math.floor(i / 3) * 22];
}

/** API restrictions — 데모는 세 칸이 모두 켜진 채 만들어짐. 실거래 새 키는 Enable Reading만 켜져 있음. */
function Restrictions({ s, env, market, y }: { s: SceneState; env: Dark; market: Market; y: number }) {
  const { labels, trade } = restrictionsOf(env, market);
  return (
    <g>
      <T x={CX0 + 14} y={y + 4} size={SMALL} fill={BN_INK}>
        API restrictions
      </T>
      {labels.map((label, i) => {
        const [x, cy] = restrictionAt(i, y);
        const on = env === "demo" || i === 0 || (label === trade && s.on.has("trade"));
        return <BnCheck key={label} x={x} y={cy} label={label} on={on} />;
      })}
    </g>
  );
}

function restrictionsBottom(env: Dark, market: Market, y: number): number {
  return y + 18 + Math.ceil(restrictionsOf(env, market).labels.length / 3) * 22;
}

function Radio({
  x,
  y,
  label,
  selected,
  enabled,
}: {
  x: number;
  y: number;
  label: string;
  selected: boolean;
  enabled: boolean;
}) {
  const ring = enabled ? "#FFFFFF" : "#474D57";
  return (
    <g>
      <circle cx={x + 7} cy={y} r={7} fill={BN_BG} stroke={ring} strokeWidth={1.5} />
      {selected && <circle cx={x + 7} cy={y} r={3} fill={ring} />}
      <T x={x + 22} y={y} size={SMALL} fill={enabled ? BN_INK : "#5E6673"}>
        {label}
      </T>
    </g>
  );
}

const UNRESTRICTED = "Unrestricted (Less Secure)";
const IP_TITLE_GAP = 24;

/** IP access restrictions — 두 칸과 '주문 권한을 켜면 키가 지워진다'는 빨간 경고. 끝 y를 돌려줌. */
function IpAccess({ y, enabled, restricted }: { y: number; enabled: boolean; restricted: boolean }) {
  const warnX = CX0 + 14 + 22 + tw(UNRESTRICTED, SMALL) + 6;
  const r1 = y + IP_TITLE_GAP;
  return (
    <g>
      <T x={CX0 + 14} y={y + 4} bold fill={BN_INK}>
        IP access restrictions
      </T>
      <Radio x={CX0 + 14} y={r1} label={UNRESTRICTED} selected={!restricted} enabled={enabled} />
      <T x={warnX} y={r1} size={SMALL} fill={BN_RED}>
        {fit("This API Key allows access from any IP address. This is not recommended.", CX1 - 14 - warnX, SMALL)}
      </T>
      <T x={CX0 + 36} y={r1 + 16} size={SMALL} fill={BN_RED}>
        {fit(
          "To protect the safety of your funds, if the IP is unrestricted and any permission other than Reading is enabled, this API key will be deleted.",
          CX1 - CX0 - 50,
          SMALL,
        )}
      </T>
      <Radio
        x={CX0 + 14}
        y={r1 + 40}
        label="Restrict access to trusted IPs only (Recommended)"
        selected={restricted}
        enabled={enabled}
      />
    </g>
  );
}

function ipAccessBottom(y: number): number {
  return y + IP_TITLE_GAP + 40 + 14;
}

const CARD_Y = 44;

/** 키 카드 한 장의 줄 위치 — 알림 띠·키 두 줄·(전체면) 권한·IP 칸. */
function cardLayout(env: Dark, market: Market, full: boolean) {
  let cy = CARD_Y + 38;
  const notice = full ? cy : null;
  if (full) cy += 38;
  const rows = [cy, cy + 40];
  cy += 80;
  const restrictions = full ? cy : null;
  if (full) cy = restrictionsBottom(env, market, cy) + 6;
  const ip = full && env === "live" ? cy : null;
  if (ip !== null) cy = ipAccessBottom(ip) + 4;
  return { notice, rows, restrictions, ip, bottom: cy + 8 };
}

function copyTargetAt(value: string, y: number): readonly [number, number] {
  const cx = CX0 + 22 + tw(value, 12, true);
  return [cx + tw("Copy", 12, true) / 2, y + 24];
}

function KeyCard({
  s,
  env,
  market,
  full,
  copy,
}: {
  s: SceneState;
  env: Dark;
  market: Market;
  full: boolean;
  copy: boolean;
}) {
  const l = cardLayout(env, market, full);
  const values = [BN_KEYS[0], full ? BN_KEYS[1] : "•".repeat(30)];
  return (
    <g>
      <rect x={CX0} y={CARD_Y} width={CX1 - CX0} height={l.bottom - CARD_Y} rx={8} fill={BN_BG} stroke={BN_LINE} />
      <CardHead env={env} y={CARD_Y} />
      {l.notice !== null && (
        <g>
          <rect x={CX0 + 14} y={l.notice} width={CX1 - CX0 - 28} height={26} rx={6} fill="#302D22" />
          <T x={CX0 + 24} y={l.notice + 13} size={SMALL} fill={BN_INK}>
            {fit(
              "ⓘ  Make sure to copy your API Key and Secret Key now. They will not be shown in full again after you leave this page.",
              CX1 - CX0 - 48,
              SMALL,
            )}
          </T>
        </g>
      )}
      {(["API Key", "Secret Key"] as const).map((label, i) => {
        const y = l.rows[i];
        const name = `c${i + 1}`;
        return (
          <g key={label}>
            <T x={CX0 + 14} y={y + 6} size={SMALL} fill={BN_INK}>
              {label}
            </T>
            <T x={CX0 + 14} y={y + 24} bold fill={BN_INK}>
              {values[i]}
            </T>
            {copy && (
              <T x={CX0 + 22 + tw(values[i], 12, true)} y={y + 24} bold fill={s.on.has(name) ? "#FFFFFF" : BN_GOLD}>
                Copy
              </T>
            )}
          </g>
        );
      })}
      {l.restrictions !== null && <Restrictions s={s} env={env} market={market} y={l.restrictions} />}
      {l.ip !== null && <IpAccess y={l.ip} enabled={false} restricted={false} />}
    </g>
  );
}

function darkKeys(env: Dark, market: Market = "spot"): Scene {
  const l = cardLayout(env, market, true);
  const height = l.bottom + 12;
  return {
    url: DARK_URL[env],
    width: W,
    height,
    targets: { c1: copyTargetAt(BN_KEYS[0], l.rows[0]), c2: copyTargetAt(BN_KEYS[1], l.rows[1]) },
    steps: [
      { t: "move", to: "c1" },
      { t: "click" },
      { t: "on", name: "c1" },
      { t: "wait", ms: 900 },
      { t: "move", to: "c2" },
      { t: "click" },
      { t: "on", name: "c2" },
      { t: "wait", ms: 1800 },
    ],
    draw: (s) => (
      <g>
        <BnChrome env={env} height={height} />
        <KeyCard s={s} env={env} market={market} full copy />
      </g>
    ),
  };
}

function darkExisting(env: Dark): Scene {
  const height = cardLayout(env, "spot", false).bottom + 12;
  return {
    url: DARK_URL[env],
    width: W,
    height,
    targets: {},
    steps: [],
    draw: (s) => (
      <g>
        <BnChrome env={env} height={height} />
        <KeyCard s={s} env={env} market="spot" full={false} copy={false} />
      </g>
    ),
  };
}

// --- 실거래 허용 IP·주문 권한(Edit restrictions) -----------------------------------------

const R_REST = CARD_Y + 40;
const CANCEL_W = tw("Cancel", SMALL) + 20;
const SAVE_W = tw("Save", SMALL) + 24;
const SAVE_X1 = CX1 - 14 - CANCEL_W - 8;
const IPBOX_HINT = "When entering more than one IP address, please separate them with spaces.";

/** 키 카드의 Edit restrictions → 'Restrict access to trusted IPs only' → 이 기기 주소 입력·Confirm → 주문 권한 → Save.
 *  카드 머리·API restrictions·IP access restrictions만 그림(키 줄은 앞 단계에서 이미 복사함).
 *  주문 권한은 시장마다 다름 — 현물 Enable Spot & Margin & Stock Trading, 선물 Enable Futures. */
function liveRestrict(market: Market): Scene {
  const { labels, trade } = restrictionsOf("live", market);
  const ipY = restrictionsBottom("live", market, R_REST) + 6;
  // 'Restrict access…'를 고르면 그 아래 나오는 IP 입력 칸 — 노란 테두리·안내 글·오른쪽 Confirm(바이낸스 화면 2026-10-15).
  const box = { x0: CX0 + 30, y0: ipAccessBottom(ipY) + 8, x1: 0, y1: 0 };
  box.x1 = Math.min(CX1 - 14, box.x0 + 440);
  box.y1 = box.y0 + 30;
  const confirmX = box.x1 - 12;
  const bottom = box.y1 + 14;
  const height = bottom + 12;
  return {
    url: DARK_URL.live,
    width: W,
    height,
    targets: {
      edit: editTarget(CARD_Y),
      restrict: [CX0 + 21, ipY + IP_TITLE_GAP + 40],
      ipin: [box.x0 + Math.min(200, (box.x1 - box.x0) * 0.45), (box.y0 + box.y1) / 2],
      confirm: [confirmX - tw("Confirm", 12, true) / 2, (box.y0 + box.y1) / 2],
      trade: (() => {
        const [x, y] = restrictionAt(labels.indexOf(trade), R_REST);
        return [x + 6.5, y + 6.5] as const;
      })(),
      save: [SAVE_X1 - SAVE_W / 2, CARD_Y + 19],
    },
    steps: [
      { t: "move", to: "edit" },
      { t: "click" },
      { t: "show", layer: "edit" },
      { t: "wait", ms: 500 },
      { t: "move", to: "restrict" },
      { t: "click" },
      { t: "on", name: "restrict" },
      { t: "show", layer: "ipbox" },
      { t: "wait", ms: 400 },
      { t: "move", to: "ipin" },
      { t: "click" },
      { t: "focus", input: "ipin" },
      { t: "type", input: "ipin" },
      { t: "move", to: "confirm" },
      { t: "click" },
      { t: "wait", ms: 500 },
      { t: "move", to: "trade" },
      { t: "click" },
      { t: "on", name: "trade" },
      { t: "move", to: "save" },
      { t: "click" },
      { t: "wait", ms: 1600 },
    ],
    draw: (s) => {
      const editing = s.layers.has("edit");
      return (
        <g>
          <BnChrome env="live" height={height} />
          <rect x={CX0} y={CARD_Y} width={CX1 - CX0} height={bottom - CARD_Y} rx={8} fill={BN_BG} stroke={BN_LINE} />
          <CardHead env="live" y={CARD_Y} />
          <Restrictions s={s} env="live" market={market} y={R_REST} />
          <IpAccess y={ipY} enabled={editing} restricted={s.on.has("restrict")} />
          {s.layers.has("ipbox") && (
            <g className="ex-layer">
              <InputBox
                s={s}
                name="ipin"
                x0={box.x0}
                y0={box.y0}
                x1={box.x1}
                y1={box.y1}
                placeholder={fit(IPBOX_HINT, box.x1 - box.x0 - 90, SMALL)}
                line={BN_GOLD}
                focus={BN_GOLD}
                radius={6}
                fill={BN_BG}
                ink={BN_INK}
                hint={BN_MUTE}
                hintSize={SMALL}
              />
              <T x={confirmX} y={(box.y0 + box.y1) / 2} bold fill="#5E6673" anchor="end">
                Confirm
              </T>
            </g>
          )}
          {/* Edit restrictions를 누르면 머리 오른쪽이 Save·Cancel로 바뀌고 IP 칸을 고를 수 있게 됨. */}
          {editing && (
            <g className="ex-layer">
              <rect x={CX1 - 200} y={CARD_Y + 6} width={198} height={28} fill={BN_BG} />
              <rect x={CX1 - 14 - CANCEL_W} y={CARD_Y + 7} width={CANCEL_W} height={24} rx={4} fill={BN_CHECK_OFF} />
              <T x={CX1 - 14 - CANCEL_W / 2} y={CARD_Y + 19} size={SMALL} fill={BN_INK} anchor="middle">
                Cancel
              </T>
              <rect x={SAVE_X1 - SAVE_W} y={CARD_Y + 7} width={SAVE_W} height={24} rx={4} fill={BN_YELLOW} />
              <T x={SAVE_X1 - SAVE_W / 2} y={CARD_Y + 19} size={SMALL} anchor="middle">
                Save
              </T>
            </g>
          )}
        </g>
      );
    },
  };
}

export const binanceLiveRestrict = liveRestrict("spot");
export const binanceLiveRestrictFutures = liveRestrict("futures");
export const binanceDemoKeys = darkKeys("demo");
export const binanceDemoExisting = darkExisting("demo");
export const binanceLiveKeys = darkKeys("live");
export const binanceLiveKeysFutures = darkKeys("live", "futures");
export const binanceLiveExisting = darkExisting("live");
