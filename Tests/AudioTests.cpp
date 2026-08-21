#include <algorithm>
#include <chrono>
#include <cstdio>
#include <cstdlib>
#include <cstring>
#include <filesystem>
#include <fstream>
#include <iterator>
#include <string>
#include <thread>
#include <vector>

#include "Core.h"
#include "vorbis/vorbisfile.h"

extern "C" TCHAR GPackage[64] = TEXT("AudioTests");

namespace
{
class FTestMalloc : public FMalloc
{
public:
	void* Malloc(DWORD Count, const TCHAR*) override { return std::malloc(Count ? Count : 1); }
	void* Realloc(void* Original, DWORD Count, const TCHAR*) override
	{
		return std::realloc(Original, Count ? Count : 1);
	}
	void Free(void* Original) override { std::free(Original); }
	void DumpAllocs() override {}
	void HeapCheck() override {}
	void Init() override {}
	void Exit() override {}
};

FTestMalloc GTestMalloc;
INT GFailures = 0;

void Require(bool Condition, const char* Message)
{
	if (!Condition)
	{
		std::fprintf(stderr, "FAIL: %s\n", Message);
		++GFailures;
	}
}

std::basic_string<TCHAR> ToTChar(const std::filesystem::path& Path)
{
	const std::string Utf8 = Path.string();
	std::basic_string<TCHAR> Result;
	Result.reserve(Utf8.size());
	for (unsigned char Ch : Utf8)
		Result.push_back(static_cast<TCHAR>(Ch));
	return Result;
}

std::filesystem::path StockOggPath()
{
	return std::filesystem::path(__FILE__).parent_path().parent_path()
		/ "HarryPotter2/Unreal/Music/sm_bur_PlayfulFail_01.ogg";
}

void WriteBytes(const std::filesystem::path& Path, const std::vector<BYTE>& Bytes)
{
	std::ofstream Output(Path, std::ios::binary | std::ios::trunc);
	Output.write(reinterpret_cast<const char*>(Bytes.data()), static_cast<std::streamsize>(Bytes.size()));
	Require(Output.good(), "temporary audio input is writable");
}

std::vector<BYTE> ReadBytes(const std::filesystem::path& Path)
{
	std::ifstream Input(Path, std::ios::binary);
	return std::vector<BYTE>(std::istreambuf_iterator<char>(Input), std::istreambuf_iterator<char>());
}

bool WaitForChunks(FFileStream* Streams, INT StreamId, INT Expected)
{
	const auto Deadline = std::chrono::steady_clock::now() + std::chrono::seconds(5);
	while (std::chrono::steady_clock::now() < Deadline)
	{
		if (Streams->ChunksRemaining(StreamId) >= Expected)
			return true;
		std::this_thread::sleep_for(std::chrono::milliseconds(1));
	}
	return false;
}

std::vector<BYTE> DecodeOgg(const std::filesystem::path& Path)
{
	OggVorbis_File File;
	std::vector<BYTE> Result;
	if (ov_fopen(Path.string().c_str(), &File) != 0)
	{
		Require(false, "stock Ogg opens through libvorbisfile");
		return Result;
	}

	vorbis_info* Info = ov_info(&File, -1);
	const ogg_int64_t Frames = ov_pcm_total(&File, -1);
	if (Info && Frames > 0)
	{
		// ov_pcm_total is a frame count, not a byte count.
		const ogg_int64_t Bytes = Frames * Info->channels * static_cast<ogg_int64_t>(sizeof(SWORD));
		if (Bytes > 0 && Bytes < static_cast<ogg_int64_t>(MAXINT))
			Result.reserve(static_cast<size_t>(Bytes));
	}

	char Buffer[8192];
	INT Bitstream = 0;
	for (;;)
	{
		const long Count = ov_read(&File, Buffer, sizeof(Buffer), 0, 2, 1, &Bitstream);
		if (Count == 0)
			break;
		if (Count < 0)
		{
			Require(false, "stock Ogg decodes without a hole or corruption");
			break;
		}
		Result.insert(Result.end(), Buffer, Buffer + Count);
	}
	ov_clear(&File);
	return Result;
}

void TestRegularLifecycle(const std::filesystem::path& TempDir)
{
	const std::filesystem::path RegularPath = TempDir / "regular.bin";
	const std::vector<BYTE> Source = {1, 2, 3, 4, 5, 6};
	WriteBytes(RegularPath, Source);
	const std::basic_string<TCHAR> Filename = ToTChar(RegularPath);

	FFileStream* Streams = FFileStream::Init(2);
	Require(Streams != NULL, "stream manager initializes");
	Require(FFileStream::Init(9) == Streams, "Init is idempotent");
	Require(FFileStream::MaxStreams == 2, "idempotent Init preserves the live capacity");

	BYTE Initial[4] = {0, 0, 0, 0};
	const INT StreamId = Streams->CreateStream(Filename.c_str(), 4, 1, Initial, ST_Regular, NULL);
	Require(StreamId == 0, "regular stream uses the first slot");
	if (StreamId >= 0)
	{
		const FStream& State = FFileStream::Streams[StreamId];
		Require(State.Type == ST_Regular && State.ChunkSize == 4 && State.NumSamples == -1,
			"regular stream metadata preserves type, chunk size, and sample sentinel");
		Require(State.Data == Initial + 4, "regular stream advances Data by bytes actually read");
	}
	Require(std::memcmp(Initial, Source.data(), 4) == 0, "regular initial chunk preserves file bytes");
	Require(Streams->IsStreamAlive(StreamId), "full regular chunk does not report EOF");

	BYTE Tail[4] = {0xcc, 0xcc, 0xcc, 0xcc};
	Streams->RequestChunks(StreamId, 1, Tail);
	Require(WaitForChunks(Streams, StreamId, 1), "regular queued read completes");
	Require(Tail[0] == 5 && Tail[1] == 6, "regular queued read continues at the file position");
	Require(!Streams->IsStreamAlive(StreamId), "short regular read reports EOF");
	Require(Streams->ChunksRemaining(StreamId) == 1, "completed regular request increments queue count");
	Streams->DecrementChunkCount(StreamId);
	Require(Streams->ChunksRemaining(StreamId) == 0, "consumer decrement removes the completed chunk");
	Streams->DecrementChunkCount(StreamId);
	Require(Streams->ChunksRemaining(StreamId) == 0, "completed count never underflows");
	Streams->DestroyStream(StreamId, 0);

	BYTE Reuse[2] = {0, 0};
	const INT ReusedId = Streams->CreateStream(Filename.c_str(), 2, 1, Reuse, ST_Regular, NULL);
	Require(ReusedId == StreamId, "destroyed stream slot is immediately reusable");
	Streams->DestroyStream(ReusedId, 0);

	FFileStream::Destroy();
	Require(FFileStream::Instance == NULL && FFileStream::Streams == NULL, "Destroy releases singleton storage");
	Require(FFileStream::Destroyed == 2, "Destroy publishes joined-worker state");
	FFileStream::Destroy();
	Require(FFileStream::Destroyed == 2, "Destroy is idempotent");
}

void TestQueuedDestroy(const std::filesystem::path& TempDir)
{
	constexpr INT ChunkSize = 64;
	constexpr INT ChunkCount = 4096;
	std::vector<BYTE> Source(static_cast<size_t>(ChunkSize) * ChunkCount);
	for (size_t Index = 0; Index < Source.size(); ++Index)
		Source[Index] = static_cast<BYTE>((Index * 37u + 11u) & 0xffu);

	const std::filesystem::path Path = TempDir / "queued.bin";
	WriteBytes(Path, Source);
	const std::basic_string<TCHAR> Filename = ToTChar(Path);
	std::vector<BYTE> Output(Source.size(), 0);

	FFileStream* Streams = FFileStream::Init(1);
	const INT StreamId = Streams->CreateStream(Filename.c_str(), ChunkSize, 0, Output.data(), ST_Regular, NULL);
	Require(StreamId == 0, "queued-destroy stream creates");
	Streams->RequestChunks(StreamId, ChunkCount, Output.data());
	Streams->DestroyStream(StreamId, 1);
	Require(Output == Source, "queued destroy drains every requested regular byte exactly once");

	BYTE Reuse[ChunkSize] = {};
	const INT ReusedId = Streams->CreateStream(Filename.c_str(), ChunkSize, 0, Reuse, ST_Regular, NULL);
	Require(ReusedId == StreamId, "queued destroy leaves the slot reusable");
	Streams->DestroyStream(ReusedId, 0);
	FFileStream::Destroy();
}

void TestOggEOFLoopAndErrors(const std::filesystem::path& TempDir)
{
	const std::filesystem::path OggPath = StockOggPath();
	Require(std::filesystem::is_regular_file(OggPath), "stock Ogg fixture exists");
	const std::vector<BYTE> Pcm = DecodeOgg(OggPath);
	Require(Pcm.size() > 64, "stock Ogg has decoded PCM");
	Require(Pcm.size() + 64 < static_cast<size_t>(MAXINT), "stock Ogg fits one test request");
	const std::basic_string<TCHAR> Filename = ToTChar(OggPath);

	FFileStream* Streams = FFileStream::Init(1);
	std::vector<BYTE> NonLoop(Pcm.size() + 64, 0x7f);
	OggVorbis_File* NonLoopState = new OggVorbis_File;
	const INT NonLoopId = Streams->CreateStream(
		Filename.c_str(), static_cast<INT>(NonLoop.size()), 1, NonLoop.data(), ST_Ogg, NonLoopState);
	Require(NonLoopId == 0, "non-looping Ogg stream creates");
	if (NonLoopId >= 0)
	{
		const FStream& State = FFileStream::Streams[NonLoopId];
		Require(State.Type == ST_Ogg && State.TDD == NonLoopState && State.NumSamples == -1,
			"Ogg stream preserves caller-visible decoder metadata");
		Require(State.Data == NonLoop.data(), "Ogg decoding preserves the chunk base Data pointer");
		Require(std::equal(Pcm.begin(), Pcm.end(), NonLoop.begin()), "Ogg stream yields libvorbisfile PCM bytes");
		Require(std::all_of(NonLoop.begin() + static_cast<std::ptrdiff_t>(Pcm.size()), NonLoop.end(),
			[](BYTE Value) { return Value == 0; }), "non-looping Ogg zero-fills its final partial request");
		Require(!Streams->IsStreamAlive(NonLoopId), "non-looping Ogg EOF is observable");
		Streams->DestroyStream(NonLoopId, 0);
	}
	else
	{
		delete NonLoopState;
	}

	std::vector<BYTE> Loop(Pcm.size() + 64, 0);
	OggVorbis_File* LoopState = new OggVorbis_File;
	const INT LoopId = Streams->CreateStream(
		Filename.c_str(), static_cast<INT>(Loop.size()), 1, Loop.data(), ST_OggLooping, LoopState);
	Require(LoopId == 0, "looping Ogg stream reuses the slot");
	if (LoopId >= 0)
	{
		Require(std::equal(Pcm.begin(), Pcm.end(), Loop.begin()), "looping Ogg preserves its first pass");
		Require(std::equal(Pcm.begin(), Pcm.begin() + 64,
			Loop.begin() + static_cast<std::ptrdiff_t>(Pcm.size())), "looping Ogg fills past EOF from its beginning");
		Require(Streams->IsStreamAlive(LoopId), "healthy looping Ogg remains alive");
		Streams->DestroyStream(LoopId, 0);
	}
	else
	{
		delete LoopState;
	}

	const std::filesystem::path InvalidPath = TempDir / "invalid.ogg";
	WriteBytes(InvalidPath, std::vector<BYTE>{'n', 'o', 't', 'o', 'g', 'g'});
	const std::basic_string<TCHAR> InvalidFilename = ToTChar(InvalidPath);
	BYTE InvalidOutput[32] = {};
	OggVorbis_File* InvalidState = new OggVorbis_File;
	Require(Streams->CreateStream(InvalidFilename.c_str(), sizeof(InvalidOutput), 1,
		InvalidOutput, ST_Ogg, InvalidState) == -1, "invalid Ogg header is rejected");
	delete InvalidState;

	std::vector<BYTE> Encoded = ReadBytes(OggPath);
	Require(Encoded.size() > 4096, "stock Ogg is large enough for truncation coverage");
	Encoded.resize(Encoded.size() / 2);
	const std::filesystem::path TruncatedPath = TempDir / "truncated.ogg";
	WriteBytes(TruncatedPath, Encoded);
	const std::basic_string<TCHAR> TruncatedFilename = ToTChar(TruncatedPath);
	std::vector<BYTE> TruncatedOutput(Pcm.size() + 64, 0x55);
	OggVorbis_File* TruncatedState = new OggVorbis_File;
	const INT TruncatedId = Streams->CreateStream(
		TruncatedFilename.c_str(), static_cast<INT>(TruncatedOutput.size()), 1,
		TruncatedOutput.data(), ST_Ogg, TruncatedState);
	if (TruncatedId < 0)
	{
		delete TruncatedState;
		Require(true, "truncated Ogg is rejected during seekable open");
	}
	else
	{
		Require(!Streams->IsStreamAlive(TruncatedId), "accepted truncated Ogg reaches deterministic EOF/error");
		Require(std::all_of(TruncatedOutput.end() - 64, TruncatedOutput.end(),
			[](BYTE Value) { return Value == 0; }), "accepted truncated Ogg zero-fills unread output");
		Streams->DestroyStream(TruncatedId, 0);
	}
	FFileStream::Destroy();
}

void FillXABlock(TLazyArray<BYTE>& Raw)
{
	Raw(0) = 0x04; // Predictor zero, range four.
	for (INT Index = 1; Index < 15; ++Index)
		Raw(Index) = static_cast<BYTE>((Index << 4) | (15 - Index));
}

std::vector<SWORD> DecodeExpectedXA(TLazyArray<BYTE>& Raw)
{
	FEAXABlockDecoder Decoder;
	Decoder.ResetState();
	Require(Decoder.Feed(Raw.GetData(), Raw.Num(), 28), "synthetic XA block feeds reference decoder");
	std::vector<SWORD> Result(28);
	Require(Decoder.Decode(Result.data(), 28) == 28, "synthetic XA reference block decodes");
	return Result;
}

void TestXAPartialsAndLoop()
{
	TLazyArray<BYTE> Raw(15);
	FillXABlock(Raw);
	const std::vector<SWORD> Expected = DecodeExpectedXA(Raw);
	FFileStream* Streams = FFileStream::Init(1);

	std::vector<SWORD> First(5, 0);
	const INT PartialId = Streams->CreateStream(&Raw, 28, 10, 1, First.data(), ST_XA, NULL);
	Require(PartialId == 0, "XA partial stream creates");
	if (PartialId >= 0)
	{
		const FStream& State = FFileStream::Streams[PartialId];
		Require(State.Type == ST_XA && State.Handle == NULL && State.TDD != NULL && State.NumSamples == 28,
			"XA stream keeps package data out of worker state and preserves decoder metadata");
		Require(State.Data == First.data(), "XA decoding preserves the chunk base Data pointer");
	}
	Require(std::equal(First.begin(), First.end(), Expected.begin()), "XA initial partial returns requested samples");

	std::vector<SWORD> Second(5, 0);
	Streams->RequestChunks(PartialId, 1, Second.data());
	Require(WaitForChunks(Streams, PartialId, 1), "XA queued partial completes");
	Require(std::equal(Second.begin(), Second.end(), Expected.begin() + 5), "XA residue continues without dropping samples");
	Streams->DestroyStream(PartialId, 0);

	std::vector<SWORD> End(34, static_cast<SWORD>(1234));
	const INT EndId = Streams->CreateStream(&Raw, 28, static_cast<INT>(End.size() * sizeof(SWORD)), 1,
		End.data(), ST_XA, NULL);
	Require(EndId == 0, "XA EOF stream reuses its slot");
	Require(std::equal(Expected.begin(), Expected.end(), End.begin()), "XA EOF preserves all decoded samples");
	Require(std::all_of(End.begin() + 28, End.end(), [](SWORD Value) { return Value == 0; }),
		"XA EOF zero-fills the remaining sample request");
	Require(!Streams->IsStreamAlive(EndId), "XA EOF is observable");
	Streams->DestroyStream(EndId, 0);

	std::vector<SWORD> Loop(34, 0);
	const INT LoopId = Streams->CreateStream(&Raw, 28, static_cast<INT>(Loop.size() * sizeof(SWORD)), 1,
		Loop.data(), ST_XALooping, NULL);
	Require(LoopId == 0, "looping XA stream reuses its slot");
	Require(std::equal(Expected.begin(), Expected.end(), Loop.begin()), "looping XA preserves its first pass");
	Require(std::equal(Expected.begin(), Expected.begin() + 6, Loop.begin() + 28),
		"looping XA refills a partial request from the encoded beginning");
	Require(Streams->IsStreamAlive(LoopId), "healthy looping XA remains alive");
	Streams->DestroyStream(LoopId, 0);
	FFileStream::Destroy();
}

void TestRepeatedTeardownHasNoWorker(const std::filesystem::path& TempDir)
{
	const std::filesystem::path Path = TempDir / "cycles.bin";
	std::vector<BYTE> Source(1024 * 64, 0x6d);
	WriteBytes(Path, Source);
	const std::basic_string<TCHAR> Filename = ToTChar(Path);

	for (INT Cycle = 0; Cycle < 20; ++Cycle)
	{
		std::vector<BYTE> Output(Source.size(), 0);
		FFileStream* Streams = FFileStream::Init(1);
		Require(Streams != NULL, "cycle Init creates one worker");
		const INT StreamId = Streams->CreateStream(Filename.c_str(), 64, 0, Output.data(), ST_Regular, NULL);
		Require(StreamId == 0, "cycle stream creates");
		Streams->RequestChunks(StreamId, 1024, Output.data());
		FFileStream::Destroy();
		Require(FFileStream::Destroyed == 2, "cycle Destroy joins its worker");
		Require(FFileStream::Instance == NULL, "cycle Destroy clears the singleton");

		const std::vector<BYTE> Snapshot = Output;
		std::this_thread::sleep_for(std::chrono::milliseconds(2));
		Require(Output == Snapshot, "no worker writes after Destroy returns");
	}
}
} // namespace

int main()
{
	GMalloc = &GTestMalloc;
	const std::filesystem::path TempDir = std::filesystem::temp_directory_path()
		/ ("hp2-audio-tests-" + std::to_string(
			std::chrono::steady_clock::now().time_since_epoch().count()));
	std::error_code Error;
	std::filesystem::create_directories(TempDir, Error);
	Require(!Error, "temporary audio directory is created");

	TestRegularLifecycle(TempDir);
	TestQueuedDestroy(TempDir);
	TestOggEOFLoopAndErrors(TempDir);
	TestXAPartialsAndLoop();
	TestRepeatedTeardownHasNoWorker(TempDir);

	FFileStream::Destroy();
	std::filesystem::remove_all(TempDir, Error);
	if (Error)
		Require(false, "temporary audio directory is removed");

	if (GFailures != 0)
	{
		std::fprintf(stderr, "%d audio lifecycle test(s) failed\n", GFailures);
		return 1;
	}
	std::puts("audio lifecycle tests passed");
	return 0;
}
