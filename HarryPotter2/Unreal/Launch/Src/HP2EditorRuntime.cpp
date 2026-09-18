/*=============================================================================
	HP2EditorRuntime.cpp: Minimal Editor package storage for the game runtime.
=============================================================================*/

#include "Engine.h"

class UEditorEngine;

IMPLEMENT_PACKAGE(Editor);

// The native game registers transaction classes but never creates an editor.
EDITOR_API UEditorEngine* GEditor = NULL;
