/* Dumps utf8proc's UTF8PROC_CASEFOLD mapping for every Unicode scalar
 * value (U+0000..U+10FFFF, excluding the surrogate range U+D800..U+DFFF,
 * which isn't a valid scalar value and can't be encoded as UTF-8) as one
 * "<codepoint hex> <folded UTF-8 bytes as hex, or ERR>" line per
 * codepoint.
 *
 * Regenerating crates/hp2-import/src/fold_table.rs:
 *
 *   1. Check out utf8proc at the commit pinned in
 *      Build/CMake/HP2Dependencies.cmake's hp2_utf8proc FetchContent
 *      declaration (at the time this was written:
 *      79cdd5ab40c79afa559f689ffc13b76812dee1ac, utf8proc 2.12.0,
 *      Unicode 18.0) -- or, equivalently, point -I/the link step at an
 *      already-configured build's out/<preset>/_deps/hp2_utf8proc-src
 *      and .../hp2_utf8proc-build/libutf8proc.a.
 *   2. cc -O2 -std=c17 -I<utf8proc checkout> dump_utf8proc_casefold.c \
 *        <utf8proc checkout or build>/libutf8proc.a -o dump
 *   3. ./dump > cpp_table.txt
 *   4. python3 generate_fold_table.py cpp_table.txt \
 *        > ../src/fold_table.rs
 *
 * See crates/hp2-import/src/lower.rs's module doc for why this table
 * exists instead of linking utf8proc into the Rust side directly (the
 * plan specifies pure-Rust casefolding; this generator is how the
 * resulting table stays provably exact against the pinned library
 * without actually linking it).
 */
#include <utf8proc.h>
#include <stdio.h>

int main(void) {
    for (utf8proc_int32_t cp = 0; cp <= 0x10FFFF; ++cp) {
        if (cp >= 0xD800 && cp <= 0xDFFF) {
            continue;
        }
        unsigned char buf[4];
        utf8proc_ssize_t n = utf8proc_encode_char(cp, buf);
        if (n <= 0) {
            continue;
        }
        utf8proc_uint8_t *folded = NULL;
        utf8proc_ssize_t size = utf8proc_map(buf, n, &folded, UTF8PROC_CASEFOLD);
        if (size < 0) {
            printf("%06x ERR\n", cp);
            continue;
        }
        printf("%06x ", cp);
        for (utf8proc_ssize_t i = 0; i < size; ++i) {
            printf("%02x", folded[i]);
        }
        printf("\n");
        utf8proc_free(folded);
    }
    return 0;
}
