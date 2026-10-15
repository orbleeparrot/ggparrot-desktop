// 바이낸스 선물 데모(demo.binance.com)·실거래(binance.com) API Management — 어두운 화면·노란 버튼.
// 키 만들기(Create API → System generated → 이름 → Next). 키 카드·허용 IP 그림은 binanceCard.tsx.
import {
  BN_DIM,
  BN_GOLD,
  BN_GRAY,
  BN_INK,
  BN_LINE,
  BN_MUTE,
  BN_NOTE,
  BN_PANEL,
  BN_YELLOW,
  BN_BG,
  BnChrome,
  DARK_URL,
  fit,
  SMALL,
  tw,
  type Dark,
} from "./binanceKit";
import { CheckMark, INK, T } from "./kit";
import { W, type Scene, type SceneState } from "./types";

/** 오른쪽 위 버튼들 — [이름, 왼쪽 x, 폭]. 오른쪽 끝부터 채움. */
function pageButtons(env: Dark): [string, number, number][] {
  const labels =
    env === "demo" ? ["Create API", "Delete all API"] : ["Create API", "Create Tax Report API", "Delete all API"];
  const out: [string, number, number][] = [];
  let x1 = W - 14;
  for (const label of [...labels].reverse()) {
    const bw = tw(label, SMALL) + 22;
    out.unshift([label, x1 - bw, bw]);
    x1 -= bw + 8;
  }
  return out;
}

/** API Management 첫 화면 — 제목·오른쪽 위 버튼·안내 줄·노란 갈색 상자. */
function BnPage({ s, env }: { s: SceneState; env: Dark }) {
  return (
    <g>
      <T x={14} y={60} bold size={16} fill={BN_INK}>
        API Management
      </T>
      {pageButtons(env).map(([label, x, bw]) => {
        const primary = label === "Create API";
        const lit = primary && s.on.has("create");
        return (
          <g key={label}>
            <rect
              x={x}
              y={48}
              width={bw}
              height={26}
              rx={6}
              fill={primary ? (lit ? "#E5BF22" : BN_YELLOW) : BN_GRAY}
              className="ex-fade"
            />
            <T x={x + bw / 2} y={61} size={SMALL} anchor="middle" fill={primary ? INK : BN_INK}>
              {label}
            </T>
          </g>
        );
      })}
      <T x={14} y={90} size={SMALL} fill={BN_MUTE}>
        1. Each account can create up to 30 API Keys.
      </T>
      <T x={14} y={106} size={SMALL} fill={BN_MUTE}>
        {fit("2. Do not disclose your API Key, Secret Key (HMAC) or Private Key (Ed25519, RSA) to anyone.", 492, SMALL)}
      </T>
      <rect x={14} y={124} width={492} height={40} rx={6} fill={BN_NOTE} />
      {env === "demo" ? (
        <g>
          <T x={26} y={137} size={SMALL} fill={BN_GOLD}>
            {fit("Spot Demo API Documentation: https://developers.binance.com/docs/binance-spot-api-docs", 468, SMALL)}
          </T>
          <T x={26} y={152} size={SMALL} fill={BN_GOLD}>
            {fit("Futures API Documentation: https://developers.binance.com/docs/derivatives", 468, SMALL)}
          </T>
        </g>
      ) : (
        <g>
          <rect x={24} y={137} width={13} height={13} rx={2} fill={BN_INK} />
          <CheckMark x={24} y={137} size={13} stroke={BN_BG} width={1.6} />
          <T x={44} y={143} size={SMALL} fill={BN_INK}>
            {fit(
              "By checking this box, all existing API Key(s) on your master account and sub-accounts will be subject to Default Security Controls.",
              446,
              SMALL,
            )}
          </T>
        </g>
      )}
    </g>
  );
}

function createTarget(env: Dark): readonly [number, number] {
  const [, x, bw] = pageButtons(env)[0];
  return [x + bw / 2, 61];
}

function KeyIcon({ x, y }: { x: number; y: number }) {
  return (
    <g>
      <circle cx={x + 9} cy={y + 9} r={9} fill={BN_GOLD} />
      <circle cx={x + 9} cy={y + 9} r={3} fill={BN_PANEL} />
      <line x1={x + 15} y1={y + 15} x2={x + 28} y2={y + 28} stroke={BN_GOLD} strokeWidth={4} />
    </g>
  );
}

function YellowButton(p: {
  s: SceneState;
  name?: string;
  x0: number;
  y0: number;
  x1: number;
  y1: number;
  label: string;
  fill?: string;
  ink?: string;
}) {
  const fill = p.fill ?? BN_YELLOW;
  const lit = !!p.name && p.s.on.has(p.name);
  return (
    <g>
      <rect
        x={p.x0}
        y={p.y0}
        width={p.x1 - p.x0}
        height={p.y1 - p.y0}
        rx={6}
        fill={lit ? (fill === BN_YELLOW ? "#E5BF22" : "#474D57") : fill}
        className="ex-fade"
      />
      <T x={(p.x0 + p.x1) / 2} y={(p.y0 + p.y1) / 2} bold anchor="middle" fill={p.ink ?? INK}>
        {p.label}
      </T>
    </g>
  );
}

const PERM_H = 346;
const MX0 = W * 0.12;
const MX1 = W * 0.88;
const KEY_TYPES = [
  [
    "System generated",
    ["Works using HMAC symmetric encryption. An API Key", "and Secret Key will be provided to you."],
    true,
  ],
  ["Self-generated", ["Works using Ed25519 or RSA asymmetric encryption.", "You create your own key pair."], false],
] as const;
const LX0 = W * 0.2;
const LX1 = W * 0.8;

function darkPerm(env: Dark): Scene {
  return {
    url: DARK_URL[env],
    width: W,
    height: PERM_H,
    targets: {
      create: createTarget(env),
      next1: [W / 2, 297],
      label: [LX1 - 60, 152],
      next2: [((LX0 + LX1) / 2 + 6 + LX1 - 18) / 2, 195],
    },
    steps: [
      { t: "move", to: "create" },
      { t: "click" },
      { t: "on", name: "create" },
      { t: "show", layer: "type" },
      { t: "wait", ms: 700 },
      { t: "move", to: "next1" },
      { t: "click" },
      { t: "hide", layer: "type" },
      { t: "show", layer: "label" },
      { t: "move", to: "label" },
      { t: "click" },
      { t: "focus", input: "label" },
      { t: "type", input: "label", text: "gg_parrot" },
      { t: "move", to: "next2" },
      { t: "click" },
      { t: "on", name: "next2" },
      { t: "wait", ms: 1400 },
    ],
    draw: (s) => {
      const mid = (LX0 + LX1) / 2;
      const typed = s.typed.label ?? "";
      const focused = s.focus.has("label");
      return (
        <g>
          <BnChrome env={env} height={PERM_H} />
          <BnPage s={s} env={env} />
          {/* Choose API Key type — System generated가 골라져 있음(노란 테두리·오른쪽 위 표시). */}
          {s.layers.has("type") && (
            <g className="ex-layer">
              <rect x={0} y={33} width={W} height={PERM_H - 33} fill={BN_DIM} />
              <rect x={MX0} y={46} width={MX1 - MX0} height={286} rx={12} fill={BN_PANEL} />
              <T x={MX0 + 18} y={68} bold fill={BN_INK}>
                Choose API Key type
              </T>
              <T x={MX1 - 20} y={68} fill={BN_INK} anchor="middle">
                ✕
              </T>
              {KEY_TYPES.map(([title, lines, chosen], i) => {
                const oy = 90 + i * 92;
                return (
                  <g key={title}>
                    <rect
                      x={MX0 + 18}
                      y={oy}
                      width={MX1 - MX0 - 36}
                      height={84}
                      fill={chosen ? BN_PANEL : BN_GRAY}
                      stroke={chosen ? BN_GOLD : "none"}
                    />
                    {chosen && (
                      <polygon points={`${MX1 - 40},${oy} ${MX1 - 18},${oy} ${MX1 - 18},${oy + 22}`} fill={BN_GOLD} />
                    )}
                    <KeyIcon x={MX0 + 32} y={oy + 22} />
                    <T x={MX0 + 78} y={oy + 18} bold fill={BN_INK}>
                      {title}
                    </T>
                    {lines.map((line, j) => (
                      <T key={line} x={MX0 + 78} y={oy + 40 + j * 14} size={SMALL} fill={BN_INK}>
                        {line}
                      </T>
                    ))}
                  </g>
                );
              })}
              <YellowButton s={s} name="next1" x0={MX0 + 18} y0={280} x1={MX1 - 18} y1={314} label="Next" />
            </g>
          )}
          {/* Create API — 'Label API Key to proceed'에 이름을 넣고 Next. */}
          {s.layers.has("label") && (
            <g className="ex-layer">
              <rect x={0} y={33} width={W} height={PERM_H - 33} fill={BN_DIM} />
              <rect x={LX0} y={70} width={LX1 - LX0} height={156} rx={12} fill={BN_PANEL} />
              <T x={LX0 + 18} y={94} bold fill={BN_INK}>
                Create API
              </T>
              <T x={LX1 - 20} y={94} fill={BN_INK} anchor="middle">
                ✕
              </T>
              <T x={LX0 + 18} y={126} size={SMALL} fill={BN_INK}>
                Label API Key to proceed
              </T>
              <rect
                x={LX0 + 18}
                y={138}
                width={LX1 - LX0 - 36}
                height={28}
                rx={6}
                fill={BN_PANEL}
                stroke={focused ? BN_YELLOW : BN_LINE}
                strokeWidth={focused ? 1.5 : 1}
              />
              {typed && (
                <T x={LX0 + 28} y={152} bold fill={BN_INK}>
                  {typed}
                  {focused && <tspan className="ex-caret">|</tspan>}
                </T>
              )}
              <YellowButton
                s={s}
                x0={LX0 + 18}
                y0={180}
                x1={mid - 6}
                y1={210}
                label="Cancel"
                fill="#363C45"
                ink={BN_INK}
              />
              <YellowButton s={s} name="next2" x0={mid + 6} y0={180} x1={LX1 - 18} y1={210} label="Next" />
            </g>
          )}
        </g>
      );
    },
  };
}

export const binanceDemoPerm = darkPerm("demo");
export const binanceLivePerm = darkPerm("live");
