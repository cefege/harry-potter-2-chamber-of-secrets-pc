/*=============================================================================
	FOutputDeviceStdout.h: ANSI stdout output device.
	Copyright 1997-1999 Epic Games, Inc. All Rights Reserved.

	Revision history:
		* Created by Tim Sweeney
=============================================================================*/

//
// ANSI stdout output device.
//
class FOutputDeviceStdout : public FOutputDevice
{
public:
	void Serialize( const TCHAR* V, EName Event )
	{
		// TCHAR is wchar_t on this platform: "%s" would print only the
		// first character and stop at its zero byte. "%ls" converts the
		// whole wide string to multibyte output.
		fprintf( stdout, "%ls\n", V );
		fflush( stdout );
	}
};

/*-----------------------------------------------------------------------------
	The End.
-----------------------------------------------------------------------------*/
