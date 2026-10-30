# 기여 안내

## 준비

- Node 22.12 이상(`.nvmrc`는 26), Rust stable(`src-tauri/rust-toolchain.toml`이 rustfmt·clippy를 함께 깖).
- macOS에서 설치 파일까지 만들려면 Xcode.

```bash
npm ci
npm run tauri dev
```

- 브라우저만으로 화면을 보려면 `npm run dev`(가짜 백엔드 `src/mock.ts`).
- 환경 변수는 `.env.example` 참고.

## PR 전에 확인

```bash
npm run format:check && npm run typecheck && npm test && npm run check:versions
cd src-tauri && cargo test --workspace && cargo clippy --workspace --all-targets
```

## 규칙

- 코드·문구·커밋 규칙: [docs/conventions.md](docs/conventions.md)
  - 주석·문서는 개조식(1.1) — `npm test`가 검사함. AI 도구용 요약은 [AGENTS.md](AGENTS.md).
- 변경은 [CHANGELOG.md](CHANGELOG.md)의 이번 버전 칸에 적음. 사용자에게 보이는 것은 `추가`·`변경`·`수정`·`삭제`·`보안`, 개발자용은 `### 개발`(앱·릴리스 본문에는 나가지 않음).
- 비밀(키·인증서·`.env`)을 커밋하지 않음.
  - CI가 gitleaks로 전체 기록을 검사함.
  - 로컬 검사: `gitleaks git --redact .`
- 다음 작업 항목: [docs/backlog.md](docs/backlog.md)
