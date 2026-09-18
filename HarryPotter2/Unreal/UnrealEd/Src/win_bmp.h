// -----------------------------------------------------------------------------
//           _              _                         _     
//          (_)            | |                       | |    
// __      ___ _ __        | |__  _ __ ___  _ __     | |__  
// \ \ /\ / / | '_ \       | '_ \| '_ ` _ \| '_ \    | '_ \ 
//  \ V  V /| | | | |      | |_) | | | | | | |_) | _ | | | |
//   \_/\_/ |_|_| |_|      |_.__/|_| |_| |_| .__/ (_)|_| |_|
//                   ______                | |              
//                  |______|               |_|              
// -----------------------------------------------------------------------------
//
// Created on  : 03/15/2002
// Authored by : Elijah Emerson
//
// Description : The class described in this header wraps the functionality of windows bitmaps.
//
// -----------------------------------------------------------------------------

#ifndef _WIN_BMP_H_
#define _WIN_BMP_H_

//*** WINNT DEFS ***
#ifndef _WINNT_

typedef void *PVOID;
#ifdef STRICT
typedef void *HANDLE;
#define DECLARE_HANDLE(name) struct name##__ { int unused; }; typedef struct name##__ *name
#else
typedef PVOID HANDLE;
#define DECLARE_HANDLE(name) typedef HANDLE name
#endif
typedef HANDLE *PHANDLE;

#endif //end _WINNT_


//*** WIN DEFS ***
#ifndef _WINDEF_

#define NULL	0
#ifndef GDI_INTERNAL
DECLARE_HANDLE(HBITMAP);
DECLARE_HANDLE( HDC );
#endif

#endif//end _WINDEF_


// --- DEFAULT DEFINES
#define DEFAULT_NUM_CHANNELS 4

// ---------------------------------------------------------------------------------
//	CLASS NAME:		CBitmap
//	CREATION DATE:	3/15/2002
//
//	DESCRIPTION:	This is a class that wraps up the functionality of windows bitmaps.
//					There are capabilities for loading/saving, and blitting.
//					Only 24 and 32 bits per pixel bitmaps are used.
// ---------------------------------------------------------------------------------
class CBitmap
{
private:
	HDC				m_hDCBmp;			// Main drawing surface.  Used in many GDI functions
	
	HBITMAP			m_hThisBmp;			// Main Bmp Handle.. needs to be selected into a device context
	HBITMAP			m_hOldBmp;			// Holds the Bmp returned when selecting the main Bmp into the DC.. select back when finished with the DC
	
	int				m_Width;			// Pixel width of the bmp
	int				m_Height;			// Pixel height of the bmp
	int				m_Channels;			// Number of Bytes per pixel (3 for 24 bit, 4 for 24bit plus alpha[32 bit] )
	
	unsigned char * m_pBits;			// Pointer to the beginning of the writable memory for this Bmp
	unsigned char * m_pFirstLine;		// Pointer to the memory of the top left of the Bmp (Windows Bmp's are upside down in memory)
	int				m_BytesPerLine;		// Number of bytes per horizontil line in the Bmp (must be DWORD aligned)
	int				m_Stride;			// This is the number of bytes needed to increment from the beginning of one line to the beginning of the next line
	
	void			Clear				( void );

public:
					CBitmap				( void ) { Clear();		}
					~CBitmap			( void ) { Destroy();	}

	bool			Create				( int Width, int Height, int Channels = DEFAULT_NUM_CHANNELS);
	bool			CreateFromDesktop	( int SrcX, int SrcY, int SrcWidth, int SrcHeight, int Channels = DEFAULT_NUM_CHANNELS);

	void			Destroy				( void );
	bool			Copy				( const CBitmap& BmpToCopy);

	void			Draw				( HDC hDestDC, int DstX, int DstY, int SrcX, int SrcY, int SrcWidth, int SrcHeight) const;
	void			Draw				( HDC hDestDC, int DstX, int DstY) const;

	bool			LoadBmpFile			( const char* FileName);
	bool			SaveBmpFile			( const char* FileName) const;
	
	CBitmap&		operator =			( const CBitmap &BmpRight);
	bool			operator ==			( const CBitmap &BmpRight) const;
	bool			operator !=			( const CBitmap &BmpRight) const;
										
	bool			IsInitilized		( void ) const		  { return m_hDCBmp!=0; }
	int				GetWidth			( void ) const		  { return m_Width;		}
	int				GetHeight			( void ) const		  { return m_Height;	}
	int				GetChannels			( void ) const		  { return m_Channels;	}
	HDC				GetBmpDC			( void ) const		  { return m_hDCBmp;	}
	HBITMAP			GetBmpHandle		( void ) const		  { return m_hThisBmp;	}
	unsigned char*	GetMemoryStart		( void ) const		  { return m_pBits;		}
	unsigned char*	GetMemoryEnd		( void ) const		  { return m_pBits + ((m_Height - 1) * m_BytesPerLine) + (m_Width) * m_Channels; }
	unsigned char*	GetLinePtr			( int y) const		  { return m_pFirstLine + (y * m_Stride); }
	unsigned char*	GetPixelPtr			( int x, int y) const { return (x == 0) ? (GetLinePtr(y)) : (GetLinePtr(y) + (x * m_Channels)); }

};

#endif // _WIN_BMP_H_

///////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////
// win_bmp.h - End of file
// Thanks to Fluid Studios for their comment generator
///////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////

