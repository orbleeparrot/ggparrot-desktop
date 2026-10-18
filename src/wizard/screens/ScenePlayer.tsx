// 장면 재생기 — 커서가 목표로 미끄러져 가서(CSS transition) 누르고(CSS keyframes 물결) 칸을 켜거나 주소를 찍음.
// 그림일 뿐 아무 데도 보내지 않음. 움직임 줄이기 설정이면 움직이지 않고 '다 누른 모습' 한 장을 보임.
// 실제 화면으로 오해하지 않게 회색 받침 위에 [예시 화면] 표시와 함께 놓고, 멈춤·재생 단추를 둠.
import { MonitorPlay, Pause, Play } from "lucide-react";
import { SwapLabel } from "../../components/SwapLabel";
import { useEffect, useMemo, useState } from "react";
import { EMPTY_STATE, type Scene, type SceneState, type Step } from "./types";

const MOVE_MS = 520;
const LOOP_PAUSE_MS = 900;

interface Cursor {
  x: number;
  y: number;
  visible: boolean;
  pressed: boolean;
  glide: boolean;
  clicks: number;
}

function prefersReducedMotion(): boolean {
  return typeof window !== "undefined" && !!window.matchMedia?.("(prefers-reduced-motion: reduce)").matches;
}

function apply(s: SceneState, step: Step, ip: string): SceneState {
  switch (step.t) {
    case "on":
      return { ...s, on: new Set(s.on).add(step.name) };
    case "show":
      return { ...s, layers: new Set(s.layers).add(step.layer) };
    case "hide": {
      const layers = new Set(s.layers);
      layers.delete(step.layer);
      return { ...s, layers };
    }
    case "focus":
      return { ...s, focus: new Set(s.focus).add(step.input) };
    case "type":
      return { ...s, typed: { ...s.typed, [step.input]: step.text ?? ip } };
    default:
      return s;
  }
}

/** 움직임 없이 보일 한 장 — 처음 'hide' 앞까지(겹친 창이 열린 채로) 모두 적용한 모습. */
export function stillFrame(scene: Scene, ip: string): SceneState {
  let s = EMPTY_STATE;
  for (const step of scene.steps) {
    if (step.t === "hide") break;
    s = apply(s, step, ip);
  }
  return s;
}

export function ScenePlayer({ scene, ip, label }: { scene: Scene; ip: string; label: string }) {
  const motionOff = useMemo(prefersReducedMotion, []);
  const [paused, setPaused] = useState(false);
  // 멈추면 움직임 줄이기와 같이 '다 누른 모습' 한 장을 보임. 다시 재생하면 처음부터.
  const reduced = motionOff || paused;
  const home = { x: scene.width * 0.7, y: scene.height * 0.8 };
  const [s, setS] = useState<SceneState>(() => (reduced ? stillFrame(scene, ip) : EMPTY_STATE));
  const [cursor, setCursor] = useState<Cursor>({ ...home, visible: false, pressed: false, glide: false, clicks: 0 });

  useEffect(() => {
    if (reduced) {
      setS(stillFrame(scene, ip));
      return;
    }
    if (scene.steps.length === 0) return;
    // 멈췄다 다시 재생하면 '다 한 모습'에서 빈 화면으로 돌아가 처음부터 돎.
    setS(EMPTY_STATE);
    let timer: ReturnType<typeof setTimeout> | undefined;
    let alive = true;
    const after = (ms: number, fn: () => void) => {
      timer = setTimeout(() => alive && fn(), ms);
    };

    const run = (i: number) => {
      if (i >= scene.steps.length) {
        after(LOOP_PAUSE_MS, restart);
        return;
      }
      const step = scene.steps[i];
      const next = (ms: number) => after(ms, () => run(i + 1));
      switch (step.t) {
        case "move": {
          const [x, y] = scene.targets[step.to] ?? [home.x, home.y];
          setCursor((c) => ({ ...c, x, y, visible: true, glide: true }));
          next(MOVE_MS + 80);
          return;
        }
        case "click":
          setCursor((c) => ({ ...c, pressed: true, clicks: c.clicks + 1 }));
          after(120, () => {
            setCursor((c) => ({ ...c, pressed: false }));
            next(140);
          });
          return;
        case "type": {
          const text = step.text ?? ip;
          const typeChar = (n: number) => {
            setS((prev) => ({ ...prev, typed: { ...prev.typed, [step.input]: text.slice(0, n) } }));
            if (n >= text.length) next(260);
            else after(55, () => typeChar(n + 1));
          };
          typeChar(0);
          return;
        }
        case "wait":
          next(step.ms);
          return;
        default:
          setS((prev) => apply(prev, step, ip));
          next(step.t === "show" ? 260 : step.t === "focus" ? 160 : 200);
      }
    };

    const restart = () => {
      setS(EMPTY_STATE);
      setCursor((c) => ({ ...c, x: home.x, y: home.y, visible: false, glide: false, pressed: false }));
      after(300, () => run(0));
    };

    after(400, () => run(0));
    return () => {
      alive = false;
      clearTimeout(timer);
    };
    // 장면·주소가 바뀌면 처음부터 다시 돎.
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [scene, ip, reduced]);

  return (
    <figure className="ex-stage">
      <figcaption className="ex-caption">
        <span className="ex-badge">
          <MonitorPlay size={13} aria-hidden="true" />
          예시 화면
        </span>
        <span className="ex-caption-text">{label} 사이트에서 할 일을 그림으로 보여 줘요</span>
        {!motionOff && (
          <button
            type="button"
            className="ex-toggle"
            onClick={() => setPaused((p) => !p)}
            title={paused ? "다시 재생" : "멈추고 완성된 화면 보기"}
          >
            <SwapLabel
              active={paused ? 1 : 0}
              items={[
                <>
                  <Pause size={12} aria-hidden="true" />
                  멈춤
                </>,
                <>
                  <Play size={12} aria-hidden="true" />
                  재생
                </>,
              ]}
            />
          </button>
        )}
      </figcaption>
      <div className="ex-window" aria-hidden="true">
        <div className="ex-bar">
          <span className="ex-dots">
            <i />
            <i />
            <i />
          </span>
          <span className="ex-url">{scene.url}</span>
        </div>
        <svg className="ex-screen" viewBox={`0 0 ${scene.width} ${scene.height}`} role="img" aria-hidden="true">
          <rect width={scene.width} height={scene.height} fill="#FFFFFF" />
          {scene.draw(s)}
          {!reduced && (
            <g
              className={`ex-cursor ${cursor.visible ? "is-visible" : ""}`}
              style={{
                transform: `translate(${cursor.x}px, ${cursor.y}px)`,
                transition: cursor.glide
                  ? `transform ${MOVE_MS}ms cubic-bezier(0.45, 0, 0.55, 1), opacity 160ms`
                  : "opacity 160ms",
              }}
            >
              {cursor.clicks > 0 && <circle key={cursor.clicks} className="ex-ripple" r={11} />}
              <path className={`ex-arrow ${cursor.pressed ? "is-pressed" : ""}`} d="M0 0 L10.5 6 L6 7.5 L3.75 12 Z" />
            </g>
          )}
        </svg>
      </div>
    </figure>
  );
}
