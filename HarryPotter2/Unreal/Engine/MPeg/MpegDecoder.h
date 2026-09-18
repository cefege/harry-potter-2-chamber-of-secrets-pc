#if!defined(C_MPEG_DECODER_H_INCLUDED)
#define C_MPEG_DECODER_H_INCLUDED

class CMpegBase;
class TbFile;

class CMpegDecoder
{
public:
	CMpegDecoder();
	~CMpegDecoder();

	// Static initialisation: pass a pointer to the static data and size of
	// the compressed data.
	bool	Init( void * pData, int size );

	// Streaming initialisation: Pass a pointer to the open TbFile file and the offset
	// within this file and the size of the compressed data.
	bool	Init( TbFile * pFile, int offset, int size );

	// Returns the number of bytes in the decoded frame.
	int		GetSkipSize( void );

	// Call this to execute a decode, pass in a buffer where the data will get 
	// decoded to, and the amount you requier.
	int		Decode( void * pBuffer, int amount );

	// Reset any internal buffer pointers for looping.
	void	Reset( void );

	// Call this when everything is done (gets called in the deconstructor).
	bool	Close( void );

private:
	CMpegBase * mpBase;
};

#endif //C_MPEG_DECODER_H_INCLUDED