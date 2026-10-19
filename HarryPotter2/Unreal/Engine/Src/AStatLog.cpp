/*=============================================================================
	AStatLog.cpp: Unreal Tournament stat logging.
	Copyright 1997-1999 Epic Games, Inc. All Rights Reserved.

	Revision history:
		* Created by Tim Sweeney
=============================================================================*/

#include "EnginePrivate.h"
#include "hp2_rs.h"
#include "Hp2StatLogServices.h"

/*
	The stat log natives are implemented in Rust (crates/hp2-engine/src/stat_log.rs).
	This file keeps class registration, the two natives that stay in C++
	(GetGMTRef, GetMapFileName), and forwarders that pass the engine object pointers
	to the Rust exports through GHp2StatLogServices.
*/

/* Raises the failure a non-zero status from the Rust side stands for. CheckExpr is only
   used for a failed check; File and Line come from the call site. A failed check raises
   only when DO_CHECK is on, like the check() macro it mirrors. */
static void Hp2StatLogRaise( INT Status, const ANSICHAR* CheckExpr, const ANSICHAR* File, INT Line )
{
	switch( Status )
	{
	case HP2_STAT_LOG_STATUS_OK:
		break;
	case HP2_STAT_LOG_STATUS_CHECK_FAILED:
#if DO_CHECK
		appFailAssert( CheckExpr, File, Line );
#endif
		break;
	case HP2_STAT_LOG_STATUS_MD5_OVERFLOW:
		appErrorf( TEXT("UTF-16LE text exceeds the 32-bit MD5 contract") );
		break;
	case HP2_STAT_LOG_STATUS_ARCHIVE_OVERFLOW:
		appErrorf( TEXT("Stat log entry exceeds the 32-bit archive contract") );
		break;
	case HP2_STAT_LOG_STATUS_PANIC:
		appErrorf( TEXT("Stat log native panicked") );
		break;
	default:
		appErrorf( TEXT("Unexpected stat log status %i"), Status );
		break;
	}
}

/*-----------------------------------------------------------------------------
	Stat Log Implementation.
-----------------------------------------------------------------------------*/

#if ENGINE_VERSION>=230
IMPLEMENT_CLASS(AMutator);
#endif

void AStatLog::execExecuteLocalLogBatcher( FFrame& Stack, RESULT_DECL )
{
	guard(AStatLog::execExecuteLocalLogBatcher);
	P_FINISH;

	hp2_rs_engine_stat_log_execute_local_log_batcher( &GHp2StatLogServices, this );

	unguardexec;
}

void AStatLog::execExecuteSilentLogBatcher( FFrame& Stack, RESULT_DECL )
{
	guard(AStatLog::execExecuteSilentLogBatcher);
	P_FINISH;

	hp2_rs_engine_stat_log_execute_silent_log_batcher( &GHp2StatLogServices, this );

	unguardexec;
}

void AStatLog::execBatchLocal( FFrame& Stack, RESULT_DECL )
{
	guard(AStatLog::execBatchLocal);
	P_FINISH;

	hp2_rs_engine_stat_log_batch_local( &GHp2StatLogServices, this );

	unguardexec;
}

void AStatLog::execBrowseRelativeLocalURL( FFrame& Stack, RESULT_DECL )
{
	guard(AStatLog::execBrowseRelativeLocalURL);
	P_GET_STR(URL);
	P_FINISH;

	hp2_rs_engine_stat_log_browse_relative_local_url( &GHp2StatLogServices, &URL );

	unguardexec;
}

void AStatLog::execExecuteWorldLogBatcher( FFrame& Stack, RESULT_DECL )
{
	guard(AStatLog::execExecuteWorldLogBatcher);
	P_FINISH;

	Hp2StatLogRaise( hp2_rs_engine_stat_log_execute_world_log_batcher( &GHp2StatLogServices, this ),
		"Level->Game", __FILE__, __LINE__ );

	unguardexec;
}


void AStatLog::execInitialCheck( FFrame& Stack, RESULT_DECL )
{
	guard(AStatLog::execInitialCheck);
	P_GET_OBJECT(AGameInfo, Game);
	P_FINISH;

	Hp2StatLogRaise( hp2_rs_engine_stat_log_initial_check( &GHp2StatLogServices, this, Game ),
		"", __FILE__, __LINE__ );

	unguardexec;
}

void AStatLog::execLogMutator( FFrame& Stack, RESULT_DECL )
{
	guard(AStatLog::execInitialCheck);
	P_GET_OBJECT(AMutator, M);
	P_FINISH;

	hp2_rs_engine_stat_log_log_mutator( &GHp2StatLogServices, this, M );

	unguardexec;
}

void AStatLog::execGetGMTRef( FFrame& Stack, RESULT_DECL )
{
	guard(AStatLog::execGetGMTRef);
	P_FINISH;

	*(FString*)Result = appGetGMTRef();

	unguardexec;
}

void AStatLog::execGetMapFileName( FFrame& Stack, RESULT_DECL )
{
	guard(AStatLog::execGetMapFileName);
	P_FINISH;

	*(FString*)Result = XLevel->URL.Map;

	unguardexec;
}

void AStatLog::execGetPlayerChecksum( FFrame& Stack, RESULT_DECL )
{
	guard(AStatLog::execGetPlayerChecksum);
	P_GET_OBJECT(APlayerPawn, P);
	P_GET_STR_REF(Checksum);
	P_FINISH;

	Hp2StatLogRaise( hp2_rs_engine_stat_log_get_player_checksum( &GHp2StatLogServices, P, Checksum ),
		"", __FILE__, __LINE__ );

	unguardexec;
}

void AStatLogFile::execOpenLog( FFrame& Stack, RESULT_DECL )
{
	guard(AStatLogFile::execOpenLog);
	P_FINISH;

	hp2_rs_engine_stat_log_open_log( &GHp2StatLogServices, this, TEXT("..") PATH_SEPARATOR TEXT("Logs") );

	unguardexec;
}

void AStatLogFile::execCloseLog( FFrame& Stack, RESULT_DECL )
{
	guard(AStatLogFile::execCloseLog);
	P_FINISH;

	hp2_rs_engine_stat_log_close_log( &GHp2StatLogServices, this );

	unguardexec;
}

void AStatLogFile::execWatermark( FFrame& Stack, RESULT_DECL )
{
	guard(AStatLogFile::execWatermark);
	P_GET_STR(EventString);
	P_FINISH;

	Hp2StatLogRaise( hp2_rs_engine_stat_log_watermark( &GHp2StatLogServices, this, &EventString ),
		"Runtime && Runtime->Context", __FILE__, __LINE__ );

	unguardexec;
}

void AStatLogFile::execGetChecksum( FFrame& Stack, RESULT_DECL )
{
	guard(AStatLogFile::execGetChecksum);
	P_GET_STR_REF(Checksum);
	P_FINISH;

	Hp2StatLogRaise( hp2_rs_engine_stat_log_get_checksum( &GHp2StatLogServices, this, Checksum ),
		"Runtime && Runtime->Context", __FILE__, __LINE__ );

	unguardexec;
}

void AStatLogFile::execFileFlush( FFrame& Stack, RESULT_DECL )
{
	guard(AStatLogFile::execFileFlush);
	P_FINISH;

	hp2_rs_engine_stat_log_file_flush( &GHp2StatLogServices, this );

	unguardexec;
}

void AStatLogFile::execFileLog( FFrame& Stack, RESULT_DECL )
{
	guard(AStatLogFile::execFileLog);
	P_GET_STR(EventString);
	P_FINISH;

	Hp2StatLogRaise( hp2_rs_engine_stat_log_file_log( &GHp2StatLogServices, this, &EventString ),
		"", __FILE__, __LINE__ );

	unguardexec;
}


/*-----------------------------------------------------------------------------
	UCheckSumCommandlet.
-----------------------------------------------------------------------------*/

class UCheckSumCommandlet : public UCommandlet
{
	DECLARE_CLASS(UCheckSumCommandlet,UCommandlet,CLASS_Transient,Engine);

	INT Main( const TCHAR* FileName )
	{
		guard(UCheckSumCommandlet::Main);
		const INT Status = hp2_rs_engine_stat_log_checksum_commandlet_main( &GHp2StatLogServices, FileName );
		if( Status == HP2_STAT_LOG_STATUS_FILE_MISSING )
			appErrorf( TEXT("Could not open file: %s"), FileName );
		Hp2StatLogRaise( Status, "", __FILE__, __LINE__ );
		return 0;
		unguard;
	}
};
IMPLEMENT_CLASS(UCheckSumCommandlet)
void RegisterCheckSumCommandletClass()
{
	UCheckSumCommandlet::StaticClass();
}

/*
	appMD5UpdateStringUtf16LE: encodes Text as UTF-16LE (with an optional CRLF) and feeds
	the bytes to Context. The encoding and MD5 update live in Rust.
*/
void appMD5UpdateStringUtf16LE( FMD5Context* Context, const FString& Text, UBOOL AppendLineEnding )
{
	Hp2StatLogRaise( hp2_rs_engine_stat_log_md5_update_text( &GHp2StatLogServices, Context, *Text, Text.Len(), AppendLineEnding ),
		"", __FILE__, __LINE__ );
}


/*-----------------------------------------------------------------------------
	The end.
-----------------------------------------------------------------------------*/
