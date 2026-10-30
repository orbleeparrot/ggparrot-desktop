# AI 도구 안내

- 이 저장소에서 일하는 AI 도구(Claude Code·Codex·Cursor·GitHub Copilot 등)가 읽는 파일.
- 규칙 원본은 [docs/conventions.md](docs/conventions.md). 여기와 다르면 conventions가 맞음.
- 사람용 준비·확인 순서는 [CONTRIBUTING.md](CONTRIBUTING.md).

## 문체 — 꼭 지킴

- 코드 주석·문서(README·CHANGELOG `### 개발`·`docs/`)·커밋·PR 본문은 **개조식**(conventions 1.1).
  - 끝맺음은 명사형(~함·~임·~됨·~음·~없음) 또는 명사. "~한다·~했다·~이다·~합니다·~해요"는 쓰지 않음.
  - 물음꼴 "~는가·~인가"도 쓰지 않음 — "~는지·~인지".
  - 조사는 앞말에 붙임. 코드 표기·영문·숫자 뒤도 같음(`exit_id`를, README에, 18임).
  - 가운뎃점은 앞뒤를 붙임(그리드·마틴게일).
  - 화면 문구(해요체)나 단추 이름을 옮겨 적을 때는 따옴표·대괄호로 감쌈('되돌릴 수 없어요', [시작]).
- 주석·문서는 **현재 상태**만 적음.
  - 날짜·"예전에는"·"한 번 겪은 일"·"옛 서버" 같은 이력은 쓰지 않음. 버전별 변경은 `CHANGELOG.md`에만.
  - 지금 코드와 어긋난 주석을 보면 같이 고침.
- 사용자 화면 문구는 **해요체**(conventions 2). CHANGELOG의 사용자 분류(`### 추가`·`### 변경`·`### 수정`·`### 삭제`·`### 보안`)도 해요체.

## 확인

- `npm test`가 문체 검사(`scripts/comment-style.test.mjs`)를 함께 돌림. 실패하면 `파일:줄 [규칙]`과 이유가 나옴 — 고친 뒤 다시 돌림.
- PR 전 검사 전체는 CONTRIBUTING.md의 "PR 전에 확인".

## 작업 방식

- 브랜치에서 작업하고 PR로 올림. `main`에는 스쿼시 머지만(conventions 6).
- PR 하나에 패치 버전 하나(`src-tauri/Cargo.toml` 한 곳)와 `CHANGELOG.md` 버전 칸 하나(conventions 1 "버전 올리기").
