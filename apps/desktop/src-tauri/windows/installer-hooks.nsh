!macro NSIS_HOOK_PREUNINSTALL
  IfSilent silent_uninstall interactive_uninstall

  interactive_uninstall:
    MessageBox MB_YESNO|MB_ICONQUESTION|MB_DEFBUTTON2 "是否同时删除 Polaris 默认应用数据？$\r$\n$\r$\n选择“否”会保留学习数据，重装后可继续使用。若数据库曾改选到其他位置，请先在应用设置中使用“全部清除”。" IDYES delete_default_data IDNO preserve_data

  silent_uninstall:
    IfFileExists "$APPDATA\app.polaris.desktop\delete-on-uninstall.marker" delete_default_data preserve_data

  delete_default_data:
    DetailPrint "Deleting user-confirmed Polaris default application data"
    RMDir /r "$APPDATA\app.polaris.desktop"
    DeleteRegValue HKCU "Software\Microsoft\Windows\CurrentVersion\Run" "Polaris"
    nsExec::Exec 'cmdkey.exe /delete:Polaris/LLM/Fast'
    nsExec::Exec 'cmdkey.exe /delete:Polaris/LLM/Strong'
    nsExec::Exec 'cmdkey.exe /delete:Polaris/Embedding'

  preserve_data:
!macroend
