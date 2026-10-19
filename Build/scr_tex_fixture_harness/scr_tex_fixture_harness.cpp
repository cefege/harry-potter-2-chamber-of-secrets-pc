// Fixture harness for UnScrTex.cpp. Includes the ORIGINAL oracle TU (git 4914c7e,
// copied into the work dir as UnScrTex.oracle.cpp, -O3 -g) so the static helpers
// DrawString/GetCharSize and the members DrawTile/PostLoad/Tick run unmodified.
// Texture locking uses the real UTexture::Lock/Unlock/Tick from the pre-port UnTex.o.
// Exec thunks need a live FFrame and are not exercised here. Every record carries
// its inputs so the Rust test can replay it. See gen_scr_tex.sh.
#include <cmath>
#include <cstdint>
#include <cstdio>
#include <cstdlib>
#include <cstring>
#include <new>
// The harness must set and read the private members the ctor/PostLoad/Tick use
// (OldSourceTex, LocalSourceBitmap, PaletteMap). Standard headers come first so
// only the engine declarations see the redefinition.
#define private public
#include "UnScrTex.oracle.cpp"
#undef private

extern "C" char scr_vtable_UTexture[] __asm__("__ZTV8UTexture");
extern "C" char scr_vtable_UPalette[] __asm__("__ZTV8UPalette");
extern "C" char scr_vtable_UScriptedTexture[] __asm__("__ZTV16UScriptedTexture");

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

// Real implementations of the runtime helpers the oracle objects call.
DOUBLE appSqrt(DOUBLE X) { return std::sqrt(X); }
void* appMemmove(void* D, const void* S, INT N) { return std::memmove(D, S, N); }
void appMemcpy(void* D, const void* S, INT N) { std::memcpy(D, S, N); }
void appMemset(void* D, INT C, INT N) { std::memset(D, C, N); }
void appFailAssert(const ANSICHAR* E, const ANSICHAR* F, INT L) {
    std::fprintf(stderr, "assert %s %s:%d\n", E, F, L);
    std::abort();
}

// Deterministic clock: LocalTime = appSeconds().GetFloat() in Tick.
static double g_now = 0.0;
FTime appSeconds() { return FTime(g_now); }

static uint64_t rng_state = 0x5C4D7EC5F00DBEEFull;
static uint32_t rnd() {
    rng_state = rng_state * 6364136223846793005ull + 1442695040888963407ull;
    return (uint32_t)(rng_state >> 33);
}
static float frnd(float lo, float hi) {
    return lo + (hi - lo) * ((float)(rnd() & 0xffffff) / 16777216.0f);
}
static uint32_t bits(float f) { uint32_t u; std::memcpy(&u, &f, 4); return u; }
static uint32_t int4(FColor c) { return (uint32_t)c.R | ((uint32_t)c.G << 8) | ((uint32_t)c.B << 16) | ((uint32_t)c.A << 24); }

static void print_hex(const BYTE* p, int n) {
    if (n == 0) { std::printf(" -"); return; }
    std::printf(" ");
    for (int i = 0; i < n; ++i) std::printf("%02x", p[i]);
}

enum { NPAL = 6, NTEX = 14, NFONT = 3, NCASE = 48, NPL = 24 };
static UPalette* g_pal[NPAL];
static UTexture* g_tex[NTEX];
static UFont* g_font[NFONT];

static int pal_id(const void* p) {
    for (int i = 0; i < NPAL; ++i) if ((const void*)g_pal[i] == p) return i;
    return -1;
}
static int tex_id(const void* t) {
    if (!t) return -1;
    for (int i = 0; i < NTEX; ++i) if ((const void*)g_tex[i] == t) return i;
    return -1;
}

static UPalette* make_palette(const FColor* src) {
    UPalette* p = (UPalette*)std::calloc(1, sizeof(UPalette));
    *(char**)p = scr_vtable_UPalette + 16;
    p->Colors.Empty(NUM_PAL_COLORS);
    for (int i = 0; i < NUM_PAL_COLORS; ++i) p->Colors.AddItem(src[i]);
    return p;
}

static UTexture* make_texture(int ub, int vb, UPalette* pal, bool zero_heavy) {
    UTexture* t = (UTexture*)std::calloc(1, sizeof(UTexture));
    *(char**)t = scr_vtable_UTexture + 16;
    t->UBits = ub; t->VBits = vb;
    t->USize = 1 << ub; t->VSize = 1 << vb;
    t->UClamp = t->USize; t->VClamp = t->VSize;
    t->Format = TEXF_P8;
    t->Palette = pal;
    t->Mips.Add(1);
    new (&t->Mips(0)) FMipmap((BYTE)ub, (BYTE)vb);
    FMipmap& m = t->Mips(0);
    for (INT i = 0; i < m.DataArray.Num(); ++i)
        m.DataArray(i) = (zero_heavy && rnd() % 3) ? 0 : (BYTE)(rnd() & 0xff);
    return t;
}

static void print_texture(int id, UTexture* t) {
    FMipmap& m = t->Mips(0);
    std::printf("tex %d %d %d %d", id, (int)t->UBits, (int)t->VBits, pal_id(t->Palette));
    print_hex(&m.DataArray(0), m.DataArray.Num());
    std::printf("\n");
}

static UScriptedTexture* make_scr(int ub, int vb, UPalette* pal, const BYTE* init) {
    UScriptedTexture* s = (UScriptedTexture*)std::calloc(1, sizeof(UScriptedTexture));
    *(char**)s = scr_vtable_UScriptedTexture + 16;
    s->UBits = ub; s->VBits = vb;
    s->USize = 1 << ub; s->VSize = 1 << vb;
    s->UClamp = s->USize; s->VClamp = s->VSize;
    s->Format = TEXF_P8;
    s->Palette = pal;
    // Same fields the ctor sets (the ctor itself is not called; see header note).
    s->bParametric = 1;
    s->bRealtime = 1;
    s->SourceTexture = NULL;
    s->OldSourceTex = NULL;
    s->LocalSourceBitmap = NULL;
    s->NotifyActor = NULL;
    s->AnimNext = s;
    s->AnimCur = s;
    s->PaletteMap = new TMap<UTexture*, TArray<BYTE>>;
    s->Mips.Add(1);
    new (&s->Mips(0)) FMipmap((BYTE)ub, (BYTE)vb);
    FMipmap& m = s->Mips(0);
    for (INT i = 0; i < m.DataArray.Num(); ++i) m.DataArray(i) = init[i];
    return s;
}

static void print_dest(UScriptedTexture* s) {
    FMipmap& m = s->Mips(0);
    print_hex(&m.DataArray(0), m.DataArray.Num());
    std::printf("\n");
}

static UFont* make_font(int cpp, bool remapped) {
    UFont* f = (UFont*)std::calloc(1, sizeof(UFont));
    new (&f->Pages) TArray<FFontPage>;
    new (&f->CharRemap) TMap<TCHAR, TCHAR>;
    f->CharactersPerPage = cpp;
    f->IsRemapped = remapped;
    return f;
}

static void add_page(UFont* f, UTexture* tex, int nchars, const int* glyph) {
    FFontPage pg;
    pg.Texture = tex;
    new (&pg.Characters) TArray<FFontCharacter>;
    for (int c = 0; c < nchars; ++c) {
        FFontCharacter ch;
        ch.StartU = glyph[c * 4 + 0];
        ch.StartV = glyph[c * 4 + 1];
        ch.USize = glyph[c * 4 + 2];
        ch.VSize = glyph[c * 4 + 3];
        pg.Characters.AddItem(ch);
    }
    f->Pages.AddItem(pg);
}

// UObject::PostLoad (Core/Src/UnObj.cpp) without the guard macros; the object system is not linked.
void UObject::PostLoad() {
	SetFlags(RF_DebugPostLoad);
}

int main() {
    // Palettes.
    for (int i = 0; i < NPAL; ++i) {
        FColor pal[NUM_PAL_COLORS];
        for (int c = 0; c < NUM_PAL_COLORS; ++c)
            pal[c] = FColor(rnd() & 0xff, rnd() & 0xff, rnd() & 0xff, rnd() & 0xff);
        g_pal[i] = make_palette(pal);
        std::printf("pal %d", i);
        for (int c = 0; c < NUM_PAL_COLORS; ++c) std::printf(" %08x", int4(pal[c]));
        std::printf("\n");
    }

    // Source textures (power-of-two, unequal dimensions on purpose).
    for (int i = 0; i < NTEX; ++i) {
        int ub = 1 + (int)(rnd() % 6);
        int vb = 1 + (int)(rnd() % 6);
        g_tex[i] = make_texture(ub, vb, g_pal[rnd() % NPAL], (i % 3) == 2);
        print_texture(i, g_tex[i]);
    }

    // Fonts. Font 2 is remapped; page textures may be NULL.
    for (int fi = 0; fi < NFONT; ++fi) {
        int cpp = 1 + (int)(rnd() % 16);
        bool remapped = (fi == 2);
        g_font[fi] = make_font(cpp, remapped);
        std::printf("font %d %d %d\n", fi, cpp, remapped ? 1 : 0);
        if (remapped) {
            for (int k = 0; k < 12; ++k) {
                TCHAR from = (TCHAR)(0x20 + rnd() % 0x60);
                TCHAR to = (TCHAR)(rnd() % 0x180);
                g_font[fi]->CharRemap.Set(from, to);
                std::printf("remap %d %d %d\n", fi, (int)from, (int)to);
            }
        }
        int npages = 1 + (int)(rnd() % 4);
        for (int p = 0; p < npages; ++p) {
            int tid = (rnd() % 5 == 0) ? -1 : (int)(rnd() % NTEX);
            int nchars = (int)(rnd() % (cpp + 3));
            int glyph[64 * 4];
            for (int c = 0; c < nchars; ++c) {
                glyph[c * 4 + 0] = (int)(rnd() % 40);
                glyph[c * 4 + 1] = (int)(rnd() % 40);
                glyph[c * 4 + 2] = 1 + (int)(rnd() % 20);
                glyph[c * 4 + 3] = 1 + (int)(rnd() % 20);
            }
            add_page(g_font[fi], tid < 0 ? NULL : g_tex[tid], nchars, glyph);
            std::printf("page %d %d %d %d", fi, p, tid, nchars);
            for (int c = 0; c < nchars; ++c)
                std::printf(" %d %d %d %d", glyph[c * 4], glyph[c * 4 + 1], glyph[c * 4 + 2], glyph[c * 4 + 3]);
            std::printf("\n");
        }
    }

    // GetCharSize over every font and a code point range that spans every page
    // plus out-of-range and remapped characters.
    for (int fi = 0; fi < NFONT; ++fi) {
        for (int ch = 0; ch < 0x140; ++ch) {
            INT w = 0, h = 0;
            GetCharSize(g_font[fi], (TCHAR)ch, w, h);
            std::printf("chr %d %d %d %d\n", fi, ch, (int)w, (int)h);
        }
    }

    // Scripted-texture cases: DrawTile, DrawString, PostLoad, Tick.
    for (int ci = 0; ci < NCASE; ++ci) {
        int ub = 2 + (int)(rnd() % 5);
        int vb = 2 + (int)(rnd() % 5);
        UPalette* pal = g_pal[rnd() % NPAL];
        int n = (1 << ub) * (1 << vb);
        BYTE* init = (BYTE*)std::malloc(n);
        for (int i = 0; i < n; ++i) init[i] = (BYTE)(rnd() & 0xff);
        UScriptedTexture* s = make_scr(ub, vb, pal, init);
        std::printf("case %d %d %d %d", ci, ub, vb, pal_id(pal));
        print_hex(init, n);
        std::printf("\n");
        std::free(init);

        int nops = 3 + (int)(rnd() % 5);
        for (int oi = 0; oi < nops; ++oi) {
            g_now = 0.25 * (ci * 8 + oi);
            if (rnd() % 2) {
                // DrawTile, with the texture locked the way execDrawTile does it.
                int tid = (int)(rnd() % NTEX);
                UTexture* t = g_tex[tid];
                float tu = (float)t->USize, tv = (float)t->VSize;
                FLOAT X = frnd(-6.0f, (float)s->USize + 6.0f);
                FLOAT Y = frnd(-6.0f, (float)s->VSize + 6.0f);
                FLOAT XL = frnd(0.5f, 24.0f);
                FLOAT YL = frnd(0.5f, 24.0f);
                FLOAT U = frnd(-4.0f, tu + 4.0f);
                FLOAT V = frnd(-4.0f, tv + 4.0f);
                FLOAT UL = frnd(0.5f, tu);
                FLOAT VL = frnd(0.5f, tv);
                if (rnd() % 8 == 0) UL = -UL;
                if (rnd() % 8 == 0) VL = -VL;
                UBOOL masked = (UBOOL)(rnd() % 2);
                UBOOL use_color = (UBOOL)(rnd() % 2);
                // The oracle's (bUseColor && !bMasked) branch dereferences the NULL PalMap (UB, crash).
                // Force masked in that case; the Rust port documents the no-op for it.
                if (use_color) masked = 1;
                BYTE color = (BYTE)(rnd() & 0xff);
                FTextureInfo info;
                std::memset(&info, 0, sizeof(info));
                t->Lock(info, appSeconds(), 0, NULL);
                s->DrawTile(X, Y, XL, YL, U, V, UL, VL, t, &info, masked, use_color, color);
                t->Unlock(info);
                std::printf("tile %d %08x %08x %08x %08x %08x %08x %08x %08x %d %d %d %d",
                            ci, bits(X), bits(Y), bits(XL), bits(YL), bits(U), bits(V), bits(UL), bits(VL),
                            tid, (int)masked, (int)use_color, (int)color);
                print_dest(s);
            } else {
                // DrawString over a random font with valid and out-of-range text.
                int fi = (int)(rnd() % NFONT);
                INT DrawX = (INT)(rnd() % 64) - 8;
                INT DrawY = (INT)(rnd() % 48) - 8;
                UBOOL use_color = (UBOOL)(rnd() % 2);
                BYTE entry = (BYTE)(rnd() & 0xff);
                int len = 1 + (int)(rnd() % 12);
                TCHAR text[32];
                for (int k = 0; k < len; ++k) {
                    uint32_t r = rnd() % 10;
                    text[k] = (TCHAR)(r < 8 ? 0x20 + rnd() % 0x60 : 0x80 + rnd() % 0x180);
                }
                text[len] = 0;
                INT lx = DrawString(s, g_font[fi], DrawX, DrawY, text, use_color, entry);
                std::printf("str %d %d %d %d %d %d %d", ci, (int)DrawX, (int)DrawY, fi,
                            (int)use_color, (int)entry, len);
                for (int k = 0; k < len; ++k) std::printf(" %d", (int)text[k]);
                std::printf(" %d", (int)lx);
                print_dest(s);
            }
        }
    }

    // PostLoad and Tick: source texture selection, palette copy, LocalSourceBitmap.
    for (int pi = 0; pi < NPL; ++pi) {
        int ub = 2 + (int)(rnd() % 5);
        int vb = 2 + (int)(rnd() % 5);
        UPalette* pal = g_pal[rnd() % NPAL];
        int n = (1 << ub) * (1 << vb);
        BYTE* init = (BYTE*)std::malloc(n);
        for (int i = 0; i < n; ++i) init[i] = (BYTE)(rnd() & 0xff);
        UScriptedTexture* s = make_scr(ub, vb, pal, init);
        std::printf("case %d %d %d %d", NCASE + pi, ub, vb, pal_id(pal));
        print_hex(init, n);
        std::printf("\n");
        std::free(init);

        for (int step = 0; step < 2; ++step) {
            int src = (rnd() % 5 == 0) ? -1 : (int)(rnd() % NTEX);
            s->SourceTexture = src < 0 ? NULL : g_tex[src];
            g_now = 1.0 + pi * 0.5 + step * 0.125;
            s->PostLoad();
            std::printf("pl %d %d", NCASE + pi, src);
            if (s->LocalSourceBitmap) {
                print_hex(s->LocalSourceBitmap, (int)s->USize * (int)s->VSize);
            } else {
                std::printf(" -");
            }
            std::printf(" %d\n", pal_id(s->Palette));
        }

        for (int step = 0; step < 2; ++step) {
            g_now = 4.0 + pi * 0.5 + step * 0.25;
            FLOAT d = frnd(0.0f, 0.5f);
            s->Tick(d);
            std::printf("tk %d %08x", NCASE + pi, bits(d));
            print_dest(s);
        }
    }
    return 0;
}
