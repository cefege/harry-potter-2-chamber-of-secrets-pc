#include "EnginePrivate.h"
#include <cstdio>
#include <cstdlib>
#include <cstring>
int main() {
    static const int vals[] = {0, 1, 100, 32767, 32768, 65535, 65536, -1, 70000, 40000, 20000, 2147483647, -2147483647 - 1};
    static const int rates[] = {0, 1, -1, 50, -50, 32768, -32768, 65535, -65535, 1000, -1000, 3, -3, 65536, -65536, 2147483647, 2147483646, -2147483646};
    const int nv = sizeof(vals)/sizeof(vals[0]);
    const int nr = sizeof(rates)/sizeof(rates[0]);
    AActor* a = (AActor*)calloc(1, sizeof(AActor));
    int count = 0;
    for (int fd = 0; fd < 2; ++fd)
    for (int rd = 0; rd < 2; ++rd) {
        a->bFixedRotationDir = fd;
        a->bRotateToDesired = rd;
        for (int c = 0; c < nv; ++c)
        for (int d = 0; d < nv; ++d)
        for (int r = 0; r < nr; ++r) {
            int out = a->fixedTurn(vals[c], vals[d], rates[r]);
            printf("%d %d %d %d %d %d\n", vals[c], vals[d], rates[r], fd, rd, out);
            ++count;
        }
    }
    fprintf(stderr, "cases %d\n", count);
    return 0;
}
