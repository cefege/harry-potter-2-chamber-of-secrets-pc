// -----------------------------------------------------------------------------
//  ____  _ _                             _     
// |  _ \(_) |                           | |    
// | |_) |_| |_ _ __ ___   __ _ _ __     | |__  
// |  _ <| | __| '_ ` _ \ / _` | '_ \    | '_ \ 
// | |_) | | |_| | | | | | (_| | |_) | _ | | | |
// |____/|_|\__|_| |_| |_|\__,_| .__/ (_)|_| |_|
//                             | |              
//                             |_|              
//
// -----------------------------------------------------------------------------
// Originally created on 07/25/2002
//
// Description : 
// This is a class that wraps up the functionality of windows bitmaps.  
// There are capabilities for loading/saving, blitting, and blending bitmaps.  
// Only 24 and 32 bits per pixel bitmaps are used.
//
// Copyright 2002, Amaze Entertainment, all rights reserved.
// -----------------------------------------------------------------------------
#ifndef _BITMAP_H_
#define _BITMAP_H_

//******************
//*** WINNT DEFS ***
//******************
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

//********************
//*** WINDOWS DEFS ***
//********************
#ifndef _WINDEF_

#define NULL	0

#ifndef GDI_INTERNAL
DECLARE_HANDLE(HBITMAP);
DECLARE_HANDLE( HDC );
#endif

#endif//end _WINDEF_


#define DEFAULT_NUM_CHANNELS 4

class CBitmap
{
private:
	char			m_Filename[256];	// bitmap's filename (if loaded from a file, used for debug purposes)
	HDC				m_hDCBmp;			// Main drawing surface.  Used in many GDI functions
	
	HBITMAP			m_hThisBmp;			// Main Bmp Handle.. needs to be selected into a device context
	HBITMAP			m_hOldBmp;			// Holds the Bmp returned when selecting the main Bmp into the DC.. select back when finished with the DC
	
	int				m_Width;			// Pixel width of the Bmp
	int				m_Height;			// Pixel height of the Bmp
	int				m_Channels;			// Number of Bytes per pixel (3 for 24 bit, 4 for 24bit plus alpha[32 bit] )
	
	unsigned char * m_pBits;			// Pointer to the beginning of the writable memory for this Bmp
	unsigned char * m_pFirstLine;		// Pointer to the memory of the top left of the Bmp (Windows Bmp's are upside down in memory)
	int				m_BytesPerLine;		// Number of bytes per horizontil line in the Bmp (must be DWORD aligned)
	int				m_Stride;			// This is the number of bytes needed to increment from the beginning of one line to the beginning of the next line
	
	void			Clear();
	
public:
	// --- Base Functions
					CBitmap					( void )	{ Clear();	 }
					~CBitmap				( void )	{ Destroy(); }
	void			Destroy					( void );
	
	// --- Overloaded Operators
	bool			operator == 			( const CBitmap &BmpRight ) const;
	bool			operator != 			( const CBitmap &BmpRight ) const;
	CBitmap&		operator =				( const CBitmap &BmpRight );
	
	bool			CreateBmp				( int Width, int Height, int Channels = DEFAULT_NUM_CHANNELS );
	bool			CopyBmp					( const CBitmap &BmpToCopy );
		
	bool			IsValidBmp				( void )  const { return (m_hDCBmp != 0); }
	unsigned char*	GetLinePtr				( int y ) const { return m_pFirstLine + (y * m_Stride); }
	unsigned char*	GetPixelPtr				( int x, int y ) const { return (x == 0) ? (GetLinePtr(y)) : (GetLinePtr(y) + (x * m_Channels)); }
	
	int				GetWidth				( void ) const { return m_Width;  }
	int				GetHeight				( void ) const { return m_Height; }
	int				GetChannels				( void ) const { return m_Channels;  }
	HDC				GetBmpDC				( void ) const { return m_hDCBmp; }
	HBITMAP			GetBmpHandle			( void ) const { return m_hThisBmp;  }
	unsigned char*	GetMemoryStart			( void ) const { return m_pBits; }
	unsigned char*	GetMemoryEnd			( void ) const { return m_pBits + ((m_Height - 1) * m_BytesPerLine) + (m_Width) * m_Channels; }
	
	void			Draw					( HDC hDestDC, int DstX, int DstY, int SrcX, int SrcY, int SrcWidth, int SrcHeight) const;
	void			Draw					( HDC hDestDC, int DstX, int DstY) const;
	void			DrawStretched			( HDC hDestDC, int DstX, int DstY, int DstWidth, int DstHeight, int SrcX, int SrcY, int SrcWidth, int SrcHeight) const;
	void			DrawStretched			( HDC hDestDC, int DstX, int DstY, int DstWidth, int DstHeight ) const;
	
	bool			LoadBmpFile				( const char *FileName );
	bool			SaveBmpFile				( const char *FileName ) const;

	bool			BlendBmpsTranslucent	(const CBitmap &BmpBottom, const CBitmap &BmpTop, float fPercent, int DstX, int DstY, int SrcX, int SrcY, int SrcWidth, int SrcHeight);
	bool			BlendBmpsTranslucent	(const CBitmap &BmpBottom, const CBitmap &BmpTop, float fPercent, int DstX, int DstY);

};

#endif // _BITMAP_H_


