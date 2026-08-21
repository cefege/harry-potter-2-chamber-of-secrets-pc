# Microsoft Developer Studio Project File - Name="UnrealEd" - Package Owner=<4>
# Microsoft Developer Studio Generated Build File, Format Version 6.00
# ** DO NOT EDIT **

# TARGTYPE "Win32 (x86) Application" 0x0101

CFG=UnrealEd - Win32 Debug
!MESSAGE This is not a valid makefile. To build this project using NMAKE,
!MESSAGE use the Export Makefile command and run
!MESSAGE 
!MESSAGE NMAKE /f "UnrealEd.mak".
!MESSAGE 
!MESSAGE You can specify a configuration when running NMAKE
!MESSAGE by defining the macro CFG on the command line. For example:
!MESSAGE 
!MESSAGE NMAKE /f "UnrealEd.mak" CFG="UnrealEd - Win32 Debug"
!MESSAGE 
!MESSAGE Possible choices for configuration are:
!MESSAGE 
!MESSAGE "UnrealEd - Win32 Release" (based on "Win32 (x86) Application")
!MESSAGE "UnrealEd - Win32 Debug" (based on "Win32 (x86) Application")
!MESSAGE 

# Begin Project
# PROP AllowPerConfigDependencies 0
# PROP Scc_ProjName ""$/unreal/UnrealEd", TNHAAAAA"
# PROP Scc_LocalPath ".."
CPP=cl.exe
MTL=midl.exe
RSC=rc.exe

!IF  "$(CFG)" == "UnrealEd - Win32 Release"

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
# ADD CPP /nologo /Zp4 /MD /vd0 /GX /O2 /Ob2 /I "..\..\Core\Inc" /I "..\..\Engine\Inc" /I "..\..\Window\Inc" /I "..\..\Editor\Inc" /I "..\Inc" /D "NDEBUG" /D "_WINDOWS" /D "UNICODE" /D "_UNICODE" /D "WIN32" /D _WIN32_IE=0x0200 /FD /c
# SUBTRACT CPP /WX /YX /Yc /Yu
# ADD BASE MTL /nologo /D "NDEBUG" /mktyplib203 /o "NUL" /win32
# ADD MTL /nologo /D "NDEBUG" /mktyplib203 /o "NUL" /win32
# ADD BASE RSC /l 0x409 /d "NDEBUG"
# ADD RSC /l 0x409 /d "NDEBUG"
BSC32=bscmake.exe
# ADD BASE BSC32 /nologo
# ADD BSC32 /nologo
LINK32=link.exe
# ADD BASE LINK32 kernel32.lib user32.lib gdi32.lib winspool.lib comdlg32.lib advapi32.lib shell32.lib ole32.lib oleaut32.lib uuid.lib /nologo /subsystem:windows /machine:I386
# ADD LINK32 comctl32.lib comdlg32.lib ..\..\Core\Lib\Core.lib ..\..\Engine\Lib\Engine.lib ..\..\Window\Lib\Window.lib ..\..\Editor\Lib\Editor.lib user32.lib kernel32.lib gdi32.lib advapi32.lib shell32.lib /nologo /base:"0x10E00000" /subsystem:windows /incremental:yes /machine:I386 /out:"..\..\System\UnrealEd.exe"
# SUBTRACT LINK32 /debug

!ELSEIF  "$(CFG)" == "UnrealEd - Win32 Debug"

# PROP BASE Use_MFC 0
# PROP BASE Use_Debug_Libraries 1
# PROP BASE Output_Dir "Debug"
# PROP BASE Intermediate_Dir "Debug"
# PROP BASE Target_Dir ""
# PROP Use_MFC 0
# PROP Use_Debug_Libraries 1
# PROP Output_Dir "..\Lib"
# PROP Intermediate_Dir "Debug"
# PROP Ignore_Export_Lib 0
# PROP Target_Dir ""
# ADD BASE CPP /nologo /W3 /Gm /GX /Zi /Od /D "WIN32" /D "_DEBUG" /D "_WINDOWS" /YX /FD /c
# ADD CPP /nologo /Zp4 /MDd /W4 /vd0 /GX /ZI /Od /I "..\..\Core\Inc" /I "..\..\Engine\Inc" /I "..\..\Window\Inc" /I "..\..\Editor\Inc" /I "..\Inc" /D "_DEBUG" /D "_WINDOWS" /D "UNICODE" /D "_UNICODE" /D "WIN32" /D _WIN32_IE=0x0200 /FR /FD /c
# SUBTRACT CPP /WX /YX
# ADD BASE MTL /nologo /D "_DEBUG" /mktyplib203 /o "NUL" /win32
# ADD MTL /nologo /D "_DEBUG" /mktyplib203 /o "NUL" /win32
# ADD BASE RSC /l 0x409 /d "_DEBUG"
# ADD RSC /l 0x409 /d "_DEBUG"
BSC32=bscmake.exe
# ADD BASE BSC32 /nologo
# ADD BSC32 /nologo
LINK32=link.exe
# ADD BASE LINK32 kernel32.lib user32.lib gdi32.lib winspool.lib comdlg32.lib advapi32.lib shell32.lib ole32.lib oleaut32.lib uuid.lib /nologo /subsystem:windows /debug /machine:I386 /pdbtype:sept
# ADD LINK32 comctl32.lib comdlg32.lib ..\..\Core\Lib\Core.lib ..\..\Engine\Lib\Engine.lib ..\..\Window\Lib\Window.lib ..\..\Editor\Lib\Editor.lib user32.lib kernel32.lib gdi32.lib advapi32.lib shell32.lib /nologo /base:"0x10E00000" /subsystem:windows /debug /machine:I386 /out:"..\..\System\UnrealEd.exe" /pdbtype:sept

!ENDIF 

# Begin Target

# Name "UnrealEd - Win32 Release"
# Name "UnrealEd - Win32 Debug"
# Begin Group "Src"

# PROP Default_Filter "*.cpp"
# Begin Source File

SOURCE=.\BuildSheet.cpp
# End Source File
# Begin Source File

SOURCE=.\Main.cpp
# End Source File
# Begin Source File

SOURCE=.\SurfPropSheet.cpp
# End Source File
# End Group
# Begin Group "Inc"

# PROP Default_Filter "*.h"
# Begin Source File

SOURCE=..\Inc\BottomBar.h
# End Source File
# Begin Source File

SOURCE=..\Inc\Browser.h
# End Source File
# Begin Source File

SOURCE=..\Inc\BrowserActor.h
# End Source File
# Begin Source File

SOURCE=..\Inc\BrowserGroup.h
# End Source File
# Begin Source File

SOURCE=..\Inc\BrowserMaster.h
# End Source File
# Begin Source File

SOURCE=..\Inc\BrowserMesh.h
# End Source File
# Begin Source File

SOURCE=..\Inc\BrowserMusic.h
# End Source File
# Begin Source File

SOURCE=..\Inc\BrowserSound.h
# End Source File
# Begin Source File

SOURCE=..\Inc\BrowserTexture.h
# End Source File
# Begin Source File

SOURCE=..\Inc\BuildSheet.h
# End Source File
# Begin Source File

SOURCE=..\Inc\ButtonBar.h
# End Source File
# Begin Source File

SOURCE=..\Inc\CodeFrame.h
# End Source File
# Begin Source File

SOURCE=..\Inc\DlgAddSpecial.h
# End Source File
# Begin Source File

SOURCE=..\Inc\DlgBrushBuilder.h
# End Source File
# Begin Source File

SOURCE=..\Inc\DlgBrushImport.h
# End Source File
# Begin Source File

SOURCE=..\Inc\DlgBuildOptions.h
# End Source File
# Begin Source File

SOURCE=..\Inc\DlgMapImport.h
# End Source File
# Begin Source File

SOURCE=..\Inc\DlgProgress.h
# End Source File
# Begin Source File

SOURCE=..\Inc\DlgRename.h
# End Source File
# Begin Source File

SOURCE=..\Inc\DlgScaleLights.h
# End Source File
# Begin Source File

SOURCE=..\Inc\DlgSearchActors.h
# End Source File
# Begin Source File

SOURCE=..\Inc\DlgTexProp.h
# End Source File
# Begin Source File

SOURCE=..\Inc\DlgTexReplace.h
# End Source File
# Begin Source File

SOURCE=..\Inc\DlgViewportConfig.h
# End Source File
# Begin Source File

SOURCE=..\Inc\Extern.h
# End Source File
# Begin Source File

SOURCE=..\Inc\MatineeSheet.h
# End Source File
# Begin Source File

SOURCE=..\Inc\MRUList.h
# End Source File
# Begin Source File

SOURCE=.\Res\resource.h
# End Source File
# Begin Source File

SOURCE=..\Inc\SurfPropSheet.h
# End Source File
# Begin Source File

SOURCE=..\Inc\TopBar.h
# End Source File
# Begin Source File

SOURCE=..\Inc\TwoDeeShapeEditor.h
# End Source File
# Begin Source File

SOURCE=..\Inc\ViewportFrame.h
# End Source File
# End Group
# Begin Group "Res"

# PROP Default_Filter ""
# Begin Group "UDelaunayRes"

# PROP Default_Filter ""
# End Group
# Begin Source File

SOURCE=.\Res\bb_grid1.bmp
# End Source File
# Begin Source File

SOURCE=.\Res\bb_lock1.bmp
# End Source File
# Begin Source File

SOURCE=.\Res\bb_log_w.bmp
# End Source File
# Begin Source File

SOURCE=.\Res\bb_rotat.bmp
# End Source File
# Begin Source File

SOURCE=.\Res\bb_vtx_s.bmp
# End Source File
# Begin Source File

SOURCE=.\Res\bb_zoomc.bmp
# End Source File
# Begin Source File

SOURCE=.\Res\bmp00001.bmp
# End Source File
# Begin Source File

SOURCE=.\Res\bmp00002.bmp
# End Source File
# Begin Source File

SOURCE=.\Res\bmp00003.bmp
# End Source File
# Begin Source File

SOURCE=.\Res\bmp00004.bmp
# End Source File
# Begin Source File

SOURCE=.\Res\bmp00005.bmp
# End Source File
# Begin Source File

SOURCE=.\Res\bmp00006.bmp
# End Source File
# Begin Source File

SOURCE=.\Res\bmp00007.bmp
# End Source File
# Begin Source File

SOURCE=.\res\bmp00008.bmp
# End Source File
# Begin Source File

SOURCE=.\res\bmp00009.bmp
# End Source File
# Begin Source File

SOURCE=.\res\bmp00010.bmp
# End Source File
# Begin Source File

SOURCE=.\res\bmp00011.bmp
# End Source File
# Begin Source File

SOURCE=.\res\bmp00012.bmp
# End Source File
# Begin Source File

SOURCE=.\res\bmp00013.bmp
# End Source File
# Begin Source File

SOURCE=.\res\bmp00014.bmp
# End Source File
# Begin Source File

SOURCE=.\res\bmp00015.bmp
# End Source File
# Begin Source File

SOURCE=.\res\bmp00016.bmp
# End Source File
# Begin Source File

SOURCE=.\res\bmp00017.bmp
# End Source File
# Begin Source File

SOURCE=.\res\bmp00018.bmp
# End Source File
# Begin Source File

SOURCE=.\res\bmp00019.bmp
# End Source File
# Begin Source File

SOURCE=.\Res\bmp00022.bmp
# End Source File
# Begin Source File

SOURCE=.\res\bmp00023.bmp
# End Source File
# Begin Source File

SOURCE=.\res\bmp00024.bmp
# End Source File
# Begin Source File

SOURCE=.\res\bmp00025.bmp
# End Source File
# Begin Source File

SOURCE=.\res\bmp00026.bmp
# End Source File
# Begin Source File

SOURCE=.\res\bmp00027.bmp
# End Source File
# Begin Source File

SOURCE=.\res\bmp00028.bmp
# End Source File
# Begin Source File

SOURCE=.\res\bmp2dse_.bmp
# End Source File
# Begin Source File

SOURCE=.\res\browsers.bmp
# End Source File
# Begin Source File

SOURCE=.\res\browsert.bmp
# End Source File
# Begin Source File

SOURCE=.\Res\cf_toolb.bmp
# End Source File
# Begin Source File

SOURCE=.\Res\Icon.bmp
# End Source File
# Begin Source File

SOURCE=.\res\idbm_2ds.bmp
# End Source File
# Begin Source File

SOURCE=.\res\idbm_add.bmp
# End Source File
# Begin Source File

SOURCE=.\Res\idbm_bac.bmp
# End Source File
# Begin Source File

SOURCE=.\res\idbm_bui.bmp
# End Source File
# Begin Source File

SOURCE=.\res\idbm_buildall.bmp
# End Source File
# Begin Source File

SOURCE=.\res\idbm_del.bmp
# End Source File
# Begin Source File

SOURCE=.\res\idbm_dow.bmp
# End Source File
# Begin Source File

SOURCE=.\res\idbm_edi.bmp
# End Source File
# Begin Source File

SOURCE=.\Res\idbm_exe.bmp
# End Source File
# Begin Source File

SOURCE=.\res\idbm_fil.bmp
# End Source File
# Begin Source File

SOURCE=.\Res\idbm_for.bmp
# End Source File
# Begin Source File

SOURCE=.\res\idbm_mes.bmp
# End Source File
# Begin Source File

SOURCE=.\res\idbm_mus.bmp
# End Source File
# Begin Source File

SOURCE=.\res\idbm_new.bmp
# End Source File
# Begin Source File

SOURCE=.\res\idbm_pla.bmp
# End Source File
# Begin Source File

SOURCE=.\res\idbm_sur.bmp
# End Source File
# Begin Source File

SOURCE=.\res\idbm_tex.bmp
# End Source File
# Begin Source File

SOURCE=.\res\idbm_unr.bmp
# End Source File
# Begin Source File

SOURCE=.\Res\idbm_vie.bmp
# End Source File
# Begin Source File

SOURCE=.\Res\Logo.bmp
# End Source File
# Begin Source File

SOURCE=.\Res\TOOLBAR.BMP
# End Source File
# Begin Source File

SOURCE=.\Res\toolbar1.bmp
# End Source File
# Begin Source File

SOURCE=.\Res\Unreal.ico
# End Source File
# Begin Source File

SOURCE=.\Res\UnrealEd.ico
# End Source File
# Begin Source File

SOURCE=.\Res\UnrealEd.rc
# End Source File
# End Group
# End Target
# End Project
