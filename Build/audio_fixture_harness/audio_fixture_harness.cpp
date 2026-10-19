// Fixture harness for the FWaveModInfo wave methods and FLipSyncData::GetMouthPos in
// UnAudio.cpp: ReadWaveInfo, NoiseGateFilter, HalveData, Reduce16to8, HalveReduce16to8,
// UpdateWaveData (after each size-changing op) and GetSampleWindowAmplitude.
// Includes the ORIGINAL oracle TU (pinned pre-port commit, copied into the work dir as
// UnAudio.oracle.cpp, -O3 -g) so the methods run unmodified. Each record carries its input
// bytes, the WavData bytes after the operation and the C++ field values, so the Rust test
// can replay it. Inputs are well-formed RIFF/WAVE blobs only. See gen_audio.sh.
#include <cstdint>
#include <cstdio>
#include <cstdlib>
#include <cstring>
#include <new>
#include <vector>

#include "UnAudio.oracle.cpp"
#include <cmath>

// appFmod is UnAnsi.cpp:107 (fmod). UnAnsi.cpp is not linked here; without a definition the
// generator stubs it as data, and the mouth lookup then jumps into it.
CORE_API DOUBLE appFmod(DOUBLE Y, DOUBLE X) { return fmod(Y, X); }

// Real allocator behind GMalloc (TArray growth goes through it).
class HarnessMalloc : public FMalloc {
public:
    void* Malloc(DWORD Count, const TCHAR*) override { return std::malloc(Count ? Count : 1); }
    void* Realloc(void* Original, DWORD Count, const TCHAR*) override { return std::realloc(Original, Count ? Count : 1); }
    void Free(void* Original) override { std::free(Original); }
    void DumpAllocs() override {}
    void HeapCheck() override {}
    void Init() override {}
    void Exit() override {}
};
static HarnessMalloc harness_malloc;
FMalloc* GMalloc = &harness_malloc;

void appMemcpy(void* D, const void* S, INT N) { std::memcpy(D, S, N); }
void* appMemmove(void* D, const void* S, INT N) { return std::memmove(D, S, N); }
void appMemset(void* D, INT C, INT N) { std::memset(D, C, N); }
void appFailAssert(const ANSICHAR* E, const ANSICHAR* F, INT L) {
    std::fprintf(stderr, "assert %s %s:%d\n", E, F, L);
    std::abort();
}

static uint64_t rng_state = 0xA0D10F1C7E57ull;
static uint32_t rnd() {
    rng_state = rng_state * 6364136223846793005ull + 1442695040888963407ull;
    return (uint32_t)(rng_state >> 33);
}

static void put16(std::vector<uint8_t>& v, uint32_t x) {
    v.push_back((uint8_t)(x & 0xff));
    v.push_back((uint8_t)((x >> 8) & 0xff));
}
static void put32(std::vector<uint8_t>& v, uint32_t x) {
    put16(v, x & 0xffff);
    put16(v, x >> 16);
}
static void put_tag(std::vector<uint8_t>& v, const char* t) {
    for (int i = 0; i < 4; ++i) v.push_back((uint8_t)t[i]);
}

// A well-formed RIFF/WAVE blob: fmt (PCM), data, an optional LIST chunk after the data
// (`trail`, so the bytes after the sample data are non-empty), and an optional smpl chunk
// with `loops` loops.
// Bytes after the data chunk when `trail` is set. Overhanging GetSampleWindowAmplitude
// windows read at most 0.2 * data length (<= 160 bytes for the sizes below) past the data.
static const uint32_t kTrailer = 176;
static std::vector<uint8_t> make_wav(int ch, int bits, uint32_t sps, uint32_t nsamp, int loops, bool trail) {
    uint32_t ba = (uint32_t)(ch * bits / 8);
    uint32_t dlen = nsamp * ba;
    uint32_t smpl_len = loops ? 36 + 24 * (uint32_t)loops : 0;
    uint32_t riff = 4 + (8 + 16) + (8 + dlen) + (trail ? 8 + 4 + kTrailer : 0) + (loops ? 8 + smpl_len : 0);
    std::vector<uint8_t> v;
    put_tag(v, "RIFF");
    put32(v, riff);
    put_tag(v, "WAVE");
    put_tag(v, "fmt ");
    put32(v, 16);
    put16(v, 1);
    put16(v, (uint32_t)ch);
    put32(v, sps);
    put32(v, sps * ba);
    put16(v, ba);
    put16(v, (uint32_t)bits);
    put_tag(v, "data");
    put32(v, dlen);
    for (uint32_t i = 0; i < dlen; ++i) v.push_back((uint8_t)(rnd() & 0xff));
    if (trail) {
        put_tag(v, "LIST");
        put32(v, 4 + kTrailer);
        put_tag(v, "INFO");
        for (uint32_t i = 0; i < kTrailer; ++i) v.push_back((uint8_t)(0x5a + i));
    }
    if (loops) {
        put_tag(v, "smpl");
        put32(v, smpl_len);
        put32(v, 0);  // manufacturer
        put32(v, 0);  // product
        put32(v, 0);  // sample period
        put32(v, 60); // midi unity note
        put32(v, 0);  // midi pitch fraction
        put32(v, 0);  // smpte format
        put32(v, 0);  // smpte offset
        put32(v, (uint32_t)loops);
        put32(v, 0);  // sampler data
        for (int l = 0; l < loops; ++l) {
            uint32_t start = (uint32_t)(l * 3) % nsamp;
            uint32_t end = (start + nsamp / 4) % nsamp;
            put32(v, (uint32_t)l); // cue id
            put32(v, 0);           // type
            put32(v, start);
            put32(v, end);
            put32(v, 0);           // fraction
            put32(v, 0);           // play count
        }
    }
    return v;
}

static void print_hex(const BYTE* p, DWORD n) {
    if (n == 0) { std::printf(" -"); return; }
    std::printf(" ");
    for (DWORD i = 0; i < n; ++i) std::printf("%02x", p[i]);
}

static void print_data(TArray<BYTE>& arr) {
    if (arr.Num() == 0) print_hex(nullptr, 0);
    else print_hex(&arr(0), arr.Num());
}

static long off(const void* p, const BYTE* base) {
    return p ? (long)((const BYTE*)p - base) : -1;
}

// Zeroing matches the constructor's defined state (NoiseGate and SampleLoopsNum are the
// only fields it sets, both to 0) and makes every other field deterministic in the dump.
static void fresh(FWaveModInfo& w) {
    std::memset(&w, 0, sizeof w);
}

// Field values in the order the Rust test replays them.
struct HdrVals { DWORD sps, avg, ba, bits, ch, wds, master; };

// Reads the header field at pointer p (into the pre-update buffer at base) from the live arr.
template <class T>
static T at_off(TArray<BYTE>& arr, const BYTE* base, const T* p) {
    T v;
    std::memcpy(&v, &arr(0) + (p ? ((const BYTE*)p - base) : 0), sizeof(T));
    return v;
}

static HdrVals hdr_at(FWaveModInfo& w, TArray<BYTE>& arr, const BYTE* base) {
    return HdrVals{ at_off(arr, base, w.pSamplesPerSec), at_off(arr, base, w.pAvgBytesPerSec),
                    at_off(arr, base, w.pBlockAlign), at_off(arr, base, w.pBitsPerSample),
                    at_off(arr, base, w.pChannels), at_off(arr, base, w.pWaveDataSize),
                    at_off(arr, base, w.pMasterSize) };
}

static void dump_at(FWaveModInfo& w, const BYTE* base, const HdrVals& h) {
    std::printf(" %u %d %u %u %u %u %u %u %u %u %ld %ld %ld %ld %d",
        (unsigned)w.SampleDataSize,
        (int)w.SampleLoopsNum,
        (unsigned)w.OldBitsPerSample,
        (unsigned)h.sps,
        (unsigned)h.avg,
        (unsigned)h.ba,
        (unsigned)h.bits,
        (unsigned)h.ch,
        (unsigned)h.wds,
        (unsigned)h.master,
        off(w.SampleDataStart, base),
        off(w.SampleDataEnd, base),
        off(w.WaveDataEnd, base),
        w.SampleLoopsNum > 0 ? off(w.pSampleLoop, base) : -1L,
        (int)w.NoiseGate);
}

static void dump(FWaveModInfo& w, TArray<BYTE>& arr) {
    const BYTE* base = arr.Num() ? &arr(0) : nullptr;
    dump_at(w, base, hdr_at(w, arr, base));
}

static void load(const std::vector<uint8_t>& blob, TArray<BYTE>& arr) {
    arr.Empty();
    arr.Add((INT)blob.size());
    std::memcpy(&arr(0), blob.data(), blob.size());
}

typedef void (FWaveModInfo::*WaveOp)();
struct OpDef {
    const char* name;
    WaveOp fn;
};
static const OpDef ops[] = {
    {"noise", &FWaveModInfo::NoiseGateFilter},
    {"halve", &FWaveModInfo::HalveData},
    {"reduce", &FWaveModInfo::Reduce16to8},
    {"halvereduce", &FWaveModInfo::HalveReduce16to8},
};
// Size-changing ops followed by UpdateWaveData (trailing-chunk shift and loop rescale).
static const OpDef upd_ops[] = {
    {"halveupd", &FWaveModInfo::HalveData},
    {"reduceupd", &FWaveModInfo::Reduce16to8},
    {"halvereduceupd", &FWaveModInfo::HalveReduce16to8},
};

static uint32_t float_bits(float f) {
    uint32_t u;
    std::memcpy(&u, &f, 4);
    return u;
}

static void emit(const std::vector<uint8_t>& blob) {
    // read: header parse only; WavData is unchanged.
    {
        TArray<BYTE> arr;
        FWaveModInfo w;
        fresh(w);
        load(blob, arr);
        UBOOL ok = w.ReadWaveInfo(arr);
        std::printf("read");
        print_hex(blob.data(), (DWORD)blob.size());
        std::printf(" %d", ok ? 1 : 0);
        if (ok) dump(w, arr);
        else std::printf(" -");
        print_data(arr);
        std::printf("\n");
    }
    // Each wave operation runs on a fresh read of the same blob.
    for (const OpDef& op : ops) {
        TArray<BYTE> arr;
        FWaveModInfo w;
        fresh(w);
        load(blob, arr);
        if (!w.ReadWaveInfo(arr)) continue;
        (w.*op.fn)();
        std::printf("%s", op.name);
        print_hex(blob.data(), (DWORD)blob.size());
        std::printf(" 1");
        dump(w, arr);
        print_data(arr);
        std::printf("\n");
    }
    // Size-changing op, then UpdateWaveData; the update result follows the input bytes.
    for (const OpDef& op : upd_ops) {
        TArray<BYTE> arr;
        FWaveModInfo w;
        fresh(w);
        load(blob, arr);
        if (!w.ReadWaveInfo(arr)) continue;
        (w.*op.fn)();
        // Header pointers are stale after UpdateWaveData's Remove (UnTemplate.h:558). Read the
        // header from the live array at each field's offset from the pre-update base, and never
        // dereference the stale pointers.
        const BYTE* oldbase = &arr(0);
        UBOOL upd = w.UpdateWaveData(arr);
        std::printf("%s", op.name);
        print_hex(blob.data(), (DWORD)blob.size());
        std::printf(" %d", upd ? 1 : 0);
        dump_at(w, oldbase, hdr_at(w, arr, oldbase));
        print_data(arr);
        std::printf("\n");
    }
    // Times and windows are fractions of the clip's sample-data duration, so every window
    // stays inside the blob. Trailing windows that run past the data (frac 0.9/1.0 + window)
    // overhang into the LIST payload, as the C loop does. Windows whose start would precede
    // the data start (leading/centered near t=0) are skipped: that would read the header.
    const float fracs[] = {0.0f, 0.5f, 0.9f, 1.0f};
    const float wfracs[] = {0.05f, 0.2f};
    for (float tf : fracs) {
        for (float sf : wfracs) {
            for (int mode = 0; mode < 3; ++mode) {
                TArray<BYTE> arr;
                FWaveModInfo w;
                fresh(w);
                load(blob, arr);
                if (!w.ReadWaveInfo(arr)) continue;
                float dur = (float)w.SampleDataSize / (float)(*w.pAvgBytesPerSec);
                float t = tf * dur;
                float s = sf * dur;
                float start = mode == 1 ? t - s : (mode == 2 ? t - s / 2 : t);
                if (start < 0.0f) continue;
                // GetSampleWindowAmplitude reads up to t+s (or t+s/2 when centered). Bytes past the
                // data end must lie inside the blob, so skip windows that would run past it.
                float past = (t + s - dur) * (float)(*w.pAvgBytesPerSec);
                long remain = (long)blob.size() - (long)(w.SampleDataEnd - &arr(0));
                if (past + 2.0f > (float)remain) continue;
                DWORD r = w.GetSampleWindowAmplitude(t, s, (eSampleWindowType)mode);
                std::printf("win");
                print_hex(blob.data(), (DWORD)blob.size());
                std::printf(" %08x %08x %d %u\n", float_bits(t), float_bits(s), mode, (unsigned)r);
            }
        }
    }
}

// FLipSyncData::GetMouthPos over an amplitude track of `duration` seconds. The track is
// sized so every non-final lookup index stays inside the array (the C code reads it
// unchecked). Each record carries the track, the duration, the time and the result.
static void emit_mouth(uint32_t extra, float duration) {
    USound* owner = static_cast<USound*>(std::calloc(1, sizeof(USound)));
    owner->Duration = duration;
    FLipSyncData* lip = new (std::calloc(1, sizeof(FLipSyncData))) FLipSyncData(owner);
    const float iv = FLipSyncData::fAmpSampleInterval;
    uint32_t n = (uint32_t)(duration / iv) + 2 + extra;
    lip->Add((INT)n);
    for (uint32_t i = 0; i < n; ++i) (*lip)(i) = (BYTE)(rnd() & 0xff);

    const float ts[] = {
        -0.1f,
        0.0f,
        duration,
        duration + 0.5f,
        iv * 3.0f,
        duration - iv,
        (float)rnd() / 2147483648.0f * duration,
        (float)rnd() / 2147483648.0f * duration,
    };
    for (float t : ts) {
        FLOAT r = lip->GetMouthPos(t);
        std::printf("mouth");
        print_data(*lip);
        std::printf(" %08x %08x %08x\n", float_bits(duration), float_bits(t), float_bits(r));
    }
}

int main() {
    const int chs[] = {1, 2};
    const int bitss[] = {8, 16};
    const uint32_t spss[] = {11025, 22050, 44100};
    const uint32_t nsamps[] = {64, 200};
    const int loopss[] = {0, 1, 3};
    for (int ch : chs)
        for (int bits : bitss)
            for (uint32_t sps : spss)
                for (uint32_t n : nsamps)
                    for (int loops : loopss)
                        for (int trail = 0; trail < 2; ++trail)
                            emit(make_wav(ch, bits, sps, n, loops, trail != 0));
    const float durations[] = {0.011f, 0.3f, 1.0f, 2.37f};
    for (float d : durations)
        for (uint32_t extra : {0u, 3u})
            emit_mouth(extra, d);
    return 0;
}
