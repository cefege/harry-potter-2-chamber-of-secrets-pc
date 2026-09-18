#!/usr/bin/env python3
"""Focused byte fixtures and a read-only HPParticle Fire1 probe contract."""

from __future__ import annotations

import struct
import sys
import unittest
from pathlib import Path
from types import SimpleNamespace

REPOSITORY_ROOT = Path(__file__).resolve().parent.parent
sys.path.insert(0, str(REPOSITORY_ROOT / "Build"))

import fire_texture_tail_probe as probe  # noqa: E402
import package79_reference as p79  # noqa: E402


class FireTextureTailProbeContracts(unittest.TestCase):
    def _decode(self, payload: bytes, has_comp_mips: bool = False) -> dict[str, object]:
        reader = p79.PackageReader("fire-tail-fixture.utx", payload)
        parsed = SimpleNamespace(
            exports=[SimpleNamespace(serial_offset=0, serial_size=len(payload), record_offset=0)]
        )
        return probe.decode_fire_texture_tail(reader, parsed, 0, 0, has_comp_mips)

    @staticmethod
    def _mip(data: bytes, u_size: int, v_size: int, u_bits: int, v_bits: int, lazy_skip: int = -1) -> bytes:
        return (
            struct.pack("<i", lazy_skip)
            + p79.encode_compact_index(len(data))
            + data
            + struct.pack("<iiBB", u_size, v_size, u_bits, v_bits)
        )

    def test_primary_mip_and_sparks_follow_native_wire_order(self) -> None:
        payload = (
            p79.encode_compact_index(1)
            + self._mip(b"\x10\x20\x30\x40", 2, 2, 1, 1)
            + p79.encode_compact_index(2)
            + bytes((0, 200, 1, 2, 3, 4, 5, 6))
            + bytes((4, 100, 7, 8, 9, 10, 11, 12))
        )

        report = self._decode(payload)

        self.assertEqual(report["dimensions"], {"u_size": 2, "v_size": 2})
        self.assertEqual(report["mip_count"], 1)
        self.assertEqual(report["compressed_mip_count"], 0)
        self.assertEqual(report["spark_count"], 2)
        self.assertEqual(
            report["spark_list"],
            [
                {"type": 0, "heat": 200, "x": 1, "y": 2, "byte_a": 3, "byte_b": 4, "byte_c": 5, "byte_d": 6},
                {"type": 4, "heat": 100, "x": 7, "y": 8, "byte_a": 9, "byte_b": 10, "byte_c": 11, "byte_d": 12},
            ],
        )
        self.assertEqual(
            report["spark_types"],
            [
                {"value": 0, "name": "SPARK_Burn", "count": 1},
                {"value": 4, "name": None, "count": 1},
            ],
        )
        self.assertTrue(report["has_spark_burn"])

    def test_compressed_mip_block_precedes_spark_tail(self) -> None:
        payload = (
            p79.encode_compact_index(1)
            + self._mip(b"\x01", 1, 1, 0, 0)
            + p79.encode_compact_index(1)
            + self._mip(b"\x02\x03", 4, 4, 2, 2)
            + p79.encode_compact_index(1)
            + bytes((8, 20, 30, 40, 50, 60, 70, 80))
        )

        report = self._decode(payload, has_comp_mips=True)

        self.assertEqual(report["mip_count"], 1)
        self.assertEqual(report["compressed_mip_count"], 1)
        self.assertEqual(report["spark_count"], 1)
        self.assertEqual(report["spark_list"][0]["type"], 8)  # type: ignore[index]
        self.assertFalse(report["has_spark_burn"])

    def test_spark_count_cannot_exceed_remaining_eight_byte_records(self) -> None:
        payload = p79.encode_compact_index(0) + p79.encode_compact_index(2) + bytes(8)

        with self.assertRaisesRegex(p79.PackageFormatError, r"FSpark count 2 exceeds remaining tail bytes 8"):
            self._decode(payload)

    @unittest.skipUnless(probe.DEFAULT_ARCHIVE.is_file(), "HPParticle.u is not installed under the prototype data root")
    def test_hp_particle_fire1_read_only_probe(self) -> None:
        report = probe.probe_archive(probe.DEFAULT_ARCHIVE)

        self.assertEqual(report["format"], probe.REPORT_FORMAT)
        self.assertEqual(report["export"]["path"].casefold(), probe.TARGET_EXPORT_PATH)  # type: ignore[index]
        self.assertEqual(report["class"], "Fire.FireTexture")
        self.assertEqual(report["export"]["index"], probe.TARGET_EXPORT_INDEX)  # type: ignore[index]
        self.assertIsInstance(report["archive"]["sha256"], str)  # type: ignore[index]
        self.assertEqual(len(report["archive"]["sha256"]), 64)  # type: ignore[index]
        self.assertEqual(report["dimensions"], {"u_size": 128, "v_size": 128})
        self.assertIn("ref", report["palette_ref"])  # type: ignore[operator]
        self.assertEqual(report["spark_count"], 105)
        self.assertEqual(report["spark_types"], [{"value": 0, "name": "SPARK_Burn", "count": 105}])
        self.assertTrue(report["has_spark_burn"])


if __name__ == "__main__":
    unittest.main()
