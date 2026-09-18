# Microsoft Developer Studio Project File - Name="ALAudio" - Package Owner=<4>
# Microsoft Developer Studio Generated Build File, Format Version 6.00
# ** DO NOT EDIT **

# TARGTYPE "Win32 (x86) Dynamic-Link Library" 0x0102

CFG=ALAudio - Win32 Debug
!MESSAGE This is not a valid makefile. To build this project using NMAKE,
!MESSAGE use the Export Makefile command and run
!MESSAGE 
!MESSAGE NMAKE /f "ALAudio.mak".
!MESSAGE 
!MESSAGE You can specify a configuration when running NMAKE
!MESSAGE by defining the macro CFG on the command line. For example:
!MESSAGE 
!MESSAGE NMAKE /f "ALAudio.mak" CFG="ALAudio - Win32 Debug"
!MESSAGE 
!MESSAGE Possible choices for configuration are:
!MESSAGE 
!MESSAGE "ALAudio - Win32 Release" (based on "Win32 (x86) Dynamic-Link Library")
!MESSAGE "ALAudio - Win32 Debug" (based on "Win32 (x86) Dynamic-Link Library")
!MESSAGE 

# Begin Project
# PROP AllowPerConfigDependencies 0
# PROP Scc_ProjName ""$/Warfare/ALAudio/Src", IKIAAAAA"
# PROP Scc_LocalPath "."
CPP=cl.exe
MTL=midl.exe
RSC=rc.exe

!IF  "$(CFG)" == "ALAudio - Win32 Release"

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
# ADD BASE CPP /nologo /MT /W3 /GX /O2 /D "WIN32" /D "NDEBUG" /D "_WINDOWS" /YX /FD /c
# ADD CPP /nologo /Zp4 /MD /W4 /GX /Ox /Ot /Oa /Og /Oi /I "..\..\Core\Inc" /I "..\..\Engine\Inc" /I "..\..\OpenAL\Inc" /I "..\Inc" /I "..\..\Vorbis\Inc" /I "..\..\EAAudioCodec\Include" /I "..\..\metoolkit\include" /D ALAUDIO_API=__declspec(dllexport) /D "_WINDOWS" /D "NDEBUG" /D "UNICODE" /D "_UNICODE" /D "WIN32" /FAs /FR /YX"ALAudio.h" /FD /c
# ADD BASE MTL /nologo /D "NDEBUG" /mktyplib203 /win32
# ADD MTL /nologo /D "NDEBUG" /mktyplib203 /win32
# ADD BASE RSC /l 0x409 /d "NDEBUG"
# ADD RSC /l 0x409 /d "NDEBUG"
BSC32=bscmake.exe
# ADD BASE BSC32 /nologo
# ADD BSC32 /nologo
LINK32=link.exe
# ADD BASE LINK32 kernel32.lib user32.lib gdi32.lib winspool.lib comdlg32.lib advapi32.lib shell32.lib ole32.lib oleaut32.lib uuid.lib odbc32.lib odbccp32.lib /nologo /subsystem:windows /dll /machine:I386
# ADD LINK32 ..\..\Core\Lib\Core.lib ..\..\Engine\Lib\Engine.lib kernel32.lib user32.lib gdi32.lib winspool.lib comdlg32.lib advapi32.lib shell32.lib ole32.lib oleaut32.lib uuid.lib odbc32.lib odbccp32.lib OpenAL32.lib eax.lib eaxguid.lib ogg.lib vorbis.lib vorbisfile.lib /nologo /subsystem:windows /dll /machine:I386 /out:"..\..\System\ALAudio.dll" /libpath:"..\..\Vorbis\Lib\release" /libpath:"..\..\OpenAL\Lib\\" /libpath:"..\..\EAAudioCodec\lib"

!ELSEIF  "$(CFG)" == "ALAudio - Win32 Debug"

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
# ADD BASE CPP /nologo /MTd /W3 /Gm /GX /ZI /Od /D "WIN32" /D "_DEBUG" /D "_WINDOWS" /YX /FD /GZ /c
# ADD CPP /nologo /Zp4 /MDd /W4 /WX /Gm /GX /ZI /Od /I "..\..\Core\Inc" /I "..\..\Engine\Inc" /I "..\..\OpenAL\Inc" /I "..\Inc" /I "..\..\Vorbis\Inc" /I "..\..\EAAudioCodec\Include" /I "..\..\metoolkit\include" /D "_WINDOWS" /D ALAUDIO_API=__declspec(dllexport) /D "UNICODE" /D "_UNICODE" /D "_DEBUG" /D "WIN32" /D "DEBUG" /FAs /FR /FD /c
# SUBTRACT CPP /YX
# ADD BASE MTL /nologo /D "_DEBUG" /mktyplib203 /win32
# ADD MTL /nologo /D "_DEBUG" /mktyplib203 /win32
# ADD BASE RSC /l 0x409 /d "_DEBUG"
# ADD RSC /l 0x409 /d "_DEBUG"
BSC32=bscmake.exe
# ADD BASE BSC32 /nologo
# ADD BSC32 /nologo
LINK32=link.exe
# ADD BASE LINK32 kernel32.lib user32.lib gdi32.lib winspool.lib comdlg32.lib advapi32.lib shell32.lib ole32.lib oleaut32.lib uuid.lib odbc32.lib odbccp32.lib /nologo /subsystem:windows /dll /debug /machine:I386 /pdbtype:sept
# ADD LINK32 ..\..\Core\Lib\Core.lib ..\..\Engine\Lib\Engine.lib kernel32.lib user32.lib gdi32.lib winspool.lib comdlg32.lib advapi32.lib shell32.lib ole32.lib oleaut32.lib uuid.lib odbc32.lib odbccp32.lib OpenAL32.lib eax.lib eaxguid.lib ogg.lib vorbis.lib vorbisfile.lib /nologo /subsystem:windows /dll /debug /machine:I386 /nodefaultlib:"LIBCMT" /out:"..\..\System\ALAudio.dll" /pdbtype:sept /libpath:"..\..\Vorbis\Lib\debug" /libpath:"..\..\OpenAL\Lib\\" /libpath:"..\..\EAAudioCodec\lib"

!ENDIF 

# Begin Target

# Name "ALAudio - Win32 Release"
# Name "ALAudio - Win32 Debug"
# Begin Group "Inc"

# PROP Default_Filter ""
# Begin Source File

SOURCE=..\Inc\ALAudio.h
# End Source File
# Begin Source File

SOURCE=..\Inc\ALAudioSubsystem.h
# End Source File
# End Group
# Begin Group "Src"

# PROP Default_Filter ""
# Begin Source File

SOURCE=.\ALAudio.cpp
# End Source File
# Begin Source File

SOURCE=.\ALAudioPrivate.h
# End Source File
# Begin Source File

SOURCE=.\ALAudioSubsystem.cpp
# End Source File
# End Group
# End Target
# End Project
