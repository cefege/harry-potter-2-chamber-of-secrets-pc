# Microsoft Developer Studio Project File - Name="Engine" - Package Owner=<4>
# Microsoft Developer Studio Generated Build File, Format Version 6.00
# ** DO NOT EDIT **

# TARGTYPE "Win32 (x86) Dynamic-Link Library" 0x0102

CFG=Engine - Win32 Debug
!MESSAGE This is not a valid makefile. To build this project using NMAKE,
!MESSAGE use the Export Makefile command and run
!MESSAGE 
!MESSAGE NMAKE /f "Engine.mak".
!MESSAGE 
!MESSAGE You can specify a configuration when running NMAKE
!MESSAGE by defining the macro CFG on the command line. For example:
!MESSAGE 
!MESSAGE NMAKE /f "Engine.mak" CFG="Engine - Win32 Debug"
!MESSAGE 
!MESSAGE Possible choices for configuration are:
!MESSAGE 
!MESSAGE "Engine - Win32 Release" (based on "Win32 (x86) Dynamic-Link Library")
!MESSAGE "Engine - Win32 Debug" (based on "Win32 (x86) Dynamic-Link Library")
!MESSAGE 

# Begin Project
# PROP AllowPerConfigDependencies 0
# PROP Scc_ProjName ""$/Demo/unreal/Engine", "
# PROP Scc_LocalPath ".."
CPP=cl.exe
MTL=midl.exe
RSC=rc.exe

!IF  "$(CFG)" == "Engine - Win32 Release"

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
# ADD CPP /nologo /Zp4 /MD /W4 /WX /vd0 /GX /O2 /Ob2 /I "..\..\Core\Inc" /I "..\Inc" /D ENGINE_API=__declspec(dllexport) /D "_WINDOWS" /D "NDEBUG" /D "UNICODE" /D "_UNICODE" /D "WIN32" /Yu"EnginePrivate.h" /FD /Zm256 /c
# SUBTRACT CPP /Fr
# ADD BASE MTL /nologo /D "NDEBUG" /mktyplib203 /o "NUL" /win32
# ADD MTL /nologo /D "NDEBUG" /mktyplib203 /o "NUL" /win32
# ADD BASE RSC /l 0x409 /d "NDEBUG"
# ADD RSC /l 0x409 /d "NDEBUG"
BSC32=bscmake.exe
# ADD BASE BSC32 /nologo
# ADD BSC32 /nologo
LINK32=link.exe
# ADD BASE LINK32 kernel32.lib user32.lib gdi32.lib winspool.lib comdlg32.lib advapi32.lib shell32.lib ole32.lib oleaut32.lib uuid.lib odbc32.lib odbccp32.lib /nologo /subsystem:windows /dll /machine:I386
# ADD LINK32 ..\..\Core\Lib\Core.lib ..\Lib\s3tc.lib ..\lib\encvag.lib /nologo /base:"0x10300000" /subsystem:windows /dll /incremental:yes /machine:I386 /nodefaultlib:"LIBC" /out:"..\..\System\Engine.dll"
# SUBTRACT LINK32 /pdb:none

!ELSEIF  "$(CFG)" == "Engine - Win32 Debug"

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
# ADD BASE CPP /nologo /MTd /W3 /Gm /GX /Zi /Od /D "WIN32" /D "_DEBUG" /D "_WINDOWS" /YX /FD /c
# ADD CPP /nologo /Zp4 /MDd /W4 /WX /vd0 /GX /ZI /Od /I "..\..\Core\Inc" /I "..\Inc" /D "_WINDOWS" /D ENGINE_API=__declspec(dllexport) /D "UNICODE" /D "_UNICODE" /D "_DEBUG" /D "WIN32" /FR /Yu"EnginePrivate.h" /FD /D /Zm256 /c
# ADD BASE MTL /nologo /D "_DEBUG" /mktyplib203 /o "NUL" /win32
# ADD MTL /nologo /D "_DEBUG" /mktyplib203 /o "NUL" /win32
# ADD BASE RSC /l 0x409 /d "_DEBUG"
# ADD RSC /l 0x409 /d "_DEBUG"
BSC32=bscmake.exe
# ADD BASE BSC32 /nologo
# ADD BSC32 /nologo
LINK32=link.exe
# ADD BASE LINK32 kernel32.lib user32.lib gdi32.lib winspool.lib comdlg32.lib advapi32.lib shell32.lib ole32.lib oleaut32.lib uuid.lib odbc32.lib odbccp32.lib /nologo /subsystem:windows /dll /debug /machine:I386 /pdbtype:sept
# ADD LINK32 ..\..\Core\Lib\Core.lib ..\Lib\s3tc.lib /nologo /base:"0x10300000" /subsystem:windows /dll /debug /debugtype:both /machine:I386 /nodefaultlib:"LIBC" /out:"..\..\System\Engine.dll" /pdbtype:sept
# SUBTRACT LINK32 /pdb:none

!ENDIF 

# Begin Target

# Name "Engine - Win32 Release"
# Name "Engine - Win32 Debug"
# Begin Group "Src"

# PROP Default_Filter "*.cpp"
# Begin Source File

SOURCE=.\Amd3d.h
# End Source File
# Begin Source File

SOURCE=.\AStatLog.cpp

!IF  "$(CFG)" == "Engine - Win32 Release"

# ADD CPP /Yc"EnginePrivate.h"

!ELSEIF  "$(CFG)" == "Engine - Win32 Debug"

!ENDIF 

# End Source File
# Begin Source File

SOURCE=.\Engine.cpp

!IF  "$(CFG)" == "Engine - Win32 Release"

# ADD CPP /Yu

!ELSEIF  "$(CFG)" == "Engine - Win32 Debug"

# ADD CPP /Yc"EnginePrivate.h"

!ENDIF 

# End Source File
# Begin Source File

SOURCE=.\EnginePrivate.h
# End Source File
# Begin Source File

SOURCE=.\palette.cpp
# End Source File
# Begin Source File

SOURCE=.\ULodMesh.cpp
# End Source File
# Begin Source File

SOURCE=.\UnActCol.cpp
# End Source File
# Begin Source File

SOURCE=.\UnActor.cpp
# End Source File
# Begin Source File

SOURCE=.\UnAudio.cpp
# End Source File
# Begin Source File

SOURCE=.\UnCamera.cpp
# End Source File
# Begin Source File

SOURCE=.\UnCamMgr.cpp
# End Source File
# Begin Source File

SOURCE=.\UnCanvas.cpp
# End Source File
# Begin Source File

SOURCE=.\UnCon.cpp
# End Source File
# Begin Source File

SOURCE=.\UnDynBsp.cpp
# End Source File
# Begin Source File

SOURCE=.\UnEngine.cpp
# End Source File
# Begin Source File

SOURCE=.\UnEngineNative.cpp
# End Source File
# Begin Source File

SOURCE=.\UnFont.cpp
# End Source File
# Begin Source File

SOURCE=.\UnFPoly.cpp
# End Source File
# Begin Source File

SOURCE=.\UnGame.cpp
# End Source File
# Begin Source File

SOURCE=.\UnGesture.cpp
# End Source File
# Begin Source File

SOURCE=.\UnIn.cpp
# End Source File
# Begin Source File

SOURCE=.\UnLevAct.cpp
# End Source File
# Begin Source File

SOURCE=.\UnLevel.cpp
# End Source File
# Begin Source File

SOURCE=.\UnLevTic.cpp
# End Source File
# Begin Source File

SOURCE=.\UnMesh.cpp
# End Source File
# Begin Source File

SOURCE=.\UnModel.cpp
# End Source File
# Begin Source File

SOURCE=.\UnMover.cpp
# End Source File
# Begin Source File

SOURCE=.\UnParams.cpp
# End Source File
# Begin Source File

SOURCE=.\UnParticle.cpp
# End Source File
# Begin Source File

SOURCE=.\UnParticleFX.cpp
# End Source File
# Begin Source File

SOURCE=.\UnParticleList.cpp
# End Source File
# Begin Source File

SOURCE=.\UnPath.cpp
# End Source File
# Begin Source File

SOURCE=.\UnPath.h
# End Source File
# Begin Source File

SOURCE=.\UnPawn.cpp
# End Source File
# Begin Source File

SOURCE=.\UnPhysic.cpp
# End Source File
# Begin Source File

SOURCE=.\UnPlayer.cpp
# End Source File
# Begin Source File

SOURCE=.\UnPrim.cpp
# End Source File
# Begin Source File

SOURCE=.\UnReach.cpp
# End Source File
# Begin Source File

SOURCE=.\UnRenderIterator.cpp
# End Source File
# Begin Source File

SOURCE=.\UnReplay.cpp
# End Source File
# Begin Source File

SOURCE=.\UnRoute.cpp
# End Source File
# Begin Source File

SOURCE=.\UnScript.cpp
# End Source File
# Begin Source File

SOURCE=.\UnScrTex.cpp
# End Source File
# Begin Source File

SOURCE=.\UnSkeletalMesh.cpp
# End Source File
# Begin Source File

SOURCE=.\UnTex.cpp
# End Source File
# Begin Source File

SOURCE=.\UnTrace.cpp
# End Source File
# Begin Source File

SOURCE=.\UnURL.cpp
# End Source File
# Begin Source File

SOURCE=.\UnWind.cpp
# End Source File
# Begin Source File

SOURCE=.\winstuff.cpp
# SUBTRACT CPP /YX /Yc /Yu
# End Source File
# End Group
# Begin Group "Inc"

# PROP Default_Filter "*.h"
# Begin Source File

SOURCE=..\Inc\AActor.h
# End Source File
# Begin Source File

SOURCE=..\Inc\AAmmo.h
# End Source File
# Begin Source File

SOURCE=..\Inc\ABrush.h
# End Source File
# Begin Source File

SOURCE=..\Inc\ACamera.h
# End Source File
# Begin Source File

SOURCE=..\Inc\ACarcass.h
# End Source File
# Begin Source File

SOURCE=..\Inc\AGameReplicationInfo.h
# End Source File
# Begin Source File

SOURCE=..\Inc\AInterpolationManager.h
# End Source File
# Begin Source File

SOURCE=..\Inc\AInterpolationPoint.h
# End Source File
# Begin Source File

SOURCE=..\Inc\AInventory.h
# End Source File
# Begin Source File

SOURCE=..\Inc\ALevelInfo.h
# End Source File
# Begin Source File

SOURCE=..\Inc\AMover.h
# End Source File
# Begin Source File

SOURCE=..\Inc\AParticleFX.h
# End Source File
# Begin Source File

SOURCE=..\Inc\APawn.h
# End Source File
# Begin Source File

SOURCE=..\Inc\APickup.h
# End Source File
# Begin Source File

SOURCE=..\Inc\APlayerPawn.h
# End Source File
# Begin Source File

SOURCE=..\Inc\APlayerReplicationInfo.h
# End Source File
# Begin Source File

SOURCE=..\Inc\AWeapon.h
# End Source File
# Begin Source File

SOURCE=..\Inc\AWind.h
# End Source File
# Begin Source File

SOURCE=..\Inc\AZoneInfo.h
# End Source File
# Begin Source File

SOURCE=..\Inc\ENCVAG.H
# End Source File
# Begin Source File

SOURCE=..\Inc\Engine.h
# End Source File
# Begin Source File

SOURCE=..\Inc\EngineClasses.h
# End Source File
# Begin Source File

SOURCE=..\Inc\Palette.h
# End Source File
# Begin Source File

SOURCE=..\Inc\S3tc.h
# End Source File
# Begin Source File

SOURCE=..\Inc\UGesture.h
# End Source File
# Begin Source File

SOURCE=..\Inc\UI3DL2Listener.h
# End Source File
# Begin Source File

SOURCE=..\Inc\ULevelSummary.h
# End Source File
# Begin Source File

SOURCE=..\Inc\UnActor.h
# End Source File
# Begin Source File

SOURCE=..\Inc\UnAnim.h
# End Source File
# Begin Source File

SOURCE=..\Inc\UnAudio.h
# End Source File
# Begin Source File

SOURCE=..\Inc\UnCamera.h
# End Source File
# Begin Source File

SOURCE=..\Inc\UnCon.h
# End Source File
# Begin Source File

SOURCE=..\Inc\UnDDraw.h
# End Source File
# Begin Source File

SOURCE=..\Inc\UnDynBsp.h
# End Source File
# Begin Source File

SOURCE=..\Inc\UnEngine.h
# End Source File
# Begin Source File

SOURCE=..\Inc\UnEngineGnuG.h
# End Source File
# Begin Source File

SOURCE=..\Inc\UnEngineNative.h
# End Source File
# Begin Source File

SOURCE=..\Inc\UnEngineWin.h
# End Source File
# Begin Source File

SOURCE=..\Inc\UnGame.h
# End Source File
# Begin Source File

SOURCE=..\Inc\UnIn.h
# End Source File
# Begin Source File

SOURCE=..\Inc\UnInterpolationPoint.h
# End Source File
# Begin Source File

SOURCE=..\Inc\UnLevel.h
# End Source File
# Begin Source File

SOURCE=..\Inc\UnMesh.h
# End Source File
# Begin Source File

SOURCE=..\Inc\UnModel.h
# End Source File
# Begin Source File

SOURCE=..\Inc\UnObj.h
# End Source File
# Begin Source File

SOURCE=..\Inc\UnParticle.h
# End Source File
# Begin Source File

SOURCE=..\Inc\UnParticleFX.h
# End Source File
# Begin Source File

SOURCE=..\Inc\UnParticleList.h
# End Source File
# Begin Source File

SOURCE=..\Inc\UnPlayer.h
# End Source File
# Begin Source File

SOURCE=..\Inc\UnPrim.h
# End Source File
# Begin Source File

SOURCE=..\Inc\UnReach.h
# End Source File
# Begin Source File

SOURCE=..\Inc\UnRender.h
# End Source File
# Begin Source File

SOURCE=..\Inc\UnRenderIterator.h
# End Source File
# Begin Source File

SOURCE=..\Inc\UnRenderIteratorSupport.h
# End Source File
# Begin Source File

SOURCE=..\Inc\UnRenDev.h
# End Source File
# Begin Source File

SOURCE=..\Inc\UnReplay.h
# End Source File
# Begin Source File

SOURCE=..\Inc\UnScrTex.h
# End Source File
# Begin Source File

SOURCE=..\Inc\UnSkeletalMesh.h
# End Source File
# Begin Source File

SOURCE=..\Inc\UnSoundContainer.h
# End Source File
# Begin Source File

SOURCE=..\Inc\UnStats.h
# End Source File
# Begin Source File

SOURCE=..\Inc\UnTex.h
# End Source File
# Begin Source File

SOURCE=..\Inc\UnURL.h
# End Source File
# End Group
# Begin Group "Classes"

# PROP Default_Filter "*.uc"
# Begin Source File

SOURCE=..\Classes\Actor.uc
# End Source File
# Begin Source File

SOURCE=..\Classes\ActorShadow.uc
# End Source File
# Begin Source File

SOURCE=..\Classes\Ambient_light.uc
# End Source File
# Begin Source File

SOURCE=..\Classes\AmbientSound.uc
# End Source File
# Begin Source File

SOURCE=..\Classes\Ambushpoint.uc
# End Source File
# Begin Source File

SOURCE=..\Classes\Ammo.uc
# End Source File
# Begin Source File

SOURCE=..\Classes\AnimChannel.uc
# End Source File
# Begin Source File

SOURCE=..\Classes\AssertMover.uc
# End Source File
# Begin Source File

SOURCE=..\Classes\AttachMover.uc
# End Source File
# Begin Source File

SOURCE=..\Classes\Bitmap.uc
# End Source File
# Begin Source File

SOURCE=..\Classes\BlockAll.uc
# End Source File
# Begin Source File

SOURCE=..\Classes\BlockAllButProjectile.uc
# End Source File
# Begin Source File

SOURCE=..\Classes\BlockMonsters.uc
# End Source File
# Begin Source File

SOURCE=..\Classes\BlockPlayer.uc
# End Source File
# Begin Source File

SOURCE=..\Classes\Brush.uc
# End Source File
# Begin Source File

SOURCE=..\Classes\ButtonMarker.uc
# End Source File
# Begin Source File

SOURCE=..\Classes\Camera.uc
# End Source File
# Begin Source File

SOURCE=..\Classes\candleflame.uc
# End Source File
# Begin Source File

SOURCE=..\Classes\Canvas.uc
# End Source File
# Begin Source File

SOURCE=..\Classes\Carcass.uc
# End Source File
# Begin Source File

SOURCE=..\Classes\cEyeBlinkAnimChannel.uc
# End Source File
# Begin Source File

SOURCE=..\Classes\cFacialAnimChannel.uc
# End Source File
# Begin Source File

SOURCE=..\Classes\cHeadLookAnimChannel.uc
# End Source File
# Begin Source File

SOURCE=..\Classes\ClipMarker.uc
# End Source File
# Begin Source File

SOURCE=..\Classes\Console.uc
# End Source File
# Begin Source File

SOURCE=..\Classes\Counter.uc
# End Source File
# Begin Source File

SOURCE=..\Classes\DamageType.uc
# End Source File
# Begin Source File

SOURCE=..\Classes\Darklight.uc
# End Source File
# Begin Source File

SOURCE=..\Classes\Decal.uc
# End Source File
# Begin Source File

SOURCE=..\Classes\Decoration.uc
# End Source File
# Begin Source File

SOURCE=..\Classes\DemoRecSpectator.uc
# End Source File
# Begin Source File

SOURCE=..\Classes\Dispatcher.uc
# End Source File
# Begin Source File

SOURCE=..\Classes\Effects.uc
# End Source File
# Begin Source File

SOURCE=..\Classes\ElevatorMover.uc
# End Source File
# Begin Source File

SOURCE=..\Classes\ElevatorTrigger.uc
# End Source File
# Begin Source File

SOURCE=..\Classes\Engine.uc
# End Source File
# Begin Source File

SOURCE=..\Classes\Engine.upkg
# End Source File
# Begin Source File

SOURCE=..\Classes\FogLight.uc
# End Source File
# Begin Source File

SOURCE=..\Classes\Fragment.uc
# End Source File
# Begin Source File

SOURCE=..\Classes\GameInfo.uc
# End Source File
# Begin Source File

SOURCE=..\Classes\GameReplicationInfo.uc
# End Source File
# Begin Source File

SOURCE=..\Classes\GameSaveInfo.uc
# End Source File
# Begin Source File

SOURCE=..\Classes\Gesture.uc
# End Source File
# Begin Source File

SOURCE=..\Classes\GradualMover.uc
# End Source File
# Begin Source File

SOURCE=..\Classes\GridMover.uc
# End Source File
# Begin Source File

SOURCE=..\Classes\GridMover2.uc
# End Source File
# Begin Source File

SOURCE=..\Classes\GridResetTrigger.uc
# End Source File
# Begin Source File

SOURCE=..\Classes\HomeBase.uc
# End Source File
# Begin Source File

SOURCE=..\Classes\HUD.uc
# End Source File
# Begin Source File

SOURCE=..\Classes\I3DL2Listener.uc
# End Source File
# Begin Source File

SOURCE=..\Classes\ImpactSoundSet.uc
# End Source File
# Begin Source File

SOURCE=..\Classes\Info.uc
# End Source File
# Begin Source File

SOURCE=..\Classes\InternetInfo.uc
# End Source File
# Begin Source File

SOURCE=..\Classes\InterpolationManager.uc
# End Source File
# Begin Source File

SOURCE=..\Classes\InterpolationPoint.uc
# End Source File
# Begin Source File

SOURCE=..\Classes\Inventory.uc
# End Source File
# Begin Source File

SOURCE=..\Classes\InventorySpot.uc
# End Source File
# Begin Source File

SOURCE=..\Classes\Keypoint.uc
# End Source File
# Begin Source File

SOURCE=..\Classes\LevelInfo.uc
# End Source File
# Begin Source File

SOURCE=..\Classes\LevelSummary.uc
# End Source File
# Begin Source File

SOURCE=..\Classes\LiftCenter.uc
# End Source File
# Begin Source File

SOURCE=..\Classes\LiftExit.uc
# End Source File
# Begin Source File

SOURCE=..\Classes\Light.uc
# End Source File
# Begin Source File

SOURCE=..\Classes\LocalMessage.uc
# End Source File
# Begin Source File

SOURCE=..\Classes\locationid.uc
# End Source File
# Begin Source File

SOURCE=..\Classes\LoopMover.uc
# End Source File
# Begin Source File

SOURCE=..\Classes\MapList.uc
# End Source File
# Begin Source File

SOURCE=..\Classes\Menu.uc
# End Source File
# Begin Source File

SOURCE=..\Classes\MessagingSpectator.uc
# End Source File
# Begin Source File

SOURCE=..\Classes\MixMover.uc
# End Source File
# Begin Source File

SOURCE=..\Classes\Mover.uc
# End Source File
# Begin Source File

SOURCE=..\Classes\MusicEvent.uc
# End Source File
# Begin Source File

SOURCE=..\Classes\MusicTrigger.uc
# End Source File
# Begin Source File

SOURCE=..\Classes\Mutator.uc
# End Source File
# Begin Source File

SOURCE=..\Classes\NavigationPoint.uc
# End Source File
# Begin Source File

SOURCE=..\Classes\Palette.uc
# End Source File
# Begin Source File

SOURCE=..\Classes\ParticleFX.uc
# End Source File
# Begin Source File

SOURCE=..\Classes\ParticleTrigger.uc
# End Source File
# Begin Source File

SOURCE=..\Classes\PathNode.uc
# End Source File
# Begin Source File

SOURCE=..\Classes\PatrolPoint.uc
# End Source File
# Begin Source File

SOURCE=..\Classes\Pawn.uc
# End Source File
# Begin Source File

SOURCE=..\Classes\Pickup.uc
# End Source File
# Begin Source File

SOURCE=..\Classes\Player.uc
# End Source File
# Begin Source File

SOURCE=..\Classes\PlayerPawn.uc
# End Source File
# Begin Source File

SOURCE=..\Classes\PlayerReplicationInfo.uc
# End Source File
# Begin Source File

SOURCE=..\Classes\PlayerStart.uc
# End Source File
# Begin Source File

SOURCE=..\Classes\Projectile.uc
# End Source File
# Begin Source File

SOURCE=..\Classes\RenderIterator.uc
# End Source File
# Begin Source File

SOURCE=..\Classes\ReplicationInfo.uc
# End Source File
# Begin Source File

SOURCE=..\Classes\RotatingMover.uc
# End Source File
# Begin Source File

SOURCE=..\Classes\RoundRobin.uc
# End Source File
# Begin Source File

SOURCE=..\Classes\SavedMove.uc
# End Source File
# Begin Source File

SOURCE=..\Classes\ScaledSprite.uc
# End Source File
# Begin Source File

SOURCE=..\Classes\ScoreBoard.uc
# End Source File
# Begin Source File

SOURCE=..\Classes\Scout.uc
# End Source File
# Begin Source File

SOURCE=..\Classes\ScriptedTexture.uc
# End Source File
# Begin Source File

SOURCE=..\Classes\SkyZoneInfo.uc
# End Source File
# Begin Source File

SOURCE=..\Classes\Sound_FX.uc
# End Source File
# Begin Source File

SOURCE=..\Classes\SoundContainer.uc
# End Source File
# Begin Source File

SOURCE=..\Classes\SpawnNotify.uc
# End Source File
# Begin Source File

SOURCE=..\Classes\SpecialEvent.uc
# End Source File
# Begin Source File

SOURCE=..\Classes\SpecialLit.uc
# End Source File
# Begin Source File

SOURCE=..\Classes\Spectator.uc
# End Source File
# Begin Source File

SOURCE=..\Classes\SplineManager.uc
# End Source File
# Begin Source File

SOURCE=..\Classes\Spotlight.uc
# End Source File
# Begin Source File

SOURCE=..\Classes\Sprite.uc
# End Source File
# Begin Source File

SOURCE=..\Classes\StatLog.uc
# End Source File
# Begin Source File

SOURCE=..\Classes\StatLogFile.uc
# End Source File
# Begin Source File

SOURCE=..\Classes\Teleporter.uc
# End Source File
# Begin Source File

SOURCE=..\Classes\TestInfo.uc
# End Source File
# Begin Source File

SOURCE=..\Classes\TestObj.uc
# End Source File
# Begin Source File

SOURCE=..\Classes\Texture.uc
# End Source File
# Begin Source File

SOURCE=..\Classes\TimedCue.uc
# End Source File
# Begin Source File

SOURCE=..\Classes\Torch_light.uc
# End Source File
# Begin Source File

SOURCE=..\Classes\Trigger.uc
# End Source File
# Begin Source File

SOURCE=..\Classes\TriggerLight.uc
# End Source File
# Begin Source File

SOURCE=..\Classes\TriggerMarker.uc
# End Source File
# Begin Source File

SOURCE=..\Classes\Triggers.uc
# End Source File
# Begin Source File

SOURCE=..\Classes\TriggerWind.uc
# End Source File
# Begin Source File

SOURCE=..\Classes\TurnToController.uc
# End Source File
# Begin Source File

SOURCE=..\Classes\VoicePack.uc
# End Source File
# Begin Source File

SOURCE=..\Classes\WarpZoneInfo.uc
# End Source File
# Begin Source File

SOURCE=..\Classes\WarpZoneMarker.uc
# End Source File
# Begin Source File

SOURCE=..\Classes\WayBeacon.uc
# End Source File
# Begin Source File

SOURCE=..\Classes\Weapon.uc
# End Source File
# Begin Source File

SOURCE=..\Classes\Wind.uc
# End Source File
# Begin Source File

SOURCE=..\Classes\ZoneInfo.uc
# End Source File
# Begin Source File

SOURCE=..\Classes\ZoneTrigger.uc
# End Source File
# End Group
# Begin Group "Net"

# PROP Default_Filter ""
# Begin Source File

SOURCE=.\UnBunch.cpp
# End Source File
# Begin Source File

SOURCE=..\Inc\UnBunch.h
# End Source File
# Begin Source File

SOURCE=.\UnChan.cpp
# End Source File
# Begin Source File

SOURCE=..\Inc\UnChan.h
# End Source File
# Begin Source File

SOURCE=.\UnConn.cpp
# End Source File
# Begin Source File

SOURCE=..\Inc\UnConn.h
# End Source File
# Begin Source File

SOURCE=.\UnDemoPenLev.cpp
# End Source File
# Begin Source File

SOURCE=..\Inc\UnDemoPenLev.h
# End Source File
# Begin Source File

SOURCE=.\UnDemoRec.cpp
# End Source File
# Begin Source File

SOURCE=..\Inc\UnDemoRec.h
# End Source File
# Begin Source File

SOURCE=.\UnDownload.cpp
# End Source File
# Begin Source File

SOURCE=..\Inc\UnDownload.h
# End Source File
# Begin Source File

SOURCE=..\Inc\UnNet.h
# End Source File
# Begin Source File

SOURCE=.\UnNetDrv.cpp
# End Source File
# Begin Source File

SOURCE=..\Inc\UnNetDrv.h
# End Source File
# Begin Source File

SOURCE=.\UnPenLev.cpp
# End Source File
# Begin Source File

SOURCE=..\Inc\UnPenLev.h
# End Source File
# End Group
# Begin Group "Int"

# PROP Default_Filter "*.int"
# Begin Source File

SOURCE=..\..\System\Engine.int
# End Source File
# End Group
# Begin Group "Mpeg"

# PROP Default_Filter ""
# Begin Source File

SOURCE=..\MPeg\MpegDecoder.h
# End Source File
# Begin Source File

SOURCE=..\MPeg\scompmpg.cpp

!IF  "$(CFG)" == "Engine - Win32 Release"

# SUBTRACT CPP /YX /Yc /Yu

!ELSEIF  "$(CFG)" == "Engine - Win32 Debug"

# ADD CPP /W3 /WX
# SUBTRACT CPP /YX /Yc /Yu

!ENDIF 

# End Source File
# Begin Source File

SOURCE=..\MPeg\scompmpg.h
# End Source File
# Begin Source File

SOURCE=..\MPeg\TbFile.cpp

!IF  "$(CFG)" == "Engine - Win32 Release"

# ADD CPP /W3
# SUBTRACT CPP /YX /Yc /Yu

!ELSEIF  "$(CFG)" == "Engine - Win32 Debug"

# ADD CPP /W3 /WX
# SUBTRACT CPP /YX /Yc /Yu

!ENDIF 

# End Source File
# Begin Source File

SOURCE=..\MPeg\TbFile.h
# End Source File
# Begin Source File

SOURCE=..\MPeg\MpegLib.lib
# End Source File
# End Group
# End Target
# End Project
