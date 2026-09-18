/*=============================================================================
	HP2EditorCommandletPlatform.cpp: Non-UI Editor hooks for fixture compilation.
=============================================================================*/
#include "EditorPrivate.h"

// The stock implementations live in the Windows-only editor host. Make never
// creates editor windows or dispatches their callbacks, but UEditorEngine's
// vtable still requires the FNotify and editor-hook definitions.
void UEditorEngine::NotifyDestroy( void* Src )
{
}

void UEditorEngine::NotifyPreChange( void* Src )
{
	if( Trans )
		Trans->Begin( TEXT("Edit Properties") );
}

void UEditorEngine::NotifyPostChange( void* Src )
{
	if( Trans )
		Trans->End();
}

void UEditorEngine::NotifyExec( void* Src, const TCHAR* Cmd )
{
}

void UEditorEngine::UpdatePropertiesWindows()
{
}

UBOOL UEditorEngine::HookExec( const TCHAR* Cmd, FOutputDevice& Ar )
{
	return 0;
}

void UEditorEngine::EdCallback( DWORD Code, UBOOL Send )
{
}
