#!/usr/bin/env python3
"""Focused canonical package-audit schema and payload projection contracts."""

from __future__ import annotations

import sys
import tempfile
import unittest
from pathlib import Path
from types import SimpleNamespace

REPOSITORY_ROOT = Path(__file__).resolve().parents[1]
sys.path.insert(0, str(REPOSITORY_ROOT / "Build"))

import package79_reference as p79  # noqa: E402


class _Resolver:
    @staticmethod
    def ref_path(reference: int) -> None:
        if reference != 0:
            raise AssertionError(f"unexpected reference {reference}")
        return None

    @staticmethod
    def export_path(index: int) -> str:
        if index != 0:
            raise AssertionError(f"unexpected export {index}")
        return "Synthetic.Foo"


class PackageAuditReferenceContracts(unittest.TestCase):
    def test_schema_and_known_payload_projection(self) -> None:
        payload = bytes(range(16))
        reader = p79.PackageReader("Synthetic.u", payload)
        parsed = SimpleNamespace(
            summary={
                "export_count": 1,
                "import_count": 0,
                "licensee": 0,
                "name_count": 1,
                "package_flags": 1,
                "version": 79,
                "import_offset": len(payload),
            },
            names=[SimpleNamespace(text="None")],
            exports=[
                SimpleNamespace(
                    class_ref=0,
                    serial_offset=0,
                    serial_size=len(payload),
                )
            ],
            resolver=_Resolver(),
            names_end=0,
        )

        audit = reader.audit_entry(parsed)

        self.assertEqual(p79.AUDIT_SCHEMA_VERSION, 2)
        self.assertEqual(audit["exports"][0]["payload"]["family"], "object")
        self.assertEqual(
            audit["exports"][0]["payload"]["sha256"],
            "be45cb2605bf36bebde684841a28f0fd43c69850a3dce5fedba69928ee3a8991",
        )
        self.assertEqual(
            audit["exports"][0]["payload"]["projection"],
            {
                "property_bytes": 1,
                "property_count": 0,
                "reader": "package79.properties",
                "status": "decoded",
                "trailing_bytes": 15,
            },
        )

    def test_unsupported_family_is_still_hash_censused(self) -> None:
        self.assertEqual(p79.non_package_family("System/readme.bin"), "binary")
        self.assertEqual(p79.non_package_family("System/Game.int"), "localization")
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            (root / "readme.bin").write_bytes(b"opaque")
            audit = p79.generate_audit(root, root)
        self.assertEqual(audit["schema_version"], 2)
        self.assertEqual(audit["non_package_payloads"][0]["family"], "binary")
        self.assertEqual(
            audit["non_package_payloads"][0]["projection"]["status"], "hash-only"
        )

    def test_malformed_dds_retains_dxt1_reader_identity(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            (root / "broken.dds").write_bytes(b"DDS ")
            audit = p79.generate_audit(root, root)
        self.assertEqual(
            audit["non_package_payloads"][0]["projection"],
            {"reader": "hp-format.dxt1", "status": "hash-only"},
        )


if __name__ == "__main__":
    unittest.main()
