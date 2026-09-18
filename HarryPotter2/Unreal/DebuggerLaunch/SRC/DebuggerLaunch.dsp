# Microsoft Developer Studio Project File - Name="DebuggerLaunch" - Package Owner=<4>
# Microsoft Developer Studio Generated Build File, Format Version 6.00
# ** DO NOT EDIT **

# TARGTYPE "Win32 (x86) Application" 0x0101

CFG=DebuggerLaunch - Win32 Release
!MESSAGE This is not a valid makefile. To build this project using NMAKE,
!MESSAGE use the Export Makefile command and run
!MESSAGE 
!MESSAGE NMAKE /f "DebuggerLaunch.mak".
!MESSAGE 
!MESSAGE You can specify a configuration when running NMAKE
!MESSAGE by defining the macro CFG on the command line. For example:
!MESSAGE 
!MESSAGE NMAKE /f "DebuggerLaunch.mak" CFG="DebuggerLaunch - Win32 Release"
!MESSAGE 
!MESSAGE Possible choices for configuration are:
!MESSAGE 
!MESSAGE "DebuggerLaunch - Win32 Release" (based on "Win32 (x86) Application")
!MESSAGE "DebuggerLaunch - Win32 Debug" (based on "Win32 (x86) Application")
!MESSAGE 

# Begin Project
# PROP AllowPerConfigDependencies 0
# PROP Scc_ProjName ""$/Warfare/DebuggerLaunch", FAAAAAAA"
# PROP Scc_LocalPath ".."
CPP=cl.exe
MTL=midl.exe
RSC=rc.exe

!IF  "$(CFG)" == "DebuggerLaunch - Win32 Release"

# PROP BASE Use_MFC 0
# PROP BASE Use_Debug_Libraries 0
# PROP BASE Output_Dir "Release"
# PROP BASE Intermediate_Dir "Release"
# PROP BASE Target_Dir ""
# PROP Use_MFC 0
# PROP Use_Debug_Libraries 0
# PROP Output_Dir "..\Lib"
# PROP Intermediate_Dir "Release"
# PROP Ignore_Export_Lib 0
# PROP Target_Dir ""
# ADD BASE CPP /nologo /W3 /GX /O2 /D "WIN32" /D "NDEBUG" /D "_WINDOWS" /YX /FD /c
# ADD CPP /nologo /Zp4 /MT /W4 /WX /GX /Ob2 /I "..\..\Core\Inc" /I "..\..\Engine\Inc" /I "..\..\Window\Inc" /I "." /I "..\Inc" /I "..\..\metoolkit\include" /D "_WINDOWS" /D "NDEBUG" /D "UNICODE" /D "_UNICODE" /D _WIN32_IE=0x0200 /D "WIN32" /FR /Yu"DebuggerLaunchPrivate.h" /FD /Zm256 /c
# ADD BASE MTL /nologo /D "NDEBUG" /mktyplib203 /o "NUL" /win32
# ADD MTL /nologo /D "NDEBUG" /mktyplib203 /o "NUL" /win32
# ADD BASE RSC /l 0x409 /d "NDEBUG"
# ADD RSC /l 0x409 /d "NDEBUG"
BSC32=bscmake.exe
# ADD BASE BSC32 /nologo
# ADD BSC32 /nologo
LINK32=link.exe
# ADD BASE LINK32 kernel32.lib user32.lib gdi32.lib winspool.lib comdlg32.lib advapi32.lib shell32.lib ole32.lib oleaut32.lib uuid.lib odbc32.lib odbccp32.lib /nologo /subsystem:windows /machine:I386
# ADD LINK32 gdi32.lib comctl32.lib comdlg32.lib ..\..\Window\Lib\Window.lib ..\..\Core\Lib\Core.lib ..\..\Engine\Lib\Engine.lib user32.lib kernel32.lib gdi32.lib advapi32.lib shell32.lib /nologo /base:"0x10900000" /subsystem:windows /incremental:yes /machine:I386 /out:"..\..\System\UDebugger.exe"
# SUBTRACT LINK32 /debug

!ELSEIF  "$(CFG)" == "DebuggerLaunch - Win32 Debug"

# PROP BASE Use_MFC 0
# PROP BASE Use_Debug_Libraries 0
# PROP BASE Output_Dir "DebuggerLaunch___Win32_Debug"
# PROP BASE Intermediate_Dir "DebuggerLaunch___Win32_Debug"
# PROP BASE Ignore_Export_Lib 0
# PROP BASE Target_Dir ""
# PROP Use_MFC 0
# PROP Use_Debug_Libraries 0
# PROP Output_Dir "..\Lib"
# PROP Intermediate_Dir "Debug"
# PROP Ignore_Export_Lib 0
# PROP Target_Dir ""
# ADD BASE CPP /nologo /Zp4 /MD /W4 /WX /vd0 /GX /O2 /Ob2 /I "..\..\Core\Inc" /I "..\..\Engine\Inc" /I "..\..\Window\Inc" /I "." /D "_WINDOWS" /D "NDEBUG" /D "UNICODE" /D "_UNICODE" /D "WIN32" /Yu"DebuggerLaunchPrivate.h" /FD /Zm256 /c
# ADD CPP /nologo /Zp4 /MD /W4 /Gm /GX /ZI /Od /I "..\..\Core\Inc" /I "..\..\Engine\Inc" /I "..\..\Window\Inc" /I "." /I "..\Inc" /I "..\..\metoolkit\include" /D "_WINDOWS" /D "UNICODE" /D "_UNICODE" /D "_DEBUG" /D _WIN32_IE=0x0200 /D "WIN32" /Fr /Yu"DebuggerLaunchPrivate.h" /FD /Zm256 /c
# ADD BASE MTL /nologo /D "NDEBUG" /mktyplib203 /o "NUL" /win32
# ADD MTL /nologo /D "NDEBUG" /mktyplib203 /o "NUL" /win32
# ADD BASE RSC /l 0x409 /d "NDEBUG"
# ADD RSC /l 0x409 /d "NDEBUG"
BSC32=bscmake.exe
# ADD BASE BSC32 /nologo
# ADD BSC32 /nologo
LINK32=link.exe
# ADD BASE LINK32 ..\..\Window\Lib\Window.lib ..\..\Core\Lib\Core.lib ..\..\Engine\Lib\Engine.lib user32.lib kernel32.lib gdi32.lib advapi32.lib shell32.lib /nologo /base:"0x10900000" /subsystem:windows /incremental:yes /machine:I386 /out:"..\..\System\UW.exe"
# ADD LINK32 comctl32.lib comdlg32.lib ..\..\Window\Lib\Window.lib ..\..\Core\Lib\Core.lib ..\..\Engine\Lib\Engine.lib user32.lib kernel32.lib gdi32.lib advapi32.lib shell32.lib /nologo /base:"0x10900000" /subsystem:windows /incremental:yes /debug /machine:I386 /out:"..\..\System\UDebugger.exe" /pdbtype:sept

!ENDIF 

# Begin Target

# Name "DebuggerLaunch - Win32 Release"
# Name "DebuggerLaunch - Win32 Debug"
# Begin Group "Src"

# PROP Default_Filter "*.cpp;*.h"
# Begin Source File

SOURCE=.\DebuggerLaunch.cpp
# ADD CPP /Yc"DebuggerLaunchPrivate.h"
# End Source File
# Begin Source File

SOURCE=.\DebuggerLaunchPrivate.h
# End Source File
# Begin Source File

SOURCE=.\Res\DebuggerLaunchRes.h
# End Source File
# Begin Source File

SOURCE=.\UnDebuggerLogic.cpp
# End Source File
# Begin Source File

SOURCE=.\UnDebuggerWindow.cpp
# End Source File
# End Group
# Begin Group "Res"

# PROP Default_Filter "*.rc"
# Begin Source File

SOURCE=.\Res\DebuggerLaunchRes.rc
# End Source File
# Begin Source File

SOURCE=.\Res\Unreal.ico
# End Source File
# End Group
# Begin Group "Inc"

# PROP Default_Filter "*.h"
# Begin Source File

SOURCE=..\Inc\COMCTHLP.H
# End Source File
# Begin Source File

SOURCE=..\Inc\UnDebuggerWindow.h
# End Source File
# End Group
# Begin Source File

SOURCE=.\Res\cursor1.cur
# End Source File
# Begin Source File

SOURCE=.\Res\ID_DEBUG_TOOLBAR.bmp
# End Source File
# End Target
# End Project
