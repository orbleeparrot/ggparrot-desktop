# 껄무새 (GG Parrot)

- 껄무새 웹에서 만든 매크로를 이 PC에서 실행하는 프로그램.
- Tauri 2(Rust + React)로 만듦.
- **macOS와 Windows**에서 같은 방식으로 동작함.
- 앱 버전:
  - `src-tauri/Cargo.toml`의 `[workspace.package] version` 한 곳에만 적음.
  - `v{major}.{minor}.{patch}` 체계를 씀. 지금은 `0.18.x`이고 커밋마다 패치 번호를 올림.
- 이름:
  - 화면에는 한국어 **껄무새**, 영문 **GG Parrot**.
  - 저장소·실행 파일·패키지·키체인·데이터 폴더는 웹 저장소(`gg_parrot`)와 구분되게 `ggparrot-desktop`.
  - 번들 식별자는 `com.ggparrot.desktop`(역도메인 형식).
  - 웹의 **실행기 열기**는 `ggparrot://` 링크로 이 앱을 엶.
- 서버 계약 버전: **18**.
  - 서버는 `runner_version`을 정수로만 받아 최소 버전과 비교함.
- 대상: macOS 11 이상(Apple Silicon·Intel), Windows 10 이상(x64)
- 모바일(iOS·Android)은 지원하지 않음.
  - OS가 백그라운드 앱을 멈춰 5초 간격의 서버 보고를 유지할 수 없기 때문.
- 변경 기록: [CHANGELOG.md](CHANGELOG.md)
- 다음 작업: [docs/backlog.md](docs/backlog.md)

## 화면

| 실행 | 매크로 추가 |
|---|---|
| ![두 매크로 동시 실행](docs/screenshots/running.png) | ![매크로 추가 창](docs/screenshots/library.png) |
| **설정·키** | **설정·창 닫아도 계속 실행** |
| ![설정](docs/screenshots/settings.png) | ![창을 닫아도 계속 실행](docs/screenshots/display.png) |
| **메뉴 막대(트레이) 메뉴** | **정보·업데이트** |
| ![트레이 메뉴](docs/screenshots/tray.png) | ![정보](docs/screenshots/about.png) |
| **연결 도우미·예시 화면** | |
| ![거래소 화면 예시](docs/screenshots/wizard-example.png) | |

업데이트(로컬 업데이트 서버로 실제 설치까지 시험한 화면):

| 버전 옆 알림 | 내려받는 중 |
|---|---|
| ![새 버전 알림](docs/screenshots/update-notice.png) | ![진행률](docs/screenshots/update-downloading.png) |
| **설치 준비** | **다시 시작 후** |
| ![다시 시작해 설치](docs/screenshots/update-ready.png) | ![업데이트 완료](docs/screenshots/updated.png) |

설치 화면:

| macOS 디스크 이미지 | Windows 설치 프로그램 그림 |
|---|---|
| ![dmg](docs/screenshots/dmg.png) | ![nsis](docs/screenshots/windows-installer-art.png) |

## 동작

- 매크로는 파일(`.ggm.json`)로 열거나 웹의 **실행기 열기**(`ggparrot://launch/...`)로 받음.
- 실행 모드는 모의(기본)·테스트넷(바이낸스만)·실전.
  - 실전은 확인 창을 한 번 더 거침.
- 시작하면 서버에 세션을 만들고 5초마다 상태를 보냄.
  - 진입·청산은 서버 명령으로만 함.
  - 로컬에는 손절·일일 최대손실·최대 보유시간 안전망만 남음.
- 마이페이지 **내 매크로 실행 현황**에서 원격으로 **매크로만 종료** 또는 **청산 후 종료**를 할 수 있음.
- 거래소 API 키·시크릿은 서버로 보내지 않음.
  - 서버로 가는 것은 회원 키와 구동 상태·실행 기록뿐임.
- 실행 기록은 서버가 저장했다고 답한 것만 지움.
  - 그 전까지 로컬 보관함(SQLite)에 남겨 껐다 켜도 이어서 보냄.

## 기능

| 항목 | 내용 |
|---|---|
| 플랫폼 | macOS `.dmg`·Windows 설치 파일(`-setup.exe`, 사용자 폴더에 설치·관리자 권한 불필요) |
| 실행 | 여러 매크로 동시 실행(같은 거래소·종목은 하나만) |
| 화면 | 실행 목록·상세, 따로 여는 설정 화면, 사람이 읽는 활동 기록(원문은 개발자 로그) |
| 키 | OS 키체인(macOS 키체인·Windows 자격 증명 관리자)에 둠. 설정 한곳에서 언제나 고침. 바이낸스 테스트넷 키 칸은 따로 |
| 매크로 파일 | 앱 데이터 폴더의 `macros` 목록·가져오기·끌어다 놓기·웹 [실행기 열기] |
| 껄무새 웹 | 머리줄 지구본 단추·메뉴(껄무새 › 껄무새 웹 열기)·트레이·매크로 추가 창에서 https://gg-parrot.vercel.app/ 을 기본 브라우저로 엶 |
| 바이낸스 연동 | REST API에 직접 서명해 부름 |
| 웹 연결 등록 | 설치할 때 `ggparrot://`를 등록함. Windows는 켤 때 HKCU 등록을 다시 함 |
| 업데이트 | 앱 안에서 확인·내려받기·설치·다시 시작(서명 확인) |
| 창 닫기 | 메뉴 막대·알림 영역 아이콘으로 숨어 계속 실행(macOS는 Dock 아이콘도 숨김). 설정에서 끌 수 있음 |

- 로컬 파일은 모두 앱 데이터 폴더 `ggparrot-desktop` 아래에 있음(macOS `~/Library/Application Support`, Windows `%LOCALAPPDATA%`).
  - 매크로 폴더 `macros/`
  - 실행 목록 `runs.json`
  - 설정 `prefs.json`
  - 실행 기록 `activity/`
- 실행 목록에는 키가 들어가지 않음.
- 키체인 항목 이름은 `ggparrot-desktop`.
- 웹에서 받은 매크로 파일은 브라우저의 다운로드 폴더에 저장됨.
  - 창에 끌어다 놓거나 [폴더로 가져오기]로 넣음.
- 웹의 [실행기 열기]는 파일 없이 바로 연결함.

## 서버와 앱의 역할 — 서버가 멈추면

- 매매 판단은 서버 전략이 하고, 앱은 그 명령을 거래소에 주문함.
- 앱이 스스로 판단하는 것은 안전망뿐임.

| 일 | 판단 | 서버와 연결이 끊기면 |
|---|---|---|
| 진입(매수·공매도) | 서버 전략의 명령 | 하지 않음(새 진입 멈춤) |
| 전략의 청산 신호 | 서버 전략의 명령 | 하지 않음 |
| 손절 %·하루 최대 손실·최대 보유시간 | 앱(매크로의 `risk`) | 계속 지킴 — 닿으면 앱이 바로 청산 |
| [청산 후 종료]·[매크로만 종료] | 사용자 | 됨 |
| 웹의 종료·정리 요청 | 웹 → 서버 → 앱 | 전달되지 않음 — 앱에서 종료해야 함 |

- 끊긴 동안 화면·트레이에 '서버 재시도 중'이 보이고, 5분이 넘으면 따로 알림.
- 앱이 꺼져 있거나 PC가 잠들면 안전망도 멈춤.
- 앱이 안전망으로 청산한 사실은 아직 서버 전략에 반영되지 않음 — [서버 설계](docs/server-stop-loss-design.md).

## 개발

- 필요한 도구: Node 22.12 이상(`.nvmrc`는 24 LTS), Rust stable(`src-tauri/rust-toolchain.toml`).
- macOS에서 설치 파일까지 만들려면 Xcode가 필요함.

```bash
npm install
```

```bash
npm run tauri dev
```

- 브라우저만으로 화면을 확인하려면 `npm run dev`를 실행하고 http://localhost:1420 을 엶.
- Tauri 밖에서는 가짜 백엔드(`src/mock.ts`)가 붙음.

테스트:

```bash
npm test
```

```bash
cd src-tauri && cargo test --workspace
```

- 포맷은 Prettier(TS·CSS·JSON, 폭 120)와 rustfmt(폭 120)로 맞춤.
- CI가 `format:check`로 확인함.

```bash
npm run format
```

로컬 백엔드에 붙이려면 서버 주소를 바꿈.

```bash
GGP_SERVER_BASE=http://localhost:8000 npm run tauri dev
```

- 시험용 환경변수: 1회 주문 상한 `MAX_ORDER_USDT`(기본 100)·`MAX_ORDER_KRW`(기본 150000), 선물 상한 기준 `ORDER_CAP_BASIS`(`notional` | `margin`), 회원 키 미리 채우기 `GGP_MEMBER_KEY`.
- `GGP_MACRO_DIR`로 매크로 폴더를 바꿀 수 있음.
- 전체 목록은 `.env.example`에 있음.
설치 파일 만들기(로컬):

```bash
npm run build:mac
```

```bash
npm run build:win
```

- 결과 파일 이름은 배포와 같음: `ggparrot-desktop_<버전>_darwin_universal.dmg`·`ggparrot-desktop_<버전>_windows_x64-setup.exe`.
  - Tauri는 화면 이름(`GG Parrot`)으로 파일을 만들고, `scripts/name-bundles.mjs`가 기술 이름으로 바꿈.
  - 위치: `src-tauri/target/universal-apple-darwin/release/bundle/dmg/`·`src-tauri/target-win/x86_64-pc-windows-gnu/release/bundle/nsis/`.
- `build:mac`은 `rustup target add x86_64-apple-darwin`이 한 번 필요함(Apple Silicon·Intel 공용 빌드).
- `build:win`은 macOS에서 Windows 설치 파일을 만듦(배포는 CI의 Windows 러너가 MSVC로 만듦).
  - `brew install nsis mingw-w64`가 필요함.
  - `makensis`는 UTF-8 로캘이 아니면 한글 이름에서 멈춤(스크립트가 로캘을 맞춤).

## 구조

```
src/                      React + TypeScript 화면 (상태는 Rust 가 소유, 화면은 스냅샷만 그림)
  types.ts                Rust ↔ 화면 계약
src-tauri/src/            Tauri 앱 레이어 (명령 · 연결 도우미 · 딥링크 · 단일 인스턴스)
src-tauri/crates/runner-core/
  brokers/                바이낸스 · 업비트 · 빗썸 · 모의
  bot.rs                  실행 루프
  server.rs               서버 클라이언트
  outbox.rs               실행 기록 보관함
  credentials.rs          키체인 저장 규칙
  connection.rs           연결 검사 · 공인 IP · 키 마스킹
docs/                     규칙 · 다음 작업 · 설계 문서
```

## 설치·업데이트

- 설치 파일 이름은 기술 이름(`ggparrot-desktop`): `ggparrot-desktop_<버전>_darwin_universal.dmg`(Apple Silicon·Intel 공용)·`ggparrot-desktop_<버전>_windows_x64-setup.exe`.
- 설치한 앱의 이름은 화면 이름: macOS `GG Parrot.app`, Windows 시작 메뉴 'GG Parrot'.
- **macOS**:
  - `.dmg`를 열고 앱을 응용 프로그램 폴더로 끌어 놓음.
  - Finder·Dock에는 macOS 언어가 한국어면 '껄무새', 아니면 'GG Parrot'으로 보임.
  - 디스크 이미지나 다운로드 폴더에서 바로 실행하면 업데이트를 설치할 수 없어 앱이 옮기라고 안내함.
- **Windows**:
  - `-setup.exe` 하나만 배포함(MSI 없음).
  - 사용자 폴더(`%LOCALAPPDATA%\Programs`)에 설치하므로 관리자 권한이 필요 없음.
  - 시작 메뉴와 제어판 프로그램 제거에 'GG Parrot'으로 등록됨.
- **앱 안 업데이트**:
  - 켤 때와 6시간마다 확인함.
  - 새 버전이 있으면 머리줄의 버전 표시 옆에 알림이 뜸.
  - 설정 › 정보 › 업데이트에서 내려받기를 누르면 진행 막대와 크기가 보이고, 이때는 매크로가 계속 돎.
  - 다 받으면 서명을 확인하고 **다시 시작해 설치**를 누를 때 설치함.
  - 실행 중인 매크로가 있으면 설치하지 않음.
  - Windows는 설치 창이 잠깐 뜬 뒤 다시 켜짐.
- 설치 화면 그림(`src-tauri/installer/`)은 `swift scripts/installer-art.swift`로 다시 만듦.
- 앱·트레이 아이콘 원본은 `assets/*.svg`. `src-tauri/icons/`에는 번들(`tauri.conf.json`의 `bundle.icon`)과 트레이(`tray.rs`)가 쓰는 파일만 둠.
- 견본 매크로: `docs/samples/btc-rsi.ggm.json`(창에 끌어다 놓아 시험).

## 배포

- 저장소: https://github.com/orbleeparrot/ggparrot-desktop
- 설치 파일·업데이트 파일은 모두 이 저장소의 **GitHub Releases** 한 곳에 둠. 다른 패키지 저장소(GitHub Packages·npm 등)는 쓰지 않음.

| 무엇 | 어디 |
|---|---|
| 설치 파일(`.dmg`·`-setup.exe`) | Releases의 각 버전 |
| 업데이트 묶음·서명(`.app.tar.gz`·`.sig`·`-setup.exe.sig`) | 같은 릴리스 |
| 업데이트 목록(`latest.json`) | 같은 릴리스. 앱은 `releases/latest/download/latest.json`을 읽음 |
| 업데이트 서명 키 | Settings › Secrets and variables › Actions의 `TAURI_SIGNING_PRIVATE_KEY`·`TAURI_SIGNING_PRIVATE_KEY_PASSWORD`(로컬 키 파일: `~/.tauri/ggparrot-desktop-updater.key`, 공개키는 `tauri.conf.json`) |

순서(자동):

1. 커밋마다 버전을 올리고 `CHANGELOG.md`에 그 버전 칸을 적어 `main`에 푸시함.
2. CI가 통과하면 **release** 워크플로가 그 버전의 릴리스 **초안**을 자동으로 만듦.
   - macOS(universal)·Windows 설치 파일과 앱 업데이트 파일을 붙임 — 초안에서 바로 내려받아 시험할 수 있음.
   - 본문은 CHANGELOG에서 자동으로(사용자용 분류만, 지난 배포 뒤 배포하지 않은 버전 칸을 합침, `### 개발`은 뺌).
   - 맨 아래 설치 안내(`<!-- web-only -->` 아래)는 웹 릴리스 페이지에만 보이고 앱의 '바뀐 점'에는 보이지 않음.
   - 이전 초안은 지우고 최신 커밋의 초안 하나만 남김. 초안은 사용자와 앱 업데이트에 보이지 않음.
3. Releases에서 초안을 확인하고 [Publish release]를 누름 — **이때 배포됨.**
   - 태그 `v<버전>`이 이때 생기고, 앱의 업데이트 확인이 새 버전을 봄(latest 릴리스만 읽음).
   - **release-published** 워크플로가 CHANGELOG에 배포 표시(날짜, 함께 나간 칸은 `배포하지 않음(<버전>에 포함)`)를 하고 `main`에 커밋함.
4. 주의
   - 초안 본문을 GitHub에서 고치면 웹 페이지만 바뀜. 앱의 '바뀐 점'은 빌드 때 CHANGELOG 내용 — 고치려면 CHANGELOG를 고쳐 다시 푸시(초안이 새로 만들어짐).
   - Publish한 릴리스는 파일을 바꿀 수 없음(Settings › General › Releases › Enable release immutability). 잘못 나갔으면 버전을 올려 다시 배포.
   - 미리 보기: `npm run release:notes -- v<버전> --preview`. 초안을 다시 만들려면 Actions › release › [Run workflow](main).

참고:

- 앱 안 업데이트는 서명된 파일만 설치함.
- 워크플로의 액션은 커밋 SHA로 고정해 둠(Dependabot이 올림, 저장소 설정으로도 강제).
- 태그 이름은 환경변수로만 스크립트에 넘김.
- 코드 서명: macOS는 Apple 인증서가 없으면 임시 서명(ad-hoc), Windows는 서명 없음. 첫 실행 때 경고가 뜸. 넣는 방법은 [docs/code-signing.md](docs/code-signing.md).

웹 연동:

- 웹(`gg_parrot/frontend`)은 지금 Windows에서만 실행기를 안내함.
  - macOS에서 **실행기 열기**를 쓰려면 웹의 OS 판별과 다운로드 링크(`runnerDownload.js`, `backend/app/runner_release.py`)를 따로 갱신해야 함.

## 주의

- 실거래(실전 모드)는 실제 자금이 움직임.
- 이 프로그램은 투자 조언이 아니며 손익 책임은 사용자에게 있음.
