// 바이낸스 현물 테스트넷(testnet.binance.vision) — 검은 머리줄·연회색 바탕·파란 글자 링크·노란 버튼.
// API Keys 첫 화면에서 키를 만들고, [복사]가 없어 값을 골라 복사함.
import {
  BN_GOLD,
  BnMark,
  fit,
  MARK_END,
  SMALL,
  TN_BAR,
  TN_BG,
  TN_CHECK,
  TN_HEAD,
  TN_KEYS,
  TN_LINE,
  TN_LINK,
  TN_SELECT,
  tw,
} from "./binanceKit";
import { CheckMark, INK, InputBox, T } from "./kit";
import { W, type Scene, type SceneState } from "./types";

function TnChrome({ height }: { height: number }) {
  const bw = tw("Log out", 12, true) + 14;
  return (
    <g>
      <rect x={0} y={32} width={W} height={height - 32} fill={TN_BG} />
      <rect x={0} y={0} width={W} height={32} fill={TN_BAR} />
      <BnMark x={12} cy={16} />
      <T x={MARK_END + 14} y={16} fill="#FFFFFF">
        Spot Test Network
      </T>
      <rect x={W - 12 - bw} y={7} width={bw} height={18} rx={3} fill={BN_GOLD} />
      <T x={W - 12 - bw / 2} y={16} bold anchor="middle">
        Log out
      </T>
    </g>
  );
}

function TnButton({ s, name, x, y, label }: { s: SceneState; name?: string; x: number; y: number; label: string }) {
  const bw = tw(label, 12, true) + 14;
  const lit = !!name && s.on.has(name);
  return (
    <g>
      <rect x={x} y={y} width={bw} height={22} rx={3} fill={lit ? "#D9A40A" : BN_GOLD} className="ex-fade" />
      <T x={x + bw / 2} y={y + 11} bold anchor="middle">
        {label}
      </T>
    </g>
  );
}

const TN_COLS = [14, 14 + 492 * 0.2, 14 + 492 * 0.36, 14 + 492 * 0.74, 506] as const;

/** API Keys 목록(키 하나). 표 아래 끝 y를 돌려줌. */
function TnTable({ s, y }: { s: SceneState; y: number }) {
  const c = TN_COLS;
  const editW = tw("Edit", 12, true) + 14;
  return (
    <g>
      <rect x={c[0]} y={y} width={c[4] - c[0]} height={24} fill={TN_HEAD} stroke={TN_LINE} />
      {["Key Type", "Description", "Permissions"].map((label, i) => (
        <T key={label} x={c[i] + 8} y={y + 12} bold fill="#7A7F87">
          {label}
        </T>
      ))}
      <rect x={c[0]} y={y + 24} width={c[4] - c[0]} height={30} fill="#FFFFFF" stroke={TN_LINE} />
      {[c[1], c[2], c[3]].map((x) => (
        <line key={x} x1={x} y1={y + 24} x2={x} y2={y + 54} stroke={TN_LINE} />
      ))}
      <T x={c[0] + 8} y={y + 39}>
        HMAC-SHA-256
      </T>
      <T x={c[1] + 8} y={y + 39}>
        TEST
      </T>
      <T x={c[2] + 8} y={y + 39} size={SMALL}>
        {fit("TRADE, USER_DATA, USER_STREAM", c[3] - c[2] - 14, SMALL)}
      </T>
      <TnButton s={s} x={c[3] + 8} y={y + 28} label="Edit" />
      <TnButton s={s} x={c[3] + 12 + editW} y={y + 28} label="Revoke" />
    </g>
  );
}

function TnCheckbox({ x, y, label, disabled }: { x: number; y: number; label: string; disabled?: boolean }) {
  return (
    <g>
      {disabled ? (
        <rect x={x} y={y} width={13} height={13} rx={2} fill="#F1F1F1" stroke="#C9C9C9" />
      ) : (
        <g>
          <rect x={x} y={y} width={13} height={13} rx={2} fill={TN_CHECK} />
          <CheckMark x={x} y={y} size={13} stroke="#FFFFFF" width={1.6} />
        </g>
      )}
      <T x={x + 21} y={y + 6.5} fill={disabled ? "#9AA0A6" : INK}>
        {label}
      </T>
    </g>
  );
}

const TN_URL = "testnet.binance.vision";
const TN_GEN = "Generate HMAC-SHA-256 Key";
const TN_PERM_H = 338;

export const binanceTestnetPerm: Scene = {
  url: TN_URL,
  width: W,
  height: TN_PERM_H,
  targets: {
    gen: [14 + tw(TN_GEN) * 0.5, 154],
    desc: [180, 107],
    go: [14 + (tw("Generate", 12, true) + 14) / 2, 295],
  },
  steps: [
    { t: "wait", ms: 500 },
    { t: "move", to: "gen" },
    { t: "click" },
    { t: "show", layer: "form" },
    { t: "move", to: "desc" },
    { t: "click" },
    { t: "focus", input: "desc" },
    { t: "type", input: "desc", text: "gg_parrot" },
    { t: "move", to: "go" },
    { t: "click" },
    { t: "on", name: "go" },
    { t: "wait", ms: 1400 },
  ],
  draw: (s) => (
    <g>
      <TnChrome height={TN_PERM_H} />
      {/* 첫 화면: API Keys 목록 아래 'Generate HMAC-SHA-256 Key' 링크. 누르면 위에 양식이 덮임. */}
      <T x={14} y={56} bold size={16}>
        API Keys
      </T>
      <TnTable s={s} y={76} />
      <T x={14} y={154} fill={TN_LINK}>
        {TN_GEN}
      </T>
      <T x={14} y={180} fill={TN_LINK}>
        Register Public Key
      </T>
      {s.layers.has("form") && (
        <g className="ex-layer">
          <rect x={0} y={33} width={W} height={TN_PERM_H - 33} fill={TN_BG} />
          <T x={14} y={56} bold size={16}>
            Generate HMAC-SHA-256 key
          </T>
          <T x={14} y={82} bold>
            Description:
          </T>
          <InputBox
            s={s}
            name="desc"
            x0={14}
            y0={94}
            x1={214}
            y1={120}
            placeholder=""
            line="#8F8F8F"
            focus={TN_CHECK}
          />
          <T x={14} y={132} size={SMALL} fill="#444444">
            {fit(
              "Needs to be a unique name containing between 1 and 20 letters, numbers, dashes or underscores.",
              492,
              SMALL,
            )}
          </T>
          <T x={14} y={154} bold>
            Key permissions:
          </T>
          {(
            [
              ["TRADE", false],
              ["USER_DATA", false],
              ["USER_STREAM", false],
              ["FIX_API", true],
              ["FIX_API_READ_ONLY", true],
            ] as const
          ).map(([label, disabled], i) => (
            <TnCheckbox key={label} x={14} y={168 + i * 22} label={label} disabled={disabled} />
          ))}
          <TnButton s={s} name="go" x={14} y={284} label="Generate" />
          <T x={14} y={324} fill={TN_LINK}>
            Back
          </T>
        </g>
      )}
    </g>
  ),
};

/** 키를 고르는 줄 — 이 화면에는 [복사]가 없어 값을 골라 복사함. 커서 옆에 누르는 키를 잠깐 띄움. */
function TnKeyRow({ s, i, y, label, value }: { s: SceneState; i: number; y: number; label: string; value: string }) {
  const lx = 14 + tw(label) + 8;
  const vw = tw(value);
  const kx = lx + vw * 0.55 + 16;
  const copy = copyKeys();
  return (
    <g>
      {s.on.has(`s${i}`) && <rect x={lx - 2} y={y - 9} width={vw + 4} height={18} fill={TN_SELECT} />}
      <T x={14} y={y}>
        {label}
      </T>
      <T x={lx} y={y}>
        {value}
      </T>
      {s.layers.has(`k${i}`) && (
        <g className="ex-layer">
          <rect x={kx} y={y + 6} width={tw(copy, 12, true) + 14} height={22} rx={4} fill={INK} />
          <T x={kx + 7} y={y + 17} bold fill="#FFFFFF">
            {copy}
          </T>
        </g>
      )}
    </g>
  );
}

/** 복사 단축키 — macOS는 ⌘C, 그 밖에는 Ctrl+C. */
function copyKeys(): string {
  return typeof navigator !== "undefined" && /Mac/i.test(`${navigator.platform} ${navigator.userAgent}`)
    ? "⌘C"
    : "Ctrl+C";
}

const TN_KEY_ROWS = [
  ["API Key:", TN_KEYS[0], 114],
  ["Secret Key:", TN_KEYS[1], 150],
] as const;

export const binanceTestnetKeys: Scene = {
  url: TN_URL,
  width: W,
  height: 208,
  targets: Object.fromEntries(
    TN_KEY_ROWS.map(([label, value, y], i) => [`s${i + 1}`, [14 + tw(label) + 8 + tw(value) * 0.55, y] as const]),
  ),
  steps: [
    { t: "move", to: "s1" },
    { t: "click" },
    { t: "click" },
    { t: "on", name: "s1" },
    { t: "show", layer: "k1" },
    { t: "wait", ms: 900 },
    { t: "hide", layer: "k1" },
    { t: "move", to: "s2" },
    { t: "click" },
    { t: "click" },
    { t: "on", name: "s2" },
    { t: "show", layer: "k2" },
    { t: "wait", ms: 900 },
    { t: "hide", layer: "k2" },
    { t: "wait", ms: 1400 },
  ],
  draw: (s) => {
    const lead = "Save these values right now.";
    return (
      <g>
        <TnChrome height={208} />
        <T x={14} y={56} bold size={16}>
          HMAC-SHA-256 Key registered
        </T>
        <T x={14} y={84} bold>
          {lead}
        </T>
        <T x={14 + tw(lead, 12, true) + 4} y={84}>
          They won&apos;t be shown ever again!
        </T>
        {TN_KEY_ROWS.map(([label, value, y], i) => (
          <TnKeyRow key={label} s={s} i={i + 1} y={y} label={label} value={value} />
        ))}
        <T x={14} y={192} fill={TN_LINK}>
          Back
        </T>
      </g>
    );
  },
};

export const binanceTestnetExisting: Scene = {
  url: TN_URL,
  width: W,
  height: 172,
  targets: {},
  steps: [],
  draw: (s) => (
    <g>
      <TnChrome height={172} />
      <T x={14} y={56} bold size={16}>
        API Keys
      </T>
      <TnTable s={s} y={76} />
      <T x={14} y={154} fill={TN_LINK}>
        {TN_GEN}
      </T>
    </g>
  ),
};
