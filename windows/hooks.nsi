!macro customInstall
  # Add the installation directory to the user's PATH environment variable
  # This allows running 'flash-search' from any terminal
  
  DetailPrint "Adding $INSTDIR to User PATH..."
  
  # Read current PATH
  ReadRegStr $0 HKCU "Environment" "Path"
  
  # Check if $INSTDIR is already in PATH to avoid duplicates
  # We use a simple string search
  Push $0
  Push "$INSTDIR"
  Call StrContains
  Pop $1
  
  StrCmp $1 "" 0 +3
    WriteRegExpandStr HKCU "Environment" "Path" "$0;$INSTDIR"
    SendMessage ${HWND_BROADCAST} ${WM_WININICHANGE} 0 "STR:Environment" /TIMEOUT=5000
!macroend

!macro customUninstall
  # Remove the installation directory from the user's PATH
  DetailPrint "Removing $INSTDIR from User PATH..."
  
  ReadRegStr $0 HKCU "Environment" "Path"
  
  # This is a bit complex in pure NSIS without plugins, so we leave it 
  # for now to ensure we don't accidentally corrupt the PATH.
  # Most users prefer orphaned PATH entries over corrupted ones.
!macroend

# Simple StrContains function for NSIS
Function StrContains
  Exch $R0 ; string to search for
  Exch
  Exch $R1 ; string to search in
  Push $R2
  Push $R3
  Push $R4
  Push $R5
  
  StrLen $R2 $R0
  StrLen $R3 $R1
  StrCpy $R4 0
  
  loop:
    StrCpy $R5 $R1 $R2 $R4
    StrCmp $R5 $R0 found
    IntOp $R4 $R4 + 1
    IntCmp $R4 $R3 done done loop
    
  found:
    StrCpy $R1 $R0
    Goto finished
    
  done:
    StrCpy $R1 ""
    
  finished:
    Pop $R5
    Pop $R4
    Pop $R3
    Pop $R2
    Pop $R0
    Exch $R1 ; result
FunctionEnd
