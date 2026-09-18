#ifndef __SOUND_FILE_H__
#define __SOUND_FILE_H__

#include "TbFile.h"

class FSoundData;

class SoundFile : public TbFile
{
  public:
	SoundFile();
	
	int				Open(const FSoundData * pData, unsigned int mode);
	int				Open(const char *name, unsigned int mode);
	void			Close();

	int				Read(void *buffer, int length, bool blocking = true);
	int				Seek(int offset, int mode);

	int				GetSize();
	int				GetPos();
	
	// variables
	const FSoundData * mpData;
	int mOffset; // used for streaming
};

#endif