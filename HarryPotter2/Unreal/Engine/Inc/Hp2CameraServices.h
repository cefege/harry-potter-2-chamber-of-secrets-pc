#ifndef _INC_HP2CAMERASERVICES_H
#define _INC_HP2CAMERASERVICES_H

// C callbacks the Rust UnCamera port (crates/hp2-engine/src/camera*.rs) calls.
// Field order must match `CameraServices` on the Rust side.
// Object pointers are typed `void` here so this header stays free of Engine includes.
// Value pointers (FPlane, FVector, FCoords) are `const void` for the same reason; their layouts
// are read on the Rust side through hp2_abi.
//
// Throw exposure (item E). Thunks that can raise appErrorf/appThrowf/appFailAssert, which go
// through GError->Logf and can throw under the guarded launcher (SDLLaunch.cpp:5868):
//   Hp2CameraArchiveSerialize        write failure, FFileManagerAnsi.h:238 (no-fail flags).
//   Hp2CameraFileSize                read failure, FFileManagerAnsi.h:219.
//   Hp2CameraCreate*FileWriter       NoFail flags, FFileManagerUnix.h:477/483 (flags not verified).
//   Hp2CameraImportTextBuffer        ImportObject appThrowf/appErrorf, UnObj.cpp:1184, 2227-2485.
//   Hp2CameraViewportSnapBuffAdd,
//   Hp2CameraViewportHitSizesAdd     TArray allocator contract, UnFile.h:613/619.
//   Viewport, RenderDev, Input, NetDriver, Hit, Connection, and player exec thunks forward into engine
//                                    bodies with check()/appErrorfSlow sites; not audited per body.
// A C++ exception that unwinds through Rust frames is undefined behaviour. This port has the same
// exposure as the committed console and input ports. Unguarded runs exit via appRequestExit.
struct Hp2CameraServices
{
	// GetOuterUClient()->MaintainVerticalFOV
	int   (*viewport_maintain_vertical_fov)( void* Viewport );
	// Viewport->RenDev->SetSceneNode(Frame)
	void  (*render_dev_set_scene_node)( void* RenDev, void* Frame );
	// &GMath.ViewCoords
	const void* (*gmath_view_coords)( void );
	// GMath.SinTab(Index)
	float (*gmath_sin_tab)( int Index );
	// GMath.CosTab(Index)
	float (*gmath_cos_tab)( int Index );
	// RenDev->Draw2DLine(Frame, Color, LineFlags, P1, P2)
	void  (*render_dev_draw_2d_line)( void* RenDev, void* Frame, const void* Color, unsigned int LineFlags, const void* P1, const void* P2 );
	// RenDev->Draw2DPoint(Frame, Color, LineFlags, X1, Y1, X2, Y2, Z)
	void  (*render_dev_draw_2d_point)( void* RenDev, void* Frame, const void* Color, unsigned int LineFlags, float X1, float Y1, float X2, float Y2, float Z );
	// ParseCommand(&Cursor, Token)
	int   (*parse_command)( void* Cursor, const void* Token );
	// ParseToken(Cursor, Buf, Len, 0)
	int   (*parse_token)( void* Cursor, void* Buf, int Len );
	// ParseLine(Cursor, Buf, Len)
	int   (*parse_line)( void* Cursor, void* Buf, int Len );
	// Parse(Stream, Match, Value)
	int   (*parse_int)( const void* Stream, const void* Match, int* Value );
	// appAtoi(S)
	int   (*app_atoi)( const void* S );
	// appUserDir()
	const void* (*app_user_dir)( void );
	// DO_CHECK as compiled into this TU (nonzero = check live)
	int   (*check_enabled)( void );
	// GFileManager->FileSize(Path)
	int   (*file_size)( const void* Path );
	// GFileManager->CreateFileWriter(Path)
	void* (*create_file_writer)( const void* Path );
	// GFileManager->CreateFileWriter(*(GSys->SaveSlotPath * File))
	void* (*create_slot_file_writer)( const void* File );
	// GFileManager->CreateFileWriter(*(FString(appUserDir()) * File))
	void* (*create_user_dir_file_writer)( const void* File );
	// Ar->Serialize(Data, Count)
	void  (*archive_serialize)( void* Ar, const void* Data, int Count );
	// delete Ar
	void  (*archive_delete)( void* Ar );
	// Ar.Logf(Fmt)
	void  (*log_format)( void* Ar, const void* Fmt );
	// Ar.Logf(Fmt, Str)
	void  (*log_str)( void* Ar, const void* Fmt, const void* Arg );
	// Ar.Logf(Fmt, Int)
	void  (*log_int)( void* Ar, const void* Fmt, int Arg );
	// Ar.Logf(NAME_ExecWarning, Fmt, Str)
	void  (*log_exec_warning)( void* Ar, const void* Fmt, const void* Arg );
	// debugf(Fmt, Str)
	void  (*debugf_str)( const void* Fmt, const void* Arg );
	// LocalizeError("FileNotFound", TEXT("Core"))
	const void* (*localize_file_not_found)( void );
	// GIsRunning
	int   (*is_running)( void );
	// Viewport->CurrentTime = appSeconds()
	void  (*viewport_stamp_current_time)( void* Viewport );
	// Viewport->CurrentNetSpeed = Speed
	void  (*viewport_set_current_net_speed)( void* Viewport, int Speed );
	// SHOWALL: Actors(i)->bHidden = 0 for every actor on the level
	void  (*viewport_show_all_actors)( void* Viewport );
	// REPORT branch: FStringOutputDevice report, Ar.Logf(*Str, Cmd), clipboard copy
	void  (*viewport_report)( void* Viewport, const void* Cmd, void* Ar );
	// GetOuterUClient()->Engine->MouseDelta(Viewport, Kind, 0, 0)
	void  (*viewport_mouse_delta)( void* Viewport, int Kind );
	// Viewport->UpdateInput(Flags)
	void  (*viewport_update_input)( void* Viewport, int Flags );
	// Console && Console->ScriptConsoleExec(Cmd, Ar, Actor)
	int   (*viewport_script_console_exec)( void* Viewport, const void* Cmd, void* Ar );
	// UPlayer::Exec(Cmd, Ar) (base implementation, non-virtual)
	int   (*viewport_player_exec)( void* Viewport, const void* Cmd, void* Ar );
	// Viewport->Exec(Cmd, Ar) (virtual)
	int   (*viewport_exec)( void* Viewport, const void* Cmd, void* Ar );
	// Actor->GetLevel()->NetDriver
	void* (*viewport_net_driver)( void* Viewport );
	// Actor->GetLevel()->Model->Nodes.Num()
	int   (*viewport_model_node_count)( void* Viewport );
	// SnapBuff.Empty()
	void  (*viewport_snap_buff_empty)( void* Viewport );
	// SnapBuff.Add(Count)
	void  (*viewport_snap_buff_add)( void* Viewport, int Count );
	// HitSizes.AddItem(Size)
	void  (*viewport_hit_sizes_add)( void* Viewport, int Size );
	// HitSizes.Pop()
	int   (*viewport_hit_sizes_pop)( void* Viewport );
	// Input->Exec(Cmd, Ar) (virtual)
	int   (*input_exec)( void* Input, const void* Cmd, void* Ar );
	// Input->ReadInput(Delta, *GLog) (virtual)
	void  (*input_read_input)( void* Input, float Delta );
	// RenDev->Exec(Cmd, Ar) (virtual)
	int   (*render_dev_exec)( void* RenDev, const void* Cmd, void* Ar );
	// RenDev->ReadPixels(Dest) (virtual)
	void  (*render_dev_read_pixels)( void* RenDev, void* Dest );
	// RenDev->Lock(FlashScale, FlashFog, ScreenClear, Flags, HitData, HitSize) (virtual)
	void  (*render_dev_lock)( void* RenDev, const void* FlashScale, const void* FlashFog, const void* ScreenClear, unsigned int Flags, void* HitData, int* HitSize );
	// RenDev->Unlock(Blit) (virtual)
	void  (*render_dev_unlock)( void* RenDev, int Blit );
	// RenDev->PushHit(Hit, Size) (virtual)
	void  (*render_dev_push_hit)( void* RenDev, const void* Hit, int Size );
	// RenDev->PopHit(Size, Force) (virtual)
	void  (*render_dev_pop_hit)( void* RenDev, int Size, int Force );
	// RenDev->DetailTextures
	int   (*render_dev_detail_textures)( void* RenDev );
	// RenDev->DetailTextures ^= 1; RenDev->SaveConfig()
	void  (*render_dev_toggle_detail_textures)( void* RenDev );
	// HHitProxy::Size
	int   (*hit_proxy_size)( const void* Hit );
	// HHitProxy::Size = Size
	void  (*hit_proxy_set_size)( void* Hit, int Size );
	// HHitProxy::Parent = Parent
	void  (*hit_proxy_set_parent)( void* Hit, void* Parent );
	// NetDriver->ServerConnection
	void* (*net_driver_server_connection)( void* Driver );
	// NetDriver->MaxClientRate
	int   (*net_driver_max_client_rate)( void* Driver );
	// Connection->URL.HasOption(TEXT("LAN"))
	int   (*connection_has_lan_option)( void* Connection );
	// Connection->CurrentNetSpeed = Speed
	void  (*connection_set_current_net_speed)( void* Connection, int Speed );
	// Connection->Logf(Fmt)
	void  (*connection_logf_format)( void* Connection, const void* Fmt );
	// Connection->Logf(Fmt, Int)
	void  (*connection_logf_int)( void* Connection, const void* Fmt, int Arg );
	// GetDefault<UPlayer>()->ConfiguredInternetSpeed = Rate; GetDefault<UPlayer>()->SaveConfig()
	void  (*player_set_configured_internet_speed)( int Rate );
	// ImportObject<UTextBuffer>(GetTransientPackage(), NAME_None, 0, File); may be null
	void* (*import_text_buffer)( const void* File );
	// UTextBuffer::Text (the FString data pointer)
	const void* (*text_buffer_text)( void* Text );
	// Text->AddToRoot()
	void  (*text_buffer_add_to_root)( void* Text );
	// Text->RemoveFromRoot(); delete Text
	void  (*text_buffer_release)( void* Text );
};

extern const Hp2CameraServices GHp2CameraServices;

#endif // _INC_HP2CAMERASERVICES_H
