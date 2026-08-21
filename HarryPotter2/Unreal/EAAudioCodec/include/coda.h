/* Copyright (C) Electronic Arts Canada Inc. 1999-2001. All rights reserved. */

/****************************************************************************
* This header file contains all prototypes and structures that are necessary
* to use the CODA (CODec for Audio) libraries.
****************************************************************************/

#ifndef CODA_H
#define CODA_H 1

#include <stddef.h>

#if defined(SIMEX_LIB)
namespace SIMEX {
#elif defined(SND_LIB)
namespace SND {
#endif

// Version of the CODA library.

#define CODA_VERSION_MAJOR          1
#define CODA_VERSION_MINOR          0
#define CODA_VERSION_PATCH          1
#define CODA_VERSION                ((CODA_VERSION_MAJOR << 16) | (CODA_VERSION_MINOR << 8) | CODA_VERSION_PATCH)


#if !defined(CODA_SIZE_T)
#if defined(MAC) || defined(GC)
#define CODA_SIZE_T std::size_t
#else
#define CODA_SIZE_T size_t
#endif
#endif

// Return codes.

const int CODA_OK =             0;
const int CODAERR_NOT_READY =   -1;
const int CODAERR_UNSUPPORTED = -2;

// Utility functions.

typedef void *(*CODANewFunctionType)(CODA_SIZE_T byteSize
#if (DEBUG >= 1)
    , const char *description
#endif
);
typedef void (*CODADeleteFunctionType)(void *voidPtr);

void CODASetNew(CODANewFunctionType pNew);
void CODASetDelete(CODADeleteFunctionType pDelete);

//-----------------------------------------------------------------------

// Xbox ADPCM decode to signed 16-bit class

class CXboxADPCMDecS16 
{
public:
	CXboxADPCMDecS16();		
	~CXboxADPCMDecS16() {}		
	
    int Feed(void *pData, int Bytes);
    int Decode(short *pDst[], int Samples);

    void *operator new(CODA_SIZE_T size);
    void operator delete(void *ptr);

private:
    inline int DecodeDelta(int EncodedSample);
    inline int DecodeSample(int EncodedSample);
    unsigned char *GetEncodedBlock(void);
    void DecodeBlock(unsigned char *pSrc, short *pDst);
    
    unsigned char *mpEncodedSamples;
    int mEncodedBytes;
    unsigned char mEncodedBlock[36];
    
    unsigned char mEncodedBlockBytes;
    unsigned char mBlockSamples;
	char pad[2];

    int mPredSample;
    int mStepIndex;

    short mDecodedBlock[64];
};

// Xbox ADPCM decode to float class

class CXboxADPCMDecF
{
public:
	CXboxADPCMDecF() {}		
	~CXboxADPCMDecF() {}		
	
    int Feed(void *pData, int Bytes);
    int Decode(float *pDst[], int Samples);

    void *operator new(CODA_SIZE_T size);
    void operator delete(void *ptr);
                  
private:
    CXboxADPCMDecS16 DecS16;
};

//-----------------------------------------------------------------------

// EAXA_BLK decoders

//user-tangible XA structures
typedef struct XA16STATE
{
    short sample1;
    short sample2;    
} XA16STATE;

typedef struct XAFSTATE
{
    float sample1;
    float sample2;
} XAFSTATE;

//actual struct used in assembly decoders
typedef struct MXAPACKET16
{
    int numframes;
    short sample1;
    short sample2;
    unsigned char *psrc;
    short *pdst;
} MXAPACKET16;

typedef struct MXAPACKETF
{
    int numframes;
    float sample1;
    float sample2;
    unsigned char *psrc;
    float *pdst;
} MXAPACKETF;

//miscellaneous XA related variables
typedef struct EAXAVARS16
{
    int residual;
	int numsamples;
	int sampledatasize;	
	short *presidue;
    short xablock[30];
} EAXAVARS16;

typedef struct EAXAVARSF
{
    int residual;
	int numsamples;
	int sampledatasize;
	float *presidue;
    float xablockprev[2];
    float xablock[30];
} EAXAVARSF;


/////////////////////////////////////////////////////////////////////
//
// The processor should determine which struct and decoding function
// must be used.
//
/////////////////////////////////////////////////////////////////////

// EAXA_BLK decoder to 16 bits

class CEAXABLKDec
{
public:
	CEAXABLKDec();		
	~CEAXABLKDec() {}		
    void *operator new(CODA_SIZE_T size);
    void operator delete(void *ptr);
		    
    int Feed(void *pSampleData, int sampleDataSize, int numSamples);	
	int Decode(short *pDstBuf[], int numSamples);
	XA16STATE GetState();
	void SetState(XA16STATE *pxav);
	
private:
	EAXAVARS16 xav;
	MXAPACKET16 xap;  
};


// EAXA_BLK decoders to float

class CEAXABLKDecf 
{
public:	
	CEAXABLKDecf();
	~CEAXABLKDecf() {}		
    void *operator new(CODA_SIZE_T size);
    void operator delete(void *ptr);
	
    int Feed(void *pSampleData, int sampleDataSize, int numSamples);	
	int Decode(float *pDstBuf[], int numSamples);
	XAFSTATE GetState();
	void SetState(XAFSTATE *pxav);
	
private:
	void (*decodexa)(MXAPACKETF *pxap);
	EAXAVARSF xav;
	MXAPACKETF xap;
};

//-----------------------------------------------------------------------

// GCADPCM decoder

#define CODA_GCADPCM_BYTES_PER_BLOCK     8
#define CODA_GCADPCM_SAMPLES_PER_BLOCK   14
#define CODA_GCADPCM_TABLE_MAX_SIZE      16			//16 shorts, or 33 bytes to be exact


class CGCADPCMDec 
{
public:	
	CGCADPCMDec();
	~CGCADPCMDec() {}		
    void *operator new(CODA_SIZE_T size);
    void operator delete(void *ptr);
	
    int Feed(void *pSampleData, int sampleDataSize, int numSamples);	
	int Decode(short *pDstBuf[], int samples);
	void SetState(unsigned char *pstate, int ignoresamples);
	void SetTable(short *ptable);
	
private:
	void decodega(int samples);

	unsigned char *mpSrc;
	short *mpDst;	
	int mRemainingSamples;
	int mSampleDataSize;
	int mSamplesDecoded;
	int mMustSetState;  //flag
	short mCoeftable[CODA_GCADPCM_TABLE_MAX_SIZE];

	//previous two samples decoded
    short mSample0;
    short mSample1;
	
	//Residue related
	int mResidual;
	short *mpResidue;	
	short mGaBlock[16];

	//to be set by SetState()
	int mIgnoreSamples;
	short mLoopSample0;
    short mLoopSample1;
	unsigned char mLoopAdpcm;
};

class CGCADPCMDecf
{
public:	
	CGCADPCMDecf();
	~CGCADPCMDecf() {}		
    void *operator new(CODA_SIZE_T size);
    void operator delete(void *ptr);
	
    int Feed(void *pSampleData, int sampleDataSize, int numSamples);	
	int Decode(float *pDstBuf[], int samples);
	void SetState(unsigned char *pstate, int ignoresamples);
	void SetTable(short *ptable);
	
private:
	void decodega(int samples);

	unsigned char *mpSrc;
	float *mpDst;	
	int mRemainingSamples;
	int mSampleDataSize;
	int mSamplesDecoded;
	int mMustSetState;  //flag
	short mCoeftable[CODA_GCADPCM_TABLE_MAX_SIZE];

	//previous two samples decoded
    float mSample0;
    float mSample1;
	
	//Residue related
	int mResidual;
	float *mpResidue;
	float mGaBlock[16];	

	//to be set by SetState()
	int mIgnoreSamples;
	float mLoopSample0; 
    float mLoopSample1;
	unsigned char mLoopAdpcm;
};


#if 1
//-----------------------------------------------------------------------

// SIGN8 decoders

class CSign8DecS16 
{
public:	
	CSign8DecS16();
	~CSign8DecS16() {}		
    void *operator new(CODA_SIZE_T size);
    void operator delete(void *ptr);
	
    int Feed(void *pSampleData, int sampleDataSize, int numSamples);	
	int Decode(short *pDstBuf[], int samples);
	
private:
	unsigned char *mpSrc;
	short *mpDst;	
	int mRemainingSamples;
	int mSampleDataSize;	
};

class CSign8DecSf
{
public:	
	CSign8DecSf();
	~CSign8DecSf() {}		
    void *operator new(CODA_SIZE_T size);
    void operator delete(void *ptr);
	
    int Feed(void *pSampleData, int sampleDataSize, int numSamples);	
	int Decode(float *pDstBuf[], int samples);
	
private:
	unsigned char *mpSrc;
	float *mpDst;	
	int mRemainingSamples;
	int mSampleDataSize;
};


//-----------------------------------------------------------------------

// SIGN16 decoders

class CSign16DecS16 
{
public:	
	CSign16DecS16();
	~CSign16DecS16() {}		
    void *operator new(CODA_SIZE_T size);
    void operator delete(void *ptr);
	
    int Feed(void *pSampleData, int sampleDataSize, int numSamples);	
	int Decode(short *pDstBuf[], int samples);
	
private:
	unsigned char *mpSrc;
	short *mpDst;	
	int mRemainingSamples;
	int mSampleDataSize;	
};

class CSign16DecSf
{
public:	
	CSign16DecSf();
	~CSign16DecSf() {}		
    void *operator new(CODA_SIZE_T size);
    void operator delete(void *ptr);
	
    int Feed(void *pSampleData, int sampleDataSize, int numSamples);	
	int Decode(float *pDstBuf[], int samples);
	
private:	
	void (*decodef)(int frames, short *psrc, float *pdst);
	unsigned char *mpSrc;
	float *mpDst;	
	int mRemainingSamples;
	int mSampleDataSize;
};

// Interleaved
class CSign16IntDecS16 
{
public:	
	CSign16IntDecS16();
	~CSign16IntDecS16() {}		
    void *operator new(CODA_SIZE_T size);
    void operator delete(void *ptr);
	
    int Feed(void *pSampleData, int sampleDataSize, int numSamples);	
	int Decode(short *pDstBuf[], int samples);
	int GetState();
	void SetState(int *pchannel);
	
private:
	unsigned char *mpSrc;
	short *mpDst;	
	int mRemainingSamples;
	int mSampleDataSize;
	int mChannels;
};
//-----------------------------------------------------------------------

// SIGN16 BIG-Endian decoders

class CSign16BigDecS16
{
public:	
	CSign16BigDecS16();
	~CSign16BigDecS16() {}		
    void *operator new(CODA_SIZE_T size);
    void operator delete(void *ptr);
	
    int Feed(void *pSampleData, int sampleDataSize, int numSamples);	
	int Decode(short *pDstBuf[], int samples);
	
private:
	unsigned char *mpSrc;
	short *mpDst;	
	int mRemainingSamples;
	int mSampleDataSize;	
};

class CSign16BigDecSf
{
public:	
	CSign16BigDecSf();
	~CSign16BigDecSf() {}		
    void *operator new(CODA_SIZE_T size);
    void operator delete(void *ptr);
	
    int Feed(void *pSampleData, int sampleDataSize, int numSamples);	
	int Decode(float *pDstBuf[], int samples);
	
private:
	void (*decodef)(int frames, short *psrc, float *pdst);
	unsigned char *mpSrc;
	float *mpDst;	
	int mRemainingSamples;
	int mSampleDataSize;
};

// Interleaved
class CSign16BigIntDecS16
{
public:	
	CSign16BigIntDecS16();
	~CSign16BigIntDecS16() {}		
    void *operator new(CODA_SIZE_T size);
    void operator delete(void *ptr);
	
    int Feed(void *pSampleData, int sampleDataSize, int numSamples);	
	int Decode(short *pDstBuf[], int samples);
	int GetState();
	void SetState(int *pchannel);
	
private:
	unsigned char *mpSrc;
	short *mpDst;	
	int mRemainingSamples;
	int mSampleDataSize;
	int mChannels;
};

//-----------------------------------------------------------------------

// SIGN24 decoders

class CSign24DecS16 
{
public:	
	CSign24DecS16();
	~CSign24DecS16() {}		
    void *operator new(CODA_SIZE_T size);
    void operator delete(void *ptr);
	
    int Feed(void *pSampleData, int sampleDataSize, int numSamples);	
	int Decode(short *pDstBuf[], int samples);
	
private:
	unsigned char *mpSrc;
	short *mpDst;	
	int mRemainingSamples;
	int mSampleDataSize;	
};

class CSign24IntDecS16 
{
public:	
	CSign24IntDecS16();
	~CSign24IntDecS16() {}		
    void *operator new(CODA_SIZE_T size);
    void operator delete(void *ptr);
	
    int Feed(void *pSampleData, int sampleDataSize, int numSamples);	
	int Decode(short *pDstBuf[], int samples);
	int GetState();
	void SetState(int *pchannel);
	
private:
	unsigned char *mpSrc;
	short *mpDst;	
	int mRemainingSamples;
	int mSampleDataSize;
	int mChannels;
};
//-----------------------------------------------------------------------

// VAG decoders

//user-tangible VAG structures
typedef struct VAGFSTATE
{
    long sample1;
    long sample2;
} VAGFSTATE;


#define CODA_VAG_SAMPLES_PER_BLOCK	28
#define CODA_VAG_BYTES_PER_BLOCK	16


/***** Filter Mode 0 *****/

#define	CODAI_F0MUL1	0x00000000L				/* sample-1 x 0               */
#define	CODAI_F0MUL2	0x00000000L				/* sample-2 x 0               */

/***** Filter Mode 1 *****/

#define	CODAI_F1MUL1	0x0000F000L				/* sample-1 x 0.9375          */
#define	CODAI_F1MUL2	0x00000000L          	/* sample-2 x 0               */

/***** Filter Mode 2 *****/

#define	CODAI_F2MUL1	0x0001CC00L				/* sample-1 x 1.796875        */
#define	CODAI_F2MUL2	0xFFFF3000L          	/* sample-2 x -0.8125         */

/***** Filter Mode 3 *****/

#define	CODAI_F3MUL1	0x00018800L				/* sample-1 x 1.53125         */
#define	CODAI_F3MUL2	0xFFFF2400L          	/* sample-2 x -0.859375       */

/***** Filter Mode 4 *****/

#define	CODAI_F4MUL1	0x0001E800L				/* sample-1 x 1.90625         */
#define	CODAI_F4MUL2	0xFFFF1000L          	/* sample-2 x -0.9375         */


class CVAGBLKDecf
{
public:	
	CVAGBLKDecf();
	~CVAGBLKDecf() {}		
    void *operator new(CODA_SIZE_T size);
    void operator delete(void *ptr);
	
    int Feed(void *pSampleData, int sampleDataSize, int numSamples);	
	int Decode(float *pDstBuf[], int samples);
	VAGFSTATE GetState();
	void SetState(VAGFSTATE *pstate, int ignoresamples);
	
private:
	void decodevag(int samples);

	unsigned char *mpSrc;
	float *mpDst;	
	int mRemainingSamples;
	int mSampleDataSize;
	int mSamplesDecoded;

	//previous two samples decoded
    long mSample1;
    long mSample2;
	
	//Residue related
	int mResidual;
	float *mpResidue;
	float mVagBlock[30];	//first two slots are dummies used to decode

	//to be set by SetState()
	int mIgnoreSamples;
	int mFilter;	//a backup
	int mShift;		//a backup
};

//-----------------------------------------------------------------------

// MicroTalk decoders

//user accessible struct (with Set/GetState)
typedef struct MTFSTATE
{
    unsigned int shiftreg;
	unsigned int bufframes;
	int doWholeInit;
} MTFSTATE;

#if 0
#define UT_ORDER        12
#define UT_SUBWIN       108             // must be a multiple of 2
#define UT_WINDOW       (4 * UT_SUBWIN) // must be a multiple of UT_ORDER


typedef struct UTALKSTATE
{
    unsigned char *dataptr;
    unsigned int shiftreg;
    int bitcount;

    int voicing_threshold;
    float stepval[64];

    float coeff[UT_ORDER];
    float output[UT_ORDER];

    float history[3 * UT_SUBWIN];
    float data[UT_WINDOW];        // must follow history
} UTALKSTATE;

class CMTBLKDecf
{
public:	
	CMTBLKDecf();
	~CMTBLKDecf() {}		
    void *operator new(CODA_SIZE_T size);
    void operator delete(void *ptr);
	
    int Feed(void *pSampleData, int sampleDataSize, int numSamples);	
	int Decode(float *pDstBuf[], int samples);
	MTFSTATE GetState();
	void SetState(MTFSTATE *pstate, int ignoresamples);
	
private:
	UTALKSTATE mutstate;
	unsigned int mbufframes;
	float *mpDst;	
	int mRemainingSamples;
	int mSampleDataSize;
	int mdoWholeInit;		//flag
};
#endif


//-----------------------------------------------------------------------
#endif //closing #if 0


//=======================================================================
//
// MPEG audio CODA classes:
//
//=======================================================================

// decoder + assumed encoder latency for each MPEG audio layer

const int MPEG_AUDIO_LAYER_LATENCY[] = { 0, 481, 481, 1105 };


//-----------------------------------------------------------------------
// This stuff was moved here from LbMpeg.h so we wouldn't have to
// include LbMpeg.h in coda.h
//-----------------------------------------------------------------------

#if !defined(XBOX) && !defined(PS2)
#define MPEG_FLOAT_CODE
#endif

#if defined(WIN) || defined(XBOX) || defined(PS2)
#define MPEG_INTEGER_CODE
#endif

#define USE_SKIP_BUFFER
//#define EQUALIZER_SUPPORT


// layer 2 structures

struct al_table 
{
    short bits;
    short d;
};


// layer 3 specific structures

#define HUFFBITS unsigned int

struct huffcodetab 
{
    char tablename[3];              // string, containing table_description   
    unsigned int xlen;              // max. x-index+                          
    unsigned int ylen;              // max. y-index+                          
    unsigned int linbits;           // number of linbits                      
    unsigned int linmax;            // max number to be stored in linbits     
    int ref;                        // a positive value indicates a reference 
    HUFFBITS *table;                // pointer to array[xlen][ylen]           
    unsigned char *hlen;            // pointer to array[xlen][ylen]           
    const unsigned char(*val)[2];   // decoder tree                           
    unsigned int treelen;           // length of decoder tree                 
};

typedef struct 
{
    unsigned int part2_3_length;
    unsigned int big_values;
    unsigned int global_gain;
    unsigned int scalefac_compress;
    unsigned int window_switching_flag;
    unsigned int block_type;
    unsigned int mixed_block_flag;
    unsigned int table_select[3];
    unsigned int subblock_gain[3];
    unsigned int region0_count;
    unsigned int region1_count;
    unsigned int preflag;
    unsigned int scalefac_scale;
    unsigned int count1table_select;
} gr_info_s;

typedef struct 
{
    unsigned int main_data_begin;
    unsigned int private_bits;

    struct 
    {
        unsigned int scfsi[4];
        gr_info_s gr[2];
    } ch[2];
} III_side_info_t;

typedef struct 
{
    int l[23];            /* [cb] */
    int s[3][13];         /* [window][cb] */
} III_scalefac_t[2];


#define BITRESERVESIZE 2048  // must be a power of two

class Bit_Reserve 
{
public:
    Bit_Reserve() { reset(); }
    ~Bit_Reserve() {}

    void reset();
    unsigned int hsstell() const;
    unsigned int hgetbits(unsigned int N);
    unsigned int hget1bit();
    void hputbuf(int val);
    void rewindNbits(int N);
    void rewindNbytes(int N);

private:
    unsigned int  mInPtr,mOutPtr,mCached,mCacheData;
    unsigned char mBuffer[BITRESERVESIZE];
};

#define         SSLIMIT     18
#define         SBLIMIT     32


// mpeg decoder classes

class CMpegLayer12
{
protected:
    union
    {
#if defined(MPEG_FLOAT_CODE)
        float buffs[2][2][0x120];
#endif
#if defined(MPEG_INTEGER_CODE)
        short buffsMMX[2][2][0x120];
#endif
    };

public:
    short   pad1[32];
    short   mFrameBuf[2304];
    short   pad2[32];

    CMpegLayer12();        
    virtual         ~CMpegLayer12();

#ifdef EQUALIZER_SUPPORT
    float           Equalizer_Bands[32];
#endif
    int             ProcessHeader(unsigned int hdr);
    int             Open(void *buf, int filesize);      // pointer to compressed data and comp. length
    int             Open(unsigned int header, int filesize);// pointer to compressed data 
    int             Close( void );                      // call when finished
    int             Read(void *, int);                  // call to actually unpack the audio data, passing an output
                                                        // buffer (16 bit signed audio) and number of bytes to unpack
#ifdef USE_SKIP_BUFFER
    int             GetSkipSize( void ){ return (1152*2) * ((mChannels == 2) ? 2 : 1); }
#endif //USE_SKIP_BUFFER

    void            Reset( void );
    virtual int     Decode(short *);
    void            Seek(void *buf);

    int             Real_mSampFreq;                     // compressed data sampling freq (real frequency)
    int             mSampFreq;                          // sampling freq (playback - half 'real' if half rate)
    int             mFrameSize;                         // (real) size of a sample frame in bytes
    unsigned char   mLayer;                             
    int             mBitRate;                           // bitrate of compressed stream
    int             cFrameSize;                         // compressed size of a sample frame in bytes
    int             dec_FrameSize;                      // size of a sample frame (half real size if half rate)
    int             mNFrames;                           
    int             mFileLen;                           // length of UNCOMPRESSED data. DO NOT READ (UNPACK) MORE THAN THIS AMOUNT!
    int             mFilePos;                           // current uncompressed position

    static void     Init_Tables(void);
    static void     Close_Tables(void);

    int             DecodeHeader();

protected:
    int             Decode_Layer1(short *);
    int             Decode_Layer2(short *);

    // Polyphase specific

    void            OpenSynth();
    void            CloseSynth();
    void            PolySynth(short *, float*);
    void            PolySynthMMX(short *samples, short *bandPtr);

    // layer 2 specific
    int             mSBlimit;
    int             mJointSBlimit;
    void            II_step_two(char *bit_alloc,float fraction[3][2][32],char *scale,int x1,const struct al_table *alloc1 );
    void            II_step_twoMMX(const int *,char *bit_alloc,short frac[3][2][32],char *scale,const struct al_table *alloc1 );

    unsigned char * mBufPtr;
    unsigned char * mBufPtrNext;
    unsigned int    mShiftReg;
    int             mShiftRegBits;

    unsigned char   mBandOffset;
    unsigned char   mMPEG25;
    unsigned char   mLSF;
    unsigned char   sfreq;
    unsigned char   max_gr;
    unsigned char   mVersion;
    unsigned char   mErrorProt;
    unsigned char   mBitRateIdx;    
    unsigned char   mSampFreqIdx;   
    unsigned char   mPadding;
    unsigned char   mMode;  
    unsigned char   mModeExt;   
    unsigned char   mCopyright;
    unsigned char   mOriginal; 
    int             stereo_step;
    int             mSlotSize;
    int             mNSlots;
    int             mFSlots;
    int             mSlotDiv;
    int             mCurFrame;
    int             mFramePos;

public:
    unsigned char   mOpened;
    unsigned int    mHeader;
    int             mChannels;

protected:
    int             GetBits(int n)
                    {
                        while (mShiftRegBits < n)
                        {
                            mShiftReg |= *mBufPtr++ << (24-mShiftRegBits);
                            mShiftRegBits += 8;
                        }

                        const int bits = (int)(mShiftReg >> (32-n));
                        mShiftReg <<= n;
                        mShiftRegBits -= n;

                        return(bits);
                    }

    int             GetHeader();
    int             GetHeader(unsigned int header);
    virtual int     OpenLayer();
    int             CloseLayer();
    void            get_sb_stereo(int i,char balloc[SBLIMIT][2],unsigned int scale[2][SBLIMIT],short *fr,const int *muls);
    void            get_sb_mono(int i,char balloc[SBLIMIT],unsigned int scale[SBLIMIT],short *fr,const int *muls);
};

class CMpegLayer123 : public CMpegLayer12
{
public:
    virtual int     Decode(short *);
    virtual int     OpenLayer();

protected:
    Bit_Reserve     br;
    III_side_info_t si;
    III_scalefac_t  scalefac;

    int     Decode_Layer3(short *outsamp);
    float   prevblck[2][SBLIMIT*SSLIMIT];   // for layer 3
    short   nonzero[2];
    int     frame_start;
    bool    get_side_info();
    void    get_scale_factors(unsigned int ch, unsigned int gr);
    void    get_LSF_scale_data(unsigned int ch, unsigned int gr);
    void    get_LSF_scale_factors(unsigned int ch, unsigned int gr);
    void    huffman_decode(unsigned int ch, unsigned int gr,int is_1d[SBLIMIT*SSLIMIT],int part2_start);
    int     huffman_decoder(const struct huffcodetab *h, int *x, int *y, int *v,int *w);
    void    i_stereo_k_values(unsigned int is_pos, unsigned int io_type, unsigned int i,float  k[2][SBLIMIT*SSLIMIT] );
    void    dequantize_sample(float xr[SBLIMIT][SSLIMIT], unsigned int ch, unsigned int gr,int is_1d[SBLIMIT*SSLIMIT]);
    void    reorder(float xr[SBLIMIT][SSLIMIT], unsigned int ch, unsigned int gr,float out_1d[SBLIMIT*SSLIMIT]);
    void    stereo2(unsigned int gr,float lr[2][SBLIMIT][SSLIMIT],float ro[2][SBLIMIT][SSLIMIT],float  k[2][SBLIMIT*SSLIMIT]);
    void    antialias(unsigned int ch, unsigned int gr,float out_1d[SBLIMIT*SSLIMIT]);
    void    hybrid(unsigned int ch, unsigned int gr,float out_1d[SBLIMIT*SSLIMIT]);
};


//-----------------------------------------------------------------------
// Finally, the CODA classes!
//-----------------------------------------------------------------------

// MPEG Layer 1 audio decode to signed 16 bit

class CMpegL1DecS16
{
public:
    CMpegL1DecS16() { Reset(); }
    ~CMpegL1DecS16() {}

    void *operator new(CODA_SIZE_T size);
    void operator delete(void *ptr);

    int  Feed(void *pSampleData, int sampleDataSize, int numSamples = -1);
    int  Decode(short *pDstBuf[], int numSamples);
    void Reset();

private:
    CMpegLayer12    mDecoder;
    unsigned char   *mInputData;
    int             mInputSize;
    int             mMaxSamples;
    int             mDecodedCount;
    int             mDecodedStart;
    int             mLatency;
    bool            mReset;
};


// MPEG Layer 1 audio decode to float

class CMpegL1DecF
{
public:
    CMpegL1DecF() { Reset(); }
    ~CMpegL1DecF() {}

    void *operator new(CODA_SIZE_T size);
    void operator delete(void *ptr);

    int  Feed(void *pSampleData, int sampleDataSize, int numSamples = -1);
    int  Decode(float *pDstBuf[], int numSamples);
    void Reset();

private:
    CMpegLayer12    mDecoder;
    unsigned char   *mInputData;
    int             mInputSize;
    int             mMaxSamples;
    int             mDecodedCount;
    int             mDecodedStart;
    int             mLatency;
    bool            mReset;
};


// MPEG Layer 2 audio decode to signed 16 bit

class CMpegL2DecS16
{
public:
    CMpegL2DecS16() { Reset(); }
    ~CMpegL2DecS16() {}

    void *operator new(CODA_SIZE_T size);
    void operator delete(void *ptr);

    int  Feed(void *pSampleData, int sampleDataSize, int numSamples = -1);
    int  Decode(short *pDstBuf[], int numSamples);
    void Reset();

private:
    CMpegLayer12    mDecoder;
    unsigned char   *mInputData;
    int             mInputSize;
    int             mMaxSamples;
    int             mDecodedCount;
    int             mDecodedStart;
    int             mLatency;
    bool            mReset;
};


// MPEG Layer 2 audio decode to float

class CMpegL2DecF
{
public:
    CMpegL2DecF() { Reset(); }
    ~CMpegL2DecF() {}

    void *operator new(CODA_SIZE_T size);
    void operator delete(void *ptr);

    int  Feed(void *pSampleData, int sampleDataSize, int numSamples = -1);
    int  Decode(float *pDstBuf[], int numSamples);
    void Reset();

private:
    CMpegLayer12    mDecoder;
    unsigned char   *mInputData;
    int             mInputSize;
    int             mMaxSamples;
    int             mDecodedCount;
    int             mDecodedStart;
    int             mLatency;
    bool            mReset;
};


// MPEG Layer 3 audio decode to signed 16 bit

class CMpegL3DecS16
{
public:
    CMpegL3DecS16() { Reset(); }
    ~CMpegL3DecS16() {}

    void *operator new(CODA_SIZE_T size);
    void operator delete(void *ptr);

    int  Feed(void *pSampleData, int sampleDataSize, int numSamples = -1);
    int  Decode(short *pDstBuf[], int numSamples);
    void Reset();

private:
    CMpegLayer123   mDecoder;
    unsigned char   *mInputData;
    int             mInputSize;
    int             mMaxSamples;
    int             mDecodedCount;
    int             mDecodedStart;
    int             mLatency;
    bool            mReset;
};


// MPEG Layer 3 audio decode to float

class CMpegL3DecF
{
public:
    CMpegL3DecF() { Reset(); }
    ~CMpegL3DecF() {}

    void *operator new(CODA_SIZE_T size);
    void operator delete(void *ptr);

    int  Feed(void *pSampleData, int sampleDataSize, int numSamples = -1);
    int  Decode(float *pDstBuf[], int numSamples);
    void Reset();

private:
    CMpegLayer123   mDecoder;
    unsigned char   *mInputData;
    int             mInputSize;
    int             mMaxSamples;
    int             mDecodedCount;
    int             mDecodedStart;
    int             mLatency;
    bool            mReset;
};


// MPEG Layer 1,2,3 audio decode to signed 16 bit

class CMpegL123DecS16
{
public:
    CMpegL123DecS16() { Reset(); }
    ~CMpegL123DecS16() {}

    void *operator new(CODA_SIZE_T size);
    void operator delete(void *ptr);

    int  Feed(void *pSampleData, int sampleDataSize, int numSamples = -1);
    int  Decode(short *pDstBuf[], int numSamples);
    void Reset();

private:
    CMpegLayer123   mDecoder;
    unsigned char   *mInputData;
    int             mInputSize;
    int             mMaxSamples;
    int             mDecodedCount;
    int             mDecodedStart;
    int             mLatency;
    bool            mReset;
};


// MPEG Layer 1,2,3 audio decode to float

class CMpegL123DecF
{
public:
    CMpegL123DecF() { Reset(); }
    ~CMpegL123DecF() {}

    void *operator new(CODA_SIZE_T size);
    void operator delete(void *ptr);

    int  Feed(void *pSampleData, int sampleDataSize, int numSamples = -1);
    int  Decode(float *pDstBuf[], int numSamples);
    void Reset();

private:
    CMpegLayer123   mDecoder;
    unsigned char   *mInputData;
    int             mInputSize;
    int             mMaxSamples;
    int             mDecodedCount;
    int             mDecodedStart;
    int             mLatency;
    bool            mReset;
};

//-----------------------------------------------------------------------


#if defined(SIMEX_LIB) || defined(SND_LIB)
}
#endif


#endif

