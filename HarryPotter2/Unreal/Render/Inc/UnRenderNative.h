/*=============================================================================
	UnRenderNative.h: Native function lookup table for static libraries.
	Copyright 2000 Epic Games, Inc. All Rights Reserved.

	Revision history:
		* Created by Brandon Reinhart
=============================================================================*/

#ifndef UNRENDERNATIVE_H
#define UNRENDERNATIVE_H

#if __STATIC_LINK

/* No native execs. */

#define AUTO_INITIALIZE_REGISTRANTS_RENDER \
	URender::StaticClass();

#endif

#endif