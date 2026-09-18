/*=============================================================================
	UnPenLev.cpp: Unreal pending level class.
	Copyright 1997-1999 Epic Games, Inc. All Rights Reserved.

	Revision history:
		* Created by Tim Sweeney
=============================================================================*/

#include "EnginePrivate.h"
#include "UnNet.h"

/*-----------------------------------------------------------------------------
	UPendingLevel implementation.
-----------------------------------------------------------------------------*/

//
// Constructor.
//
UPendingLevel::UPendingLevel( UEngine* InEngine, const FURL& InURL )
:	ULevelBase( InEngine, InURL )
{}
IMPLEMENT_CLASS(UPendingLevel);


/*-----------------------------------------------------------------------------
	UNetPendingLevel implementation.
-----------------------------------------------------------------------------*/

//
// Constructor.
//
UNetPendingLevel::UNetPendingLevel( UEngine* InEngine, const FURL& InURL )
:	UPendingLevel( InEngine, InURL )
{
	guard(UNetPendingLevel::UNetPendingLevel);

	// Init.
	Error     = TEXT("");
	NetDriver = NULL;

	// Try to create network driver.
	UClass* NetDriverClass = StaticLoadClass( UNetDriver::StaticClass(), NULL, TEXT("ini:Engine.Engine.NetworkDevice"), NULL, LOAD_NoFail, NULL );
	NetDriver = ConstructObject<UNetDriver>( NetDriverClass );
	if( NetDriver->InitConnect( this, URL, Error ) )
	{
		// Send initial message.
		NetDriver->ServerConnection->Logf( TEXT("HELLO REVISION=0 MINVER=%i VER=%i"), ENGINE_MIN_NET_VERSION, ENGINE_VERSION );
		NetDriver->ServerConnection->FlushNet();
	}
	else
	{
		delete NetDriver;
		NetDriver=NULL;
	}
	unguard;
}

//
// FNetworkNotify interface.
//
EAcceptConnection UNetPendingLevel::NotifyAcceptingConnection()
{
	guard(UNetPendingLevel::NotifyAcceptingConnection);
	return ACCEPTC_Reject;
	unguard;
}
void UNetPendingLevel::NotifyAcceptedConnection( class UNetConnection* Connection )
{
	guard(UNetPendingLevel::NotifyAcceptedConnection);
	unguard;
}
UBOOL UNetPendingLevel::NotifyAcceptingChannel( class UChannel* Channel )
{
	guard(UNetPendingLevel::NotifyAcceptingChannel);
	return 0;
	unguard;
}
ULevel* UNetPendingLevel::NotifyGetLevel()
{
	guard(UNetPendingLevel::NotifyGetLevel);
	return NULL;
	unguard;
}
void UNetPendingLevel::NotifyReceivedText( UNetConnection* Connection, const TCHAR* Text )
{
	guard(UNetPendingLevel::NotifyReceivedText);
	check(Connection==NetDriver->ServerConnection);
	debugf( NAME_DevNet, TEXT("PendingLevel received: %s"), Text );

	// This client got a response from the server.
	if( ParseCommand(&Text,TEXT("UPGRADE")) )
	{
		// Report mismatch.
		INT RemoteMinVer=0, RemoteVer=0;
		Parse( Text, TEXT("MINVER="), RemoteMinVer );
		Parse( Text, TEXT("VER="),    RemoteVer    );
		if( ENGINE_VERSION < RemoteMinVer )
		{
			// Upgrade message.
			Error = LocalizeError("ClientOutdated");
		}
		else
		{
			// Downgrade message.
			Error = LocalizeError("ServerOutdated");
		}
	}
	else if( ParseCommand(&Text,TEXT("FAILURE")) )
	{
		// Report problem to user.
		Error = LocalizeError("RejectedByServer");
	}
	else if( ParseCommand(&Text,TEXT("FAILCODE")) )
	{
		// record failure reason and URL.
		FURL NewURL(URL);
		for( INT i=NewURL.Op.Num()-1; i>=0; i-- )
			if( NewURL.Op(i).Left(9).Caps()==TEXT("PASSWORD=") )
				NewURL.Op.Remove( i );
		FailCode = Text;
		FailURL = NewURL.String();
	}
	else if( ParseCommand( &Text, TEXT("USES") ) )
	{
		// Dependency information.
		FPackageInfo& Info = *new(Connection->PackageMap->List)FPackageInfo(NULL);
		TCHAR PackageName[NAME_SIZE]=TEXT("");
		Parse( Text, TEXT("GUID=" ), Info.Guid );
		Parse( Text, TEXT("GEN=" ),  Info.RemoteGeneration );
		Parse( Text, TEXT("SIZE="),  Info.FileSize );
		Parse( Text, TEXT("FLAGS="), Info.PackageFlags );
		Parse( Text, TEXT("PKG="), PackageName, ARRAY_COUNT(PackageName) );
		Parse( Text, TEXT("FNAME="), Info.URL );
		Info.Parent = CreatePackage(NULL,PackageName);
	}
	else if( ParseCommand(&Text,TEXT("USERFLAG")) )
	{
		Connection->UserFlags = appAtoi(Text);
	}
	else if( ParseCommand( &Text, TEXT("CHALLENGE") ) )
	{
		// Challenged by server.
		INT RemoteStats = 0, i=0;
		Parse( Text, TEXT("VER="), Connection->NegotiatedVer );
		Parse( Text, TEXT("CHALLENGE="), Connection->Challenge );

		Parse( Text, TEXT("STATS="), RemoteStats );
		if (RemoteStats == 1)
		{
			// Get the Player Pawn.
			APlayerPawn* PlayerPawn = Engine->Client->Viewports(0)->Actor;

			// Calculate the player's checksum for ngWorldStats.
			FString URLOption;
			if (PlayerPawn->ngWorldSecret == TEXT(""))
			{
				URLOption += FString::Printf( TEXT("Checksum=NoChecksum") );
			} else {
				FMD5Context PContext;
				appMD5Init( &PContext );
				appMD5UpdateStringUtf16LE( &PContext, PlayerPawn->PlayerReplicationInfo->PlayerName, 0 );
				appMD5UpdateStringUtf16LE( &PContext, PlayerPawn->ngWorldSecret, 0 );
				BYTE Digest[16];
				appMD5Final( Digest, &PContext );
				URLOption += FString::Printf( TEXT("Checksum=") );
				for (i=0; i<16; i++)
					URLOption += FString::Printf(TEXT("%02x"), Digest[i]);
			}
			URL.AddOption( *URLOption );
		}

		FURL PartialURL(URL);
		PartialURL.Host = TEXT("");
		for( i=URL.Op.Num()-1; i>=0; i-- )
			if( URL.Op(i).Left(5)==TEXT("game=") )
				URL.Op.Remove( i );
		NetDriver->ServerConnection->Logf( TEXT("NETSPEED %i"), Connection->CurrentNetSpeed );
		NetDriver->ServerConnection->Logf( TEXT("LOGIN RESPONSE=%i URL=%s"), Engine->ChallengeResponse(Connection->Challenge), *PartialURL.String() );
		NetDriver->ServerConnection->FlushNet();
	}
	else if( ParseCommand( &Text, TEXT("DLMGR") ) )
	{
		INT i = Connection->DownloadInfo.AddZeroed();
		Parse( Text, TEXT("CLASS="), Connection->DownloadInfo(i).ClassName );
		Parse( Text, TEXT("PARAMS="), Connection->DownloadInfo(i).Params );
		ParseUBOOL( Text, TEXT("COMPRESSION="), Connection->DownloadInfo(i).Compression );
		Connection->DownloadInfo(i).Class = StaticLoadClass( UDownload::StaticClass(), NULL, *Connection->DownloadInfo(i).ClassName, NULL, LOAD_NoWarn | LOAD_Quiet, NULL );
		if( !Connection->DownloadInfo(i).Class )
			Connection->DownloadInfo.Remove(i);
	}
	else if( ParseCommand( &Text, TEXT("WELCOME") ) )
	{
		// Server accepted connection.
		debugf( NAME_DevNet, TEXT("Welcomed by server: %s"), Text );

		// Parse welcome message.
		Parse( Text, TEXT("LEVEL="), URL.Map );
		ParseUBOOL( Text, TEXT("LONE="), LonePlayer );
		Parse( Text, TEXT("CHALLENGE="), Connection->Challenge );

		// Make sure all packages we need are downloadable.
		for( INT i=0; i<Connection->PackageMap->List.Num(); i++ )
		{
			TCHAR Filename[256];
			FPackageInfo& Info = Connection->PackageMap->List(i);
			if( !appFindPackageFile( Info.Parent->GetName(), &Info.Guid, Filename ) )
			{
				appSprintf( Filename, TEXT("%s%s"), Info.Parent->GetName(), DLLEXT );
				if( GFileManager->FileSize(Filename) <= 0 )
				{
					// We need to download this package.
					FilesNeeded++;
					Info.PackageFlags |= PKG_Need;
					if( !NetDriver->AllowDownloads || !(Info.PackageFlags & PKG_AllowDownload) )
					{
						Error = FString::Printf( TEXT("Downloading '%s' not allowed"), Info.Parent->GetName() );
						Connection->Channels[0]->Close();
						Connection->FlushNet();
						if( Connection->RemoteVersion <= 420 ) //!!oldver
							Connection->State = USOCK_Closed;
						return;
					}
				}
			}
		}

		// Send first download request.
		ReceiveNextFile( Connection );

		// We have successfully connected.
		Success = 1;
	}
	else
	{
		// Other command.
	}
	unguard;
}
UBOOL UNetPendingLevel::TrySkipFile()
{
	guard(UNetPendingLevel::TrySkipFile);
	return NetDriver->ServerConnection->Download && NetDriver->ServerConnection->Download->TrySkipFile();
	unguard;
}
void UNetPendingLevel::NotifyReceivedFile( UNetConnection* Connection, INT PackageIndex, const TCHAR* InError, UBOOL Skipped )
{
	guard(UNetPendingLevel::NotifyReceivedFile);
	check(Connection->PackageMap->List.IsValidIndex(PackageIndex));

	// Map pack to package.
	FPackageInfo& Info = Connection->PackageMap->List(PackageIndex);
	if( *InError )
	{
		if( Connection->DownloadInfo.Num() > 1 )
		{
			// Try with the next download method.
			Connection->DownloadInfo.Remove(0);
			ReceiveNextFile( Connection );
		}
		else
		{
			// If transfer failed, so propagate error.
			if( Error==TEXT("") )
				Error = FString::Printf( LocalizeError("DownloadFailed"), Info.Parent->GetName(), InError );
		}
	}
	else
	{
		// Now that a file has been successfully received, mark its package as downloaded.
		check(Connection==NetDriver->ServerConnection);
		check(Info.PackageFlags&PKG_Need);
		Info.PackageFlags &= ~PKG_Need;
		FilesNeeded--;
		if( Skipped )
			Connection->PackageMap->List.Remove( PackageIndex );
		// Send next download request.
		ReceiveNextFile( Connection );
	}
	unguard;
}
void UNetPendingLevel::ReceiveNextFile( UNetConnection* Connection )
{
	guard(UNetPendingLevel::ReceiveNextFile);
	for( INT i=0; i<Connection->PackageMap->List.Num(); i++ )
		if( Connection->PackageMap->List(i).PackageFlags & PKG_Need )
			{Connection->ReceiveFile( i ); return;}
	if( Connection->Download )
		delete Connection->Download;
	unguard;
}

UBOOL UNetPendingLevel::NotifySendingFile( UNetConnection* Connection, FGuid Guid )
{
	guard(UNetPendingLevel::NotifySendingFile);

	// Server has requested a file from this client.
	debugf( NAME_DevNet, LocalizeError("RequestDenied") );
	return 0;

	unguard;
}

//
// Update the pending level's status.
//
void UNetPendingLevel::Tick( FLOAT DeltaTime )
{
	guard(UNetPendingLevel::Tick);
	check(NetDriver);
	check(NetDriver->ServerConnection);

	// Handle timed out or failed connection.
	if( NetDriver->ServerConnection->State==USOCK_Closed && Error==TEXT("") )
	{
		Error = LocalizeError("ConnectionFailed");
		return;
	}

	// Update network driver.
	NetDriver->TickDispatch( DeltaTime );
	NetDriver->TickFlush();

	unguard;
}
//
// Send JOIN to other end
//
void UNetPendingLevel::SendJoin()
{
	guard(UNetPendingLevel::SendJoin);
	SentJoin = 1;
	NetDriver->ServerConnection->Logf( TEXT("JOIN") );
	NetDriver->ServerConnection->FlushNet();
	unguard;
}
IMPLEMENT_CLASS(UNetPendingLevel);

/*-----------------------------------------------------------------------------
	The End.
-----------------------------------------------------------------------------*/
