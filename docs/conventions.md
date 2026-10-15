# 코드 컨벤션

- 이 저장소의 규칙. 리뷰(사람·CodeRabbit)의 기준.
- 포맷은 도구가 맞춤. 여기에는 도구가 잡지 못하는 규칙만 적음.

## 1. 공통

- **포맷:** Prettier(TS·CSS·JSON·YAML, 마크다운 제외)·rustfmt, 줄 폭 120. `npm run format`으로 맞추고 CI가 `format:check`로 확인. 편집기 설정은 `.editorconfig`.
- **린트:** TS·React는 oxlint(`npm run lint`, 설정 `.oxlintrc.json`, 경고도 실패), Rust는 clippy(`npm run lint:rust`, `-D warnings`). CI가 둘 다 확인.
  - oxlint를 쓰는 까닭: 앱은 TypeScript 7(네이티브)이라 typescript-eslint가 쓰는 TypeScript JS API가 없음. oxlint는 자체 해석기라 TypeScript 버전과 무관하고 ESLint와 규칙 이름이 같음.
  - 잡는 규칙: 정확성(correctness) 전체 + `any` 금지, 버튼 `type` 빠짐, 훅 규칙·의존성 배열, React 컴파일러 규칙(렌더 중 ref 읽기·시계 읽기·effect 안 동기 setState 등), 안 쓰는 변수(`_`로 시작하면 예외), 잡은 오류를 다시 던질 때 `cause`.
  - 렌더 중에 바꿔야 하는 값은 ref가 아니라 상태로 두고 조건부로 갱신함(`if (prev !== x) setPrev(x)`). 최신 콜백을 담는 ref는 effect에서 갱신함.
- **인코딩·줄 끝:** UTF-8, LF(Windows 스크립트만 CRLF). `.gitattributes`가 강제.
- **이름:** 화면 이름은 `껄무새`/`GGParrot`(띄어쓰기 없음, Windows 화면은 '껄무새'). 설치 폴더·레지스트리 영문 이름은 `GGParrot`. 기술 이름(패키지·실행 파일·키체인·데이터 폴더)은 `ggparrot-desktop`. 딥링크 `ggparrot://`는 바꾸지 않음. 서버 연동 레벨(server link level, `SERVER_LINK_LEVEL`, 서버 필드 `runner_version`, 지금 "19")은 그 레벨의 서버 연동 기능을 모두 갖춘 뒤에만 올림(서버가 기능별 최소 레벨로 기능을 열고 닫음). '버전'·'계약'·'프로토콜'이라 부르지 않음.
- **버전:** `src-tauri/Cargo.toml`의 `[workspace.package] version` 한 곳에만 적음. 두 크레이트는 물려받고, `tauri.conf.json`·`package.json`에는 적지 않음(`npm run check:versions`가 확인). 태그는 `v{버전}`.
- **버전 올리기:** 앱 버전은 `0.<서버 연동 레벨>.<패치>`(지금 `0.19.x`). 커밋마다 패치를 하나 올림(`0.19.0` → `0.19.1`). 서버 연동 레벨을 올리면 마이너를 그 숫자로 바꾸고 패치는 0부터(`0.19.5` → `0.20.0`). 테스트가 마이너와 서버 연동 레벨이 같은지 확인. `CHANGELOG.md`에 그 버전 칸(`## [0.19.1] - 미배포`)을 만들어 그 커밋의 변경을 적음.
- **변경 기록(`CHANGELOG.md`):** 버전 칸에서 릴리스 본문과 앱의 '바뀐 점'을 만듦. 지난 배포 뒤 배포하지 않은 칸은 분류별로 합쳐지고, `### 개발`은 빠짐.
  - 사용자용 분류(`추가`·`변경`·`수정`·`삭제`·`보안`)에는 사용자가 알 말로, 화면·동작이 바뀐 것만 적음. 파일 이름·함수 이름·테스트 이야기는 쓰지 않음.
  - 빌드·CI·테스트·코드 정리·문서는 `### 개발`에 적음. 릴리스 본문과 앱에는 나가지 않음(`scripts/release-notes-lib.mjs`).
  - 분류 앞 소개 글(예: '첫 배포.')은 합쳐지는 칸 모두에서 모음.
  - 배포 전 확인: `npm run release:notes -- v<버전> --preview`. 배포 표시는 초안을 공개하면 자동(release-published). 손으로 할 때는 `npm run release:mark -- v<버전>`.
- **주석·문서:** 한국어. "무엇"보다 "왜"를 적음. 문체는 아래 1.1의 개조식.

### 1.1 주석·문서 문체: 개조식

- **대상:** 코드 주석, 저장소의 모든 문서(README·CHANGELOG·`docs/`·PR 양식), 설정 파일 주석. 사용자 화면 문구는 대상이 아님(2. 해요체).
- **기준 시점:** 주석·문서는 현재 상태 기준으로 적음.
  - 지금 코드가 무엇을 왜 하는지만 적음. "0.18.x에서 바꿈"·"이전에는" 같은 버전별 변경 이력은 `CHANGELOG.md`에만 적음.
  - 다른 앱·옛 구현과 견주거나 그 이름을 빌려 설명하지 않음.
- **끝맺음:** 명사형(~함·~임·~됨·~음·~없음) 또는 명사로 끝냄. "~한다·~했다·~이다·~합니다·~해요"는 쓰지 않음.
  - 예: "실행 슬롯에 반영한다." → "실행 슬롯에 반영함."
  - 예: "옮겼다" → "옮김", "18 이다" → "18임"
  - 금지 규칙은 "~하지 않음" 또는 "~ 금지".
- **항목:** 한 줄(한 항목)에 한 내용. 긴 문장은 나눔.
- **제목:** 마크다운 `#` 뒤 한 칸 띄움(`## 현상`).
- **띄어쓰기:**
  - 조사는 앞말에 붙임. 코드 표기·영문·숫자 뒤도 같음(`exit_id`를, README에, 18임).
  - 조사는 읽는 소리의 받침에 맞춤(`in_position`이, `acks`를, `_RollbackJob`으로). 애매하면 문장을 바꿔 조사를 피함.
  - 가운뎃점(·)은 앞뒤를 붙임(그리드·마틴게일).

## 2. 사용자 문구(한국어)

- **해요체**로 씀: "시작했어요", "확인하세요". 합니다체와 섞지 않음.
- **조사는 앞말 받침에 맞춤.** 버전·숫자 뒤는 `src/state/particle.ts`의 `iga`·`euro`를 씀(`v0.18.2로`, `v0.18.3이`). 자산 이름처럼 바뀌는 낱말 뒤에는 조사를 붙이지 않게 문장을 짬.
- **용어:** 매크로(전략·봇 X), 시작·[시작](실행 버튼 X), 회원 키, 거래소 키, 모의·테스트넷·실전, 활동 기록, 매크로 폴더.
- **오류 문구:** 무슨 일이 있었는지 + 사용자가 할 일. 원시 오류(HTTP·프록시·서명 URL)는 그대로 보이지 않음. `safe_preflight_reason`·`mask_secrets`를 거침.
- **버튼:** 동사로 끝냄("내려받기", "다시 시작해 설치"). 결과를 되돌릴 수 없으면 확인 대화상자를 둠.

## 3. Rust (`src-tauri`)

**구조**

- `crates/runner-core`: GUI와 무관한 로직(브로커·서버·보관함·키 저장·봇 루프). Tauri에 의존하지 않음.
- `src/shared/*`: 앱 컨트롤러. 관심사별 파일이 각자 `impl Shared`를 가짐. 상태·출력은 `shared/mod.rs`.
- `src/state/*`: `Core`(상태·로직)와 `render`(화면 스냅샷 DTO). DTO는 `src/types.ts`와 필드가 같아야 함(camelCase).
- 파일이 400줄을 넘으면 나눔. 큰 모듈의 테스트는 `<모듈>/tests.rs`로 뺌.

**동시성**

- `std::sync::Mutex` 잠금을 `.await` 너머까지 들고 있지 않음. 잠금 안에서 파일·네트워크 I/O 금지.
- 메뉴·트레이·창 API는 메인 스레드에서만 부름. 다른 스레드에서는 `app.run_on_main_thread`로 넘김(막히는 호출 금지).
- 상태를 바꾸면 `emit()`으로 화면에 알림. 로그 한 줄마다 전체 상태를 보내지 않음(`emit_activity_only`).

**오류·돈**

- 네트워크·파일·사용자 입력에서 온 값에 `unwrap()`·`expect()` 금지(테스트·상수 정규식·HMAC 키 제외).
- 주문은 다시 보내지 않음. 결과를 모르면 같은 client id로 **조회만** 함. 거래소가 분명히 거절한 것과 결과를 모르는 것을 구분.
- 결과를 모르는 포지션(`position_uncertain`)이면 추가 주문 없이 실행을 멈춤. 분명한 실패는 재시도·알림으로 처리.
- HTTP 요청은 반드시 제한 시간을 둠(`runner_core::net::http_client`, 요청별 `.timeout`).
- 수량·가격 반올림은 `money.rs`의 함수만 씀(내림·거래소 단위).

**린트·테스트**

- `cargo clippy --workspace --all-targets` 경고 0. `#[allow(...)]`은 이유 주석과 함께.
- 거래소·서버 동작은 `wiremock`으로, 봇은 `FakeBroker`로 테스트. 실제 키체인·네트워크를 쓰는 테스트는 `#[ignore]`.

## 4. TypeScript·React (`src`)

- **계약:** Rust와 주고받는 모양은 `src/types.ts` 하나에 둠. Rust DTO를 바꾸면 같은 커밋에서 `types.ts`·`mock.ts`·`test/fixtures.ts`를 함께 고침.
- **상태:** 진실은 Rust가 가짐. 화면은 `runner:state` 스냅샷을 그리고, 바꾸려면 `act("명령", args)`를 부름. 화면에서 상태를 흉내 내 바꾸지 않음.
- **브라우저 미리보기:** `npm run dev`는 `src/mock.ts` 가짜 백엔드로 돎. 새 명령을 추가하면 mock에도 추가.
- **컴포넌트:** 함수 컴포넌트 + 훅. 파일 하나에 공개 컴포넌트 하나. 스타일은 `src/styles/*.css`의 토큰(`var(--…)`)만 쓰고 색을 직접 적지 않음(거래소 화면 그림 제외).
- **접근성:** 버튼은 `<button type="button">`, 아이콘만 있는 버튼은 `aria-label`. 대화상자는 `Dialog`를 씀(포커스 가두기·Esc).
- **타입:** `any` 금지. 좁힐 수 없으면 `unknown` 후 확인.
- **테스트:** vitest + Testing Library. 문구가 아니라 동작(역할·상태)을 확인.

## 5. 보안·민감 정보

- 거래소 키·시크릿·회원 키는 **OS 키체인에만** 둠. 파일(`runs.json`·`prefs.json`·로그·SQLite)과 서버로 보내지 않음.
- 화면으로는 "채워졌는지"만 보냄. 값은 [보기]를 눌렀을 때만 그 칸 하나.
- 로그·오류·활동 기록은 키 값을 가림(`mask_secrets`). 새 로그 경로를 만들면 이 함수를 거치게 함.
- `Debug` 출력에 키가 찍히지 않게, 키를 담는 타입은 `Debug`를 직접 구현.
- 저장소에 비밀을 커밋하지 않음. `.env*`·인증서·서명 키는 `.gitignore`로 막혀 있고, CI가 gitleaks로 전체 기록을 검사. 테스트용 가짜 토큰은 실제 토큰 모양을 통째로 적지 말고 실행 중에 붙여 만듦.
- 웹뷰 권한(`capabilities/default.json`)은 메인 창·이벤트·창 조작만. 원격 페이지에는 IPC를 열지 않음.

## 6. 커밋·브랜치·PR

- **커밋 제목:** `종류(범위): 한국어 요약`. 종류는 `feat`·`fix`·`refactor`·`perf`·`docs`·`test`·`chore`·`ci`·`style`. 범위는 `core`·`app`·`ui`·`update`·`keys`·`bot`·`api`·`bundle` 등.
- **본문:** 무엇이 문제였고 왜 이렇게 고쳤는지. 동작이 바뀌지 않는 리팩토링은 "(동작 변경 없음)"을 적음. 문체는 개조식.
- **브랜치:** `main`은 항상 배포 가능. 작업은 `feat/…`·`fix/…` 브랜치에서 PR로.
- **PR 전에:** `npm run format:check`·`npm run typecheck`·`npm test`·`cargo test --workspace`·`cargo clippy` 통과. 화면이 바뀌면 스크린샷을 붙임.
- **변경 기록:** `CHANGELOG.md`의 이번 버전 칸에. 사용자에게 보이는 변경은 사용자용 분류, 나머지는 `### 개발`(1. 공통 참고).
