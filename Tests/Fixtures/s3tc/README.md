# s3tc/encode-v1.bin

Generator: pinned libSquish `CompressMasked` with flags
`kDxt1 | kColourClusterFit` (0x09), exactly as
`HarryPotter2/Unreal/Engine/Src/S3tcCompat.cpp` calls it through
`hp2_s3tc_compress_block`. Captured from the native build before any native
deletion for the Rust DXT1 encoder's acceptance.

Layout: 4096 records of 74 bytes each (64 RGBA + 2 little-endian mask
+ 8 DXT1 block), contiguous, no per-record padding. Total 303,104 bytes.

Inputs: xorshift64* with seed `0x0123456789abcdefULL`; byte `j` of the 64
RGBA pixels is `next() & 0xff`. The 16-bit mask cycles the fixed pattern
`{0xffff, 0x00ff, 0xff00, 0x5555, 0xaaaa, 0x000f, 0xf000, 0x0f0f}` by
record index `i & 7`. No metric pointer is supplied.

Output: the 8-byte DXT1 block big-endian-laid as written by libSquish,
then the two endpoint RGB565 values and the four index bytes, in that
on-disk order.

SHA-256: 1f904b8b2ff29bb4a2151dcdee1d9a6c317f4a79678a4bddc7b2d9596603eb0f

The throwaway generator was `capture_s3tc.cpp` in the canonical boot
artifact directory; it is not part of the shipped tree. Regeneration is
possible by recompiling that file against the pinned libSquish source in
`out/macos-arm64/_deps/hp2_squish-src`.