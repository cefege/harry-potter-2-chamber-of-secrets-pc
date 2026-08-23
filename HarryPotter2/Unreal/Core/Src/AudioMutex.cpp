#include "windows.h"
#include "AudioMutex.h"
#include "core.h"

AudioMutex::AudioMutex()
{
	InitializeCriticalSection(&mCriticalSection);
}

AudioMutex::~AudioMutex ()
{
	DeleteCriticalSection(&mCriticalSection);
}

void AudioMutex::Lock ()
{
	EnterCriticalSection(&mCriticalSection);
}

void AudioMutex::Unlock ()
{
	LeaveCriticalSection(&mCriticalSection);
}


AudioMutexGrabber::AudioMutexGrabber (AudioMutex * pMutex): mpMutex (pMutex) 
{
	mpMutex->Lock ();
}

AudioMutexGrabber::~AudioMutexGrabber ()
{
	mpMutex->Unlock ();
}



AudioMutex audioFileMutex;
