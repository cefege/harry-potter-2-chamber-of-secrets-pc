/*=============================================================================
	UnFireNative.h: Native function lookup table for static libraries.
	Copyright 2000 Epic Games, Inc. All Rights Reserved.

	Revision history:
		* Created by Brandon Reinhart
=============================================================================*/

#ifndef UNFIRENATIVE_H
#define UNFIRENATIVE_H

#if __STATIC_LINK
/* No native execs.
DECLARE_NATIVE_TYPE(Fire,UFractalTexture);
DECLARE_NATIVE_TYPE(Fire,UFireTexture);
DECLARE_NATIVE_TYPE(Fire,UWaterTexture);
DECLARE_NATIVE_TYPE(Fire,UWaveTexture);
DECLARE_NATIVE_TYPE(Fire,UWetTexture);
DECLARE_NATIVE_TYPE(Fire,UIceTexture);
*/

#define AUTO_INITIALIZE_REGISTRANTS_FIRE \
	UFractalTexture::StaticClass(); \
	UFireTexture::StaticClass(); \
	UWaterTexture::StaticClass(); \
	UWaveTexture::StaticClass(); \
	UWetTexture::StaticClass(); \
	UIceTexture::StaticClass();
#endif

#endif
