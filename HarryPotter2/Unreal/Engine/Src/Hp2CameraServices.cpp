#include "EnginePrivate.h"
#include "UnRender.h"
#include "UnNet.h"
#include "Hp2CameraServices.h"

// Thunks for the Rust UnCamera port. Each one is a single original C++ operation.

static int Hp2CameraViewportMaintainVerticalFov( void* Viewport )
{
	return static_cast<UViewport*>(Viewport)->GetOuterUClient()->MaintainVerticalFOV;
}

static void Hp2CameraRenderDevSetSceneNode( void* RenDev, void* Frame )
{
	static_cast<URenderDevice*>(RenDev)->SetSceneNode( static_cast<FSceneNode*>(Frame) );
}

static const void* Hp2CameraGMathViewCoords( void )
{
	return &GMath.ViewCoords;
}

static float Hp2CameraGMathSinTab( int Index )
{
	return GMath.SinTab( Index );
}

static float Hp2CameraGMathCosTab( int Index )
{
	return GMath.CosTab( Index );
}

static void Hp2CameraRenderDevDraw2DLine( void* RenDev, void* Frame, const void* Color, unsigned int LineFlags, const void* P1, const void* P2 )
{
	static_cast<URenderDevice*>(RenDev)->Draw2DLine(
		static_cast<FSceneNode*>(Frame),
		*static_cast<const FPlane*>(Color),
		LineFlags,
		*static_cast<const FVector*>(P1),
		*static_cast<const FVector*>(P2) );
}

static void Hp2CameraRenderDevDraw2DPoint( void* RenDev, void* Frame, const void* Color, unsigned int LineFlags, float X1, float Y1, float X2, float Y2, float Z )
{
	static_cast<URenderDevice*>(RenDev)->Draw2DPoint(
		static_cast<FSceneNode*>(Frame),
		*static_cast<const FPlane*>(Color),
		LineFlags,
		X1, Y1, X2, Y2, Z );
}

static int Hp2CameraParseCommand( void* Cursor, const void* Token )
{
	return ParseCommand( static_cast<const TCHAR**>(Cursor), static_cast<const TCHAR*>(Token) );
}

static int Hp2CameraParseToken( void* Cursor, void* Buf, int Len )
{
	return ParseToken( *static_cast<const TCHAR**>(Cursor), static_cast<TCHAR*>(Buf), Len, 0 );
}

static int Hp2CameraParseLine( void* Cursor, void* Buf, int Len )
{
	return ParseLine( static_cast<const TCHAR**>(Cursor), static_cast<TCHAR*>(Buf), Len );
}

static int Hp2CameraParseInt( const void* Stream, const void* Match, int* Value )
{
	return Parse( static_cast<const TCHAR*>(Stream), static_cast<const TCHAR*>(Match), *Value );
}

static int Hp2CameraAppAtoi( const void* S )
{
	return appAtoi( static_cast<const TCHAR*>(S) );
}

static const void* Hp2CameraAppUserDir( void )
{
	return appUserDir();
}

static int Hp2CameraCheckEnabled( void )
{
	return DO_CHECK;
}

static int Hp2CameraFileSize( const void* Path )
{
	return GFileManager->FileSize( static_cast<const TCHAR*>(Path) );
}

static void* Hp2CameraCreateFileWriter( const void* Path )
{
	return GFileManager->CreateFileWriter( static_cast<const TCHAR*>(Path) );
}

static void* Hp2CameraCreateSlotFileWriter( const void* File )
{
	return GFileManager->CreateFileWriter( *(GSys->SaveSlotPath * static_cast<const TCHAR*>(File)) );
}

static void* Hp2CameraCreateUserDirFileWriter( const void* File )
{
	return GFileManager->CreateFileWriter( *(FString(appUserDir()) * static_cast<const TCHAR*>(File)) );
}

static void Hp2CameraArchiveSerialize( void* Ar, const void* Data, int Count )
{
	static_cast<FArchive*>(Ar)->Serialize( const_cast<void*>(Data), Count );
}

static void Hp2CameraArchiveDelete( void* Ar )
{
	delete static_cast<FArchive*>(Ar);
}

static void Hp2CameraLogFormat( void* Ar, const void* Fmt )
{
	static_cast<FOutputDevice*>(Ar)->Logf( static_cast<const TCHAR*>(Fmt) );
}

static void Hp2CameraLogStr( void* Ar, const void* Fmt, const void* Arg )
{
	static_cast<FOutputDevice*>(Ar)->Logf( static_cast<const TCHAR*>(Fmt), static_cast<const TCHAR*>(Arg) );
}

static void Hp2CameraLogInt( void* Ar, const void* Fmt, int Arg )
{
	static_cast<FOutputDevice*>(Ar)->Logf( static_cast<const TCHAR*>(Fmt), Arg );
}

static void Hp2CameraLogExecWarning( void* Ar, const void* Fmt, const void* Arg )
{
	static_cast<FOutputDevice*>(Ar)->Logf( NAME_ExecWarning, static_cast<const TCHAR*>(Fmt), static_cast<const TCHAR*>(Arg) );
}

static void Hp2CameraDebugfStr( const void* Fmt, const void* Arg )
{
	debugf( static_cast<const TCHAR*>(Fmt), static_cast<const TCHAR*>(Arg) );
}

static const void* Hp2CameraLocalizeFileNotFound( void )
{
	return LocalizeError( "FileNotFound", TEXT("Core") );
}

static int Hp2CameraIsRunning( void )
{
	return GIsRunning;
}

static void Hp2CameraViewportStampCurrentTime( void* Viewport )
{
	static_cast<UViewport*>(Viewport)->CurrentTime = appSeconds();
}

static void Hp2CameraViewportSetCurrentNetSpeed( void* Viewport, int Speed )
{
	static_cast<UViewport*>(Viewport)->CurrentNetSpeed = Speed;
}

static void Hp2CameraViewportShowAllActors( void* Viewport )
{
	UViewport* Vp = static_cast<UViewport*>(Viewport);
	for( INT i=0; i<Vp->Actor->GetLevel()->Actors.Num(); i++ )
		if( Vp->Actor->GetLevel()->Actors(i) )
			Vp->Actor->GetLevel()->Actors(i)->bHidden = 0;
}

static void Hp2CameraViewportReport( void* Viewport, const void* Cmd, void* Ar )
{
	UViewport* Vp = static_cast<UViewport*>(Viewport);
	FOutputDevice& OutAr = *static_cast<FOutputDevice*>(Ar);
	const TCHAR* CmdText = static_cast<const TCHAR*>(Cmd);
	FStringOutputDevice Str;
#if 1 //Logging added by Legend on 4/12/2000
	Str.Logf ( TEXT("Report: %s\r\n"), CmdText );
#else
	Str.Log ( TEXT("Report:\r\n") );
#endif
	Str.Logf( TEXT("   Version: %s %s\r\n"), appFromAnsi(__DATE__), appFromAnsi(__TIME__) );
	Str.Logf( TEXT("   Player class: %s\r\n"), Vp->Actor->GetClass()->GetName() );
	Str.Logf( TEXT("   URL: %s\r\n"), *Vp->Actor->GetLevel()->URL.String() );
	Str.Logf( TEXT("   Location: %i %i %i\r\n"), appRound(Vp->Actor->Location.X), appRound(Vp->Actor->Location.Y), appRound(Vp->Actor->Location.Z) );
	if( Vp->Actor->Level->Game==NULL )
	{
		Str.Logf( TEXT("   Network client\r\n") );
	}
	else
	{
		Str.Logf( TEXT("   Game class: %s\r\n"), Vp->Actor->Level->Game->GetClass()->GetName() );
		Str.Logf( TEXT("   Difficulty: %i\r\n"), Vp->Actor->Level->Game->Difficulty );
	}
#if 1 //Logging added by Legend on 4/12/2000
	OutAr.Logf( *Str, CmdText );
#endif
	appClipboardCopy( *Str );
}

static void Hp2CameraViewportMouseDelta( void* Viewport, int Kind )
{
	UViewport* Vp = static_cast<UViewport*>(Viewport);
	Vp->GetOuterUClient()->Engine->MouseDelta( Vp, Kind, 0, 0 );
}

static void Hp2CameraViewportUpdateInput( void* Viewport, int Flags )
{
	static_cast<UViewport*>(Viewport)->UpdateInput( Flags );
}

static int Hp2CameraViewportScriptConsoleExec( void* Viewport, const void* Cmd, void* Ar )
{
	UViewport* Vp = static_cast<UViewport*>(Viewport);
	return Vp->Console && Vp->Console->ScriptConsoleExec( static_cast<const TCHAR*>(Cmd), *static_cast<FOutputDevice*>(Ar), Vp->Actor );
}

static int Hp2CameraViewportPlayerExec( void* Viewport, const void* Cmd, void* Ar )
{
	return static_cast<UViewport*>(Viewport)->UPlayer::Exec( static_cast<const TCHAR*>(Cmd), *static_cast<FOutputDevice*>(Ar) );
}

static int Hp2CameraViewportExec( void* Viewport, const void* Cmd, void* Ar )
{
	return static_cast<UViewport*>(Viewport)->Exec( static_cast<const TCHAR*>(Cmd), *static_cast<FOutputDevice*>(Ar) );
}

static void* Hp2CameraViewportNetDriver( void* Viewport )
{
	return static_cast<UViewport*>(Viewport)->Actor->GetLevel()->NetDriver;
}

static int Hp2CameraViewportModelNodeCount( void* Viewport )
{
	return static_cast<UViewport*>(Viewport)->Actor->GetLevel()->Model->Nodes.Num();
}

static void Hp2CameraViewportSnapBuffEmpty( void* Viewport )
{
	static_cast<UViewport*>(Viewport)->snapBuff.Empty();
}

static void Hp2CameraViewportSnapBuffAdd( void* Viewport, int Count )
{
	static_cast<UViewport*>(Viewport)->snapBuff.Add( Count );
}

static void Hp2CameraViewportHitSizesAdd( void* Viewport, int Size )
{
	static_cast<UViewport*>(Viewport)->HitSizes.AddItem( Size );
}

static int Hp2CameraViewportHitSizesPop( void* Viewport )
{
	return static_cast<UViewport*>(Viewport)->HitSizes.Pop();
}

static int Hp2CameraInputExec( void* Input, const void* Cmd, void* Ar )
{
	return static_cast<UInput*>(Input)->Exec( static_cast<const TCHAR*>(Cmd), *static_cast<FOutputDevice*>(Ar) );
}

static void Hp2CameraInputReadInput( void* Input, float Delta )
{
	static_cast<UInput*>(Input)->ReadInput( Delta, *GLog );
}

static int Hp2CameraRenderDevExec( void* RenDev, const void* Cmd, void* Ar )
{
	return static_cast<URenderDevice*>(RenDev)->Exec( static_cast<const TCHAR*>(Cmd), *static_cast<FOutputDevice*>(Ar) );
}

static void Hp2CameraRenderDevReadPixels( void* RenDev, void* Dest )
{
	static_cast<URenderDevice*>(RenDev)->ReadPixels( static_cast<FColor*>(Dest) );
}

static void Hp2CameraRenderDevLock( void* RenDev, const void* FlashScale, const void* FlashFog, const void* ScreenClear, unsigned int Flags, void* HitData, int* HitSize )
{
	static_cast<URenderDevice*>(RenDev)->Lock(
		*static_cast<const FPlane*>(FlashScale),
		*static_cast<const FPlane*>(FlashFog),
		*static_cast<const FPlane*>(ScreenClear),
		Flags,
		static_cast<BYTE*>(HitData),
		HitSize );
}

static void Hp2CameraRenderDevUnlock( void* RenDev, int Blit )
{
	static_cast<URenderDevice*>(RenDev)->Unlock( Blit );
}

static void Hp2CameraRenderDevPushHit( void* RenDev, const void* Hit, int Size )
{
	static_cast<URenderDevice*>(RenDev)->PushHit( const_cast<BYTE*>(static_cast<const BYTE*>(Hit)), Size );
}

static void Hp2CameraRenderDevPopHit( void* RenDev, int Size, int Force )
{
	static_cast<URenderDevice*>(RenDev)->PopHit( Size, Force );
}

static int Hp2CameraRenderDevDetailTextures( void* RenDev )
{
	return static_cast<URenderDevice*>(RenDev)->DetailTextures;
}

static void Hp2CameraRenderDevToggleDetailTextures( void* RenDev )
{
	URenderDevice* Rd = static_cast<URenderDevice*>(RenDev);
	Rd->DetailTextures ^= 1;
	Rd->SaveConfig();
}

static int Hp2CameraHitProxySize( const void* Hit )
{
	return static_cast<const HHitProxy*>(Hit)->Size;
}

static void Hp2CameraHitProxySetSize( void* Hit, int Size )
{
	static_cast<HHitProxy*>(Hit)->Size = Size;
}

static void Hp2CameraHitProxySetParent( void* Hit, void* Parent )
{
	static_cast<HHitProxy*>(Hit)->Parent = static_cast<HHitProxy*>(Parent);
}

static void* Hp2CameraNetDriverServerConnection( void* Driver )
{
	return static_cast<UNetDriver*>(Driver)->ServerConnection;
}

static int Hp2CameraNetDriverMaxClientRate( void* Driver )
{
	return static_cast<UNetDriver*>(Driver)->MaxClientRate;
}

static int Hp2CameraConnectionHasLanOption( void* Connection )
{
	return static_cast<UNetConnection*>(Connection)->URL.HasOption( TEXT("LAN") );
}

static void Hp2CameraConnectionSetCurrentNetSpeed( void* Connection, int Speed )
{
	static_cast<UNetConnection*>(Connection)->CurrentNetSpeed = Speed;
}

static void Hp2CameraConnectionLogfFormat( void* Connection, const void* Fmt )
{
	static_cast<UNetConnection*>(Connection)->Logf( static_cast<const TCHAR*>(Fmt) );
}

static void Hp2CameraConnectionLogfInt( void* Connection, const void* Fmt, int Arg )
{
	static_cast<UNetConnection*>(Connection)->Logf( static_cast<const TCHAR*>(Fmt), Arg );
}

static void Hp2CameraPlayerSetConfiguredInternetSpeed( int Rate )
{
	GetDefault<UPlayer>()->ConfiguredInternetSpeed = Rate;
	GetDefault<UPlayer>()->SaveConfig();
}

static void* Hp2CameraImportTextBuffer( const void* File )
{
	return ImportObject<UTextBuffer>( UObject::GetTransientPackage(), NAME_None, 0, static_cast<const TCHAR*>(File) );
}

static const void* Hp2CameraTextBufferText( void* Text )
{
	return *static_cast<UTextBuffer*>(Text)->Text;
}

static void Hp2CameraTextBufferAddToRoot( void* Text )
{
	static_cast<UTextBuffer*>(Text)->AddToRoot();
}

static void Hp2CameraTextBufferRelease( void* Text )
{
	UTextBuffer* Buf = static_cast<UTextBuffer*>(Text);
	Buf->RemoveFromRoot();
	delete Buf;
}

const Hp2CameraServices GHp2CameraServices =
{
	Hp2CameraViewportMaintainVerticalFov,
	Hp2CameraRenderDevSetSceneNode,
	Hp2CameraGMathViewCoords,
	Hp2CameraGMathSinTab,
	Hp2CameraGMathCosTab,
	Hp2CameraRenderDevDraw2DLine,
	Hp2CameraRenderDevDraw2DPoint,
	Hp2CameraParseCommand,
	Hp2CameraParseToken,
	Hp2CameraParseLine,
	Hp2CameraParseInt,
	Hp2CameraAppAtoi,
	Hp2CameraAppUserDir,
	Hp2CameraCheckEnabled,
	Hp2CameraFileSize,
	Hp2CameraCreateFileWriter,
	Hp2CameraCreateSlotFileWriter,
	Hp2CameraCreateUserDirFileWriter,
	Hp2CameraArchiveSerialize,
	Hp2CameraArchiveDelete,
	Hp2CameraLogFormat,
	Hp2CameraLogStr,
	Hp2CameraLogInt,
	Hp2CameraLogExecWarning,
	Hp2CameraDebugfStr,
	Hp2CameraLocalizeFileNotFound,
	Hp2CameraIsRunning,
	Hp2CameraViewportStampCurrentTime,
	Hp2CameraViewportSetCurrentNetSpeed,
	Hp2CameraViewportShowAllActors,
	Hp2CameraViewportReport,
	Hp2CameraViewportMouseDelta,
	Hp2CameraViewportUpdateInput,
	Hp2CameraViewportScriptConsoleExec,
	Hp2CameraViewportPlayerExec,
	Hp2CameraViewportExec,
	Hp2CameraViewportNetDriver,
	Hp2CameraViewportModelNodeCount,
	Hp2CameraViewportSnapBuffEmpty,
	Hp2CameraViewportSnapBuffAdd,
	Hp2CameraViewportHitSizesAdd,
	Hp2CameraViewportHitSizesPop,
	Hp2CameraInputExec,
	Hp2CameraInputReadInput,
	Hp2CameraRenderDevExec,
	Hp2CameraRenderDevReadPixels,
	Hp2CameraRenderDevLock,
	Hp2CameraRenderDevUnlock,
	Hp2CameraRenderDevPushHit,
	Hp2CameraRenderDevPopHit,
	Hp2CameraRenderDevDetailTextures,
	Hp2CameraRenderDevToggleDetailTextures,
	Hp2CameraHitProxySize,
	Hp2CameraHitProxySetSize,
	Hp2CameraHitProxySetParent,
	Hp2CameraNetDriverServerConnection,
	Hp2CameraNetDriverMaxClientRate,
	Hp2CameraConnectionHasLanOption,
	Hp2CameraConnectionSetCurrentNetSpeed,
	Hp2CameraConnectionLogfFormat,
	Hp2CameraConnectionLogfInt,
	Hp2CameraPlayerSetConfiguredInternetSpeed,
	Hp2CameraImportTextBuffer,
	Hp2CameraTextBufferText,
	Hp2CameraTextBufferAddToRoot,
	Hp2CameraTextBufferRelease,
};

static_assert( sizeof(Hp2CameraServices) == 66*sizeof(void*), "Hp2CameraServices field count changed; update the Rust CameraServices layout and this assert together" );
