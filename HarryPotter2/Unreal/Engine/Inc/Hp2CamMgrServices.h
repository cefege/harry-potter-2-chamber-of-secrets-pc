#ifndef _INC_HP2CAMMGRSERVICES_H
#define _INC_HP2CAMMGRSERVICES_H

// C callbacks the Rust UnCamMgr port (crates/hp2-engine/src/cam_mgr.rs) calls.
// Field order must match `CamMgrServices` on the Rust side.
// Object pointers are typed `void` here so this header stays free of Engine includes.
struct Hp2CamMgrServices
{
	// ParseCommand(&Stream, TEXT("BRIGHTNESS"))
	int   (*parse_brightness_command)( const void* Cmd );
	// UClient::Engine->Flush(AllowPrecache)
	void  (*client_engine_flush)( void* Client, int AllowPrecache );
	// UClient::SaveConfig()
	void  (*client_save_config)( void* Client );
	// UClient::Viewports.Num()
	int   (*client_viewports_num)( void* Client );
	// UClient::Viewports(0)->Actor
	void* (*client_viewport0_actor)( void* Client );
	// UClient::Viewports(0)->Actor->eventClientMessage(Printf("Brightness level %i/10", Level), NAME_None, 0)
	void  (*client_brightness_message)( void* Client, int Level );
	// UClient::Viewports(Index)->RenDev
	void* (*client_viewport_render_dev)( void* Client, int Index );
	// URenderDevice::Flush(AllowPrecache)
	void  (*render_dev_flush)( void* RenDev, int AllowPrecache );
};

extern const Hp2CamMgrServices GHp2CamMgrServices;

#endif // _INC_HP2CAMMGRSERVICES_H
