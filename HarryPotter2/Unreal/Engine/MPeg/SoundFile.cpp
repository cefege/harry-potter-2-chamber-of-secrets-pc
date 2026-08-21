#include "SoundFile.h"
#include "..\src\EnginePrivate.h" 

#include "core.h"
#include <stdio.h>

//*************************************************************************************************************

SoundFile::SoundFile() :
mpData(NULL)
{
}

//*************************************************************************************************************

int SoundFile::Open(const FSoundData *pData, unsigned int mode)
{
	if (mpData)
		return (int)mpData;

	if ( mpData = pData )
	{
		mOffset = 0;
		return (int)mpData;
	}
	
	return 0;
}

int	SoundFile::Open(const char *name, unsigned int mode)
{
	check (false);

	return 0;
}

//*************************************************************************************************************

void SoundFile::Close()
{
	mpData = NULL;
}

//*************************************************************************************************************

int SoundFile::Read(void *buffer, int length, bool blocking)
{
	if ( mpData)
	{
		int num_read = mpData->Buffer(buffer, length, mOffset);
		mOffset += num_read;
		return num_read;
	}
	
	return 0;
}

//*************************************************************************************************************

// on success, return 0, failure, return non 0 (according to fseek)
int SoundFile::Seek(int offset, int mode)
{
	if (mpData)
	{
		switch (mode)
		{
		case SEEK_CUR:
			mOffset += offset;
			break;

		case SEEK_END:
			mOffset = mpData->Length() + offset;
			break;

		case SEEK_SET:
			mOffset = offset;
			break;
		}

		int ret = 0;

		// Need to indicate if offset is out of range ?
		if (mOffset <0 || mOffset >= mpData->Length())
			ret = -1;

		return ret; 
	}

	return -1;
}

//*************************************************************************************************************

int SoundFile::GetSize()
{
	if ( mpData)
		return mpData->Length();

	return 0;
}

//*************************************************************************************************************

int SoundFile::GetPos(void)
{
	if ( mpData)
		return mOffset;
	return 0;
}