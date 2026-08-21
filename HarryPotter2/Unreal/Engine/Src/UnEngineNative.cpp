/*=============================================================================
	UnEngineNative.cpp: Native function lookup table for static libraries.
	Copyright 2000 Epic Games, Inc. All Rights Reserved.

	Revision history:
		* Created by Brandon Reinhart
=============================================================================*/

#include "EnginePrivate.h"

#if __STATIC_LINK
#include "UnEngineNative.h"
AActorNativeInfo GEngineAActorNatives[] =
{
	MAP_NATIVE(AActor, execPollSleep)
	MAP_NATIVE(AActor, execPollFinishAnim)
	MAP_NATIVE(AActor, execPollFinishInterpolation)
	MAP_NATIVE(AActor, execGetCurrentKeyState)
	MAP_NATIVE(AActor, execMultiply_ColorFloat)
	MAP_NATIVE(AActor, execAdd_ColorColor)
	MAP_NATIVE(AActor, execMultiply_FloatColor)
	MAP_NATIVE(AActor, execSubtract_ColorColor)
	MAP_NATIVE(AActor, execVisibleCollidingActors)
	MAP_NATIVE(AActor, execVisibleActors)
	MAP_NATIVE(AActor, execRadiusActors)
	MAP_NATIVE(AActor, execTraceActors)
	MAP_NATIVE(AActor, execTouchingActors)
	MAP_NATIVE(AActor, execBasedActors)
	MAP_NATIVE(AActor, execChildActors)
	MAP_NATIVE(AActor, execAllActors)
	MAP_NATIVE(AActor, execGetNextIntDesc)
	MAP_NATIVE(AActor, execGetNextInt)
	MAP_NATIVE(AActor, execGetCacheEntry)
	MAP_NATIVE(AActor, execMoveCacheEntry)
	MAP_NATIVE(AActor, execGetURLMap)
	MAP_NATIVE(AActor, execGetNextSkin)
	MAP_NATIVE(AActor, execGetMapName)
	MAP_NATIVE(AActor, execPlayerCanSeeMe)
	MAP_NATIVE(AActor, execMakeNoise)
	MAP_NATIVE(AActor, execGetSoundDuration)
	MAP_NATIVE(AActor, execDemoPlaySound)
	MAP_NATIVE(AActor, execPlayOwnedSound)
	MAP_NATIVE(AActor, execPlaySound)
	MAP_NATIVE(AActor, execSetTimer)
	MAP_NATIVE(AActor, execDestroy)
	MAP_NATIVE(AActor, execSpawn)
	MAP_NATIVE(AActor, execFastTrace)
	MAP_NATIVE(AActor, execTrace)
	MAP_NATIVE(AActor, execSetPhysics)
	MAP_NATIVE(AActor, execFinishInterpolation)
	MAP_NATIVE(AActor, execLinkSkelAnim)
	MAP_NATIVE(AActor, execHasAnim)
	MAP_NATIVE(AActor, execFinishAnim)
	MAP_NATIVE(AActor, execGetAnimGroup)
	MAP_NATIVE(AActor, execIsAnimating)
	MAP_NATIVE(AActor, execTweenAnim)
	MAP_NATIVE(AActor, execLoopAnim)
	MAP_NATIVE(AActor, execPlayAnim)
	MAP_NATIVE(AActor, execSetOwner)
	MAP_NATIVE(AActor, execSetBase)
	MAP_NATIVE(AActor, execAutonomousPhysics)
	MAP_NATIVE(AActor, execMoveSmooth)
	MAP_NATIVE(AActor, execSetRotation)
	MAP_NATIVE(AActor, execSetLocation)
	MAP_NATIVE(AActor, execMove)
	MAP_NATIVE(AActor, execSetCollisionSize)
	MAP_NATIVE(AActor, execSetCollision)
	MAP_NATIVE(AActor, execSleep)
	MAP_NATIVE(AActor, execError)
	MAP_NATIVE(AActor, execConsoleCommand)
	MAP_NATIVE(AActor, execSaveGameExists)
	MAP_NATIVE(AActor, execPlayMusic)
	MAP_NATIVE(AActor, execStopMusic)
	MAP_NATIVE(AActor, execStopAllMusic)
	{NULL, NULL}
};
IMPLEMENT_NATIVE_HANDLER(Engine,AActor);

APawnNativeInfo GEngineAPawnNatives[] =
{
	MAP_NATIVE(APawn, execPollWaitForLanding)
	MAP_NATIVE(APawn, execPollMoveTo)
	MAP_NATIVE(APawn, execPollMoveToward)
	MAP_NATIVE(APawn, execPollStrafeTo)
	MAP_NATIVE(APawn, execPollStrafeFacing)
	MAP_NATIVE(APawn, execPollTurnToward)
	MAP_NATIVE(APawn, execPollTurnTo)
	MAP_NATIVE(APawn, execClientHearSound)
	MAP_NATIVE(APawn, execCheckValidSkinPackage)
	MAP_NATIVE(APawn, execStopWaiting)
	MAP_NATIVE(APawn, execPickAnyTarget)
	MAP_NATIVE(APawn, execPickTarget)
	MAP_NATIVE(APawn, execRemovePawn)
	MAP_NATIVE(APawn, execAddPawn)
	MAP_NATIVE(APawn, execFindBestInventoryPath)
	MAP_NATIVE(APawn, execWaitForLanding)
	MAP_NATIVE(APawn, execFindStairRotation)
	MAP_NATIVE(APawn, execPickWallAdjust)
	MAP_NATIVE(APawn, execactorReachable)
	MAP_NATIVE(APawn, execpointReachable)
	MAP_NATIVE(APawn, execEAdjustJump)
	MAP_NATIVE(APawn, execClearPaths)
	MAP_NATIVE(APawn, execFindRandomDest)
	MAP_NATIVE(APawn, execFindPathToward)
	MAP_NATIVE(APawn, execFindPathTo)
	MAP_NATIVE(APawn, execFindPath)
	MAP_NATIVE(APawn, execCanSee)
	MAP_NATIVE(APawn, execLineOfSightTo)
	MAP_NATIVE(APawn, execTurnToward)
	MAP_NATIVE(APawn, execTurnTo)
	MAP_NATIVE(APawn, execStrafeFacing)
	MAP_NATIVE(APawn, execStrafeTo)
	MAP_NATIVE(APawn, execMoveToward)
	MAP_NATIVE(APawn, execMoveTo)
	{NULL, NULL}
};
IMPLEMENT_NATIVE_HANDLER(Engine,APawn);

APlayerPawnNativeInfo GEngineAPlayerPawnNatives[] =
{
	MAP_NATIVE(APlayerPawn, execPasteFromClipboard)
	MAP_NATIVE(APlayerPawn, execCopyToClipboard)
	MAP_NATIVE(APlayerPawn, execConsoleCommand)
	MAP_NATIVE(APlayerPawn, execGetPlayerNetworkAddress)
	MAP_NATIVE(APlayerPawn, execGetEntryLevel)
	MAP_NATIVE(APlayerPawn, execGetDefaultURL)
	MAP_NATIVE(APlayerPawn, execUpdateURL)
	MAP_NATIVE(APlayerPawn, execResetKeyboard)
	MAP_NATIVE(APlayerPawn, execClientTravel)
	{NULL, NULL}
};
IMPLEMENT_NATIVE_HANDLER(Engine,APlayerPawn);

ADecalNativeInfo GEngineADecalNatives[] =
{
	MAP_NATIVE(ADecal, execDetachDecal)
	MAP_NATIVE(ADecal, execAttachDecal)
	{NULL, NULL}
};
IMPLEMENT_NATIVE_HANDLER(Engine,ADecal);

AStatLogNativeInfo GEngineAStatLogNatives[] =
{
	MAP_NATIVE(AStatLog, execGetMapFileName)
	MAP_NATIVE(AStatLog, execGetGMTRef)
	MAP_NATIVE(AStatLog, execGetPlayerChecksum)
	MAP_NATIVE(AStatLog, execLogMutator)
	MAP_NATIVE(AStatLog, execInitialCheck)
	MAP_NATIVE(AStatLog, execBrowseRelativeLocalURL)
	MAP_NATIVE(AStatLog, execExecuteWorldLogBatcher)
	MAP_NATIVE(AStatLog, execBatchLocal)
	MAP_NATIVE(AStatLog, execExecuteSilentLogBatcher)
	MAP_NATIVE(AStatLog, execExecuteLocalLogBatcher)
	{NULL, NULL}
};
IMPLEMENT_NATIVE_HANDLER(Engine,AStatLog);

AStatLogFileNativeInfo GEngineAStatLogFileNatives[] =
{
	MAP_NATIVE(AStatLogFile, execFileLog)
	MAP_NATIVE(AStatLogFile, execFileFlush)
	MAP_NATIVE(AStatLogFile, execGetChecksum)
	MAP_NATIVE(AStatLogFile, execWatermark)
	MAP_NATIVE(AStatLogFile, execCloseLog)
	MAP_NATIVE(AStatLogFile, execOpenLog)
	{NULL, NULL}
};
IMPLEMENT_NATIVE_HANDLER(Engine,AStatLogFile);

AZoneInfoNativeInfo GEngineAZoneInfoNatives[] =
{
	MAP_NATIVE(AZoneInfo, execZoneActors)
	{NULL, NULL}
};
IMPLEMENT_NATIVE_HANDLER(Engine,AZoneInfo);

AWarpZoneInfoNativeInfo GEngineAWarpZoneInfoNatives[] =
{
	MAP_NATIVE(AWarpZoneInfo, execUnWarp)
	MAP_NATIVE(AWarpZoneInfo, execWarp)
	{NULL, NULL}
};
IMPLEMENT_NATIVE_HANDLER(Engine,AWarpZoneInfo);

ALevelInfoNativeInfo GEngineALevelInfoNatives[] =
{
	MAP_NATIVE(ALevelInfo, execGetAddressURL)
	MAP_NATIVE(ALevelInfo, execGetLocalURL)
	{NULL, NULL}
};
IMPLEMENT_NATIVE_HANDLER(Engine,ALevelInfo);

AGameInfoNativeInfo GEngineAGameInfoNatives[] =
{
	MAP_NATIVE(AGameInfo, execParseKillMessage)
	MAP_NATIVE(AGameInfo, execGetNetworkNumber)
	{NULL, NULL}
};
IMPLEMENT_NATIVE_HANDLER(Engine,AGameInfo);

ANavigationPointNativeInfo GEngineANavigationPointNatives[] =
{
	MAP_NATIVE(ANavigationPoint, execdescribeSpec)
	{NULL, NULL}
};
IMPLEMENT_NATIVE_HANDLER(Engine,ANavigationPoint);

UCanvasNativeInfo GEngineUCanvasNatives[] =
{
	MAP_NATIVE(UCanvas, execStrLen)
	MAP_NATIVE(UCanvas, execDrawText)
	MAP_NATIVE(UCanvas, execDrawTile)
	MAP_NATIVE(UCanvas, execDrawActor)
	MAP_NATIVE(UCanvas, execDrawClippedActor)
	MAP_NATIVE(UCanvas, execDrawTileClipped)
	MAP_NATIVE(UCanvas, execDrawTextClipped)
	MAP_NATIVE(UCanvas, execTextSize)
	MAP_NATIVE(UCanvas, execDrawPortal)
	{NULL, NULL}
};
IMPLEMENT_NATIVE_HANDLER(Engine,UCanvas);

UConsoleNativeInfo GEngineUConsoleNatives[] =
{
	MAP_NATIVE(UConsole, execConsoleCommand)
	MAP_NATIVE(UConsole, execSaveTimeDemo)
	MAP_NATIVE(UConsole, execCreateNativeFont)
	{NULL, NULL}
};
IMPLEMENT_NATIVE_HANDLER(Engine,UConsole);

UScriptedTextureNativeInfo GEngineUScriptedTextureNatives[] =
{
	MAP_NATIVE(UScriptedTexture, execDrawText)
	MAP_NATIVE(UScriptedTexture, execDrawTile)
	MAP_NATIVE(UScriptedTexture, execDrawColoredText)
	MAP_NATIVE(UScriptedTexture, execReplaceTexture)
	MAP_NATIVE(UScriptedTexture, execTextSize)	
	{NULL, NULL}
};
IMPLEMENT_NATIVE_HANDLER(Engine,UScriptedTexture);
#endif
