; A conventional Windows install should always leave an obvious way to launch
; Grok Desktop. Tauri already creates the Start Menu shortcut; this hook makes
; the desktop shortcut deterministic for both interactive and silent installs.
!macro NSIS_HOOK_POSTINSTALL
  CreateShortcut "$DESKTOP\${PRODUCTNAME}.lnk" "$INSTDIR\${MAINBINARYNAME}.exe"
  !insertmacro SetLnkAppUserModelId "$DESKTOP\${PRODUCTNAME}.lnk"
!macroend
