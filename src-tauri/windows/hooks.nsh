; Renome's additions to the Windows installer: renome-cli on the user's
; PATH when the person asks for it.
;
; A fresh install that shows its pages asks (in the system's language);
; silent and passive installs and the program's own updates do not ask
; and leave PATH as it was. Uninstalling (not for an update) takes the
; folder out again. PATH is edited through PowerShell in the registry,
; keeping its kind (REG_EXPAND_SZ, so `%USERPROFILE%` entries still
; expand) and with no length limit, which NSIS's own strings have.

!include WinMessages.nsh

!macro RENOME_PATH_EDIT ACTION
  Push $0
  System::Call 'Kernel32::SetEnvironmentVariable(t "RENOME_PATH_DIR", t "$INSTDIR")'
  System::Call 'Kernel32::SetEnvironmentVariable(t "RENOME_PATH_DO", t "${ACTION}")'
  nsExec::Exec '"$SYSDIR\WindowsPowerShell\v1.0\powershell.exe" -NoProfile -NonInteractive -ExecutionPolicy Bypass -Command "$$d = $$env:RENOME_PATH_DIR.TrimEnd([char]92); $$k = [Microsoft.Win32.Registry]::CurrentUser.CreateSubKey(\"Environment\"); $$p = [string]$$k.GetValue(\"Path\", \"\", [Microsoft.Win32.RegistryValueOptions]::DoNotExpandEnvironmentNames); $$new = @($$p -split \";\" | Where-Object { $$_.TrimEnd([char]92) -ine $$d }) -join \";\"; if ($$env:RENOME_PATH_DO -eq \"add\") { $$base = $$new.TrimEnd([char]59); if ($$base) { $$new = $$base + \";\" + $$d } else { $$new = $$d } }; if ($$new -ne $$p) { $$k.SetValue(\"Path\", $$new, [Microsoft.Win32.RegistryValueKind]::ExpandString) }"'
  Pop $0
  ; Programs opened from now on see the new PATH.
  SendMessage ${HWND_BROADCAST} ${WM_SETTINGCHANGE} 0 "STR:Environment" /TIMEOUT=5000
  Pop $0
!macroend

!macro NSIS_HOOK_POSTINSTALL
  ${If} $PassiveMode <> 1
  ${AndIf} $UpdateMode <> 1
    Push $1
    Push $2
    IfSilent renome_path_done
    ; The question in the system's language: Portuguese, Spanish, else English.
    System::Call 'Kernel32::GetUserDefaultUILanguage() i .r1'
    IntOp $1 $1 & 0x3FF
    StrCpy $2 "Add renome-cli (Renome on the command line) to your PATH, so a terminal finds it?$\r$\n$\r$\nUninstalling Renome takes it out again."
    ${If} $1 = 0x16
      StrCpy $2 "Adicionar o renome-cli (o Renome na linha de comando) ao seu PATH, para o terminal encontrá-lo?$\r$\n$\r$\nDesinstalar o Renome o tira de novo."
    ${ElseIf} $1 = 0x0A
      StrCpy $2 "¿Añadir renome-cli (Renome en la línea de comandos) a su PATH, para que la terminal lo encuentre?$\r$\n$\r$\nDesinstalar Renome lo quita de nuevo."
    ${EndIf}
    MessageBox MB_YESNO|MB_ICONQUESTION "$2" /SD IDNO IDNO renome_path_done
    !insertmacro RENOME_PATH_EDIT "add"
    renome_path_done:
    Pop $2
    Pop $1
  ${EndIf}
!macroend

!macro NSIS_HOOK_PREUNINSTALL
  ${If} $UpdateMode <> 1
    !insertmacro RENOME_PATH_EDIT "remove"
  ${EndIf}
!macroend
