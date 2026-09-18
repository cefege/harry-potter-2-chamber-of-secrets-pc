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
// Originally created on 08/09/2001
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

#ifndef WINDOW_API
#define WINDOW_API __declspec(dllimport)
#endif

#include <stdio.h>
#include <assert.h>

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

#ifndef _FILE_DEFINED
struct _iobuf {
        char *_ptr;
        int   _cnt;
        char *_base;
        int   _flag;
        int   _file;
        int   _charbuf;
        int   _bufsiz;
        char *_tmpfname;
        };
typedef struct _iobuf FILE;
#define _FILE_DEFINED
#endif

//Adds a message to the assertion dialog
//a is an expressions, b is a string to clarify why the assertion failed
#define massert(a,b)	assert((a) && (b))


// **********************************************************************************
// DEFINES
#define	BLUE_BITMAP_OFFSET		0
#define	GREEN_BITMAP_OFFSET		1
#define	RED_BITMAP_OFFSET		2
#define	ALPHA_BITMAP_OFFSET		3

#define DEFAULT_NUM_CHANNELS 4


class WINDOW_API CBitmap
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
	
	void			Clear()
	{
		m_Filename[0]	= NULL;
		m_hDCBmp		= NULL;

		m_hThisBmp		= NULL;
		m_hOldBmp		= NULL;

		m_Width			= -1;
		m_Height		= -1;
		m_Channels		= -1;

		m_BytesPerLine	= 0;
		m_pBits			= NULL;
		m_pFirstLine	= NULL;
		m_Stride		= 0;
	}
	
public:
	// --- Base Functions
		 CBitmap	( void )	{ Clear();	 }
		 ~CBitmap	( void )	{ Destroy(); }
	void Destroy	( void )
	{
		if( m_hDCBmp)
		{
			if( m_hOldBmp != NULL )		SelectObject(m_hDCBmp, m_hOldBmp);
			if( m_hThisBmp )			DeleteObject(m_hThisBmp);
			if( m_hDCBmp )				DeleteDC(m_hDCBmp);
		}
		Clear();
	}
	
	bool			IsValid			( void )  const			{ return (m_hDCBmp != 0); }
	unsigned char*	GetLinePtr		( int y ) const			{ return m_pFirstLine + (y * m_Stride); }
	unsigned char*	GetPixelPtr		( int x, int y ) const	{ return (x == 0) ? (GetLinePtr(y)) : (GetLinePtr(y) + (x * m_Channels)); }
	int				GetWidth		( void ) const			{ return m_Width;	 }
	int				GetHeight		( void ) const			{ return m_Height;	 }
	int				GetChannels		( void ) const			{ return m_Channels; }
	HDC				GetBitmapDC		( void ) const			{ return m_hDCBmp;	 }
	HBITMAP			GetBitmapHandle	( void ) const			{ return m_hThisBmp; }
	unsigned char*	GetMemoryStart	( void ) const			{ return m_pBits;	 }
	unsigned char*	GetMemoryEnd	( void ) const			{ return m_pBits + ((m_Height - 1) * m_BytesPerLine) + (m_Width) * m_Channels; }
	
	// --- Overloaded Operators
	bool	 operator == ( const CBitmap &BmpRight ) const	{ return ( (m_Width == BmpRight.GetWidth()) && (m_Height == BmpRight.GetHeight()) && (m_Channels == BmpRight.GetChannels()) ); }
	bool	 operator != ( const CBitmap &BmpRight ) const	{ return ( (m_Width != BmpRight.GetWidth()) || (m_Height != BmpRight.GetHeight()) || (m_Channels != BmpRight.GetChannels()) ); }
	CBitmap& operator =  ( const CBitmap &BmpRight )
	{
		//check that the objects aren't actually the same instance
		if(this == &BmpRight) return *this;
		bool bRet(true);
		bRet = (bool)Copy( BmpRight );
		massert(bRet, "operator= : The right hand bitmap shouldn't be empty if assigning it to another.");
		return *this;
	}

	bool Create( int Width, int Height, int Channels )
	{
		//Coincidentally.. in this case.. the size requested is already allocated 
		//and set up so no work needs to be done.
		if(m_hDCBmp && m_Width == Width && m_Height == Height && m_Channels == Channels)
			return true;
		
		//Destroy the exsisting bmp
		Destroy();

		//must be 24 or 32 bit
		if(Channels != 3 && Channels != 4)
			return false;

		m_Width    = Width;
		m_Height   = Height;
		m_Channels = Channels;

		//Initialize this structure which is required for CreateDIBSection()
		BITMAPINFO	BmpInfo;
		memset(&BmpInfo, 0, sizeof(BITMAPINFO));

		BmpInfo.bmiHeader.biSize			= sizeof(BITMAPINFOHEADER);
		BmpInfo.bmiHeader.biWidth			= Width;
		BmpInfo.bmiHeader.biHeight			= Height;
		BmpInfo.bmiHeader.biPlanes			= (unsigned short)1;
		BmpInfo.bmiHeader.biBitCount		= (unsigned short)(Channels * 8);
		BmpInfo.bmiHeader.biCompression		= BI_RGB;
		BmpInfo.bmiHeader.biClrUsed			= (Channels==1) ? 256 : 0;
		BmpInfo.bmiHeader.biClrImportant	= 0;
		BmpInfo.bmiHeader.biSizeImage		= 0;
		BmpInfo.bmiHeader.biXPelsPerMeter	= 0;
		BmpInfo.bmiHeader.biYPelsPerMeter	= 0;
		
		//Create a device context for the display
		HDC	hDCTemp;
		hDCTemp = CreateDC(TEXT("Display"), NULL, NULL, NULL);

		if(hDCTemp)
		{
			//TODO: Look into this note.
			//Create Device Independant Bitmap and gain access to the pixels in memory with m_pBits
			//NOTE:  MSDN Warns about possible synchronization errors on Win2K when writing to this memory
			//If this is a problem.. solutions may lie in GDIFlush() or GDISetBatchLimit()
			m_hThisBmp = CreateDIBSection(hDCTemp,&BmpInfo,DIB_RGB_COLORS,(LPVOID*)&m_pBits,NULL,0);
			if (m_hThisBmp)
			{
				//Create a memory device context that is compatible with the display
				m_hDCBmp = CreateCompatibleDC( hDCTemp );

				if (m_hDCBmp)
					m_hOldBmp = (HBITMAP)SelectObject(m_hDCBmp, m_hThisBmp);  //store the 1x1 HBitmap that was in the DC because it needs to be replaced before deletion
			}
		
			//No longer needed after the memory DC is created 
			DeleteDC(hDCTemp);
		}

		//if any of these essential members are not initialized then there is a problem
		if (!m_hThisBmp || !m_hDCBmp || !m_hOldBmp || !m_pBits)
		{
			Destroy();
			return false;
		}

		//This little trick makes sure that the Number of Bytes per line(width*channels) is padded to a DWORD (4 bytes)
		//by rounding up to the next multiple of 4
		m_BytesPerLine = (m_Width * Channels + 3) & ~3;
		//m_BytesPerLine = ((m_Width * Channels + 3) / 4) * 4;

		//This is a pointer the top line of the bitmap.  It is needed because m_pBits points to the last
		//line of the Bmp due to the fact that Windows Bmp's are upside down in files and in memory.
		m_pFirstLine = m_pBits + (m_Height - 1) * m_BytesPerLine;

		//This is the offset in bytes to the next horizontal line down visually.
		m_Stride = -m_BytesPerLine;

		return true;
	}

	bool Copy( const CBitmap &BmpToCopy )
	{
		//in this case.. the bitmap we are copying is empty so there is nothing to copy
		if(!BmpToCopy.IsValid())
			return false;

		//if they aren't already equal in dimensions then recreate ourselves to the passed in dimensions
		if(*this != BmpToCopy)
		{
			if(!Create(BmpToCopy.GetWidth(), BmpToCopy.GetHeight(), BmpToCopy.GetChannels()))
				return false;
		}
		
		unsigned char * pDst, *pSrc, *pEnd;
		
		//get the star and end address
		pDst = GetMemoryStart();
		pEnd = GetMemoryEnd();
		pSrc = BmpToCopy.GetMemoryStart();

		//just keep copying pixels until we pass the address of the last pixel
		//write an 
		while(pDst < pEnd)
			*pDst++ = *pSrc++;
		
		return true;
	}

	void Draw( HDC hDestDC, int DstX, int DstY, int SrcX, int SrcY, int SrcWidth, int SrcHeight) const
	{ 
		if( !m_hDCBmp ) return;
		BitBlt(hDestDC, DstX, DstY, SrcWidth, SrcHeight, m_hDCBmp, SrcX, SrcY, SRCCOPY); 
	}
	
	void Draw( HDC hDestDC, int DstX, int DstY) const 
	{ 
		Draw(hDestDC, DstX, DstY, 0, 0, m_Width, m_Height);	
	}  
	
	void DrawStretched(HDC hDestDC, int DstX, int DstY, int DstWidth, int DstHeight, int SrcX, int SrcY, int SrcWidth, int SrcHeight) const
	{
		if( !m_hDCBmp ) return;
		StretchBlt( hDestDC, DstX, DstY, DstWidth, DstHeight, m_hDCBmp, SrcX, SrcY, SrcWidth, SrcHeight, SRCCOPY );
	}
	
	void DrawStretched(HDC hDestDC, int DstX, int DstY, int DstWidth, int DstHeight ) const
	{
		DrawStretched( hDestDC, DstX, DstY, DstWidth, DstHeight, 0, 0, m_Width, m_Height );
	}
	
	bool CreateFromDesktop(int SrcX, int SrcY, int SrcWidth, int SrcHeight, int Channels)
	{
		//Wipe out the class and make a new Bmp of the specified dimensions
		if(!Create(SrcWidth, SrcHeight, Channels))
			return false;
		
		HDC hDCScreen;
		
		//TODO: Look into using GetDCEx as it can be possible to get a DC of a specified rectangle
		// get a device context for the whole screen
		hDCScreen = GetDC(NULL);
		if(!hDCScreen)
			return false;

		//Blt the specified screen dimensions to the stored bitmap
		if(!BitBlt(m_hDCBmp, 0, 0, SrcWidth, SrcHeight, hDCScreen, SrcX, SrcY, SRCCOPY))
		{
			ReleaseDC(NULL, hDCScreen);
			return false;
		}
		ReleaseDC(NULL, hDCScreen);
		return true;
	}

	bool LoadFile( const char *FileName )
	{		
		// argument checks
		if( FileName == 0 || FileName[0] == 0 )
			return false;
		
		// open the file in read mode
		FILE *fptr = fopen(FileName, "rb");
		if(fptr == NULL)
			return false;
		
		do
		{
			//initialize the file's structures
			BITMAPINFOHEADER bih;
			BITMAPFILEHEADER bfh;
			memset(&bih, 0, sizeof(BITMAPINFOHEADER));
			memset(&bfh, 0, sizeof(BITMAPFILEHEADER));
			
			//read in the file header structure
			if( fread(&bfh, sizeof(BITMAPFILEHEADER), 1, fptr) != 1)
				break;
			
			//if the correct section of the file doesn't equal this equation
			//then this is an invalid file format or the file is corrupt
			if( bfh.bfType != ('M' * 256 + 'B') )
				break;
			
			//read in the info file header structure
			if( fread(&bih, sizeof(BITMAPINFOHEADER), 1, fptr) != 1)
				break;
			
			//all these checks must pass, otherwise we don't support this bitmap file, and must fail
			if( ( bih.biPlanes != 1 )							|| 
				( bih.biBitCount != 24 && bih.biBitCount != 32) ||
				( bih.biCompression != BI_RGB )					)
				break;
			
			massert((bih.biBitCount/8) == 3 || (bih.biBitCount/8) == 4, "LoadBmpFile: Must be a 24 or 32 bit image");
			
			//create a bitmap to fill with the file's data
			if(!Create(bih.biWidth, bih.biHeight, bih.biBitCount / 8))
				break;
			
			//now skip over the offset specified by OffBits so we can jump straight to the pixel data
			fseek(fptr, bfh.bfOffBits, SEEK_SET);
			
			//Must read in starting from the bottom because windows bitmaps present the pixel data that way
			PUCHAR pLine = NULL;
			for(int y = m_Height - 1; y >= 0; y--)
			{	
				pLine = GetLinePtr(y);
				if(fread(pLine, m_BytesPerLine, 1, fptr) != 1) 
					{ fclose(fptr); return false; }
			}
			
			// success!
			fclose(fptr);
			return true;
		
		}while(0);
		
		// failure
		fclose(fptr);
		return false;
	}

	bool SaveFile( const char *FileName) const
	{
		BITMAPINFOHEADER bih;
		BITMAPFILEHEADER bfh;
		FILE*			 fp = NULL;
		
		//initialize the file's structures
		memset(&bih, 0, sizeof(BITMAPINFOHEADER));
		memset(&bfh, 0, sizeof(BITMAPFILEHEADER));

		//open the file in write mode
		fp = fopen(FileName, "wb");

		if(fp == NULL)
			return false;

		//fill and write the file header information
		bfh.bfType = ('M' * 256 + 'B');
		bfh.bfSize = sizeof(BITMAPFILEHEADER);
		bfh.bfOffBits = sizeof(BITMAPINFOHEADER) + sizeof(BITMAPFILEHEADER);

		if(fwrite(&bfh, sizeof(BITMAPFILEHEADER), 1, fp) != 1)
		{
			fclose(fp);
			return false;
		}

		//fill and write the info header information
		bih.biPlanes		= 1;
		bih.biBitCount		= (unsigned short)(m_Channels * 8);
		bih.biSize			= sizeof(BITMAPINFOHEADER);
		bih.biCompression	= BI_RGB;
		bih.biWidth			= m_Width;
		bih.biHeight		= m_Height;
		bih.biClrUsed		= (m_Channels==1)?256:0;

		if(fwrite(&bih, sizeof(BITMAPINFOHEADER), 1, fp) != 1)
		{
			fclose(fp);
			return false;
		}

		PUCHAR pLine = NULL;
		for(int y = m_Height - 1; y >= 0; y--)
		{	
			pLine = GetLinePtr(y);
			if(fwrite(pLine, m_BytesPerLine, 1, fp) != 1)
			{
				fclose(fp);
				return false;
			}

		}
		
		//close the file
		fclose(fp);
		return true;
	}

};

#endif // _BITMAP_H_


