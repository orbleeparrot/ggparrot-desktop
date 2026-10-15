// 빗썸 API 2.0 화면(단순화) — 흰 카드·주황 원형 체크·검정 버튼.
import {
  BT_DARK,
  BT_LINE,
  BtChrome,
  BtCircle,
  copyTarget,
  CopyButton,
  DIM,
  FlatButton,
  InputBox,
  ModalBox,
  MUTED,
  T,
} from "./kit";
import { inputTarget, W, type Scene, type SceneState } from "./types";

const URL = "bithumb.com/react/api-support/management-api";

function IpRow({ s, y, x0, x1 }: { s: SceneState; y: number; x0: number; x1: number }) {
  return (
    <g>
      <InputBox
        s={s}
        name="in"
        x0={x0}
        y0={y}
        x1={x1 - 42}
        y1={y + 36}
        radius={3}
        placeholder="IP 주소를 입력해 주세요. 예) 123.123.123.123"
        line={BT_LINE}
        focus={BT_DARK}
      />
      <rect x={x1 - 36} y={y} width={36} height={36} rx={3} fill="#FFFFFF" stroke={BT_LINE} />
      <T x={x1 - 18} y={y + 18} anchor="middle" fill="#333333" size={15}>
        +
      </T>
    </g>
  );
}

export const bithumbIp: Scene = {
  url: URL,
  width: W,
  height: 146,
  targets: { in: inputTarget(14, 98, 464, 134) },
  steps: [
    { t: "move", to: "in" },
    { t: "click" },
    { t: "focus", input: "in" },
    { t: "type", input: "in" },
    { t: "wait", ms: 1600 },
  ],
  draw: (s) => (
    <g>
      <BtChrome />
      <T x={14} y={80} bold>
        IP 주소 등록 (필수)
      </T>
      <T x={136} y={80} fill="#8B8F98">
        (최대 5개 입력 가능)
      </T>
      <IpRow s={s} y={98} x0={14} x1={506} />
    </g>
  ),
};

const PERMS: Array<[string, string]> = [
  ["자산조회", "t1"],
  ["주문조회", "t2"],
  ["주문하기", "t3"],
  ["출금조회", ""],
  ["출금하기", ""],
  ["입금조회", ""],
  ["입금하기", ""],
];
const COL = (W - 28) / 4;
const AGREE: Array<[string, boolean, string]> = [
  ["모두 동의", true, "#111111"],
  ["개인정보 수집 및 이용 동의 (필수)", false, "#6B6F78"],
  ["API 이용 주의사항 확인 및 동의 (필수)", false, "#6B6F78"],
];

export const bithumbPerm: Scene = {
  url: URL,
  width: W,
  height: 326,
  targets: { t1: [24, 110], t2: [24 + COL, 110], t3: [24 + 2 * COL, 110], t4: [36, 184], t6: [260, 282] },
  steps: [
    { t: "move", to: "t1" },
    { t: "click" },
    { t: "on", name: "t1" },
    { t: "move", to: "t2" },
    { t: "click" },
    { t: "on", name: "t2" },
    { t: "move", to: "t3" },
    { t: "click" },
    { t: "on", name: "t3" },
    { t: "move", to: "t4" },
    { t: "click" },
    { t: "on", name: "t4" },
    { t: "on", name: "t6" },
    { t: "move", to: "t6" },
    { t: "click" },
    { t: "wait", ms: 1400 },
  ],
  draw: (s) => (
    <g>
      <BtChrome />
      <T x={14} y={80} bold>
        API 활성 항목 선택 (필수)
      </T>
      {PERMS.map(([label, name], i) => (
        <BtCircle
          key={label}
          s={s}
          name={name || undefined}
          x={14 + (i % 4) * COL}
          y={100 + Math.floor(i / 4) * 28}
          label={label}
        />
      ))}
      <rect x={14} y={164} width={492} height={150} rx={8} fill="#FFFFFF" stroke="#E8E9ED" />
      {AGREE.map(([label, bold, ink], i) => (
        <BtCircle key={label} s={s} name="t4" x={26} y={174 + i * 28} label={label} bold={bold} ink={ink} />
      ))}
      <FlatButton
        s={s}
        name="t6"
        x0={26}
        y0={262}
        x1={494}
        y1={302}
        radius={4}
        label="API Key 생성"
        off="#C9CBD1"
        on={BT_DARK}
      />
    </g>
  ),
};

export const bithumbKeys: Scene = {
  url: URL,
  width: W,
  height: 244,
  targets: { c1: copyTarget(482, 98), c2: copyTarget(482, 132) },
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
      <BtChrome tabs={false} />
      <ModalBox x0={12} y0={44} x1={508} y1={232} radius={8} />
      <T x={26} y={68} bold>
        API Key가 발급되었습니다.
      </T>
      <rect x={26} y={88} width={468} height={80} rx={4} fill="#F5F5FA" />
      {(
        [
          ["API Key", "6b2c78fd••••••••••••", "c1", 98],
          ["Secret key", "ZDkyNGRk••••••••••••", "c2", 132],
        ] as const
      ).map(([label, value, name, ry]) => (
        <g key={name}>
          <T x={38} y={ry + 13} bold fill="#111111">
            {label}
          </T>
          <T x={128} y={ry + 13} fill={MUTED}>
            {value}
          </T>
          <CopyButton s={s} name={name} x1={482} y={ry} line={BT_LINE} ink="#111111" onFill={BT_DARK} radius={4} />
        </g>
      ))}
      <FlatButton s={s} x0={26} y0={178} x1={494} y1={218} radius={4} label="확인" off={BT_DARK} on={BT_DARK} />
    </g>
  ),
};

function History({ tail, ghost }: { tail: string; ghost?: string }) {
  return (
    <g>
      <T x={14} y={52} bold>
        API Key 발급 내역
      </T>
      <line x1={14} y1={70} x2={506} y2={70} stroke="#EEEEEE" />
      <line x1={14} y1={114} x2={506} y2={114} stroke="#EEEEEE" />
      <T x={24} y={92} fill={MUTED}>
        xECsmJSx••••••••••••
      </T>
      {ghost && (
        <g>
          <rect x={448} y={77} width={48} height={30} rx={4} fill="#FFFFFF" stroke={BT_LINE} />
          <T x={472} y={92} fill="#111111" anchor="middle">
            {ghost}
          </T>
        </g>
      )}
      <T x={ghost ? 434 : 496} y={92} fill={tail.startsWith("●") ? "#111111" : MUTED} anchor="end">
        {tail}
      </T>
    </g>
  );
}

export const bithumbFix: Scene = {
  url: URL,
  width: W,
  height: 238,
  targets: { t2: [472, 92], in: inputTarget(45, 118, 433, 154), t5: [260, 186] },
  steps: [
    { t: "move", to: "t2" },
    { t: "click" },
    { t: "show", layer: "ovl" },
    { t: "move", to: "in" },
    { t: "click" },
    { t: "focus", input: "in" },
    { t: "type", input: "in" },
    { t: "move", to: "t5" },
    { t: "click" },
    { t: "hide", layer: "ovl" },
    { t: "wait", ms: 1400 },
  ],
  draw: (s) => (
    <g>
      <BtChrome tabs={false} />
      <History tail="● ● ●" ghost="변경" />
      {s.layers.has("ovl") && (
        <g className="ex-layer">
          <rect x={0} y={32} width={W} height={206} fill={DIM} />
          <rect x={31} y={50} width={458} height={170} rx={8} fill="#FFFFFF" />
          <T x={45} y={74} bold>
            허용 IP주소 변경
          </T>
          <T x={45} y={99} fill="#8B8F98">
            IP주소 입력 (최대 5개 입력 가능)
          </T>
          <IpRow s={s} y={118} x0={45} x1={475} />
          <FlatButton s={s} x0={45} y0={166} x1={475} y1={206} radius={4} label="변경" off={BT_DARK} on={BT_DARK} />
        </g>
      )}
    </g>
  ),
};

export const bithumbExisting: Scene = {
  url: URL,
  width: W,
  height: 126,
  targets: {},
  steps: [],
  draw: () => (
    <g>
      <BtChrome tabs={false} />
      <History tail="2027-07-09 만료" />
    </g>
  ),
};
