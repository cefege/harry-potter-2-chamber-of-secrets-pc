# Microsoft Developer Studio Project File - Name="CutScenes" - Package Owner=<4>
# Microsoft Developer Studio Generated Build File, Format Version 6.00
# ** DO NOT EDIT **

# TARGTYPE "Win32 (x86) Console Application" 0x0103

CFG=CutScenes - Win32 Debug
!MESSAGE This is not a valid makefile. To build this project using NMAKE,
!MESSAGE use the Export Makefile command and run
!MESSAGE 
!MESSAGE NMAKE /f "CutScenes.mak".
!MESSAGE 
!MESSAGE You can specify a configuration when running NMAKE
!MESSAGE by defining the macro CFG on the command line. For example:
!MESSAGE 
!MESSAGE NMAKE /f "CutScenes.mak" CFG="CutScenes - Win32 Debug"
!MESSAGE 
!MESSAGE Possible choices for configuration are:
!MESSAGE 
!MESSAGE "CutScenes - Win32 Release" (based on "Win32 (x86) Console Application")
!MESSAGE "CutScenes - Win32 Debug" (based on "Win32 (x86) Console Application")
!MESSAGE 

# Begin Project
# PROP AllowPerConfigDependencies 0
# PROP Scc_ProjName ""$/Unreal/CutScenes", BILBAAAA"
# PROP Scc_LocalPath "."
CPP=cl.exe
RSC=rc.exe

!IF  "$(CFG)" == "CutScenes - Win32 Release"

# PROP BASE Use_MFC 0
# PROP BASE Use_Debug_Libraries 0
# PROP BASE Output_Dir "Release"
# PROP BASE Intermediate_Dir "Release"
# PROP BASE Target_Dir ""
# PROP Use_MFC 0
# PROP Use_Debug_Libraries 0
# PROP Output_Dir "Release"
# PROP Intermediate_Dir "Release"
# PROP Target_Dir ""
# ADD BASE CPP /nologo /W3 /GX /O2 /D "WIN32" /D "NDEBUG" /D "_CONSOLE" /D "_MBCS" /YX /FD /c
# ADD CPP /nologo /W3 /GX /O2 /D "WIN32" /D "NDEBUG" /D "_CONSOLE" /D "_MBCS" /YX /FD /c
# ADD BASE RSC /l 0x409 /d "NDEBUG"
# ADD RSC /l 0x409 /d "NDEBUG"
BSC32=bscmake.exe
# ADD BASE BSC32 /nologo
# ADD BSC32 /nologo
LINK32=link.exe
# ADD BASE LINK32 kernel32.lib user32.lib gdi32.lib winspool.lib comdlg32.lib advapi32.lib shell32.lib ole32.lib oleaut32.lib uuid.lib odbc32.lib odbccp32.lib kernel32.lib user32.lib gdi32.lib winspool.lib comdlg32.lib advapi32.lib shell32.lib ole32.lib oleaut32.lib uuid.lib odbc32.lib odbccp32.lib /nologo /subsystem:console /machine:I386
# ADD LINK32 kernel32.lib user32.lib gdi32.lib winspool.lib comdlg32.lib advapi32.lib shell32.lib ole32.lib oleaut32.lib uuid.lib odbc32.lib odbccp32.lib kernel32.lib user32.lib gdi32.lib winspool.lib comdlg32.lib advapi32.lib shell32.lib ole32.lib oleaut32.lib uuid.lib odbc32.lib odbccp32.lib /nologo /subsystem:console /machine:I386

!ELSEIF  "$(CFG)" == "CutScenes - Win32 Debug"

# PROP BASE Use_MFC 0
# PROP BASE Use_Debug_Libraries 1
# PROP BASE Output_Dir "Debug"
# PROP BASE Intermediate_Dir "Debug"
# PROP BASE Target_Dir ""
# PROP Use_MFC 0
# PROP Use_Debug_Libraries 1
# PROP Output_Dir "Debug"
# PROP Intermediate_Dir "Debug"
# PROP Target_Dir ""
# ADD BASE CPP /nologo /W3 /Gm /GX /ZI /Od /D "WIN32" /D "_DEBUG" /D "_CONSOLE" /D "_MBCS" /YX /FD /GZ /c
# ADD CPP /nologo /W3 /Gm /GX /ZI /Od /D "WIN32" /D "_DEBUG" /D "_CONSOLE" /D "_MBCS" /YX /FD /GZ /c
# ADD BASE RSC /l 0x409 /d "_DEBUG"
# ADD RSC /l 0x409 /d "_DEBUG"
BSC32=bscmake.exe
# ADD BASE BSC32 /nologo
# ADD BSC32 /nologo
LINK32=link.exe
# ADD BASE LINK32 kernel32.lib user32.lib gdi32.lib winspool.lib comdlg32.lib advapi32.lib shell32.lib ole32.lib oleaut32.lib uuid.lib odbc32.lib odbccp32.lib kernel32.lib user32.lib gdi32.lib winspool.lib comdlg32.lib advapi32.lib shell32.lib ole32.lib oleaut32.lib uuid.lib odbc32.lib odbccp32.lib /nologo /subsystem:console /debug /machine:I386 /pdbtype:sept
# ADD LINK32 kernel32.lib user32.lib gdi32.lib winspool.lib comdlg32.lib advapi32.lib shell32.lib ole32.lib oleaut32.lib uuid.lib odbc32.lib odbccp32.lib kernel32.lib user32.lib gdi32.lib winspool.lib comdlg32.lib advapi32.lib shell32.lib ole32.lib oleaut32.lib uuid.lib odbc32.lib odbccp32.lib /nologo /subsystem:console /debug /machine:I386 /pdbtype:sept

!ENDIF 

# Begin Target

# Name "CutScenes - Win32 Release"
# Name "CutScenes - Win32 Debug"
# Begin Group "HousePointCeremonies"

# PROP Default_Filter ""
# Begin Source File

SOURCE=.\FirstHousePointCeremony.txt
# End Source File
# Begin Source File

SOURCE=.\HousePointCeremony.txt
# End Source File
# End Group
# Begin Group "WizardDueling"

# PROP Default_Filter ""
# Begin Source File

SOURCE=.\07040DuelIntro.txt
# End Source File
# Begin Source File

SOURCE=.\07070DuelSnake.txt
# End Source File
# End Group
# Begin Group "GState000"

# PROP Default_Filter ""
# Begin Source File

SOURCE=.\00001PrivetIntro.txt
# End Source File
# Begin Source File

SOURCE=.\00020FlyingFordEnd.txt
# End Source File
# Begin Source File

SOURCE=.\00020FlyingFordIntro.txt
# End Source File
# Begin Source File

SOURCE=.\00030WhompCrash.txt
# End Source File
# Begin Source File

SOURCE=.\00049PreWhompFree.txt
# End Source File
# Begin Source File

SOURCE=.\00050WhompFree.txt
# End Source File
# End Group
# Begin Group "GState010"

# PROP Default_Filter ""
# Begin Source File

SOURCE=.\1040Password.txt
# End Source File
# Begin Source File

SOURCE=.\LevelStart_EntryHallHub_GState010.txt
# End Source File
# Begin Source File

SOURCE=.\RonLeadsToGryRoom_GState010.txt
# End Source File
# End Group
# Begin Group "GState020"

# PROP Default_Filter ""
# Begin Source File

SOURCE=.\02020GoToDADA.txt
# End Source File
# Begin Source File

SOURCE=.\02060DADARictaInt.txt
# End Source File
# Begin Source File

SOURCE=.\02080DADARictaEnd.txt
# End Source File
# Begin Source File

SOURCE=.\105000GoldExplain.txt
# End Source File
# Begin Source File

SOURCE=.\GoldExplain.txt
# End Source File
# Begin Source File

SOURCE=.\GoldNotGotAllSIlverYet1.txt
# End Source File
# Begin Source File

SOURCE=.\GoldNotGotAllSIlverYet2.txt
# End Source File
# Begin Source File

SOURCE=.\levelStart_EntryHallHub_GState020.txt
# End Source File
# Begin Source File

SOURCE=.\levelStart_GrandStaircaseHub_GState020.txt
# End Source File
# Begin Source File

SOURCE=.\LevelStart_GroundsHub_GState020.txt
# End Source File
# Begin Source File

SOURCE=.\PrepareForGoToDADA2.txt
# End Source File
# End Group
# Begin Group "GState030"

# PROP Default_Filter ""
# Begin Source File

SOURCE=.\3030GoToHPc1.txt
# End Source File
# Begin Source File

SOURCE=.\LevelStart_EntryHallHub_GState03.txt
# End Source File
# Begin Source File

SOURCE=.\LevelStart_GrandStair_GState030.txt
# End Source File
# Begin Source File

SOURCE=.\LevelStart_GroundsHub_GState030.txt
# End Source File
# Begin Source File

SOURCE=.\OpenRicta.txt
# End Source File
# End Group
# Begin Group "GState035"

# PROP Default_Filter ""
# Begin Source File

SOURCE=.\LevelStart_EntryHallHub_GState035.txt
# End Source File
# Begin Source File

SOURCE=.\LevelStart_GrandStairHub_GState035.txt
# End Source File
# Begin Source File

SOURCE=.\LevelStart_GroundsHub_GState035.txt
# End Source File
# End Group
# Begin Group "GState040"

# PROP Default_Filter ""
# Begin Source File

SOURCE=.\04020GoToPotions.txt
# End Source File
# Begin Source File

SOURCE=.\04040PotionIntro.txt
# End Source File
# Begin Source File

SOURCE=.\04050PotionTeach.txt
# End Source File
# Begin Source File

SOURCE=.\04070FollowVoice.txt
# End Source File
# Begin Source File

SOURCE=.\LevelStart_EntryHallHub_GState040.txt
# End Source File
# Begin Source File

SOURCE=.\LevelStart_GrandStairHub_GState040.txt
# End Source File
# Begin Source File

SOURCE=.\LevelStart_GroundsHub_GState040.txt
# End Source File
# End Group
# Begin Group "GState045"

# PROP Default_Filter ""
# Begin Source File

SOURCE=.\04070FollowVoiceB.txt
# End Source File
# End Group
# Begin Group "GState050"

# PROP Default_Filter ""
# Begin Source File

SOURCE=.\05030GoToCharms.txt
# End Source File
# Begin Source File

SOURCE=.\05030GoToCharms2.txt
# End Source File
# Begin Source File

SOURCE=.\05040CharmSkurgeInt.txt
# End Source File
# Begin Source File

SOURCE=.\05060CharmSkurgEnd.txt
# End Source File
# Begin Source File

SOURCE=.\LevelStart_EntryHallHub_GState050.txt
# End Source File
# Begin Source File

SOURCE=.\levelStart_GrandStairHub_GState050.txt
# End Source File
# Begin Source File

SOURCE=.\LevelStart_GroundsHub_GState050.txt
# End Source File
# Begin Source File

SOURCE=.\PrepareForGoToCharms2.txt
# End Source File
# End Group
# Begin Group "GState060"

# PROP Default_Filter ""
# Begin Source File

SOURCE=.\060GenKid_HousePointCeremony.txt
# End Source File
# Begin Source File

SOURCE=.\HPCSetUp.txt
# End Source File
# Begin Source File

SOURCE=.\LevelStart_EntryHallHub_GState060.txt
# End Source File
# Begin Source File

SOURCE=.\levelStart_GrandStairHub_GState060.txt
# End Source File
# Begin Source File

SOURCE=.\LevelStart_GroundsHub_GState060.txt
# End Source File
# End Group
# Begin Group "GState065"

# PROP Default_Filter ""
# Begin Source File

SOURCE=".\06020CrimeScene(A).txt"
# End Source File
# Begin Source File

SOURCE=.\LevelStart_EntryHallHub_GState065.txt
# End Source File
# Begin Source File

SOURCE=.\LevelStart_GroundsHub_GState065.txt
# End Source File
# End Group
# Begin Group "GState070"

# PROP Default_Filter ""
# Begin Source File

SOURCE=.\07020DuelOpen.txt
# End Source File
# Begin Source File

SOURCE=.\LevelStart_EntryHallHub_GState070.txt
# End Source File
# Begin Source File

SOURCE=.\LevelStart_GroundsHub_GState070.txt
# End Source File
# End Group
# Begin Group "GState080"

# PROP Default_Filter ""
# Begin Source File

SOURCE=.\07080Parselmouth.txt
# End Source File
# Begin Source File

SOURCE=.\08020GoToHerb.txt
# End Source File
# Begin Source File

SOURCE=.\08040HerbDiffIntro.txt
# End Source File
# Begin Source File

SOURCE=.\08060HerbDiffEnd.txt
# End Source File
# Begin Source File

SOURCE=.\LevelStart_EntryHallHub_GState080.txt
# End Source File
# Begin Source File

SOURCE=.\LevelStart_GroundsHub_GState080.txt
# End Source File
# End Group
# Begin Group "GState090"

# PROP Default_Filter ""
# Begin Source File

SOURCE=.\GroundsRonGoToAdv4.txt
# End Source File
# Begin Source File

SOURCE=.\LevelStart_EntryHallHub_GState090.txt
# End Source File
# Begin Source File

SOURCE=.\LevelStart_GroundsHub_GState090.txt
# End Source File
# End Group
# Begin Group "GState095"

# PROP Default_Filter ""
# Begin Source File

SOURCE=.\ChangeToGState100NoBeanRoom.txt
# End Source File
# Begin Source File

SOURCE=.\changeToGState100YesBeanRoom.txt
# End Source File
# Begin Source File

SOURCE=.\HPCSetUp_3.txt
# End Source File
# Begin Source File

SOURCE=.\LevelStart_EntryHallHub_GState095.txt
# End Source File
# Begin Source File

SOURCE=.\ThirdGoToHPCe.txt
# End Source File
# End Group
# Begin Group "GState100"

# PROP Default_Filter ""
# Begin Source File

SOURCE=.\10030NickFrozen.txt
# End Source File
# Begin Source File

SOURCE=.\10050DumbOffice.txt
# End Source File
# Begin Source File

SOURCE=.\10050DumbOfficeB.txt
# End Source File
# Begin Source File

SOURCE=.\10060DumbEnter.txt
# End Source File
# Begin Source File

SOURCE=.\11040BitOfGoyle.txt
# End Source File
# Begin Source File

SOURCE=.\LevelStart_EntryHallHub_GState100.txt
# End Source File
# Begin Source File

SOURCE=.\levelStart_GrandStairHub_GState100.txt
# End Source File
# Begin Source File

SOURCE=.\LevelStart_GroundsHub_GState100.txt
# End Source File
# Begin Source File

SOURCE=.\RonLeadHarry.txt
# End Source File
# Begin Source File

SOURCE=.\RonLeadHarry2.txt
# End Source File
# End Group
# Begin Group "GState110"

# PROP Default_Filter ""
# Begin Source File

SOURCE=.\11060GoyleFound.txt
# End Source File
# Begin Source File

SOURCE=.\LevelStart_EntryHallHub_GState110.txt
# End Source File
# Begin Source File

SOURCE=.\LevelStart_GroundsHub_GState110.txt
# End Source File
# End Group
# Begin Group "GState115"

# PROP Default_Filter ""
# Begin Source File

SOURCE=.\12020Trans2Goyle.txt
# End Source File
# Begin Source File

SOURCE=.\LevelStart_EntryHallHub_GState115.txt
# End Source File
# Begin Source File

SOURCE=.\levelStart_GrandStairHub_GState115.txt
# End Source File
# End Group
# Begin Group "GState120"

# PROP Default_Filter ""
# Begin Source File

SOURCE=.\120000Duel01HarryLoses.txt
# End Source File
# Begin Source File

SOURCE=.\120000Duel01HarryWins.txt
# End Source File
# Begin Source File

SOURCE=.\120000Duel01LevelLoad.txt
# End Source File
# Begin Source File

SOURCE=.\12060PreAdv7WayInToCom.txt
# End Source File
# Begin Source File

SOURCE=.\12060PreAdv7WayInToCom2.txt
# End Source File
# Begin Source File

SOURCE=.\LevelStart_EntryHallHub_GState120.txt
# End Source File
# Begin Source File

SOURCE=.\levelStart_GrandStairHub_GState120.txt
# End Source File
# Begin Source File

SOURCE=.\LevelStart_GroundsHub_GState120.txt
# End Source File
# End Group
# Begin Group "GState130"

# PROP Default_Filter ""
# Begin Source File

SOURCE=.\13020GoToSponge.txt
# End Source File
# Begin Source File

SOURCE=.\13040SpongeIntro.txt
# End Source File
# Begin Source File

SOURCE=.\13060SpongeEnd.txt
# End Source File
# Begin Source File

SOURCE=.\LevelStart_EntryHallHub_GState130.txt
# End Source File
# Begin Source File

SOURCE=.\levelStart_GrandStairHub_GState130.txt
# End Source File
# Begin Source File

SOURCE=.\PutHermioneOnALanding.txt
# End Source File
# End Group
# Begin Group "GState140"

# PROP Default_Filter ""
# Begin Source File

SOURCE=.\GoToHPC4.txt
# End Source File
# Begin Source File

SOURCE=.\LevelStart_EntryHallHub_GState140.txt
# End Source File
# End Group
# Begin Group "GState145"

# PROP Default_Filter ""
# Begin Source File

SOURCE=.\14020MMCrazy.txt
# End Source File
# Begin Source File

SOURCE=.\14040Diary.txt
# End Source File
# Begin Source File

SOURCE=.\14050Riddle.txt
# End Source File
# Begin Source File

SOURCE=.\GoToMMBAgain.txt
# End Source File
# Begin Source File

SOURCE=.\LevelStart_EntryHallHub_GState145.txt
# End Source File
# Begin Source File

SOURCE=.\levelStart_GrandStairHub_GState145.txt
# End Source File
# End Group
# Begin Group "GState150"

# PROP Default_Filter ""
# Begin Source File

SOURCE=.\15020DiaryStolen.txt
# End Source File
# Begin Source File

SOURCE=.\15040HagArrest.txt
# End Source File
# Begin Source File

SOURCE=.\15050Adv9aForest.txt
# End Source File
# Begin Source File

SOURCE=.\15060Adv9bLair.txt
# End Source File
# Begin Source File

SOURCE=.\15065Adv9bHarryWins.txt
# End Source File
# Begin Source File

SOURCE=.\15070RonRescue.txt
# End Source File
# Begin Source File

SOURCE=.\LevelStart_EntryHallHub_GState150.txt
# End Source File
# Begin Source File

SOURCE=.\LevelStart_GroundsNight_GState150.txt
# End Source File
# End Group
# Begin Group "GState160"

# PROP Default_Filter ""
# Begin Source File

SOURCE=.\16050MysterySolved.txt
# End Source File
# Begin Source File

SOURCE=.\LevelStart_EntryHallHub_GState160.txt
# End Source File
# Begin Source File

SOURCE=.\levelStart_GrandStairHub_GState160.txt
# End Source File
# End Group
# Begin Group "GState170"

# PROP Default_Filter ""
# Begin Source File

SOURCE=.\17020GoToMM.txt
# End Source File
# Begin Source File

SOURCE=.\17090OpenErUp.txt
# End Source File
# Begin Source File

SOURCE=.\17150BasiliskIntroV2.txt
# End Source File
# Begin Source File

SOURCE=.\17170VoldRevealedV2.txt
# End Source File
# Begin Source File

SOURCE=.\17180VictoryV2.txt
# End Source File
# Begin Source File

SOURCE=.\LevelStart_EntryHallHub_GState170.txt
# End Source File
# End Group
# Begin Group "GState180"

# PROP Default_Filter ""
# Begin Source File

SOURCE=.\17190WrapUp.txt
# End Source File
# Begin Source File

SOURCE=.\18021LastTask.txt
# End Source File
# Begin Source File

SOURCE=.\18040HouseCup.txt
# End Source File
# Begin Source File

SOURCE=.\LevelStart_EntryHallHub_GState180.txt
# End Source File
# Begin Source File

SOURCE=.\levelStart_GrandStairHub_GState180.txt
# End Source File
# End Group
# Begin Group "Quidditch"

# PROP Default_Filter ""
# Begin Group "QLesson"

# PROP Default_Filter ""
# Begin Source File

SOURCE=.\00307QuidditchCamLesson1.txt
# End Source File
# Begin Source File

SOURCE=.\00307QuidditchCamLesson2.txt
# End Source File
# Begin Source File

SOURCE=.\00307QuidditchTut1.txt
# End Source File
# Begin Source File

SOURCE=.\00307QuidditchTut2.txt
# End Source File
# Begin Source File

SOURCE=.\00307QuidditchTut3.txt
# End Source File
# Begin Source File

SOURCE=.\00309QuidditchSlyInter.txt
# End Source File
# End Group
# Begin Source File

SOURCE=.\00308Quidditch_EnterTrenchRun.txt
# End Source File
# Begin Source File

SOURCE=.\00308Quidditch_HarryDie.txt
# End Source File
# Begin Source File

SOURCE=.\00308Quidditch_LeaveTrenchRun.txt
# End Source File
# Begin Source File

SOURCE=.\00308QuidditchIntro.txt
# End Source File
# Begin Source File

SOURCE=.\00308QuidditchLOSS_Seeker.txt
# End Source File
# Begin Source File

SOURCE=.\00308QuidditchWIN.txt
# End Source File
# Begin Source File

SOURCE=.\00308QuidditchWinCUP.txt
# End Source File
# Begin Source File

SOURCE=.\GiveMeKids.txt
# End Source File
# Begin Source File

SOURCE=.\TEMPQuidCup.txt
# End Source File
# End Group
# Begin Group "Ch1Rictusempra"

# PROP Default_Filter ""
# Begin Source File

SOURCE=.\02080Ch1BridgeCounterWRoom.txt
# End Source File
# Begin Source File

SOURCE=.\02080Ch1BridgeCounterWRoom2.txt
# End Source File
# Begin Source File

SOURCE=.\02080Ch1FireCrabIntro.txt
# End Source File
# Begin Source File

SOURCE=.\02080Ch1LastStarSucceed.txt
# End Source File
# Begin Source File

SOURCE=.\02080Ch1ShowStar.txt
# End Source File
# Begin Source File

SOURCE=.\02080Ch1SnailIntro.txt
# End Source File
# Begin Source File

SOURCE=.\Ch1RictuEnd.txt
# End Source File
# Begin Source File

SOURCE=.\Ch1RictuIntro.txt
# End Source File
# End Group
# Begin Group "Adv7SlythComRoom"

# PROP Default_Filter ""
# Begin Source File

SOURCE=.\12060Adv7BridgeFallCut.txt
# End Source File
# Begin Source File

SOURCE=.\12060Adv7EctoStreamCut.txt
# End Source File
# Begin Source File

SOURCE=.\12060DracoNotHeir.txt
# End Source File
# Begin Source File

SOURCE=.\12060GirlsKickOutGoyle.txt
# End Source File
# Begin Source File

SOURCE=.\12060GoyleBackToHarry.txt
# End Source File
# Begin Source File

SOURCE=.\12060SnapeCaughtYou.txt
# End Source File
# End Group
# Begin Source File

SOURCE=.\01050TransitionA.txt
# End Source File
# Begin Source File

SOURCE=.\03100TransitionB.txt
# End Source File
# Begin Source File

SOURCE=.\04090TransitionC.txt
# End Source File
# Begin Source File

SOURCE=.\07090TransitionD.txt
# End Source File
# Begin Source File

SOURCE=.\09030TransitionE.txt
# End Source File
# Begin Source File

SOURCE=.\12080TransitionF.txt
# End Source File
# Begin Source File

SOURCE=.\14060TransitionG.txt
# End Source File
# Begin Source File

SOURCE=.\16600TransitionI.txt
# End Source File
# Begin Source File

SOURCE=.\17200TransitionJ.txt
# End Source File
# Begin Source File

SOURCE=.\Adv3DungeonQuestEnd.txt
# End Source File
# Begin Source File

SOURCE=.\Adv8ForrestDitchRon.txt
# End Source File
# Begin Source File

SOURCE=.\Ch2SkurgeEnd.txt
# End Source File
# Begin Source File

SOURCE=.\Ch2SkurgeIntro.txt
# End Source File
# Begin Source File

SOURCE=.\Ch3DiffindoEnd.txt
# End Source File
# Begin Source File

SOURCE=.\Ch3DiffindoIntro.txt
# End Source File
# Begin Source File

SOURCE=.\Ch4SpongifyEnd.txt
# End Source File
# Begin Source File

SOURCE=.\Ch4SpongifyIntro.txt
# End Source File
# Begin Source File

SOURCE=.\HagridBumpScene020.txt
# End Source File
# Begin Source File

SOURCE=.\HagridFromDiffindo.txt
# End Source File
# Begin Source File

SOURCE=.\HagridFrontEntrance.txt
# End Source File
# Begin Source File

SOURCE=.\TransfigClass.txt
# End Source File
# Begin Source File

SOURCE=.\TransfigHarryLosesPts.txt
# End Source File
# End Target
# End Project
