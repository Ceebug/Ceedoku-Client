!macro customInstall
  WriteRegStr HKCU "Software\Microsoft\Windows\CurrentVersion\App Paths\ceedoku.exe" "" "$INSTDIR\Ceedoku Client.exe"
!macroend

!macro customUnInstall
  DeleteRegKey HKCU "Software\Microsoft\Windows\CurrentVersion\App Paths\ceedoku.exe"
!macroend