/* Copyright (C) Electronic Arts Canada Inc. 1999. All rights reserved. */

#ifndef SCOMPMPEG_H
#define SCOMPMPEG_H


#define MPEG_VER1   3
#define MPEG_VER2   2
#define MPEG_VER2_5 0


typedef struct MPEGAUDIOHDR
{
    unsigned int bitrate;
        
    unsigned short samplerate;
    unsigned short numframes;

    unsigned short framebytes;
    unsigned short padshort;

    unsigned char version;
    unsigned char layer;
    unsigned char channels;
    unsigned char mode;

    unsigned char modeext;
    unsigned char crc;
    unsigned char padded;
    unsigned char samplerateindex;

    unsigned char bitrateindex;
    char padchar[3];
} MPEGAUDIOHDR;


int SIMEXI_mpegparseheader(unsigned int hdr, MPEGAUDIOHDR *pmah);
int GetMpegFrameSize( unsigned char * pData );

#endif

