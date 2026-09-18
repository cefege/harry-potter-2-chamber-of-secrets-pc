// -----------------------------------------------------------------------------
//           _              _                                          
//          (_)            | |                                         
// __      ___ _ __        | |__  _ __ ___  _ __       ___ _ __  _ __  
// \ \ /\ / / | '_ \       | '_ \| '_ ` _ \| '_ \     / __| '_ \| '_ \ 
//  \ V  V /| | | | |      | |_) | | | | | | |_) | _ | (__| |_) | |_) |
//   \_/\_/ |_|_| |_|      |_.__/|_| |_| |_| .__/ (_) \___| .__/| .__/ 
//                   ______                | |            | |   | |    
//                  |______|               |_|            |_|   |_|    
//
// -----------------------------------------------------------------------------
// Created on  : 03/15/2002
// Authored by : Elijah Emerson
//
// Description : This is a class that wraps up the functionality of windows bitmaps.  
//				 There are capabilities for loading/saving and blitting.  
//				 Only 24 and 32 bits per pixel bitmaps are used.
//
// -----------------------------------------------------------------------------

// INCLUDES
#define WIN32_LEAN_AND_MEAN
#include <windows.h>
#include <stdio.h>
#include <assert.h>

#include "win_bmp.h"

// DEFINES
#define	BITMAP_OFFSET_BLUE		0
#define	BITMAP_OFFSET_GREEN		1
#define	BITMAP_OFFSET_RED		2
#define	BITMAP_OFFSET_ALPHA		3

//Adds a message to the assertion dialog
//a is an expression, b is a string to clarify why the assertion failed
#define massert(a,b)	assert((a) && (b))


void DisplayLastWinError( void )
{
	LPVOID lpMsgBuf;
	
	FormatMessage( 
		FORMAT_MESSAGE_ALLOCATE_BUFFER | 
		FORMAT_MESSAGE_FROM_SYSTEM | 
		FORMAT_MESSAGE_IGNORE_INSERTS,
		NULL,
		GetLastError(),
		MAKELANGID(LANG_NEUTRAL, SUBLANG_DEFAULT), // Default language
		(LPTSTR) &lpMsgBuf,
		0,
		NULL 
		);
	
	// Process any inserts in lpMsgBuf.
	// ...
	// Display the string.
	::MessageBox( NULL, (LPCTSTR)lpMsgBuf, NULL, MB_OK | MB_ICONINFORMATION );
	
	// Free the buffer.
	LocalFree( lpMsgBuf );
}

//------------------------------------------------------------------------------------------
//	Function Name:	CBitmap::Clear()
//	
//	Description:	Wipes out the data members
//------------------------------------------------------------------------------------------
void CBitmap::Clear(void)
{
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

//------------------------------------------------------------------------------------------
//	Function Name:	CBitmap::Destroy()
//	
//	Description:	Frees any bitmap data and then clears out all the data members
//------------------------------------------------------------------------------------------
void CBitmap::Destroy(void)
{
	if( m_hDCBmp)
	{
		if( m_hOldBmp != NULL )
			SelectObject(m_hDCBmp, m_hOldBmp);
		if( m_hThisBmp )
			DeleteObject(m_hThisBmp);
		if( m_hDCBmp )
			DeleteDC(m_hDCBmp);
	}
	
	Clear();
}

//------------------------------------------------------------------------------------------
//	Function Name:	CBitmap::Create()
//	
//	Param 1:		int Width	- desired width of this new bmp
//	Param 2:		int Height	- desired height of this new bmp
//	Param 3:		int Channels- number of bytes per pixel this bmp should be
//	Returns:		bool 		- true on successful creation, false otherwise
//	
//	Description:	Destroys any current bmp data and creates a new one of the specified
//					dimensions and bit depth
//------------------------------------------------------------------------------------------
bool CBitmap::Create(int Width, int Height, int Channels)
{
	//Coincidentally.. in this case.. the size requested is already allocated 
	//and set up so no work needs to be done.
	if(m_hDCBmp && m_Width == Width && m_Height == Height && m_Channels == Channels)
		return true;

	//Destroy the exsisting bmp
	Destroy();

	// must be 24 or 32 bit
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
	BmpInfo.bmiHeader.biPlanes			= 1;
	BmpInfo.bmiHeader.biBitCount		= (unsigned short)(Channels * 8);
	BmpInfo.bmiHeader.biCompression		= BI_RGB;
	BmpInfo.bmiHeader.biClrUsed			= (Channels==1) ? 256 : 0;
	BmpInfo.bmiHeader.biClrImportant	= 0;
	BmpInfo.bmiHeader.biSizeImage		= 0;
	BmpInfo.bmiHeader.biXPelsPerMeter	= 0;
	BmpInfo.bmiHeader.biYPelsPerMeter	= 0;

	// Create a device context for the display
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
			m_hDCBmp = CreateCompatibleDC(hDCTemp);

			if (m_hDCBmp)
				m_hOldBmp = (HBITMAP)SelectObject(m_hDCBmp, m_hThisBmp);  //store the 1x1 HBitmap that was in the DC because it needs to be replaced before deletion
		}
	
		//No longer needed after the memory DC is created 
		DeleteDC(hDCTemp);
	
	}

	//if any of these essential members are not initialized then there is a problem
	if (!m_hThisBmp || !m_hDCBmp || !m_hOldBmp || !m_pBits)
	{
		DisplayLastWinError();
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
/*
//------------------------------------------------------------------------------------------
//	Function Name:	CBitmap::CreateFromClient()
//	
//	Param 1:		HWND hWnd		- Handle to window with client space you wish to use.
//	Param 2:		int Channels	- number of bytes per pixel this bmp should be
//	Returns:		bool 			- true on successful creation, false otherwise
//	
//	Description:	Creates a bitmap from a screenshot of the client space for a particular window.
//------------------------------------------------------------------------------------------
bool CBitmap::CreateFromClient( HWND hWnd, int Channels )
{
	// error checking
	if( hWnd == 0 )
		return false;

	RECT rcClient;
	RECT rcWindow;

	// Get our client and window rect
	GetWindowRect(hWnd, &rcWindow);
	GetClientRect(hWnd, &rcClient);
	
	// offset the position of our client rect by the position of our window
	// and compute the client width and height.
	return CreateFromDesktop( rcWindow.left   + rcClient.left, 
							  rcWindow.top    + rcClient.top, 
							  rcClient.right  - rcClient.left,
							  rcClient.bottom - rcClient.top, 
							  Channels );
}
*/

//------------------------------------------------------------------------------------------
//	Function Name:	CBitmap::CreateFromDesktop()
//	
//	Param 1:		int SrcX		- x coordinate of the upper left corner of the desired area of the screen
//	Param 2:		int SrcY		- y coordinate of the upper left corner of the desired area of the screen
//	Param 3:		int SrcWidth	- width of the rectangle desired for capturing
//	Param 4:		int SrcHeight	- height of the rectangle desired for capturing
//	Param 5:		int Channels	- number of bytes per pixel this bmp should be
//	Returns:		bool 			- true on successful creation, false otherwise
//	
//	Description:	Creates a bitmap which is a screenshot of a passed in rectangle of the current screen
//------------------------------------------------------------------------------------------
bool CBitmap::CreateFromDesktop(int SrcX, int SrcY, int SrcWidth, int SrcHeight, int Channels)
{
	//Wipe out the class and make a new Bmp of the specified dimensions
	if(!Create(SrcWidth, SrcHeight, Channels))
		return false;

	HDC hDCScreen;
	

	//TODO: Look into using GetDCEx as it can be possible to get a DC of a specified rectangle
	
	//get a device context for the whole screen
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


//------------------------------------------------------------------------------------------
//	Function Name:	CBitmap::Draw()
//	
//	Param 1:		HDC hDestDC		- device context to draw into
//	Param 2:		int DstX		- x coordinate to draw at
//	Param 3:		int DstY		- y coordinate to draw at
//	Param 4:		int SrcX		- x coordinate to draw from
//	Param 5:		int SrcY		- y coordinate to draw from
//	Param 6:		int SrcWidth	- width of the section to draw from
//	Param 7:		int SrcHeight	- height of the section to draw from
//	
//	Description:	Draws a section of this bitmap to a device context at the given coordinates
//------------------------------------------------------------------------------------------
void CBitmap::Draw(HDC hDestDC, int DstX, int DstY, int SrcX, int SrcY, int SrcWidth, int SrcHeight) const
{
	if( !m_hDCBmp )
		return;

	BitBlt(hDestDC, DstX, DstY, SrcWidth, SrcHeight, m_hDCBmp, SrcX, SrcY, SRCCOPY);
}  

//------------------------------------------------------------------------------------------
//	Function Name:	CBitmap::Draw()
//	
//	Param 1:		HDC hDestDC	- device context to draw into
//	Param 2:		int DstX	- x coordinate to draw at
//	Param 3:		int DstY	- y coordinate to draw at
//	
//	Description:	Draws this entire bitmap to a device context.. to draw to the window's DC
//					use GetDC() for your window but be sure to call ReleaseDC() when finished drawing
//------------------------------------------------------------------------------------------
void CBitmap::Draw(HDC hDestDC, int DstX, int DstY) const
{
	Draw(hDestDC, DstX, DstY, 0, 0, m_Width, m_Height);
}  


//------------------------------------------------------------------------------------------
//	Function Name:	CBitmap::LoadBmpFile()
//	
//	Param 1:		const char *FileName- Location and name of the bmp file
//	Returns:		bool 				- true on successful loading and creation, false otherwise
//	
//	Description:	Reads in a bitmap and replaces any existing data with the new data from the file
//------------------------------------------------------------------------------------------
bool CBitmap::LoadBmpFile(const char *FileName)
{
	FILE *fp		= NULL;
	
	BITMAPINFOHEADER bih;
	BITMAPFILEHEADER bfh;

	//argument checks
	if( FileName == 0 || FileName[0] == 0 )
		return false;

	//open the file in read mode
	fp = fopen(FileName, "rb");

	if(fp == NULL)
		return false;

	//initialize the file's structures
	memset(&bih, 0, sizeof(BITMAPINFOHEADER));
	memset(&bfh, 0, sizeof(BITMAPFILEHEADER));

	bih.biSize			= 0;
	bih.biWidth			= 0;
	bih.biHeight		= 0;
	bih.biPlanes		= 0;
	bih.biBitCount		= 0;
	bih.biCompression	= 0;
	bih.biClrUsed		= 0;
	bih.biClrImportant	= 0;
	bih.biSizeImage		= 0;
	bih.biXPelsPerMeter	= 0;
	bih.biYPelsPerMeter	= 0;

	bfh.bfOffBits		= 0;
	bfh.bfReserved1		= 0;
	bfh.bfReserved2		= 0;
	bfh.bfSize			= 0;
	bfh.bfType			= 0;

	//read in the file header structure
	if(fread(&bfh, sizeof(BITMAPFILEHEADER), 1, fp) != 1)
	{
		fclose(fp);
		return false;
	}

	//if the correct section of the file doesn't equal this equation
	//then this is an invalid file format or the file is corrupt
	if(bfh.bfType != ('M' * 256 + 'B') )
	{
		fclose(fp);
		return false;
	}


	//read in the info file header structure
	if(fread(&bih, sizeof(BITMAPINFOHEADER), 1, fp) != 1)
	{
		fclose(fp);
		return false;
	}

	
	//all these must be in the file or else it isn't supported and must fail
	if(bih.biPlanes != 1)
	{
		fclose(fp);
		return false;
	}
	if(bih.biBitCount != 24 && bih.biBitCount != 32)
	{
		fclose(fp);
		return false;
	}
	if(bih.biCompression != BI_RGB)
	{
		fclose(fp);
		return false;
	}


	massert((bih.biBitCount/8) == 3 || (bih.biBitCount/8) == 4, "LoadBmpFile: Must be a 24 or 32 bit image");

	//create a bitmap to fill with the file's data
	if(!Create(bih.biWidth, bih.biHeight, bih.biBitCount / 8))
	{
		fclose(fp);
		return false;
	}


	//now skip over the offset specified by OffBits so we can jump straight to the pixel data
	fseek(fp, bfh.bfOffBits, SEEK_SET);

	//Must read in starting from the bottom because windows bitmaps present the pixel data that way
	PUCHAR pLine = NULL;
	for(int y = m_Height - 1; y >= 0; y--)
	{	
		pLine = GetLinePtr(y);
		if(fread(pLine, m_BytesPerLine, 1, fp) != 1)
		{
			fclose(fp);
			return false;
		}

	}
	
	//we are done.. so close the file and indicate success
	fclose(fp);
	return true;

}

//------------------------------------------------------------------------------------------
//	Function Name:	CBitmap::SaveBmpFile()
//	
//	Param 1:		const char *FileName- Location and name of the bmp file
//	Returns:		bool 				- true on successful saving, false otherwise
//	
//	Description:	Saves whatever bitmap data is currently held inside this class to a standard bmp file
//------------------------------------------------------------------------------------------
bool CBitmap::SaveBmpFile(const char *FileName) const
{
	BITMAPINFOHEADER bih;
	BITMAPFILEHEADER bfh;
	FILE*			 fp = NULL;


	//initialize the file's structures
	memset(&bih, 0, sizeof(BITMAPINFOHEADER));
	memset(&bfh, 0, sizeof(BITMAPFILEHEADER));

	bih.biSize			= 0;
	bih.biWidth			= 0;
	bih.biHeight		= 0;
	bih.biPlanes		= 0;
	bih.biBitCount		= 0;
	bih.biCompression	= 0;
	bih.biClrUsed		= 0;
	bih.biClrImportant	= 0;
	bih.biSizeImage		= 0;
	bih.biXPelsPerMeter	= 0;
	bih.biYPelsPerMeter	= 0;

	bfh.bfOffBits		= 0;
	bfh.bfReserved1		= 0;
	bfh.bfReserved2		= 0;
	bfh.bfSize			= 0;
	bfh.bfType			= 0;

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

//------------------------------------------------------------------------------------------
//	Function Name:	CBitmap::Copy()
//	
//	Param 1:		const CBitmap &BmpToCopy	- the bitmap that will be copied to this one
//	Returns:		bool 						- true on a successful copy, false otherwise
//	
//	Description:	Resizes this bitmap and copies the pixel data to match that of the passed in bitmap
//------------------------------------------------------------------------------------------
bool CBitmap::Copy(const CBitmap &BmpToCopy)
{
	//in this case.. the bitmap we are copying is empty so there is nothing to copy
	if(!BmpToCopy.IsInitilized())
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

//------------------------------------------------------------------------------------------
//	Function Name:	CBitmap::operator == ()
//	
//	Param 1:		const CBitmap &BmpRight	- the bitmap that this one will be compared with
//	Returns:		bool 					- true if the bitmaps are of the same width, height, and bit depth
//	
//	Description:	determines if two bitmaps are conceptually equal
//------------------------------------------------------------------------------------------
bool CBitmap::operator == (const CBitmap &BmpRight) const
{
	return ( (m_Width == BmpRight.GetWidth()) && (m_Height == BmpRight.GetHeight()) && (m_Channels == BmpRight.GetChannels()) );
}

//------------------------------------------------------------------------------------------
//	Function Name:	CBitmap::operator != ()
//	
//	Param 1:		const CBitmap &BmpRight	- the bitmap that this one will be compared with
//	Returns:		bool - true if the bitmaps are NOT the same width, height, and bit depth
//	
//	Description:	determines if two bitmaps are NOT conceptually equal in dimensions and bit depth
//------------------------------------------------------------------------------------------
bool CBitmap::operator != (const CBitmap &BmpRight) const
{
	return ( (m_Width != BmpRight.GetWidth()) || (m_Height != BmpRight.GetHeight()) || (m_Channels != BmpRight.GetChannels()) );
}

//------------------------------------------------------------------------------------------
//	Function Name:	CBitmap::operator = ()
//	
//	Param 1:		const CBitmap &BmpRight	- the bitmap that will be copied to this one
//	Returns:		CBitmap & 				- this bitmap after modification
//	
//	Description:	Resizes this bitmap and copies the pixel data to match that of the passed in bitmap
//------------------------------------------------------------------------------------------
CBitmap & CBitmap::operator = (const CBitmap &BmpRight)
{
	//check that the objects aren't actually the same instance
	if(this == &BmpRight)
		return *this;
	bool bRet = Copy( BmpRight );

	massert(bRet, "operator= : The right hand bitmap shouldn't be empty if assigning it to another.");
	return *this;
}


//------------------------------------------------------------------------------------------
// CBitmap.cpp - End of file
// Thanks to Fluid Studios for their comment generator
//------------------------------------------------------------------------------------------
