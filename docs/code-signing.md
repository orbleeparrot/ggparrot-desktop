# 코드 서명

- 업데이트 서명과 코드 서명은 별개.
  - **업데이트 서명**(`TAURI_SIGNING_PRIVATE_KEY`): 앱이 받은 업데이트 파일이 우리가 만든 것인지 확인. 필수. 설정 끝남.
  - **코드 서명**: OS가 설치 파일·앱을 믿을지 판단. 선택. 없으면 첫 실행 때 경고가 뜸.
- 코드 서명이 없어도 앱 안 업데이트는 동작함.

## 지금 상태

| | macOS | Windows |
|---|---|---|
| 코드 서명 | 임시 서명(ad-hoc, `tauri.conf.json`의 `signingIdentity: "-"`) | 없음 |
| 첫 실행 | "확인되지 않은 개발자" 경고 → 시스템 설정 › 개인정보 보호 및 보안 › [그래도 열기] | SmartScreen "Windows의 PC 보호" → [추가 정보] › [실행] |
| 앱 안 업데이트 | 동작 | 동작 |

- 임시 서명은 Apple Silicon에서 "손상된 앱"으로 막히지 않게 하는 최소한의 서명. 개발자 확인은 아님.

## macOS: 비밀값만 넣으면 적용됨

- 준비: Apple Developer Program 가입(연 $99), **Developer ID Application** 인증서.
- `release.yml`의 "macOS 코드 서명 값(있을 때만)" 단계가 값이 있을 때만 Tauri에 넘김. 코드 수정 없음.
  - 인증서 3개(`APPLE_CERTIFICATE`·`APPLE_CERTIFICATE_PASSWORD`·`APPLE_SIGNING_IDENTITY`)만 있으면 서명만 함.
  - 공증 3개(`APPLE_ID`·`APPLE_PASSWORD`·`APPLE_TEAM_ID`)까지 있으면 서명 + 공증.
  - 서명만 하고 공증하지 않으면 경고가 여전히 뜸. 경고를 없애려면 6개 모두 필요.

### 1. 인증서 만들기

- Xcode › Settings › Accounts › 팀 선택 › Manage Certificates › [+] › **Developer ID Application**
- 또는 developer.apple.com › Certificates, IDs & Profiles › Certificates › [+] › Developer ID Application

### 2. 인증서 내보내기(.p12)

- 키체인 접근 › 로그인 › 내 인증서 › "Developer ID Application: 이름 (팀 ID)" 오른쪽 클릭 › 내보내기 › 파일 형식 `.p12`, 암호 지정

### 3. 비밀값 넣기

| 이름 | 값 |
|---|---|
| `APPLE_CERTIFICATE` | `.p12` 파일의 base64 |
| `APPLE_CERTIFICATE_PASSWORD` | 2에서 정한 암호 |
| `APPLE_SIGNING_IDENTITY` | `security find-identity -v -p codesigning` 결과의 `Developer ID Application: 이름 (팀 ID)` |
| `APPLE_ID` | Apple 계정 이메일 |
| `APPLE_PASSWORD` | 앱 전용 암호(account.apple.com › 로그인 및 보안 › 앱 전용 암호). Apple 계정 암호가 아님 |
| `APPLE_TEAM_ID` | developer.apple.com › Membership details의 Team ID(10자) |

```bash
base64 -i DeveloperID.p12 | gh secret set APPLE_CERTIFICATE -R orbleeparrot/ggparrot-desktop
```

```bash
gh secret set APPLE_CERTIFICATE_PASSWORD -R orbleeparrot/ggparrot-desktop
```

- 나머지 4개도 같은 방법(`gh secret set <이름> -R orbleeparrot/ggparrot-desktop`, 값은 물을 때 입력).
- 넣은 뒤 `.p12` 파일은 지움.

### 4. 확인

- 다음 태그 빌드의 로그에 "macOS: 인증서로 서명·공증"이 찍힘.
- 받은 앱으로 확인:

```bash
spctl -a -vv "/Applications/GGParrot.app"
```

- `source=Notarized Developer ID`가 나오면 끝.

## Windows: 비밀값만으로는 안 됨(추가 작업 필요)

- Tauri는 Windows 서명을 환경변수로 받지 않음. `tauri.conf.json`의 `bundle.windows`에 서명 방법(`signCommand` 또는 `certificateThumbprint`)을 적어야 함.
- 2023년 6월부터 OV·EV 인증서의 개인키는 하드웨어 보안 장치(HSM·USB 토큰)에만 둘 수 있음. `.pfx` 파일을 비밀값으로 넣는 옛 방식은 새 인증서로는 쓰기 어려움.
- 선택지:
  - **Azure Trusted Signing**: 월 구독. CI에서 쓰기 쉬움. 가입 가능한 국가·조건은 확인 필요.
  - **클라우드 HSM 인증서**(DigiCert KeyLocker, SSL.com eSigner 등): 제공사 도구를 `signCommand`로 호출.
- 정한 뒤 할 일:
  - `release.yml` Windows 단계에 서명 도구 설치·인증 추가
  - `signCommand`는 CI에서만 `--config`로 넘김(로컬 빌드는 서명 없이)
- EV가 아닌 인증서는 서명해도 내려받은 수가 쌓일 때까지 SmartScreen 경고가 남을 수 있음.
