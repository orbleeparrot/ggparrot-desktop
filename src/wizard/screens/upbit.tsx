// 업비트 Open API 관리 화면(단순화) — 남색 머리줄·흰 바탕·네모 체크·파란 버튼.
import {
  copyTarget,
  CopyButton,
  DIM,
  FlatButton,
  InputBox,
  ModalBox,
  MUTED,
  T,
  UP_BLUE,
  UP_LINE,
  UP_NAVY,
  UpBox,
  UpChrome,
} from "./kit";
import { inputTarget, W, type Scene } from "./types";

const URL = "upbit.com/mypage/open_api_management";
const MASK_A = "a8Kq3Zt0••••••••••••";
const MASK_B = "Wm2xP7Lb••••••••••••";

export const upbitIp: Scene = {
  url: URL,
  width: W,
  height: 92,
  targets: { in: inputTarget(104, 46, 506, 80) },
  steps: [
    { t: "move", to: "in" },
    { t: "click" },
    { t: "focus", input: "in" },
    { t: "type", input: "in" },
    { t: "wait", ms: 1600 },
  ],
  draw: (s) => (
    <g>
      <UpChrome />
      <T x={14} y={63} fill="#333333">
        IP 주소 등록
      </T>
      <InputBox
        s={s}
        name="in"
        x0={104}
        y0={46}
        x1={506}
        y1={80}
        placeholder='IP 주소 입력(","로 구분하여 최대 10개)'
        line={UP_LINE}
        focus={UP_BLUE}
      />
    </g>
  ),
};

const PERMS: Array<[string, string]> = [
  ["포켓관리", ""],
  ["자산조회", "t1"],
  ["주문조회", "t2"],
  ["주문하기", "t3"],
  ["출금조회", ""],
  ["출금하기", ""],
  ["입금조회", ""],
  ["입금하기", ""],
];
const PERM_X = 104;
const PERM_COL = (506 - PERM_X) / 4;

export const upbitPerm: Scene = {
  url: URL,
  width: W,
  height: 240,
  targets: {
    ...Object.fromEntries(
      PERMS.filter(([, n]) => n).map(([, n]) => {
        const i = PERMS.findIndex(([, m]) => m === n);
        return [n, [PERM_X + (i % 4) * PERM_COL + 8, 44 + Math.floor(i / 4) * 26 + 8] as const];
      }),
    ),
    t4: [22, 110],
    t5: [36, 152],
    t6: [104, 208],
  },
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
    { t: "move", to: "t5" },
    { t: "click" },
    { t: "on", name: "t5" },
    { t: "on", name: "t6" },
    { t: "move", to: "t6" },
    { t: "click" },
    { t: "wait", ms: 1400 },
  ],
  draw: (s) => (
    <g>
      <UpChrome />
      <T x={14} y={52} fill="#333333">
        권한 선택
      </T>
      {PERMS.map(([label, name], i) => (
        <UpBox
          key={label}
          s={s}
          name={name || undefined}
          x={PERM_X + (i % 4) * PERM_COL}
          y={44 + Math.floor(i / 4) * 26}
          label={label}
        />
      ))}
      <UpBox s={s} name="t4" x={14} y={102} label="개인(신용)정보 수집 및 이용 동의(필수)" />
      <rect x={14} y={130} width={220} height={44} fill="#F9F9F9" stroke="#D3D3D3" />
      <UpBox s={s} name="t5" x={26} y={142} size={20} line="#C1C1C1" width={2} />
      <T x={56} y={152} fill="#333333">
        로봇이 아닙니다.
      </T>
      <FlatButton
        s={s}
        name="t6"
        x0={14}
        y0={188}
        x1={194}
        y1={228}
        label="Open API Key 발급받기"
        off={UP_LINE}
        on={UP_BLUE}
      />
    </g>
  ),
};

export const upbitKeys: Scene = {
  url: URL,
  width: W,
  height: 238,
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
      <UpChrome />
      <ModalBox x0={12} y0={44} x1={508} y1={226} />
      <T x={26} y={68} bold>
        Open API Key 발급 완료
      </T>
      <rect x={26} y={88} width={468} height={80} fill="#F2F7FD" />
      {(
        [
          ["Access Key", MASK_A, "c1", 98],
          ["Secret Key", MASK_B, "c2", 132],
        ] as const
      ).map(([label, value, name, ry]) => (
        <g key={name}>
          <T x={38} y={ry + 13} bold fill={UP_BLUE}>
            {label}
          </T>
          <T x={128} y={ry + 13} fill={MUTED}>
            {value}
          </T>
          <CopyButton s={s} name={name} x1={482} y={ry} line={UP_BLUE} ink={UP_BLUE} onFill={UP_BLUE} />
        </g>
      ))}
      <FlatButton s={s} x0={434} y0={178} x1={494} y1={212} label="확인" off={UP_BLUE} on={UP_BLUE} />
    </g>
  ),
};

export const upbitFix: Scene = {
  url: URL,
  width: W,
  height: 236,
  targets: { t1: [32, 66], t2: [472, 66], in: inputTarget(50, 98, 470, 132), t5: [442, 161] },
  steps: [
    { t: "move", to: "t1" },
    { t: "click" },
    { t: "on", name: "t1" },
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
      <UpChrome />
      <rect x={14} y={44} width={492} height={44} fill="#FFFFFF" stroke="#E5E7EB" />
      <UpBox s={s} name="t1" x={24} y={58} />
      <T x={50} y={66} fill={MUTED}>
        a8Kq3Zt0••••••••••••
      </T>
      <T x={434} y={66} fill="#1B3A8C" anchor="end">
        ● ● ●
      </T>
      <rect x={448} y={51} width={48} height={30} fill={UP_NAVY} />
      <T x={472} y={66} bold fill="#FFFFFF" anchor="middle">
        변경
      </T>
      {s.layers.has("ovl") && (
        <g className="ex-layer">
          <rect x={0} y={32} width={W} height={204} fill={DIM} />
          <rect x={36} y={54} width={448} height={138} fill="#FFFFFF" />
          <T x={50} y={78} bold>
            허용 IP주소 변경
          </T>
          <InputBox
            s={s}
            name="in"
            x0={50}
            y0={98}
            x1={470}
            y1={132}
            placeholder='","로 구분하여 여러개의 IP주소 등록 가능'
            line={UP_LINE}
            focus={UP_BLUE}
          />
          <rect x={352} y={144} width={56} height={34} fill="#FFFFFF" stroke={UP_BLUE} />
          <T x={380} y={161} fill={UP_BLUE} anchor="middle">
            취소
          </T>
          <rect x={414} y={144} width={56} height={34} fill={UP_BLUE} />
          <T x={442} y={161} bold fill="#FFFFFF" anchor="middle">
            확인
          </T>
        </g>
      )}
    </g>
  ),
};

export const upbitExisting: Scene = {
  url: URL,
  width: W,
  height: 100,
  targets: {},
  steps: [],
  draw: () => (
    <g>
      <UpChrome />
      <rect x={14} y={44} width={492} height={44} fill="#FFFFFF" stroke="#E5E7EB" />
      <T x={24} y={66} fill={MUTED}>
        a8Kq3Zt0••••••••••••
      </T>
      <T x={436} y={66} fill={MUTED} anchor="end">
        2027.07.01 만료
      </T>
      <rect x={448} y={52} width={48} height={28} fill="#FFFFFF" stroke={UP_BLUE} />
      <T x={472} y={66} fill={UP_BLUE} anchor="middle">
        보기
      </T>
    </g>
  ),
};
