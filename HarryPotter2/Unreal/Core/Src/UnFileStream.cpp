/*=============================================================================
	UnFileStream.cpp: Portable asynchronous audio file streaming.
=============================================================================*/

#include <condition_variable>
#include <cstdio>
#include <cstring>
#include <memory>
#include <mutex>
#include <thread>
#include <vector>

#include "CorePrivate.h"

#if SUPPORTS_PRAGMA_PACK
#pragma pack(push, 8)
#endif
#include "vorbis/vorbisfile.h"
#if SUPPORTS_PRAGMA_PACK
#pragma pack(pop)
#endif

namespace
{
struct FFileStreamRuntime
{
	std::mutex LifecycleMutex;
	std::mutex WorkMutex;
	std::condition_variable WorkReady;
	std::vector<std::unique_ptr<std::mutex> > StreamMutexes;
	std::thread Worker;
	INT PendingRequests = 0;
	UBOOL StopRequested = 0;
	UBOOL Active = 0;
};
// A sound's TLazyArray belongs to its UObject and may unload after registration.
// Keep both the encoded bytes and decoder in stream-owned storage used by the worker.
struct FXAStreamState
{
	std::vector<BYTE> EncodedData;
	FEAXABlockDecoder Decoder;
};

FFileStreamRuntime GRuntime;

UBOOL IsValidStreamId(INT StreamId)
{
	return GRuntime.Active && FFileStream::Streams && StreamId >= 0 && StreamId < FFileStream::MaxStreams;
}

std::mutex& StreamMutex(INT StreamId)
{
	return *GRuntime.StreamMutexes[static_cast<size_t>(StreamId)];
}


void CancelPendingDestinationsLocked(FStream& Stream)
{
	const INT Cancelled = static_cast<INT>(Stream.PendingCount);
	if (Cancelled <= 0)
		return;

	std::lock_guard<std::mutex> WorkLock(GRuntime.WorkMutex);
	Stream.PendingHead = 0;
	Stream.PendingCount = 0;
	check(Stream.CompletionCount >= static_cast<size_t>(Cancelled));
	for (INT Index = 0; Index < Cancelled; ++Index)
	{
		const size_t Tail = (
			Stream.CompletionHead + Stream.CompletionCount - 1)
			% FILE_STREAM_REQUEST_CAPACITY;
		check(!Stream.CompletedChunks[Tail].Ready);
		--Stream.CompletionCount;
	}
	if (Stream.CompletionCount == 0)
		Stream.CompletionHead = 0;
	GRuntime.PendingRequests -= Cancelled;
	if (GRuntime.PendingRequests < 0)
		GRuntime.PendingRequests = 0;
}

void CancelRequestsLocked(FStream& Stream)
{
	CancelPendingDestinationsLocked(Stream);
	std::lock_guard<std::mutex> WorkLock(GRuntime.WorkMutex);
	Stream.CompletionHead = 0;
	Stream.CompletionCount = 0;
}

void ResetStream(FStream& Stream)
{
	{
		std::lock_guard<std::mutex> WorkLock(GRuntime.WorkMutex);
		Stream.PendingHead = 0;
		Stream.PendingCount = 0;
		Stream.CompletionHead = 0;
		Stream.CompletionCount = 0;
	}
	Stream.Handle = NULL;
	Stream.TDD = NULL;
	Stream.FileSeek = 0;
	Stream.ChunkSize = 0;
	Stream.Locked = 0;
	Stream.Used = 0;
	Stream.EndOfFile = 0;
	Stream.NumSamples = 0;
	Stream.Type = ST_Regular;
}

UBOOL DestroyStreamResourcesLocked(INT StreamId)
{
	FStream& Stream = FFileStream::Streams[StreamId];
	CancelRequestsLocked(Stream);

	switch (Stream.Type)
	{
	case ST_Regular:
		if (Stream.Handle)
			std::fclose(static_cast<FILE*>(Stream.Handle));
		break;

	case ST_Ogg:
	case ST_OggLooping:
		if (Stream.TDD)
		{
			ov_clear(static_cast<OggVorbis_File*>(Stream.TDD));
			delete static_cast<OggVorbis_File*>(Stream.TDD);
		}
		break;

	case ST_XA:
	case ST_XALooping:
		delete static_cast<FXAStreamState*>(Stream.TDD);
		break;

	default:
		if (Stream.Handle || Stream.TDD || Stream.Used)
			return 0;
		break;
	}

	ResetStream(Stream);
	return 1;
}

UBOOL ReadRegularLocked(FStream& Stream, INT Bytes, void* Destination)
{
	FILE* File = static_cast<FILE*>(Stream.Handle);
	const size_t Requested = static_cast<size_t>(Bytes);
	const size_t Count = std::fread(Destination, 1, Requested, File);
	if (Count != Requested)
		Stream.EndOfFile = 1;
	return std::ferror(File) == 0;
}

UBOOL ReadOggLocked(FStream& Stream, INT Bytes, void* Destination)
{
	OggVorbis_File* Ogg = static_cast<OggVorbis_File*>(Stream.TDD);
	char* Dest = static_cast<char*>(Destination);
	INT Count = 0;
	UBOOL ProducedSinceSeek = 0;

	while (Count < Bytes)
	{
		const long ReadCount = ov_read(
			Ogg,
			Dest + Count,
			Bytes - Count,
			0,
			2,
			1,
			&Stream.FileSeek);

		if (ReadCount > 0)
		{
			Count += static_cast<INT>(ReadCount);
			ProducedSinceSeek = 1;
			continue;
		}

		if (ReadCount < 0)
		{
			std::memset(Dest + Count, 0, static_cast<size_t>(Bytes - Count));
			Stream.EndOfFile = 1;
			return 0;
		}

		if (Stream.Type != ST_OggLooping)
		{
			std::memset(Dest + Count, 0, static_cast<size_t>(Bytes - Count));
			Stream.EndOfFile = 1;
			return 0;
		}

		// A corrupt or empty looping stream must not spin forever at EOF.
		if (!ProducedSinceSeek || ov_pcm_seek(Ogg, 0) != 0)
		{
			std::memset(Dest + Count, 0, static_cast<size_t>(Bytes - Count));
			Stream.EndOfFile = 1;
			return 0;
		}
		ProducedSinceSeek = 0;
	}

	return 1;
}

UBOOL RefeedXA(FStream& Stream, FXAStreamState& State)
{
	const BYTE* EncodedData = State.EncodedData.empty() ? NULL : State.EncodedData.data();
	return State.Decoder.Feed(EncodedData, static_cast<INT>(State.EncodedData.size()), Stream.NumSamples);
}

UBOOL ReadXALocked(FStream& Stream, INT Bytes, void* Destination)
{
	if ((Bytes & 1) != 0)
		return 0;

	FXAStreamState* State = static_cast<FXAStreamState*>(Stream.TDD);
	FEAXABlockDecoder* Decoder = &State->Decoder;
	SWORD* Dest = static_cast<SWORD*>(Destination);
	const INT Samples = Bytes / static_cast<INT>(sizeof(SWORD));
	INT Count = 0;
	UBOOL RefeedWithoutProgress = 0;

	while (Count < Samples)
	{
		const INT SamplesRequested = Samples - Count;
		const INT SamplesRead = Decoder->Decode(Dest + Count, SamplesRequested);
		if (SamplesRead < 0 || SamplesRead > SamplesRequested)
		{
			std::memset(Dest + Count, 0, static_cast<size_t>(SamplesRequested) * sizeof(SWORD));
			Stream.EndOfFile = 1;
			return 0;
		}

		if (SamplesRead > 0)
		{
			Count += SamplesRead;
			RefeedWithoutProgress = 0;
			continue;
		}

		// Decode returns sample counts, so EOF comparisons and padding are in
		// samples rather than mistakenly comparing a sample count with Bytes.
		if (Stream.Type != ST_XALooping)
		{
			std::memset(Dest + Count, 0, static_cast<size_t>(Samples - Count) * sizeof(SWORD));
			Stream.EndOfFile = 1;
			return 1;
		}

		if (RefeedWithoutProgress || !RefeedXA(Stream, *State))
		{
			std::memset(Dest + Count, 0, static_cast<size_t>(Samples - Count) * sizeof(SWORD));
			Stream.EndOfFile = 1;
			return 0;
		}
		RefeedWithoutProgress = 1;
	}

	return 1;
}

UBOOL ReadStreamLocked(INT StreamId, INT Bytes, void* Destination)
{
	if (Bytes < 0 || !Destination)
		return 0;
	if (Bytes == 0)
		return 1;

	FStream& Stream = FFileStream::Streams[StreamId];
	switch (Stream.Type)
	{
	case ST_Regular:
		return Stream.Handle && ReadRegularLocked(Stream, Bytes, Destination);
	case ST_Ogg:
	case ST_OggLooping:
		return Stream.Handle && Stream.TDD && ReadOggLocked(Stream, Bytes, Destination);
	case ST_XA:
	case ST_XALooping:
		return Stream.TDD && ReadXALocked(Stream, Bytes, Destination);
	default:
		return 0;
	}
}

UBOOL CreateFileLocked(INT StreamId, const TCHAR* Filename)
{
	FStream& Stream = FFileStream::Streams[StreamId];
	if (Stream.Type != ST_Regular && Stream.Type != ST_Ogg && Stream.Type != ST_OggLooping)
		return 0;

	FILE* File = appFopen(Filename, "rb");
	if (!File)
		return 0;

	if (Stream.Type == ST_Regular)
	{
		Stream.Handle = File;
		Stream.TDD = NULL;
		Stream.NumSamples = -1;
		return 1;
	}

	OggVorbis_File* Ogg = static_cast<OggVorbis_File*>(Stream.TDD);
	if (!Ogg)
	{
		std::fclose(File);
		return 0;
	}

	if (ov_open(File, Ogg, NULL, 0) < 0)
	{
		// ov_open deliberately detaches the datasource on header failure.
		std::fclose(File);
		return 0;
	}

	Stream.Handle = File;
	Stream.NumSamples = -1;
	return 1;
}

UBOOL CreateXALocked(INT StreamId, TLazyArray<BYTE>* RawData, INT NumSamples)
{
	FStream& Stream = FFileStream::Streams[StreamId];
	if ((Stream.Type != ST_XA && Stream.Type != ST_XALooping) || !RawData || NumSamples < 0)
		return 0;

	RawData->Load();
	const INT EncodedBytes = RawData->Num();
	const BYTE* EncodedData = RawData->GetData();
	if (EncodedBytes < 0 || (EncodedBytes > 0 && !EncodedData))
		return 0;

	std::unique_ptr<FXAStreamState> State(new FXAStreamState());
	if (EncodedBytes > 0)
		State->EncodedData.assign(EncodedData, EncodedData + EncodedBytes);
	State->Decoder.ResetState();
	const BYTE* OwnedData = State->EncodedData.empty() ? NULL : State->EncodedData.data();
	if (!State->Decoder.Feed(OwnedData, static_cast<INT>(State->EncodedData.size()), NumSamples))
		return 0;

	Stream.Handle = NULL;
	Stream.NumSamples = NumSamples;
	Stream.TDD = State.release();
	return 1;
}

void FileStreamingWorker()
{
	for (;;)
	{
		{
			std::unique_lock<std::mutex> WorkLock(GRuntime.WorkMutex);
			GRuntime.WorkReady.wait(WorkLock, [] {
				return GRuntime.StopRequested || GRuntime.PendingRequests > 0;
			});
			if (GRuntime.StopRequested)
				break;
		}

		for (INT StreamId = 0; StreamId < FFileStream::MaxStreams; ++StreamId)
		{
			std::lock_guard<std::mutex> Lock(StreamMutex(StreamId));
			FStream& Stream = FFileStream::Streams[StreamId];
			if (!Stream.Used || Stream.PendingCount == 0)
				continue;

			void* Destination = Stream.PendingDestinations[Stream.PendingHead];
			{
				std::lock_guard<std::mutex> WorkLock(GRuntime.WorkMutex);
				Stream.PendingHead = (
					Stream.PendingHead + 1) % FILE_STREAM_REQUEST_CAPACITY;
				--Stream.PendingCount;
				if (Stream.PendingCount == 0)
					Stream.PendingHead = 0;
				if (GRuntime.PendingRequests > 0)
					--GRuntime.PendingRequests;
			}

			const UBOOL ReadSucceeded = ReadStreamLocked(
				StreamId, Stream.ChunkSize, Destination);
			if (!ReadSucceeded)
				Stream.EndOfFile = 1;

			{
				std::lock_guard<std::mutex> WorkLock(GRuntime.WorkMutex);
				FStreamCompletion* Completion = NULL;
				for (size_t Offset = 0; Offset < Stream.CompletionCount; ++Offset)
				{
					FStreamCompletion& Candidate = Stream.CompletedChunks[
						(Stream.CompletionHead + Offset)
						% FILE_STREAM_REQUEST_CAPACITY];
					if (!Candidate.Ready && Candidate.Destination == Destination)
					{
						Completion = &Candidate;
						break;
					}
				}
				check(Completion != NULL);
				Completion->Terminal = Stream.EndOfFile != 0;
				Completion->Ready = 1;
			}

			if (Stream.EndOfFile)
				CancelPendingDestinationsLocked(Stream);
		}
	}
}

INT FindAvailableStream()
{
	for (INT StreamId = 0; StreamId < FFileStream::MaxStreams; ++StreamId)
	{
		std::lock_guard<std::mutex> Lock(StreamMutex(StreamId));
		if (!FFileStream::Streams[StreamId].Used)
			return StreamId;
	}
	return -1;
}
} // namespace

FFileStream* FFileStream::Init(INT InMaxStreams)
{
	std::lock_guard<std::mutex> LifecycleLock(GRuntime.LifecycleMutex);
	if (Instance)
		return Instance;
	if (InMaxStreams <= 0)
		return NULL;

	FFileStream* NewInstance = new FFileStream();
	std::unique_ptr<FStream[]> NewStreams(new FStream[static_cast<size_t>(InMaxStreams)]);
	for (INT StreamId = 0; StreamId < InMaxStreams; ++StreamId)
		ResetStream(NewStreams[StreamId]);

	GRuntime.StreamMutexes.clear();
	GRuntime.StreamMutexes.reserve(static_cast<size_t>(InMaxStreams));
	for (INT StreamId = 0; StreamId < InMaxStreams; ++StreamId)
		GRuntime.StreamMutexes.emplace_back(new std::mutex());

	Streams = NewStreams.release();
	MaxStreams = InMaxStreams;
	Destroyed = 0;
	GRuntime.PendingRequests = 0;
	GRuntime.StopRequested = 0;
	GRuntime.Active = 1;
	Instance = NewInstance;

	try
	{
		GRuntime.Worker = std::thread(FileStreamingWorker);
	}
	catch (...)
	{
		GRuntime.Active = 0;
		delete[] Streams;
		Streams = NULL;
		MaxStreams = 0;
		delete Instance;
		Instance = NULL;
		GRuntime.StreamMutexes.clear();
		Destroyed = 2;
		return NULL;
	}

	return Instance;
}

void FFileStream::Destroy()
{
	std::lock_guard<std::mutex> LifecycleLock(GRuntime.LifecycleMutex);
	if (!Instance)
	{
		Destroyed = 2;
		return;
	}

	Destroyed = 1;
	{
		std::lock_guard<std::mutex> WorkLock(GRuntime.WorkMutex);
		GRuntime.StopRequested = 1;
	}
	GRuntime.WorkReady.notify_all();
	if (GRuntime.Worker.joinable())
		GRuntime.Worker.join();

	for (INT StreamId = 0; StreamId < MaxStreams; ++StreamId)
	{
		std::lock_guard<std::mutex> Lock(StreamMutex(StreamId));
		FStream& Stream = Streams[StreamId];
		if (Stream.Used || Stream.Handle || Stream.TDD)
			DestroyStreamResourcesLocked(StreamId);
	}

	GRuntime.Active = 0;
	delete[] Streams;
	Streams = NULL;
	MaxStreams = 0;
	delete Instance;
	Instance = NULL;
	GRuntime.StreamMutexes.clear();
	GRuntime.PendingRequests = 0;
	GRuntime.StopRequested = 0;
	Destroyed = 2;
}

INT FFileStream::CreateStream(
	TLazyArray<BYTE>* RawData,
	INT NumSamples,
	INT ChunkSize,
	INT InitialChunks,
	void* Data,
	EFileStreamType Type,
	void* TDD)
{
	(void)TDD;
	std::lock_guard<std::mutex> LifecycleLock(GRuntime.LifecycleMutex);
	if (Instance != this || ChunkSize <= 0 || InitialChunks < 0
		|| InitialChunks > MAXINT / ChunkSize || (InitialChunks > 0 && !Data))
		return -1;

	const INT StreamId = FindAvailableStream();
	if (StreamId < 0)
		return -1;

	std::lock_guard<std::mutex> Lock(StreamMutex(StreamId));
	FStream& Stream = Streams[StreamId];
	Stream.Type = Type;
	if (!CreateXALocked(StreamId, RawData, NumSamples))
	{
		ResetStream(Stream);
		return -1;
	}

	Stream.ChunkSize = ChunkSize;
	Stream.EndOfFile = 0;
	Stream.Used = 1;
	if (InitialChunks > 0)
		ReadStreamLocked(StreamId, ChunkSize * InitialChunks, Data);
	return StreamId;
}

INT FFileStream::CreateStream(
	const TCHAR* Filename,
	INT ChunkSize,
	INT InitialChunks,
	void* Data,
	EFileStreamType Type,
	void* TDD)
{
	std::lock_guard<std::mutex> LifecycleLock(GRuntime.LifecycleMutex);
	if (Instance != this || ChunkSize <= 0 || InitialChunks < 0
		|| InitialChunks > MAXINT / ChunkSize || (InitialChunks > 0 && !Data))
		return -1;

	const INT StreamId = FindAvailableStream();
	if (StreamId < 0)
		return -1;

	std::lock_guard<std::mutex> Lock(StreamMutex(StreamId));
	FStream& Stream = Streams[StreamId];
	Stream.Type = Type;
	Stream.TDD = TDD;
	if (!CreateFileLocked(StreamId, Filename))
	{
		ResetStream(Stream);
		return -1;
	}

	Stream.ChunkSize = ChunkSize;
	Stream.EndOfFile = 0;
	Stream.Used = 1;
	if (InitialChunks > 0)
		ReadStreamLocked(StreamId, ChunkSize * InitialChunks, Data);
	return StreamId;
}

void FFileStream::DestroyStream(INT StreamId, UBOOL ReadQueuedChunks)
{
	std::lock_guard<std::mutex> LifecycleLock(GRuntime.LifecycleMutex);
	if (Instance != this || !IsValidStreamId(StreamId))
		return;

	std::lock_guard<std::mutex> Lock(StreamMutex(StreamId));
	FStream& Stream = Streams[StreamId];
	if (!Stream.Used)
		return;

	if (ReadQueuedChunks)
	{
		while (Stream.PendingCount > 0)
		{
			void* Destination = Stream.PendingDestinations[Stream.PendingHead];
			Stream.PendingHead = (
				Stream.PendingHead + 1) % FILE_STREAM_REQUEST_CAPACITY;
			--Stream.PendingCount;
			if (Stream.PendingCount == 0)
				Stream.PendingHead = 0;
			{
				std::lock_guard<std::mutex> WorkLock(GRuntime.WorkMutex);
				if (GRuntime.PendingRequests > 0)
					--GRuntime.PendingRequests;
			}
			ReadStreamLocked(StreamId, Stream.ChunkSize, Destination);
		}
	}
	DestroyStreamResourcesLocked(StreamId);
}

UBOOL FFileStream::RequestChunk(INT StreamId, void* Destination)
{
	std::lock_guard<std::mutex> LifecycleLock(GRuntime.LifecycleMutex);
	if (Instance != this || !IsValidStreamId(StreamId) || !Destination)
		return 0;

	{
		std::lock_guard<std::mutex> Lock(StreamMutex(StreamId));
		FStream& Stream = Streams[StreamId];
		if (!Stream.Used || Stream.EndOfFile
			|| Stream.PendingCount >= FILE_STREAM_REQUEST_CAPACITY
			|| Stream.CompletionCount >= FILE_STREAM_REQUEST_CAPACITY)
			return 0;

		std::lock_guard<std::mutex> WorkLock(GRuntime.WorkMutex);
		if (GRuntime.PendingRequests >= MAXINT)
			return 0;

		const size_t CompletionTail = (
			Stream.CompletionHead + Stream.CompletionCount)
			% FILE_STREAM_REQUEST_CAPACITY;
		Stream.CompletedChunks[CompletionTail] =
			FStreamCompletion{ Destination, 0, 0 };
		++Stream.CompletionCount;
		const size_t PendingTail = (
			Stream.PendingHead + Stream.PendingCount)
			% FILE_STREAM_REQUEST_CAPACITY;
		Stream.PendingDestinations[PendingTail] = Destination;
		++Stream.PendingCount;
		++GRuntime.PendingRequests;
	}
	GRuntime.WorkReady.notify_one();
	return 1;
}

UBOOL FFileStream::PopCompletedChunk(INT StreamId, void*& OutDestination, UBOOL& OutTerminal)
{
	std::lock_guard<std::mutex> LifecycleLock(GRuntime.LifecycleMutex);
	if (Instance != this || !IsValidStreamId(StreamId))
		return 0;

	std::lock_guard<std::mutex> Lock(StreamMutex(StreamId));
	FStream& Stream = Streams[StreamId];
	if (Stream.CompletionCount == 0
		|| !Stream.CompletedChunks[Stream.CompletionHead].Ready)
		return 0;

	const FStreamCompletion Completion =
		Stream.CompletedChunks[Stream.CompletionHead];
	{
		std::lock_guard<std::mutex> WorkLock(GRuntime.WorkMutex);
		Stream.CompletionHead = (
			Stream.CompletionHead + 1) % FILE_STREAM_REQUEST_CAPACITY;
		--Stream.CompletionCount;
		if (Stream.CompletionCount == 0)
			Stream.CompletionHead = 0;
	}
	OutDestination = Completion.Destination;
	OutTerminal = Completion.Terminal;
	return 1;
}

UBOOL FFileStream::IsStreamAlive(INT StreamId)
{
	std::lock_guard<std::mutex> LifecycleLock(GRuntime.LifecycleMutex);
	if (Instance != this || !IsValidStreamId(StreamId))
		return 0;
	std::lock_guard<std::mutex> Lock(StreamMutex(StreamId));
	return Streams[StreamId].Used && !Streams[StreamId].EndOfFile;
}

UBOOL FFileStream::Create(INT StreamId, const TCHAR* Filename)
{
	std::lock_guard<std::mutex> LifecycleLock(GRuntime.LifecycleMutex);
	if (Instance != this || !IsValidStreamId(StreamId))
		return 0;
	std::lock_guard<std::mutex> Lock(StreamMutex(StreamId));
	return CreateFileLocked(StreamId, Filename);
}

UBOOL FFileStream::Create(INT StreamId, TLazyArray<BYTE>* RawData, INT NumSamples)
{
	std::lock_guard<std::mutex> LifecycleLock(GRuntime.LifecycleMutex);
	if (Instance != this || !IsValidStreamId(StreamId))
		return 0;
	std::lock_guard<std::mutex> Lock(StreamMutex(StreamId));
	return CreateXALocked(StreamId, RawData, NumSamples);
}

UBOOL FFileStream::Destroy(INT StreamId)
{
	std::lock_guard<std::mutex> LifecycleLock(GRuntime.LifecycleMutex);
	if (Instance != this || !IsValidStreamId(StreamId))
		return 0;
	std::lock_guard<std::mutex> Lock(StreamMutex(StreamId));
	return DestroyStreamResourcesLocked(StreamId);
}

void FFileStream::Enter(INT StreamId)
{
	if (!IsValidStreamId(StreamId))
		return;
	StreamMutex(StreamId).lock();
	Streams[StreamId].Locked = 1;
}

void FFileStream::Leave(INT StreamId)
{
	if (!IsValidStreamId(StreamId))
		return;
	Streams[StreamId].Locked = 0;
	StreamMutex(StreamId).unlock();
}


FFileStream* FFileStream::Instance = NULL;
FStream* FFileStream::Streams = NULL;
INT FFileStream::MaxStreams = 0;
INT FFileStream::Destroyed = 2;
