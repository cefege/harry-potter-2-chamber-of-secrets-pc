#ifndef _TBFILE_H
#define _TBFILE_H

//****************************************************************************************************
#define TBFILE_READONLY			0x00000000
#define TBFILE_STREAMABLE		0x00000001
#define TBFILE_NONBLOCKING		0x00000002
#define TBFILE_MULTIMEDIA		0x00000004

//****************************************************************************************************
typedef int				(*OPENCALLBACK)(const char *name, unsigned int mode);
typedef void			(*CLOSECALLBACK)(int handle);
typedef int				(*READCALLBACK)(int handle, void *buffer, int length, bool blocking);
typedef int				(*SEEKCALLBACK)(int handle, int offset, int mode);
typedef int				(*GETSIZECALLBACK)(int handle);
typedef int				(*GETPOSCALLBACK)(int handle);
typedef char const *	(*MAPFILECALLBACK)(int handle);
typedef bool			(*FLUSHCALLBACK)(int handle);

//****************************************************************************************************
class TbFile
{
public:
	TbFile();
	~TbFile();
	
	virtual int				Open(const char *name, unsigned int mode);
	virtual void			Close();

	virtual int				Read(void *buffer, int length, bool blocking = true);
	virtual int				Seek(int offset, int mode);

	virtual int				GetSize();
	virtual int				GetPos();
	
	virtual char const *	MapFile();
	virtual bool			Flush();

	// variables
	int						mHandle;
	int						mRefCount;

private:
	char	*				mpMappedFile;
};

//****************************************************************************************************
#endif
