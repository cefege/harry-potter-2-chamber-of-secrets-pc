// -----------------------------------------------------------------------------
//  ____  _ _                                              
// |  _ \(_) |                                             
// | |_) |_| |_ _ __ ___   __ _ _ __       ___ _ __  _ __  
// |  _ <| | __| '_ ` _ \ / _` | '_ \     / __| '_ \| '_ \ 
// | |_) | | |_| | | | | | (_| | |_) | _ | (__| |_) | |_) |
// |____/|_|\__|_| |_| |_|\__,_| .__/ (_) \___| .__/| .__/ 
//                             | |            | |   | |    
//                             |_|            |_|   |_|    
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


// **********************************************************************************
// INCLUDES
#define WIN32_LEAN_AND_MEAN
#include <windows.h>
#include <stdio.h>
#include <assert.h>

#include "Bitmap.h"

// **********************************************************************************
// DEFINES
#define	BLUE_BITMAP_OFFSET		0
#define	GREEN_BITMAP_OFFSET		1
#define	RED_BITMAP_OFFSET		2
#define	ALPHA_BITMAP_OFFSET		3

//Adds a message to the assertion dialog
//a is an expressions, b is a string to clarify why the assertion failed
#define massert(a,b)	assert((a) && (b))

static const float Alphas[] = 
{
0.0f, 
0.00392157f, 0.00784314f, 0.0117647f, 0.0156863f, 0.0196078f, 
0.0235294f, 0.027451f, 0.0313725f, 0.0352941f, 0.0392157f, 
0.0431373f, 0.0470588f, 0.0509804f, 0.054902f, 0.0588235f, 
0.0627451f, 0.0666667f, 0.0705882f, 0.0745098f, 0.0784314f, 
0.0823529f, 0.0862745f, 0.0901961f, 0.0941176f, 0.0980392f, 
0.101961f, 0.105882f, 0.109804f, 0.113725f, 0.117647f, 
0.121569f, 0.12549f, 0.129412f, 0.133333f, 0.137255f, 
0.141176f, 0.145098f, 0.14902f, 0.152941f, 0.156863f, 
0.160784f, 0.164706f, 0.168627f, 0.172549f, 0.176471f, 
0.180392f, 0.184314f, 0.188235f, 0.192157f, 0.196078f, 
0.2f, 0.203922f, 0.207843f, 0.211765f, 0.215686f, 
0.219608f, 0.223529f, 0.227451f, 0.231373f, 0.235294f, 
0.239216f, 0.243137f, 0.247059f, 0.25098f, 0.254902f, 
0.258824f, 0.262745f, 0.266667f, 0.270588f, 0.27451f, 
0.278431f, 0.282353f, 0.286275f, 0.290196f, 0.294118f, 
0.298039f, 0.301961f, 0.305882f, 0.309804f, 0.313725f, 
0.317647f, 0.321569f, 0.32549f, 0.329412f, 0.333333f, 
0.337255f, 0.341176f, 0.345098f, 0.34902f, 0.352941f, 
0.356863f, 0.360784f, 0.364706f, 0.368627f, 0.372549f, 
0.376471f, 0.380392f, 0.384314f, 0.388235f, 0.392157f, 
0.396078f, 0.4f, 0.403922f, 0.407843f, 0.411765f, 
0.415686f, 0.419608f, 0.423529f, 0.427451f, 0.431373f, 
0.435294f, 0.439216f, 0.443137f, 0.447059f, 0.45098f, 
0.454902f, 0.458824f, 0.462745f, 0.466667f, 0.470588f, 
0.47451f, 0.478431f, 0.482353f, 0.486275f, 0.490196f, 
0.494118f, 0.498039f, 0.501961f, 0.505882f, 0.509804f, 
0.513725f, 0.517647f, 0.521569f, 0.52549f, 0.529412f, 
0.533333f, 0.537255f, 0.541176f, 0.545098f, 0.54902f, 
0.552941f, 0.556863f, 0.560784f, 0.564706f, 0.568627f, 
0.572549f, 0.576471f, 0.580392f, 0.584314f, 0.588235f, 
0.592157f, 0.596078f, 0.6f, 0.603922f, 0.607843f, 
0.611765f, 0.615686f, 0.619608f, 0.623529f, 0.627451f, 
0.631373f, 0.635294f, 0.639216f, 0.643137f, 0.647059f, 
0.65098f, 0.654902f, 0.658824f, 0.662745f, 0.666667f, 
0.670588f, 0.67451f, 0.678431f, 0.682353f, 0.686275f, 
0.690196f, 0.694118f, 0.698039f, 0.701961f, 0.705882f, 
0.709804f, 0.713725f, 0.717647f, 0.721569f, 0.72549f, 
0.729412f, 0.733333f, 0.737255f, 0.741176f, 0.745098f, 
0.74902f, 0.752941f, 0.756863f, 0.760784f, 0.764706f, 
0.768627f, 0.772549f, 0.776471f, 0.780392f, 0.784314f, 
0.788235f, 0.792157f, 0.796078f, 0.8f, 0.803922f, 
0.807843f, 0.811765f, 0.815686f, 0.819608f, 0.823529f, 
0.827451f, 0.831373f, 0.835294f, 0.839216f, 0.843137f, 
0.847059f, 0.85098f, 0.854902f, 0.858824f, 0.862745f, 
0.866667f, 0.870588f, 0.87451f, 0.878431f, 0.882353f, 
0.886275f, 0.890196f, 0.894118f, 0.898039f, 0.901961f, 
0.905882f, 0.909804f, 0.913725f, 0.917647f, 0.921569f, 
0.92549f, 0.929412f, 0.933333f, 0.937255f, 0.941176f, 
0.945098f, 0.94902f, 0.952941f, 0.956863f, 0.960784f, 
0.964706f, 0.968627f, 0.972549f, 0.976471f, 0.980392f, 
0.984314f, 0.988235f, 0.992157f, 0.996078f, 1.0f };

// **********************************************************************************
//	Function Name:	CBitmap::Clear()
//	Created on: 	8/9/01
//	
//	Description:	Wipes out the data members
// **********************************************************************************
void CBitmap::Clear(void)
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

// **********************************************************************************
//	Function Name:	CBitmap::Destroy()
//	Created on: 	8/9/01
//	
//	Description:	Frees any bitmap data and then clears out all the data members
// **********************************************************************************
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

// **********************************************************************************
//	Function Name:	CBitmap::Draw()
//	Created on: 	8/10/01
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
// **********************************************************************************
void CBitmap::Draw(HDC hDestDC, int DstX, int DstY, int SrcX, int SrcY, int SrcWidth, int SrcHeight) const
{
	if( !m_hDCBmp )
		return;

	BitBlt(hDestDC, DstX, DstY, SrcWidth, SrcHeight, m_hDCBmp, SrcX, SrcY, SRCCOPY);
}  

// **********************************************************************************
//	Function Name:	CBitmap::Draw()
//	Created on: 	8/10/01
//	
//	Param 1:		HDC hDestDC	- device context to draw into
//	Param 2:		int DstX	- x coordinate to draw at
//	Param 3:		int DstY	- y coordinate to draw at
//	
//	Description:	Draws this entire bitmap to a device context.. to draw to the window's DC
//					use GetDC() for your window but be sure to call ReleaseDC() when finished drawing
// **********************************************************************************
void CBitmap::Draw(HDC hDestDC, int DstX, int DstY) const
{
	assert( hDestDC );

	Draw(hDestDC, DstX, DstY, 0, 0, m_Width, m_Height);
}  

void CBitmap::DrawStretched(HDC hDestDC, int DstX, int DstY, int DstWidth, int DstHeight, int SrcX, int SrcY, int SrcWidth, int SrcHeight) const
{
	if( !m_hDCBmp )
		return;

	StretchBlt( hDestDC, DstX, DstY, DstWidth, DstHeight, m_hDCBmp, SrcX, SrcY, SrcWidth, SrcHeight, SRCCOPY );
/*
BOOL StretchBlt(
  HDC hdcDest,      // handle to destination DC
  int nXOriginDest, // x-coord of destination upper-left corner
  int nYOriginDest, // y-coord of destination upper-left corner
  int nWidthDest,   // width of destination rectangle
  int nHeightDest,  // height of destination rectangle
  HDC hdcSrc,       // handle to source DC
  int nXOriginSrc,  // x-coord of source upper-left corner
  int nYOriginSrc,  // y-coord of source upper-left corner
  int nWidthSrc,    // width of source rectangle
  int nHeightSrc,   // height of source rectangle
  DWORD dwRop       // raster operation code
);*/
}

void CBitmap::DrawStretched(HDC hDestDC, int DstX, int DstY, int DstWidth, int DstHeight ) const
{
	DrawStretched( hDestDC, DstX, DstY, DstWidth, DstHeight, 0, 0, m_Width, m_Height );
}

// **********************************************************************************
//	Function Name:	CBitmap::CreateBmp()
//	Created on: 	8/9/01	
//	
//	Param 1:		int Width	- desired width of this new bmp
//	Param 2:		int Height	- desired height of this new bmp
//	Param 3:		int Channels- number of bytes per pixel this bmp should be
//	Returns:		bool 		- true on successful creation, false otherwise
//	
//	Description:	Destroys any current bmp data and creates a new one of the specified
//					dimensions and bit depth
// **********************************************************************************
bool CBitmap::CreateBmp( int Width, int Height, int Channels )
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

	m_Width = Width;
	m_Height = Height;
	m_Channels = Channels;

	//Initialize this structure which is required for CreateDIBSection()
	BITMAPINFO	BmpInfo;
	memset(&BmpInfo, 0, sizeof(BITMAPINFO));

	BmpInfo.bmiHeader.biSize			= sizeof(BITMAPINFOHEADER);
	BmpInfo.bmiHeader.biWidth			= Width;
	BmpInfo.bmiHeader.biHeight			= Height;
	BmpInfo.bmiHeader.biPlanes			= 1;
	BmpInfo.bmiHeader.biBitCount		= Channels * 8;
	BmpInfo.bmiHeader.biCompression		= BI_RGB;
	BmpInfo.bmiHeader.biClrUsed			= (Channels==1) ? 256 : 0;
	BmpInfo.bmiHeader.biClrImportant	= 0;
	BmpInfo.bmiHeader.biSizeImage		= 0;
	BmpInfo.bmiHeader.biXPelsPerMeter	= 0;
	BmpInfo.bmiHeader.biYPelsPerMeter	= 0;

	//Create a device context for the display
	HDC	hDCTemp;
	hDCTemp = CreateDC("Display", NULL, NULL, NULL);

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


// **********************************************************************************
//	Function Name:	CBitmap::LoadBmpFile()
//	Created on: 	8/9/01
//	
//	Param 1:		const char *FileName- Location and name of the bmp file
//	Returns:		bool 				- true on successful loading and creation, false otherwise
//	
//	Description:	Reads in a bitmap and replaces any existing data with the new data from the file
// **********************************************************************************
bool CBitmap::LoadBmpFile( const char *FileName )
{
	FILE *fptr		= NULL;
	
	BITMAPINFOHEADER bih;
	BITMAPFILEHEADER bfh;
	
	//argument checks
	if( FileName == 0 || FileName[0] == 0 )
		return false;
	
	//open the file in read mode
	fptr = fopen(FileName, "rb");
	
	if(fptr == NULL)
		return false;
	
	strcpy( m_Filename, FileName );

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
	if(fread(&bfh, sizeof(BITMAPFILEHEADER), 1, fptr) != 1)
	{
		fclose(fptr);
		return false;
	}
	
	//if the correct section of the file doesn't equal this equation
	//then this is an invalid file format or the file is corrupt
	if(bfh.bfType != ('M' * 256 + 'B') )
	{
		fclose(fptr);
		return false;
	}
	
	//read in the info file header structure
	if(fread(&bih, sizeof(BITMAPINFOHEADER), 1, fptr) != 1)
	{
		fclose(fptr);
		return false;
	}
	
	//all these must be in the file or else it isn't supported and must fail
	if(bih.biPlanes != 1)
	{
		fclose(fptr);
		return false;
	}
	if(bih.biBitCount != 24 && bih.biBitCount != 32)
	{
		fclose(fptr);
		return false;
	}
	if(bih.biCompression != BI_RGB)
	{
		fclose(fptr);
		return false;
	}
	
	massert((bih.biBitCount/8) == 3 || (bih.biBitCount/8) == 4, "LoadBmpFile: Must be a 24 or 32 bit image");
	
	//create a bitmap to fill with the file's data
	if(!CreateBmp(bih.biWidth, bih.biHeight, bih.biBitCount / 8))
	{
		fclose(fptr);
		return false;
	}

	//now skip over the offset specified by OffBits so we can jump straight to the pixel data
	fseek(fptr, bfh.bfOffBits, SEEK_SET);

	//Must read in starting from the bottom because windows bitmaps present the pixel data that way
	PUCHAR pLine = NULL;
	for(int y = m_Height - 1; y >= 0; y--)
	{	
		pLine = GetLinePtr(y);
		if(fread(pLine, m_BytesPerLine, 1, fptr) != 1)
		{
			fclose(fptr);
			return false;
		}
	}
	
	// we are done.. so close the file and indicate success
	fclose(fptr);

	return true;
}

// **********************************************************************************
//	Function Name:	CBitmap::SaveBmpFile()
//	Created on: 	8/9/01
//	
//	Param 1:		const char *FileName- Location and name of the bmp file
//	Returns:		bool 				- true on successful saving, false otherwise
//	
//	Description:	Saves whatever bitmap data is currently held inside this class to a standard bmp file
// **********************************************************************************
bool CBitmap::SaveBmpFile(const char *FileName) const
{
	FILE *fptr = NULL;
	BITMAPINFOHEADER bih;
	BITMAPFILEHEADER bfh;

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
	fptr = fopen(FileName, "wb");

	if(fptr == NULL)
		return false;

	//fill and write the file header information
	bfh.bfType = ('M' * 256 + 'B');
	bfh.bfSize = sizeof(BITMAPFILEHEADER);
	bfh.bfOffBits = sizeof(BITMAPINFOHEADER) + sizeof(BITMAPFILEHEADER);

	if(fwrite(&bfh, sizeof(BITMAPFILEHEADER), 1, fptr) != 1)
	{
		fclose(fptr);
		return false;
	}

	//fill and write the info header information
	bih.biPlanes = 1;
	bih.biBitCount = m_Channels * 8;
	bih.biSize = sizeof(BITMAPINFOHEADER);
	bih.biCompression = BI_RGB;
	bih.biWidth = m_Width;
	bih.biHeight = m_Height;
	bih.biClrUsed = (m_Channels==1)?256:0;

	if(fwrite(&bih, sizeof(BITMAPINFOHEADER), 1, fptr) != 1)
	{
		fclose(fptr);
		return false;
	}

	PUCHAR pLine = NULL;
	for(int y = m_Height - 1; y >= 0; y--)
	{	
		pLine = GetLinePtr(y);
		if(fwrite(pLine, m_BytesPerLine, 1, fptr) != 1)
		{
			fclose(fptr);
			return false;
		}
	}
	
	//close the file
	fclose(fptr);
	return true;
}

// **********************************************************************************
//	Function Name:	CBitmap::operator == ()
//	Created on: 	8/9/01
//	
//	Param 1:		const CBitmap &BmpRight	- the bitmap that this one will be compared with
//	Returns:		bool 					- true if the bitmaps are of the same width, height, and bit depth
//	
//	Description:	determines if two bitmaps are conceptually equal
// **********************************************************************************
bool CBitmap::operator == (const CBitmap &BmpRight) const
{
	return ( (m_Width == BmpRight.GetWidth()) && (m_Height == BmpRight.GetHeight()) && (m_Channels == BmpRight.GetChannels()) );
}

// **********************************************************************************
//	Function Name:	CBitmap::operator != ()
//	Created on: 	8/9/01
//	
//	Param 1:		const CBitmap &BmpRight	- the bitmap that this one will be compared with
//	Returns:		bool - true if the bitmaps are NOT the same width, height, and bit depth
//	
//	Description:	determines if two bitmaps are NOT conceptually equal in dimensions and bit depth
// **********************************************************************************
bool CBitmap::operator != (const CBitmap &BmpRight) const
{
	return ( (m_Width != BmpRight.GetWidth()) || (m_Height != BmpRight.GetHeight()) || (m_Channels != BmpRight.GetChannels()) );
}

// **********************************************************************************
//	Function Name:	CBitmap::operator = ()
//	Created on: 	8/10/01
//	
//	Param 1:		const CBitmap &BmpRight	- the bitmap that will be copied to this one
//	Returns:		CBitmap & 				- this bitmap after modification
//	
//	Description:	Resizes this bitmap and copies the pixel data to match that of the passed in bitmap
// **********************************************************************************
CBitmap & CBitmap::operator = (const CBitmap &BmpRight)
{
	//check that the objects aren't actually the same instance
	if(this == &BmpRight)
		return *this;
	bool bRet = CopyBmp(BmpRight);

	massert(bRet, "operator= : The right hand bitmap shouldn't be empty if assigning it to another.");
	return *this;
}

// **********************************************************************************
//	Function Name:	CBitmap::CopyBmp()
//	Created on: 	8/10/01
//	
//	Param 1:		const CBitmap &BmpToCopy	- the bitmap that will be copied to this one
//	Returns:		bool 						- true on a successful copy, false otherwise
//	
//	Description:	Resizes this bitmap and copies the pixel data to match that of the passed in bitmap
// **********************************************************************************
bool CBitmap::CopyBmp(const CBitmap &BmpToCopy)
{
	//in this case.. the bitmap we are copying is empty so there is nothing to copy
	if(!BmpToCopy.IsValidBmp())
		return false;

	//if they aren't already equal in dimensions then recreate ourselves to the passed in dimensions
	if(*this != BmpToCopy)
	{
		if(!CreateBmp(BmpToCopy.GetWidth(), BmpToCopy.GetHeight(), BmpToCopy.GetChannels()))
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


// **********************************************************************************
//	Function Name:	CBitmap::BlendBmpsTranslucent()
//	Created on: 	8/10/01
//	
//	Param 1:		const CBitmap &BmpBottom- bottom bitmap that will blend with the top
//	Param 2:		const CBitmap &BmpTop	- top bitmap that will blend with the bottom
//	Param 3:		float fPercent			- percentage to blend the top with the bottom from 0.0 to 1.0 (1.0 is solid, 0.0 is transparent)
//	Param 4:		int DstX				- destination x coordinate to blend to
//	Param 5:		int DstY				- destination y coordinate to blend to
//	Param 6:		int SrcX				- source x coordinate to blend from
//	Param 7:		int SrcY				- source y coordinate to blend from
//	Param 8:		int SrcWidth			- width of the area desired for blending on the source image
//	Param 9:		int SrcHeight			- height of the area desired for blending on the source image
//	Returns:		bool 					- true on success, false otherwise
//	
//	Description:	This will blend one bitmap with another.  The top area of blending can't be larger than the bottom bitmap.
// **********************************************************************************
bool CBitmap::BlendBmpsTranslucent(const CBitmap &BmpBottom, const CBitmap &BmpTop, float fPercent, int DstX, int DstY, int SrcX, int SrcY, int SrcWidth, int SrcHeight)
{

	//All the involved bitmaps must be valid
	if(!BmpBottom.IsValidBmp() || !BmpTop.IsValidBmp() || !IsValidBmp())
		return false;

	//the bottom and this bitmap have to be identical in dimensions because the bottom bitmap is essentially the same as this one.
	//if the bottom and this bitmap are the same instance, then a destructive blend occurs.. otherwise the 
	//bottom bitmap retains its original information
	if(*this != BmpBottom)
		return false;

	//everyone must have the same bit depth
	if(BmpBottom.GetChannels() != BmpTop.GetChannels())
		return false;

	//The rectangles cannot run out of bounds for their bitmap dimensions.. the user must clip them before passing them in so fail
	if((SrcX + SrcWidth) > BmpTop.GetWidth() || SrcX < 0 || SrcWidth <= 0)
		return false;
	if((SrcY + SrcHeight) > BmpTop.GetHeight() || SrcY < 0 || SrcHeight <= 0)
		return false;
	
	if( DstX < 0 || DstY < 0 )
		return false;
	
	// If the source is larger than clip the source to fit the dest
	if((DstX + SrcWidth) > BmpBottom.GetWidth() )
	{
		SrcWidth -= (DstX + SrcWidth) - BmpBottom.GetWidth();
	}
	
	if((DstY + SrcHeight) > BmpBottom.GetHeight() )
	{
		SrcHeight -= (DstY + SrcHeight) - BmpBottom.GetHeight();
	}

	if(fPercent < 0.0f)
		fPercent = 0.0f;
	if(fPercent > 1.0f)
		fPercent = 1.0f;

	//If we got this far.. then the rectangles don't go out of bounds for the source or the destination
	//So.. time to blend the goods

	float fFinalColor = 0;
	float fBotPercent = 1.0f - fPercent;
	float AlphafBotPercent = 0.0f, AlphaTopPercent = 0.0f;

	int y = 0, x = 0;

	PUCHAR pDstLine = 0, pBotLine = 0, pTopLine = 0;

	if(m_Channels == 3)
	{
		for(y = 0; y < SrcHeight; y++)
		{
			x = 0;
			pDstLine = GetPixelPtr(DstX + x, DstY + y);
			pBotLine = BmpBottom.GetPixelPtr(DstX + x, DstY + y);
			pTopLine = BmpTop.GetPixelPtr(SrcX + x, SrcY + y);

			for(x = 0; x < SrcWidth; x++)
			{
				//add the percentage of one color to the percentage of the other and store in the final place
				fFinalColor = pBotLine[RED_BITMAP_OFFSET] * fBotPercent; 
				fFinalColor += pTopLine[RED_BITMAP_OFFSET] * fPercent;
				pDstLine[RED_BITMAP_OFFSET]		= (UCHAR)fFinalColor;

				fFinalColor = pBotLine[GREEN_BITMAP_OFFSET] * fBotPercent; 
				fFinalColor += pTopLine[GREEN_BITMAP_OFFSET] * fPercent;
				pDstLine[GREEN_BITMAP_OFFSET]		= (UCHAR)fFinalColor;

				fFinalColor = pBotLine[BLUE_BITMAP_OFFSET] * fBotPercent; 
				fFinalColor += pTopLine[BLUE_BITMAP_OFFSET] * fPercent;
				pDstLine[BLUE_BITMAP_OFFSET]		= (UCHAR)fFinalColor;

				//advance past all the channels for this pixel onto the next pixel
				pDstLine += m_Channels;
				pBotLine += m_Channels;
				pTopLine += m_Channels;
			}
		}
	}
	else if(m_Channels == 4)
	{
		for(y = 0; y < SrcHeight; y++)
		{
			x = 0;
			pDstLine = GetPixelPtr(DstX + x, DstY + y);
			pBotLine = BmpBottom.GetPixelPtr(DstX + x, DstY + y);
			pTopLine = BmpTop.GetPixelPtr(SrcX + x, SrcY + y);

			for(x = 0; x < SrcWidth; x++)
			{
				//a value of 0 will do nothing so just show the bottom pixel
				if(pTopLine[ALPHA_BITMAP_OFFSET] == 0)
				{
					//advance past all the channels for this pixel onto the next pixel
					pDstLine += m_Channels;
					pBotLine += m_Channels;
					pTopLine += m_Channels;
					continue;
				}
				
				//Alphas[] is initialized in alphas.h
				AlphaTopPercent  = Alphas[pTopLine[ALPHA_BITMAP_OFFSET]] * fPercent;
				AlphafBotPercent = 1.0f - AlphaTopPercent;
				
				//add the percentage of one color to the percentage of the other and store in the final place
				fFinalColor = pBotLine[RED_BITMAP_OFFSET] * AlphafBotPercent; 
				fFinalColor += pTopLine[RED_BITMAP_OFFSET] * AlphaTopPercent;
				pDstLine[RED_BITMAP_OFFSET]		= (UCHAR)fFinalColor;

				fFinalColor = pBotLine[GREEN_BITMAP_OFFSET] * AlphafBotPercent; 
				fFinalColor += pTopLine[GREEN_BITMAP_OFFSET] * AlphaTopPercent;
				pDstLine[GREEN_BITMAP_OFFSET]		= (UCHAR)fFinalColor;

				fFinalColor = pBotLine[BLUE_BITMAP_OFFSET] * AlphafBotPercent; 
				fFinalColor += pTopLine[BLUE_BITMAP_OFFSET] * AlphaTopPercent;
				pDstLine[BLUE_BITMAP_OFFSET]		= (UCHAR)fFinalColor;

				//advance past all the channels for this pixel onto the next pixel
				pDstLine += m_Channels;
				pBotLine += m_Channels;
				pTopLine += m_Channels;
			}
		}
	}
	else
		return false;

	
	return true;
}

// **********************************************************************************
//	Function Name:	CBitmap::BlendBmpsTranslucent()
//	Created on: 	8/10/01
//	
//	Param 1:		const CBitmap &BmpBottom- bottom bitmap that will blend with the top
//	Param 2:		const CBitmap &BmpTop	- top bitmap that will blend with the bottom
//	Param 3:		float fPercent			- percentage to blend the top with the bottom from 0.0 to 1.0 (1.0 is solid, 0.0 is transparent)
//	Param 4:		int DstX				- destination x coordinate to blend to
//	Param 5:		int DstY				- destination y coordinate to blend to
//	Returns:		bool 					- true on success, false otherwise
//	
//	Description:	This will blend the entire top bitmap to the specified coordinates of the bottom bitmap
// **********************************************************************************
bool CBitmap::BlendBmpsTranslucent(const CBitmap &BmpBottom, const CBitmap &BmpTop, float fPercent, int DstX, int DstY)
{
	return BlendBmpsTranslucent(BmpBottom, BmpTop, fPercent, DstX, DstY, 0, 0, BmpTop.GetWidth(), BmpTop.GetHeight());
}


