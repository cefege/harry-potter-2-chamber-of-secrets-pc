// Fixture harness for UnTex.cpp. Links the ORIGINAL UnTex.o (HEAD, -O3 -g)
// with real implementations of the few runtime helpers the tex code calls and
// with data stubs for everything else. Vtables are borrowed from UnTex.o so
// virtual dispatch to Prime/ConstantTimeTick lands in the original code.
// Every record carries its inputs so the Rust test can replay it. See gen_tex.sh.
#include "EnginePrivate.h"
#include <cmath>
#include <cstdint>
#include <cstdio>
#include <cstdlib>
#include <cstring>

extern "C" char tex_vtable_UTexture[] __asm__("__ZTV8UTexture");
extern "C" char tex_vtable_UPalette[] __asm__("__ZTV8UPalette");

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

// Real implementations of the runtime helpers UnTex.o calls.
DOUBLE appSqrt(DOUBLE X) { return std::sqrt(X); }
void* appMemmove(void* D, const void* S, INT N) { return std::memmove(D, S, N); }
void appMemcpy(void* D, const void* S, INT N) { std::memcpy(D, S, N); }
void appMemset(void* D, INT C, INT N) { std::memset(D, C, N); }
void appFailAssert(const ANSICHAR* E, const ANSICHAR* F, INT L) {
    std::fprintf(stderr, "assert %s %s:%d\n", E, F, L);
    std::abort();
}

static uint64_t rng_state = 0x9E3779B97F4A7C15ull;
static uint32_t rnd() {
    rng_state = rng_state * 6364136223846793005ull + 1442695040888963407ull;
    return (uint32_t)(rng_state >> 33);
}
static uint32_t bits(float f) { uint32_t u; std::memcpy(&u, &f, 4); return u; }
static uint32_t int4(FColor c) { return (uint32_t)c.R | ((uint32_t)c.G << 8) | ((uint32_t)c.B << 16) | ((uint32_t)c.A << 24); }

static void print_palette(const char* tag, int id, const FColor* pal) {
    std::printf("%s %d", tag, id);
    for (int i = 0; i < NUM_PAL_COLORS; ++i) std::printf(" %08x", int4(pal[i]));
    std::printf("\n");
}

static UPalette* new_palette(const FColor* src) {
    UPalette* p = (UPalette*)std::calloc(1, sizeof(UPalette));
    *(char**)p = tex_vtable_UPalette + 16;
    p->Colors.Empty(NUM_PAL_COLORS);
    for (int i = 0; i < NUM_PAL_COLORS; ++i) p->Colors.AddItem(src[i]);
    return p;
}

static UTexture* new_texture() {
    UTexture* t = (UTexture*)std::calloc(1, sizeof(UTexture));
    *(char**)t = tex_vtable_UTexture + 16;
    return t;
}

int main() {
    // HSV: FGetHSV over random H,S,V.
    for (int i = 0; i < 600; ++i) {
        BYTE h = rnd() & 0xff, s = rnd() & 0xff, v = rnd() & 0xff;
        if (i < 4) { h = (BYTE)(i * 85); s = 255; v = 255; }
        FPlane p = FGetHSV(h, s, v);
        std::printf("hsv %u %u %u %08x %08x %08x %08x\n", h, s, v,
                    bits(p.X), bits(p.Y), bits(p.Z), bits(p.W));
    }

    // BestMatch over random palettes.
    for (int pi = 0; pi < 8; ++pi) {
        FColor pal[NUM_PAL_COLORS];
        for (int i = 0; i < NUM_PAL_COLORS; ++i)
            pal[i] = FColor(rnd() & 0xff, rnd() & 0xff, rnd() & 0xff, rnd() & 0xff);
        print_palette("bmpal", pi, pal);
        UPalette* p = new_palette(pal);
        for (int k = 0; k < 64; ++k) {
            FColor c(rnd() & 0xff, rnd() & 0xff, rnd() & 0xff, 0);
            INT first = (k & 1) ? 1 : (INT)(rnd() % NUM_PAL_COLORS);
            BYTE idx = p->BestMatch(c, first);
            std::printf("bm %d %08x %d %u\n", pi, int4(c), first, idx);
        }
    }

    // FixPalette over random palettes.
    for (int pi = 0; pi < 6; ++pi) {
        FColor pal[NUM_PAL_COLORS];
        for (int i = 0; i < NUM_PAL_COLORS; ++i)
            pal[i] = FColor(rnd() & 0xff, rnd() & 0xff, rnd() & 0xff, rnd() & 0xff);
        print_palette("fpin", pi, pal);
        UPalette* p = new_palette(pal);
        p->FixPalette();
        print_palette("fp", pi, &p->Colors(0));
    }

    // CacheMaxColor (RGBA7) on random odd-sized regions, sentinel max.
    for (int ci = 0; ci < 200; ++ci) {
        int usize = 1 << (1 + rnd() % 6);
        int vsize = 1 << (1 + rnd() % 6);
        int uclamp = 1 + rnd() % usize;
        int vclamp = 1 + rnd() % vsize;
        int n = usize * vsize * 4;
        BYTE* data = (BYTE*)std::malloc(n);
        for (int i = 0; i < n; ++i) data[i] = rnd() & 0xff;
        std::printf("cmcin %d %d %d %d", ci, usize, uclamp, vclamp);
        for (int i = 0; i < n; ++i) std::printf(" %02x", data[i]);
        std::printf("\n");
        FTextureInfo* ti = (FTextureInfo*)std::calloc(1, sizeof(FTextureInfo));
        FMipmapBase mip(0, 0);
        mip.DataPtr = data;
        ti->Mips[0] = &mip;
        ti->Format = TEXF_RGBA7;
        ti->USize = usize;
        ti->UClamp = uclamp;
        ti->VClamp = vclamp;
        FColor maxc(0xff, 0xff, 0xff, 0xff);
        ti->MaxColor = &maxc;
        ti->CacheMaxColor();
        std::printf("cmc %d %08x\n", ci, int4(*ti->MaxColor));
        std::free(data);
        std::free(ti);
    }

    // CreateMips (P8) + CreateColorRange on random odd-shaped textures.
    for (int ti_ = 0; ti_ < 40; ++ti_) {
        FColor pal[NUM_PAL_COLORS];
        for (int i = 0; i < NUM_PAL_COLORS; ++i)
            pal[i] = FColor(rnd() & 0xff, rnd() & 0xff, rnd() & 0xff, rnd() & 0xff);
        print_palette("mippal", ti_, pal);
        UPalette* p = new_palette(pal);
        UTexture* t = new_texture();
        t->Format = TEXF_P8;
        t->Palette = p;
        t->UBits = 1 + rnd() % 6;
        t->VBits = 1 + rnd() % 6;
        t->USize = 1 << t->UBits;
        t->VSize = 1 << t->VBits;
        t->UClamp = t->USize;
        t->VClamp = t->VSize;
        t->PolyFlags = (ti_ & 1) ? PF_Masked : 0;
        FMipmap* m0 = new (t->Mips) FMipmap(t->UBits, t->VBits);
        std::printf("mipin %d %d %d %d", ti_, (int)t->UBits, (int)t->VBits, (t->PolyFlags & PF_Masked) ? 1 : 0);
        for (INT i = 0; i < m0->DataArray.Num(); ++i) {
            m0->DataArray(i) = rnd() & 0xff;
            std::printf(" %02x", m0->DataArray(i));
        }
        std::printf("\n");
        t->CreateMips(1, 1);
        std::printf("mip %d %d %08x\n", ti_, t->Mips.Num(), int4(t->MipZero));
        for (INT k = 0; k < t->Mips.Num(); ++k) {
            FMipmap& m = t->Mips(k);
            std::printf("mipdata %d %d %d %d", ti_, k, m.USize, m.VSize);
            for (INT i = 0; i < m.DataArray.Num(); ++i) std::printf(" %02x", m.DataArray(i));
            std::printf("\n");
        }
        t->CreateColorRange();
        std::printf("cr %d %08x\n", ti_, int4(t->MaxColor));
    }

    // Tick + Prime sequences on a ring of textures.
    const int RING = 1000;
    UTexture* ring = (UTexture*)std::calloc(RING, sizeof(UTexture));
    for (int i = 0; i < RING; ++i) *(char**)&ring[i] = tex_vtable_UTexture + 16;
    for (int i = 0; i < RING; ++i) ring[i].AnimNext = &ring[(i + 1) % RING];
    for (int seq = 0; seq < 6; ++seq) {
        UTexture* t = &ring[0];
        t->MaxFrameRate = (seq == 0) ? 0.0f : (float)(rnd() % 200) / 4.0f - 5.0f;
        t->MinFrameRate = (seq == 1) ? 0.0f : (float)(rnd() % 80) / 8.0f - 1.0f;
        t->Accumulator = 0.0f;
        t->PrimeCount = rnd() % 3;
        t->PrimeCurrent = 0;
        t->AnimCur = &ring[0];
        std::printf("tickcfg %d %08x %08x %d\n", seq, bits(t->MaxFrameRate), bits(t->MinFrameRate), (int)t->PrimeCount);
        for (int step = 0; step < 40; ++step) {
            float d = (float)(rnd() % 1000) / 1000.0f * 0.3f;
            if (step == 0) d = 0.0f;
            t->Tick(d);
            std::printf("tick %d %d %08x %08x %d %d\n", seq, step, bits(d), bits(t->Accumulator),
                        (int)(t->AnimCur - ring), (int)t->PrimeCurrent);
        }
    }
    return 0;
}
