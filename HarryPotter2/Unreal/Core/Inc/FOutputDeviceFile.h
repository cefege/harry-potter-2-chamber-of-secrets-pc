/*=============================================================================
	FOutputDeviceFile.h: ANSI file output device.
	Copyright 1997-1999 Epic Games, Inc. All Rights Reserved.

	Revision history:
		* Created by Tim Sweeney
=============================================================================*/
//
// ANSI file output device.
//
class FOutputDeviceFile : public FOutputDevice
{
public:
	FOutputDeviceFile()
	: LogAr( NULL )
	, Opened( 0 )
	, Dead( 0 )
	{
		Filename[0]=0;
	}
	~FOutputDeviceFile()
	{
		if( LogAr )
		{
			Logf( NAME_Log, TEXT("Log file closed, %s"), appTimestamp() );
			delete LogAr;
			LogAr = NULL;
		}
	}
	void Serialize( const TCHAR* Data, enum EName Event )
	{
		static UBOOL Entry=0;
		if( !GIsCriticalError || Entry )
		{
			if( !FName::SafeSuppressed(Event) )
			{
				if( !LogAr && !Dead )
				{
					// Make log filename.
					if( !Filename[0] )
					{
						appStrcpy( Filename, appUserDir() );
						if
						(	!Parse(appCmdLine(), TEXT("LOG="), Filename+appStrlen(Filename), ARRAY_COUNT(Filename)-appStrlen(Filename) )
						&&	!Parse(appCmdLine(), TEXT("ABSLOG="), Filename, ARRAY_COUNT(Filename) ) )
						{
							appStrcat( Filename, appPackage() );
							appStrcat( Filename, TEXT(".log") );
						}
					}

					// Open log file.
					LogAr = GFileManager->CreateFileWriter( Filename, FILEWRITE_AllowRead|FILEWRITE_Unbuffered|(Opened?FILEWRITE_Append:0));
					if( LogAr )
					{
						Opened = 1;
#if UNICODE && !FORCE_ANSI_LOG
						BYTE UnicodeBOM[2] = {0xff,0xfe};
						LogAr->Serialize( UnicodeBOM, appCheckedIntSize(sizeof(UnicodeBOM)) );
#endif
						Logf( NAME_Log, TEXT("Log file open, %s"), appTimestamp() );
					}
					else Dead = 1;
				}
				if( LogAr && Event!=NAME_Title )
				{
#if FORCE_ANSI_LOG && UNICODE
					TCHAR Ch[1024];
					ANSICHAR ACh[1024];
					appSprintf( Ch, TEXT("%s: %s%s"), FName::SafeString(Event), Data, LINE_TERMINATOR );
					for( INT i=0; Ch[i]; i++ )
						ACh[i] = ToAnsi(Ch[i] );
					ACh[i] = 0;
					LogAr->Serialize( ACh, i );
#else
					WriteRaw( FName::SafeString(Event) );
					WriteRaw( TEXT(": ") );
					WriteRaw( Data );
					WriteRaw( LINE_TERMINATOR );
#endif
				}
				if( GLogHook )
					GLogHook->Serialize( Data, Event );
			}
		}
		else
		{
			Entry=1;
			try
			{
				// Ignore errors to prevent infinite-recursive exception reporting.
				Serialize( Data, Event );
			}
			catch( ... )
			{}
			Entry=0;
		}
	}
	FArchive* LogAr;
	TCHAR Filename[1024];
private:
	UBOOL Opened, Dead;
	void WriteRaw( const TCHAR* C )
	{
#if UNICODE
		BYTE Encoded[512];
		INT ByteCount = 0;
		while( *C )
		{
			UNICHAR Units[2];
			INT UnitCount = 1;
			if( sizeof(TCHAR)==sizeof(UNICHAR) )
			{
				Units[0] = ToUnicode( *C++ );
			}
			else
			{
				const PTRINT HostChar = static_cast<PTRINT>(*C++);
				DWORD CodePoint = HostChar>=0 && static_cast<UPTRINT>(HostChar)<=0x10ffffu ? static_cast<DWORD>(HostChar) : 0xfffdu;
				if( CodePoint>=0xd800u && CodePoint<=0xdfffu )
					CodePoint = 0xfffdu;
				if( CodePoint>0xffffu )
				{
					CodePoint -= 0x10000u;
					Units[0] = static_cast<UNICHAR>(0xd800u + (CodePoint>>10));
					Units[1] = static_cast<UNICHAR>(0xdc00u + (CodePoint&0x3ffu));
					UnitCount = 2;
				}
				else
				{
					Units[0] = static_cast<UNICHAR>(CodePoint);
				}
			}
			if( ByteCount+UnitCount*2>ARRAY_COUNT(Encoded) )
			{
				LogAr->Serialize( Encoded, ByteCount );
				ByteCount = 0;
			}
			for( INT UnitIndex=0; UnitIndex<UnitCount; UnitIndex++ )
			{
				Encoded[ByteCount++] = static_cast<BYTE>(Units[UnitIndex]&0xffu);
				Encoded[ByteCount++] = static_cast<BYTE>(Units[UnitIndex]>>8);
			}
		}
		if( ByteCount )
			LogAr->Serialize( Encoded, ByteCount );
#else
		LogAr->Serialize( const_cast<TCHAR*>(C), appStrlen(C) );
#endif
	}
};

/*-----------------------------------------------------------------------------
	The End.
-----------------------------------------------------------------------------*/
