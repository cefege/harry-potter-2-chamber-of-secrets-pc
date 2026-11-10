#!/usr/bin/env python3
"""Summarize the main thread of a macOS `sample` file: top inclusive and self frames.

usage: sample_summary.py FILE [top_n]
"""
import re, sys, collections
sys.dont_write_bytecode = True
path = sys.argv[1]; top = int(sys.argv[2]) if len(sys.argv) > 2 else 35
lines = open(path, errors="replace").read().splitlines()
start = next(i for i, l in enumerate(lines) if l.startswith("Call graph:"))
rows = []  # (depth, count, name)
thread = 0
pat = re.compile(r"^(?P<pre>[\s+!:|]*?)(?P<n>\d+)\s+(?P<name>.+?)(?:\s+\(in (?P<lib>[^)]*)\))?(?:\s+\+\s+\d+.*)?$")
for l in lines[start + 1:]:
    if l.startswith("Total number in stack") or l.startswith("Sort by top of stack"): break
    if re.match(r"^\s+\d+\s+Thread_", l):
        thread += 1
        if thread > 1: break
        continue
    m = pat.match(l)
    if m and thread == 1:
        rows.append((len(m.group("pre")), int(m.group("n")), m.group("name") + (" [" + m.group("lib") + "]" if m.group("lib") else "")))
incl = collections.Counter(); selfc = collections.Counter()
stack = []
for i, (d, n, name) in enumerate(rows):
    while stack and stack[-1][0] >= d: stack.pop()
    stack.append((d, i))
    # self = n - sum of direct children
    j = i + 1; kids = 0
    while j < len(rows) and rows[j][0] > d:
        if rows[j][0] == d + 2: kids += rows[j][1]
        j += 1
    selfc[name] += max(0, n - kids)
    incl[name] += n  # may double count recursion; adequate for ranking
total = rows[0][1] if rows else 1
def short(s):
    s = re.sub(r"_R[A-Za-z]*N[vs]?[A-Za-z]*?(\d+)", "", s)  # leave mangled names mostly alone
    return s[:150]
print("main-thread samples:", total)
print("\n== inclusive (top) ==")
for name, n in incl.most_common(top): print(f"{n/total*100:5.1f}% {n:6d}  {short(name)}")
print("\n== self (top) ==")
for name, n in selfc.most_common(top): print(f"{n/total*100:5.1f}% {n:6d}  {short(name)}")

if len(sys.argv) > 3:
    needle = sys.argv[3]; maxdepth = int(sys.argv[4]) if len(sys.argv) > 4 else 6; minpct = float(sys.argv[5]) if len(sys.argv) > 5 else 1.0
    idx = next((i for i, r in enumerate(rows) if needle in r[2]), None)
    print(f"\n== subtree of first match for {needle!r} ==")
    if idx is not None:
        base = rows[idx][0]
        for d, n, name in rows[idx:]:
            if d < base or (d == base and (d, n, name) != rows[idx]): break
            if (d - base) // 2 > maxdepth or n / total * 100 < minpct: continue
            print("  " * ((d - base) // 2) + f"{n/total*100:5.1f}% {short(name)[:130]}")
