#!/usr/bin/env python3
# Shared by Build/*_fixture_harness/gen_*.sh.
#
#   fixture_common.py flags <cc_db> <repo-relative source> <repo> <work>
#       Print one TU's compile flags from compile_commands.json, with the repo root mapped to the
#       pristine work tree. Drops -c/-o/-MF/-MT/-O*/source. Shell-quoted, for `eval "c++ $FLAGS ..."`.
#
#   fixture_common.py stubs <syms.txt> <obj>...
#       Print C++ defining every undefined symbol in syms.txt that the objects do not define.
#       Functions (BR26 branches, or UNSIGND pointer slots whose demangled name has a parameter
#       list): abort stub that prints `STUB <name>` to stderr.
#       Data (vtables/typeinfo, GOT/PAGE loads, or UNSIGND-only addresses of non-functions):
#       zero-filled 1 MiB char arrays (16-aligned), matching what the static storage of an unseeded process holds.
#       Anything unclassifiable: report UNCLASSIFIED on stderr and exit 1.
import ctypes
import json
import re
import shlex
import subprocess
import sys

DATA_PREFIXES = ('_ZTV', '_ZTI', '_ZTS')
DATA_RELOCS = {'GOTLDP', 'GOTLDPOF', 'PAGE21', 'PAGOF12', 'PAGEOFF12'}
# Linker-synthesized (static-destructor registration); never a stub.
NEVER_STUB = {'__dso_handle'}
RELOC_PAT = re.compile(r'^[0-9a-f]+\s+(?:True|False)\s+.+?\s+(True|False)\s+(\S+?)\s*(?:True|False)\s+(\S+)$')


def cmd_flags(db, rel, repo, work):
    hits = [e for e in json.load(open(db)) if e['file'].endswith('/' + rel)]
    assert len(hits) == 1, (rel, len(hits))
    toks = shlex.split(hits[0]['command'].replace(repo, work))
    out, skip = [], False
    for t in toks:
        if skip:
            skip = False
            continue
        if t in ('-o', '-MF', '-MT'):
            skip = True
            continue
        if t in ('-MD', '-MMD', '-c') or t.startswith('-O') or t.endswith('.cpp') or t.endswith('c++'):
            continue
        out.append(t)
    print(' '.join(shlex.quote(t) for t in out))


def reloc_kinds(objs):
    kinds = {}
    for o in objs:
        txt = subprocess.run(['otool', '-rv', o], capture_output=True, text=True, check=True).stdout
        for line in txt.splitlines():
            m = RELOC_PAT.match(line.strip())
            if m and m.group(1) == 'True':
                sym = m.group(3)
                sym = sym[1:] if sym.startswith('_') else sym
                kinds.setdefault(sym, set()).add(m.group(2))
    return kinds


def cmd_stubs(symfile, objs):
    libc = ctypes.CDLL(None)
    names = [l.strip() for l in open(symfile)
             if l.strip() and l.strip() not in NEVER_STUB and not hasattr(libc, l.strip())]
    kinds = reloc_kinds(objs)
    dem_out = subprocess.run(['c++filt'], input='\n'.join(names), capture_output=True, text=True,
                             check=True).stdout.splitlines()
    dem = dict(zip(names, dem_out))
    funcs, datas, bad = [], [], []
    for s in names:
        t = kinds.get(s)
        if not t:
            bad.append((s, None))
        elif s.startswith(DATA_PREFIXES) or t & DATA_RELOCS:
            datas.append(s)
        elif t <= {'BR26', 'UNSIGND'} and ('BR26' in t or '(' in dem[s]):
            funcs.append(s)
        else:
            bad.append((s, sorted(t)))
    if bad:
        for s, t in bad:
            print(f'UNCLASSIFIED {s} relocs={t}', file=sys.stderr)
        sys.exit(1)
    out = ['#include <cstdio>\n#include <cstdlib>\n']
    for i, s in enumerate(funcs):
        out.append(f'extern "C" void hp2_stub_{i}(void) __asm__("_{s}");\n')
        out.append(f'extern "C" void hp2_stub_{i}(void) {{ std::fprintf(stderr, "STUB %s\\n", "{s}"); std::abort(); }}\n')
    # Undefined Mach-O data symbols carry no size here. Reserve aligned BSS for
    # large globals such as GMath's tables and the jmp_buf-carrying __Context::Env;
    # the old 64-byte slots were smaller than macOS arm64's 192-byte jmp_buf.
    for i, s in enumerate(datas):
        out.append(f'__attribute__((used, aligned(16))) char hp2_data_{i}[1 << 20] __asm__("_{s}");\n')
    print(''.join(out))
    print(f'stub list: {len(funcs)} function (abort), {len(datas)} data (zero char[1 MiB])', file=sys.stderr)
    for s in funcs:
        print(f'stub function {s}', file=sys.stderr)
    for s in datas:
        print(f'stub data {s}', file=sys.stderr)


if __name__ == '__main__':
    if len(sys.argv) >= 6 and sys.argv[1] == 'flags':
        cmd_flags(*sys.argv[2:6])
    elif len(sys.argv) >= 4 and sys.argv[1] == 'stubs':
        cmd_stubs(sys.argv[2], sys.argv[3:])
    else:
        sys.exit(__doc__)
