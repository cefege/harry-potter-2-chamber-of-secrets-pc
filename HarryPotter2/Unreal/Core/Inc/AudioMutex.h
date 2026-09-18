#ifndef __AUDIO_MUTEX_H__
#define __AUDIO_MUTEX_H__

// Trying to include the windows headers caused lots of errors
// as they clash with the unreal.
// Need to include the body of a critical section here, so have cut 'n pasted
// all the bits needed if windows headers are not included.

#ifndef _WINDOWS_

typedef unsigned short      WORD;
typedef unsigned long       DWORD;
typedef long LONG;

//
//  Doubly linked list structure.  Can be used as either a list head, or
//  as link words.
//

typedef struct _LIST_ENTRY {
   struct _LIST_ENTRY *Flink;
   struct _LIST_ENTRY *Blink;
} LIST_ENTRY, *PLIST_ENTRY;

typedef struct _RTL_CRITICAL_SECTION_DEBUG {
    WORD   Type;
    WORD   CreatorBackTraceIndex;
    struct _RTL_CRITICAL_SECTION *CriticalSection;
    LIST_ENTRY ProcessLocksList;
    DWORD EntryCount;
    DWORD ContentionCount;
    DWORD Spare[ 2 ];
} RTL_CRITICAL_SECTION_DEBUG, *PRTL_CRITICAL_SECTION_DEBUG, RTL_RESOURCE_DEBUG, *PRTL_RESOURCE_DEBUG;

typedef struct _RTL_CRITICAL_SECTION {
    PRTL_CRITICAL_SECTION_DEBUG DebugInfo;

    //
    //  The following three fields control entering and exiting the critical
    //  section for the resource
    //

    LONG LockCount;
    LONG RecursionCount;
    HANDLE OwningThread;        // from the thread's ClientId->UniqueThread
    HANDLE LockSemaphore;
    DWORD SpinCount;
} RTL_CRITICAL_SECTION, *PRTL_CRITICAL_SECTION;

typedef RTL_CRITICAL_SECTION CRITICAL_SECTION;

#endif

// End of windows stuff

//---------------------------
// AudioMutex
//---------------------------

// Wrapper class for windows synchronisation

class CORE_API AudioMutex
{
	CRITICAL_SECTION mCriticalSection;

public:
	AudioMutex ();
	~AudioMutex ();

	void Lock ();
	void Unlock ();
};


class CORE_API AudioMutexGrabber
{
	AudioMutex * mpMutex;

public:
	AudioMutexGrabber (AudioMutex * pMutex);
	~AudioMutexGrabber ();
};



extern CORE_API AudioMutex audioFileMutex; // 

#endif