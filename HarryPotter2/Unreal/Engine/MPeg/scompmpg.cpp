/* Copyright (C) Electronic Arts Canada Inc. 1999. All rights reserved. */

#include "scompmpg.h"

//#include <TbDebug.h>
#include "Core.h"

/******************************* MPEG tables *******************************/

/***************************** Sample rate index ***************************/

/*double mpegsamplerate[2][4] = 
{
    {22.05, 24, 16, 0}, 
    {44.1, 48, 32, 0}
};*/

int mpegsamplerate[2][4] = 
{
    { 22050, 24000, 16000, 0 }, 
    { 44100, 48000, 32000, 0 }
};
/***************************** Bit rate index ******************************/

int mpegbitrate[2][3][15] = 
{
    {
        {0,32,48,56,64,80,96,112,128,144,160,176,192,224,256},
        {0,8,16,24,32,40,48,56,64,80,96,112,128,144,160},
        {0,8,16,24,32,40,48,56,64,80,96,112,128,144,160}
    },
   
    {
        {0,32,64,96,128,160,192,224,256,288,320,352,384,416,448},
        {0,32,48,56,64,80,96,112,128,160,192,224,256,320,384},
        {0,32,40,48,56,64,80,96,112,128,160,192,224,256,320}
    }
};

#if defined(_MSC_VER) && defined(_M_IX86)
#pragma warning( disable : 4035 )
unsigned int inline swap(unsigned int a)
{
	_asm
	{
		mov eax,a
		bswap eax		
	}	
}
/*#else
unsigned int __inline swap(unsigned int a)
{
	return ((a&0xff)<<24) | ((a&0xff00)<<8) | ((a>>8)&0xff00)| ((a>>24)&0xff);
}*/
#endif


int GetMpegFrameSize( unsigned char * pData )
{
	unsigned int hdr;

#if defined(_MSC_VER) && defined(_M_IX86)
	hdr = swap(*(unsigned int*)pData);
#else
	// hee hee!
	hdr = (pData[0] << 24) + (pData[1] << 16) + (pData[2] << 8) + pData[3];
#endif

	if ((hdr & 0xfff00000) == 0xfff00000)
	{ 	
		if( (4 - ((hdr >> 17) & 3)) == 2  )
		{
			int bitrateindex = (hdr >> 12) & 15;
			int padded = ((hdr >> 9) & 0x1);
			int version = (hdr >> 19) & 3;
			int bitrate = mpegbitrate[0][1][bitrateindex];
			int samplerate = mpegsamplerate[1][(hdr >> 10) & 0x3];

			if( version == MPEG_VER2 )
			{
				samplerate >>= 1;
			}
			else if( version == MPEG_VER2_5 )
			{
				samplerate >>= 2;
			}
			else if( version == MPEG_VER1 )
			{
				bitrate = mpegbitrate[1][1][bitrateindex];
			}

			return ((144000 * bitrate) / samplerate) + ((padded) ? 1 : 0);
		}
	}

	//assert(0);
	check (0);

	return 0;
}

int SIMEXI_mpegparseheader(unsigned int hdr, MPEGAUDIOHDR *pmah)
{
	if ((hdr & 0xfff00000) != 0xfff00000)
	    return (0);
 	
	pmah->layer = 4 - ((hdr >> 17) & 3);
	pmah->crc = !((hdr >> 16) & 1);
	pmah->bitrateindex = (hdr >> 12) & 15;
	pmah->padded = ((hdr >> 9) & 0x1);
	pmah->mode = ((hdr >> 6) & 0x3);
	pmah->modeext = ((hdr >> 4) & 0x3);			
#if 0
	copyright = ((hdr >> 3) & 1);
	original = ((hdr >> 2) & 1);
#endif

    /* Check for an illegal layer. */
    if (pmah->layer == 4)
        return (0);

    /* Check for an illegal bit rate. */
    if (pmah->bitrateindex == 15)
        return (0);

    pmah->version = (hdr >> 19) & 3;

    /* Check for illegal version. */
    if (pmah->version == 1)
        return (0);

	pmah->samplerateindex = (hdr >> 10) & 0x3;

    /* Check for an illegal sample rate. */
    if (pmah->samplerateindex == 3)
        return (0);

  	pmah->channels = (pmah->mode == 3) ? 1 : 2;

	pmah->samplerate = (int)(mpegsamplerate[1][pmah->samplerateindex]);

    if (pmah->version == MPEG_VER2)
        pmah->samplerate >>= 1;
        
    if (pmah->version == MPEG_VER2_5)
        pmah->samplerate >>= 2;

	if (!pmah->bitrateindex) 
	    return (0);

    if (pmah->version == MPEG_VER1)
	    pmah->bitrate = mpegbitrate[1][pmah->layer - 1][pmah->bitrateindex];
    else
	    pmah->bitrate = mpegbitrate[0][pmah->layer - 1][pmah->bitrateindex];

    /* Determine how many bytes of data are present in the frame. */
	if (pmah->layer == 1)
	{	
	    pmah->framebytes = (12000 * pmah->bitrate) / pmah->samplerate;
		pmah->framebytes = (pmah->framebytes + pmah->padded) << 2;
	    pmah->numframes = 384;
	} 
	else 
	{	
	    pmah->numframes = 1152;
        	    
	    pmah->framebytes = (144000 * pmah->bitrate) / pmah->samplerate;

		if (pmah->layer == 3 && pmah->version != MPEG_VER1)
    	{
    		pmah->framebytes >>= 1;
	        pmah->numframes = 576;
		}
  
		if (pmah->padded) 
		    pmah->framebytes++;
	}
        
    /* Subtract header size. */
//	pmah->framebytes -= 4;             

#if 0
    if (pmah->framebytes <= 0)
        printf("Illegal framebytes %i: bitrate: %i ver: %i, lay: %i, bitindex: %i\n", 
            pmah->framebytes, pmah->bitrate, pmah->version, pmah->layer, 
            pmah->bitrateindex);
#endif

    return (pmah->framebytes);
}

