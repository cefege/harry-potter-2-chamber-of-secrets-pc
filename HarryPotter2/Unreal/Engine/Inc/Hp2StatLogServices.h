#ifndef _INC_HP2STATLOGSERVICES_H
#define _INC_HP2STATLOGSERVICES_H

// C callbacks the Rust AStatLog port (crates/hp2-engine/src/stat_log.rs) calls.
// Field order must match `StatLogServices` on the Rust side.
// Object pointers and FString addresses are typed `void` here so this header stays free of
// Engine includes. FString arguments are passed as the address of the FString object.
struct Hp2StatLogServices
{
	// AActor::Level
	void*       (*actor_level)( void* Actor );
	// ALevelInfo::Game
	void*       (*level_game)( void* LevelInfo );
	// ALevelInfo::NetMode != NM_DedicatedServer
	int         (*level_not_dedicated)( void* LevelInfo );
	// AGameInfo::bExternalBatcher
	int         (*game_external_batcher)( void* Game );
	// AGameInfo::LocalLogFileName (address of FString)
	void*       (*game_local_log_file_name)( void* Game );
	// The const TCHAR* of an FString (FString::operator*), NUL-terminated
	const void* (*fstr_chars)( void* FStr );
	// FString::Len()
	int         (*fstr_len)( void* FStr );
	// FString = Text (Text is NUL-terminated)
	void        (*fstr_assign)( void* FStr, const void* Text );
	// FString += Text (Text is NUL-terminated)
	void        (*fstr_append)( void* FStr, const void* Text );
	// AStatLog::LocalBatcherURL (address of FString)
	void*       (*stat_log_local_batcher_url)( void* StatLog );
	// AStatLog::LocalLogDir (address of FString)
	void*       (*stat_log_local_log_dir)( void* StatLog );
	// AStatLog::WorldBatcherURL (address of FString)
	void*       (*stat_log_world_batcher_url)( void* StatLog );
	// AStatLog::WorldBatcherParams (address of FString)
	void*       (*stat_log_world_batcher_params)( void* StatLog );
	// GetClass()->GetDefaultObject()
	void*       (*stat_log_class_default_object)( void* StatLog );
	// AStatLog::StaticClass()->GetDefaultObject()
	void*       (*stat_log_static_default_object)( void );
	// AStatLog::bWorldBatcherError
	int         (*stat_log_get_world_batcher_error)( void* StatLog );
	// AStatLog::bWorldBatcherError = Value
	void        (*stat_log_set_world_batcher_error)( void* StatLog, int Value );
	// SaveConfig()
	void        (*stat_log_save_config)( void* StatLog );
	// AStatLog::bWorld
	int         (*stat_log_get_world)( void* StatLog );
	// AStatLogFile::StatLogFile (address of FString)
	void*       (*stat_log_file_name)( void* StatLog );
	// AStatLogFile::StatLogFinal (address of FString)
	void*       (*stat_log_file_final)( void* StatLog );
	// APlayerPawn::PlayerReplicationInfo->PlayerName (address of FString)
	void*       (*stat_log_player_name)( void* Pawn );
	// APlayerPawn::ngWorldSecret (address of FString)
	void*       (*stat_log_player_secret)( void* Pawn );
	// eventLogGameSpecial(Id, Target->GetClass()->GetFullName())
	void        (*stat_log_event_special_class)( void* StatLog, const void* Id, void* Target );
	// eventLogGameSpecial2(Id, Param1, Param2)
	void        (*stat_log_event_special2)( void* StatLog, const void* Id, const void* Param1, const void* Param2 );
	// TObjectIterator<UClass>: calls Callback(User, CastChecked<UPackage>(Class->GetOuter())) per class
	void        (*class_iterate_packages)( void (*Callback)( void* User, void* Package ), void* User );
	// UObject::GetFullName() of a package (static buffer, copy immediately)
	const void* (*package_full_name)( void* Package );
	// DLLEXT (static buffer)
	const void* (*dll_ext)( void );
	// GFileManager->FileSize(Name)
	int         (*file_size)( const void* Name );
	// GFileManager->CreateFileWriter(Name, FILEWRITE_EvenIfReadOnly)
	void*       (*file_create_writer)( const void* Name );
	// GFileManager->MakeDirectory(Name)
	void        (*file_make_directory)( const void* Name );
	// GFileManager->Move(Dst, Src, 1, 1, 1)
	void        (*file_move)( const void* Dst, const void* Src );
	// FArchive::Serialize(Data, Len)
	void        (*archive_serialize)( void* Ar, const void* Data, int Len );
	// FArchive::Flush()
	void        (*archive_flush)( void* Ar );
	// delete FArchive
	void        (*archive_delete)( void* Ar );
	// new FMD5Context
	void*       (*md5_context_new)( void );
	// appMD5Init(Context)
	void        (*md5_context_init)( void* Context );
	// appMD5Update(Context, Data, Len)
	void        (*md5_context_update)( void* Context, const void* Data, int Len );
	// appMD5Final(Digest, Context), Digest has 16 bytes
	void        (*md5_context_final)( void* Digest, void* Context );
	// delete FMD5Context
	void        (*md5_context_delete)( void* Context );
	// debugf(TEXT("%s"), Text)
	void        (*debugf_text)( const void* Text );
	// GWarn->Logf(TEXT("%s"), Text)
	void        (*warn_text)( const void* Text );
	// GIsRequestingExit = 1
	void        (*set_requesting_exit)( void );
	// appCreateProc(Url, Params, ShowWindow)
	void*       (*proc_create)( const void* Url, const void* Params, int Show );
	// appGetProcReturnCode(Handle, &Rc)
	int         (*proc_return_code)( void* Handle, int* Rc );
	// appLaunchURL(GFileManager->GetDefaultDirectory() * Url)
	void        (*launch_url)( const void* Url );
};

extern const Hp2StatLogServices GHp2StatLogServices;

#endif // _INC_HP2STATLOGSERVICES_H
