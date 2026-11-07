"""Generate synthetic EA-XA v1 reference vectors using an external vgmstream CLI.

The output is diagnostic evidence, not an automatic fixture update. No retail
game assets are required or included. HP2_ARTIFACT_DIR must exist.
"""
import array
import hashlib
import json
import os
from pathlib import Path
import subprocess
import sys
import tempfile
import wave

PACKED = bytes.fromhex("817f2ed34cb56a9908f127c45bae")


def generate(executable: Path, directory: Path, name: str, encoded: bytes, count: int):
    source = directory / (name + ".vgmstream")
    source.write_bytes(encoded)
    source.with_name(source.name + ".txth").write_text(
        f"codec = EAXA\nchannels = 1\nsample_rate = 22050\nnum_samples = {count}\n",
        encoding="ascii")
    output = directory / (name + ".wav")
    subprocess.run([str(executable), "-o", str(output), str(source)], check=True,
                   stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)
    with wave.open(str(output), "rb") as wav:
        assert (wav.getnchannels(), wav.getsampwidth(), wav.getframerate()) == (1, 2, 22050)
        raw = wav.readframes(wav.getnframes())
    samples = array.array("h")
    samples.frombytes(raw)
    if sys.byteorder != "little":
        samples.byteswap()
    if len(samples) != count:
        raise ValueError(f"{name}: expected {count} samples, got {len(samples)}")
    value = 14695981039346656037
    for byte in raw:
        value = ((value ^ byte) * 1099511628211) & ((1 << 64) - 1)
    return {"fnv1a64": f"{value:016x}", "samples": list(samples)}


def main():
    if len(sys.argv) != 2:
        raise SystemExit("usage: GenerateEaxaV1Oracle.py <vgmstream-cli>")
    exe = Path(sys.argv[1]).resolve(strict=True)
    artifacts = Path(os.environ["HP2_ARTIFACT_DIR"]).resolve(strict=True)
    with tempfile.TemporaryDirectory(prefix="eaxa-oracle-", dir=artifacts) as temporary:
        tmp = Path(temporary)
        result = {
            "generator_sha256": hashlib.sha256(exe.read_bytes()).hexdigest(),
            "predictors": [generate(exe, tmp, f"predictor-{n}", bytes([(n << 4) | 7]) + PACKED, 28) for n in range(16)],
            "ranges": [generate(exe, tmp, f"range-{n}", bytes([0x20 | n]) + PACKED, 28) for n in range(16)],
            "signed": generate(exe, tmp, "signed", bytes([0x0c]) + PACKED, 28),
            "history": generate(exe, tmp, "history", bytes([0x04] + [0x77] * 14 + [0x1c] + [0] * 14), 56),
        }
    destination = artifacts / "eaxa-v1-oracle.json"
    destination.write_text(json.dumps(result, indent=2) + "\n", encoding="utf-8")
    print(f"Wrote synthetic reference vectors to {destination}")


if __name__ == "__main__":
    main()
