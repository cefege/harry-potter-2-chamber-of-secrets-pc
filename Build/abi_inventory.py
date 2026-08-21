#!/usr/bin/env python3
"""Generate a deterministic ABI-sensitive syntax inventory for original HP2 sources."""
from __future__ import annotations
import argparse, bisect, json, os, re, sys, tempfile
from collections import Counter
from pathlib import Path

MODULES = {"Core": ("Src", "Inc"), "Engine": ("Src", "Inc"), "Render": ("Src", "Inc"), "Fire": ("Src", "Inc"), "ALAudio": ("Src", "Inc"), "Launch": ("Src",), "UCC": ("Src",), "PackageTool": ("Src",)}
TREES = tuple(f"HarryPotter2/Unreal/{m}/{s}" for m, ss in MODULES.items() for s in ss)
SUFFIXES = frozenset({".c", ".cc", ".cpp", ".cxx", ".h", ".hh", ".hpp", ".hxx", ".inl"})
ARTIFACT_DIRS = frozenset({".git", "__pycache__", "binaries", "debug", "intermediate", "release", "win32", "x64"})
CLASSES = ("wire32", "wire16", "host-pointer", "reflected-host", "raw-layout")
IDENT = re.compile(r"[A-Za-z_]\w*")
WIRE16 = frozenset({"WORD", "_WORD", "SWORD", "UNICHAR", "UNICHARU", "int16_t", "uint16_t", "short"})
WIRE32 = frozenset({"DWORD", "LONG", "INT", "UBOOL", "BITFIELD", "NAME_INDEX", "FLOAT", "int32_t", "uint32_t", "int", "float"})
HOST_SIZE = frozenset({"SIZE_T", "SSIZE_T", "PTRINT", "UPTRINT", "size_t", "ssize_t", "intptr_t", "uintptr_t"})
CAST_TYPES = WIRE16 | WIRE32 | HOST_SIZE | frozenset({"BYTE", "SBYTE", "QWORD", "SQWORD", "char", "signed", "unsigned", "__int64"})
QUALIFIERS = frozenset({"const", "volatile", "signed", "unsigned", "short", "long", "int", "char", "float", "double", "struct", "class", "enum"})
POINTER_NAME = re.compile(r"(?:ptr|pointer|handle|address|addr|buffer|data|memory|mem|base|top|vtable|object)$", re.I)


def normalize(text):
    return " ".join(text.strip().split())


def source_files(root):
    found = []
    for rel in TREES:
        tree = root / rel
        if not tree.is_dir():
            raise ValueError(f"required source tree is missing: {rel}")
        for path in tree.rglob("*"):
            if path.is_file() and path.suffix.lower() in SUFFIXES and not any(p.lower() in ARTIFACT_DIRS for p in path.relative_to(tree).parts[:-1]):
                found.append(path)
    found.sort(key=lambda p: p.relative_to(root).as_posix())
    if not found:
        raise ValueError("no source candidates found")
    return found


def decode_source(path):
    data = path.read_bytes()
    if b"\0" in data:
        raise ValueError(f"NUL byte in source candidate: {path}")
    for encoding in ("utf-8", "cp1252"):
        try:
            return data.decode(encoding), encoding
        except UnicodeDecodeError:
            pass
    raise ValueError(f"source is neither strict UTF-8 nor strict Windows-1252: {path}")


def mask_source(text, name):
    out, i = list(text), 0
    while i < len(text):
        if text.startswith("//", i):
            end = text.find("\n", i + 2); end = len(text) if end < 0 else end
            out[i:end] = " " * (end - i); i = end
        elif text.startswith("/*", i):
            end = text.find("*/", i + 2)
            if end < 0: raise ValueError(f"unterminated block comment in {name}")
            for j in range(i, end + 2):
                if text[j] != "\n": out[j] = " "
            i = end + 2
        elif text[i] in ("'", '"'):
            quote, start = text[i], i; out[i] = " "; i += 1
            while i < len(text):
                if text[i] == "\\":
                    out[i] = " "; i += 1
                    if i >= len(text): raise ValueError(f"truncated escape in {name}")
                    if text[i] != "\n": out[i] = " "
                    i += 1
                elif text[i] == quote:
                    out[i] = " "; i += 1; break
                elif text[i] == "\n": i += 1
                else: out[i] = " "; i += 1
            else: raise ValueError(f"unterminated literal at offset {start} in {name}")
        else: i += 1
    return "".join(out)


def match_close(text, opening, left, right, name):
    depth = 0
    for i in range(opening, len(text)):
        if text[i] == left: depth += 1
        elif text[i] == right:
            depth -= 1
            if depth == 0: return i
    raise ValueError(f"unmatched {left!r} at offset {opening} in {name}")


def typedef_end(masked, start, name):
    depths = {"(": 0, "[": 0, "{": 0}; closes = {")": "(", "]": "[", "}": "{"}
    for i in range(start, len(masked)):
        c = masked[i]
        if c in depths: depths[c] += 1
        elif c in closes:
            left = closes[c]
            if not depths[left]: raise ValueError(f"unbalanced typedef at offset {start} in {name}")
            depths[left] -= 1
        elif c == ";" and not any(depths.values()): return i
    raise ValueError(f"unterminated typedef at offset {start} in {name}")


def words(text): return set(IDENT.findall(text))


def classify_typedef(text, aliases):
    declared = typedef_names(text)
    tokens = words(text) - declared
    if "*" in text or tokens & HOST_SIZE or any(t.endswith("HANDLE") for t in tokens) or any(name in HOST_SIZE or name.endswith("HANDLE") for name in declared): return "host-pointer"
    if "long" in tokens: return "raw-layout"
    classes = set()
    if tokens & WIRE16 or any(aliases.get(t) == "wire16" for t in tokens): classes.add("wire16")
    if tokens & WIRE32 or any(aliases.get(t) == "wire32" for t in tokens): classes.add("wire32")
    return classes.pop() if len(classes) == 1 else "raw-layout"


def typedef_names(text):
    body = re.sub(r"^\s*typedef\b", "", text).rstrip("; \\t\r\n")
    result = set(re.findall(r"\(\s*\*\s*([A-Za-z_]\w*)", body))
    tail = body.rsplit("}", 1)[-1] if "}" in body else body
    for part in tail.split(","):
        ids = IDENT.findall(part)
        if ids and ids[-1] not in QUALIFIERS: result.add(ids[-1])
    return result


def relevant_target(target, aliases):
    tokens = words(target)
    return "*" in target or bool(tokens & CAST_TYPES) or any(aliases.get(t) in {"wire16", "wire32", "host-pointer"} for t in tokens)


def c_cast_target(target, aliases):
    target = target.strip()
    if not target or not re.fullmatch(r"[A-Za-z_][\w:\s*&]*", target) or not relevant_target(target, aliases): return False
    tokens = IDENT.findall(target)
    if "*" in target:
        return all(t in {"const", "volatile"} for t in IDENT.findall(target.rsplit("*", 1)[1]))
    return any(t in CAST_TYPES or t in aliases for t in tokens) and all(t in QUALIFIERS or t in CAST_TYPES or t in aliases for t in tokens)


def classify_cast(target, expression, aliases):
    tokens, operand = words(target), expression.lstrip()
    if "*" in target or tokens & HOST_SIZE or any(aliases.get(t) == "host-pointer" for t in tokens): return "host-pointer"
    first_match = IDENT.match(operand); first = first_match.group(0) if first_match else ""
    if operand.startswith(("&", "*")) or first in {"this", "NULL", "nullptr"} or POINTER_NAME.search(first): return "host-pointer"
    if not operand or (first and operand == first): return "raw-layout"
    if tokens & WIRE16 or any(aliases.get(t) == "wire16" for t in tokens): return "wire16"
    if tokens & WIRE32 or any(aliases.get(t) == "wire32" for t in tokens): return "wire32"
    return "raw-layout"


def make_record(path, line_starts, offset, kind, text, expression, classification):
    line_index = bisect.bisect_right(line_starts, offset) - 1
    line, column = line_index + 1, offset - line_starts[line_index] + 1
    return {"classification": classification, "column": column, "expression": normalize(expression), "id": f"{kind}:{path}:{line}:{column}", "kind": kind, "line": line, "path": path, "text": normalize(text)}


def scan(root, files):
    records, sources, aliases, encodings = [], [], {}, Counter()
    typedef_re, cpp_re, serialize_re, sizeof_re = (re.compile(p) for p in (r"\btypedef\b", r"\bCPP_PROPERTY\b", r"\bSerialize\b", r"\bsizeof\b"))
    for file in files:
        path = file.relative_to(root).as_posix(); text, encoding = decode_source(file); encodings[encoding] += 1
        masked = mask_source(text, path); lines = [0] + [m.end() for m in re.finditer("\n", text)]; sources.append((file, text, masked))
        for m in typedef_re.finditer(masked):
            end = typedef_end(masked, m.end(), path); evidence = text[m.start():end + 1]; classification = classify_typedef(evidence, aliases)
            records.append(make_record(path, lines, m.start(), "typedef", evidence, evidence, classification))
            for name in typedef_names(evidence): aliases[name] = classification if name not in aliases or aliases[name] == classification else "raw-layout"
        for m in cpp_re.finditer(masked):
            opening = m.end()
            while opening < len(masked) and masked[opening].isspace(): opening += 1
            if opening >= len(masked) or masked[opening] != "(": raise ValueError(f"CPP_PROPERTY without arguments in {path}")
            closing = match_close(masked, opening, "(", ")", path)
            records.append(make_record(path, lines, m.start(), "cpp-property", text[m.start():closing + 1], text[opening + 1:closing], "reflected-host"))
        for m in serialize_re.finditer(masked):
            opening = m.end()
            while opening < len(masked) and masked[opening].isspace(): opening += 1
            if opening >= len(masked) or masked[opening] != "(": continue
            closing = match_close(masked, opening, "(", ")", path)
            if sizeof_re.search(masked[opening + 1:closing]): records.append(make_record(path, lines, m.start(), "serialize-sizeof", text[m.start():closing + 1], text[opening + 1:closing], "raw-layout"))
    named_re = re.compile(r"\b(?:reinterpret_cast|static_cast|const_cast|dynamic_cast)\s*<"); simple_re = re.compile(r"\(([^()\n]{1,192})\)")
    for file, text, masked in sources:
        path = file.relative_to(root).as_posix(); lines = [0] + [m.end() for m in re.finditer("\n", text)]; occupied = []
        for m in named_re.finditer(masked):
            left = masked.find("<", m.start(), m.end()); right = match_close(masked, left, "<", ">", path); opening = right + 1
            while opening < len(masked) and masked[opening].isspace(): opening += 1
            if opening >= len(masked) or masked[opening] != "(": raise ValueError(f"named cast without operand in {path}")
            closing = match_close(masked, opening, "(", ")", path); target, expression = text[left + 1:right], text[opening + 1:closing]
            if relevant_target(target, aliases): records.append(make_record(path, lines, m.start(), "cast", text[m.start():closing + 1], expression, classify_cast(target, expression, aliases))); occupied.append((m.start(), closing + 1))
        for m in simple_re.finditer(masked):
            if any(begin <= m.start() < end for begin, end in occupied): continue
            target = text[m.start() + 1:m.end() - 1]
            if not c_cast_target(target, aliases): continue
            start = m.end()
            while start < len(text) and text[start].isspace(): start += 1
            if start >= len(text) or text[start] in ",;)}]": continue
            end = start
            if start < len(masked) and masked[start] == "(": end = match_close(masked, start, "(", ")", path) + 1
            else:
                token = re.match(r"(?:[&*+!~-]\s*)*(?:[A-Za-z_]\w*|0[xX][0-9A-Fa-f]+|\d+(?:\.\d*)?)", text[start:])
                if token: end += token.end()
            expression = text[start:end] or text[start:start + 1]
            records.append(make_record(path, lines, m.start(), "cast", text[m.start():max(m.end(), end)], expression, classify_cast(target, expression, aliases)))
    return records, encodings


def generate(root):
    files = source_files(root); records, encodings = scan(root, files)
    records.sort(key=lambda r: (r["path"], r["line"], r["column"], r["kind"], r["text"]))
    ids = [r["id"] for r in records]
    if len(ids) != len(set(ids)): raise ValueError("inventory IDs are not unique")
    by_class, by_kind = Counter(r["classification"] for r in records), Counter(r["kind"] for r in records)
    return {
        "counts": {"by_classification": {c: by_class.get(c, 0) for c in CLASSES}, "by_kind": dict(sorted(by_kind.items())), "files_by_encoding": dict(sorted(encodings.items())), "files_scanned": len(files), "records": len(records)},
        "metadata": {
            "allowed_classifications": list(CLASSES),
            "classification_rules": {
                "wire32": "Typedefs and unambiguously numeric casts using fixed historical 32-bit aliases, int, or float; declared alias names never classify their own underlying type.",
                "wire16": "Typedefs and unambiguously numeric casts using fixed historical 16-bit aliases or short; declared alias names never classify their own underlying type.",
                "host-pointer": "Typedefs or casts with pointer syntax, host-size aliases, HANDLE-like types, or deterministic pointer-shaped operands/names.",
                "reflected-host": "Every CPP_PROPERTY occurrence because it computes a rebuilt host-member offset.",
                "raw-layout": "Serialize calls containing sizeof, host-dependent long typedefs, typedefs with no single fixed-width rule, and syntactically ambiguous cast operands."
            },
            "detection_rules": [
                "Decode source as strict UTF-8 with strict Windows-1252 fallback; reject NULs and unterminated comments/literals.",
                "Mask comments and literals without moving offsets; record every typedef through its balanced top-level semicolon.",
                "Remove declared typedef names before classifying the underlying type; classify host-dependent long as raw-layout.",
                "Record every balanced CPP_PROPERTY occurrence and every balanced Serialize call containing sizeof.",
                "Record named and C-style casts whose destination is explicit pointer/host-size/historical integer syntax or a typedef classified that way.",
                "Pointer-shaped syntax wins; an ambiguous single-identifier cast operand is raw-layout rather than omitted."
            ],
            "excluded_artifacts": {"directory_names": sorted(ARTIFACT_DIRS), "rule": "Only source-like suffixes under configured Src/Inc trees are scanned; binary/non-C++ artifacts and artifact directories are excluded."},
            "path_format": "repository-relative POSIX", "schema_version": 1, "source_suffixes": sorted(SUFFIXES), "source_trees": list(TREES)
        },
        "records": records
    }


def write_json(path, value):
    path.parent.mkdir(parents=True, exist_ok=True); payload = json.dumps(value, ensure_ascii=False, indent=2, sort_keys=True) + "\n"
    fd, temporary = tempfile.mkstemp(prefix=f".{path.name}.", dir=path.parent)
    try:
        with os.fdopen(fd, "w", encoding="utf-8", newline="\n") as stream: stream.write(payload)
        os.replace(temporary, path)
    except BaseException:
        try: os.unlink(temporary)
        except FileNotFoundError: pass
        raise


def main(argv=None):
    parser = argparse.ArgumentParser(description=__doc__); parser.add_argument("--repo-root", type=Path, default=Path(__file__).resolve().parents[1]); parser.add_argument("--output", type=Path); args = parser.parse_args(argv)
    root = args.repo_root.resolve(); output = args.output or root / "Tests/Fixtures/abi-inventory.json"; output = output if output.is_absolute() else root / output
    inventory = generate(root); write_json(output, inventory); print(f"wrote {inventory['counts']['records']} ABI records to {output}"); return 0

if __name__ == "__main__":
    try: raise SystemExit(main())
    except (OSError, ValueError) as error: print(f"abi_inventory.py: error: {error}", file=sys.stderr); raise SystemExit(1)
