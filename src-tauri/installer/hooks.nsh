; GGP: 설치·제거 전에 실행 중인 껄무새를 끔.
; Tauri의 기본 확인(Restart Manager)은 앱에 표준 종료 요청을 보내는데, 껄무새는 창을 닫으면 트레이로 숨어
; 계속 도는 앱이라 그 요청으로 꺼지지 않음 → "종료하지 못했습니다"로 설치·제거가 멈춤.
; 그래서 기본 확인보다 먼저 프로세스를 찾아, 화면이 있는 설치·제거면 한 번 묻고 끔.
; 업데이트(/UPDATE)·자동(passive·silent) 실행은 묻지 않고 끔(앱 안 업데이트는 앱이 먼저 꺼진 뒤 실행됨).

!macro GGP_CLOSE_RUNNING_APP
  !define GGP_ID ${__LINE__} ; 펼칠 때마다 다른 이름표
  nsis_tauri_utils::FindProcessCurrentUser "${MAINBINARYNAME}.exe"
  Pop $R0
  ${If} $R0 = 0
    IfSilent ggp_kill_${GGP_ID}
    ${If} $PassiveMode = 1
      Goto ggp_kill_${GGP_ID}
    ${EndIf}
    MessageBox MB_OKCANCEL|MB_ICONEXCLAMATION "껄무새가 실행 중이에요. 끄고 계속할까요?$\n실행 중인 매크로도 멈춰요." IDOK ggp_kill_${GGP_ID}
    Abort
    ggp_kill_${GGP_ID}:
      nsis_tauri_utils::KillProcessCurrentUser "${MAINBINARYNAME}.exe"
      Pop $R0
      Sleep 1000
  ${EndIf}
  !undef GGP_ID
!macroend

!macro NSIS_HOOK_PREINSTALL
  !insertmacro GGP_CLOSE_RUNNING_APP
!macroend

!macro NSIS_HOOK_PREUNINSTALL
  !insertmacro GGP_CLOSE_RUNNING_APP
!macroend
