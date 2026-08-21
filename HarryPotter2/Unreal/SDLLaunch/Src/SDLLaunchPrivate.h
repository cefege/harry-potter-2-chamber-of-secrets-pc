/*=============================================================================
	SDLLaunchPrivate.h: Unreal launcher for X.
	Copyright 1999 Epic Games, Inc. All Rights Reserved.

Revision history:
	* Created by Daniel Vogel (based on XLaunch).
=============================================================================*/

#include <stdio.h>
#include <stdlib.h>
#include <errno.h>
#include <sys/stat.h>
#include <time.h>

#if !MACOSX
#include <malloc.h>
#endif

#include <fcntl.h>
#include "Engine.h"
#include "UnRender.h"

/*-----------------------------------------------------------------------------
	The End.
-----------------------------------------------------------------------------*/
