"""Check the compiled stock inversion handler, not the .uc source.

Set HP2_RETAIL_HGAME to a verified retail HGame.u to check that package too.
This test describes the original defect; it is not a VM-repair test.
"""
import hashlib
import os
from pathlib import Path
import sys
import unittest

ROOT = Path(__file__).resolve().parents[1]
sys.path.insert(0, str(ROOT / "Build"))
from package79_reference import Cursor, PackageReader


def handler_references(path):
    data = path.read_bytes()
    reader = PackageReader(str(path), data)
    summary = reader.parse_summary()
    names, _ = reader.parse_names(summary)
    exports, _ = reader.parse_exports(summary, names)

    def export(name, outer=None):
        matches = [(i + 1, obj) for i, obj in enumerate(exports)
                   if names[obj.object_name_index].text == name
                   and (outer is None or obj.outer_ref == outer)]
        if len(matches) != 1:
            raise AssertionError(f"expected exactly one {name}: {len(matches)}")
        return matches[0]

    page, _ = export("FEInputPage")
    invert, _ = export("MouseInvertCheck", page)
    center, _ = export("AutoCenterCamCheck", page)
    _, function = export("MouseInvertChanged", page)
    blob = data[function.serial_offset:function.serial_offset + function.serial_size]
    literal = b"set ini:Engine.PlayerPawn bInvertMouse \x00"
    if blob.count(literal) != 1:
        raise AssertionError("expected exactly one inversion SET literal")
    command_tail = blob.split(literal, 1)[1]
    if command_tail[:3] != b"\x54\x19\x01":
        raise AssertionError("unexpected expression before checkbox reference")
    cursor = Cursor(command_tail, str(path), 3, "checkbox reference")
    actual = cursor.compact_index()
    return hashlib.sha256(data).hexdigest(), invert, center, actual


class StockInputBytecodeTests(unittest.TestCase):
    def check_package(self, path):
        digest, invert, center, actual = handler_references(path)
        self.assertNotEqual(invert, center)
        self.assertEqual(actual, center, f"unexpected compiled handler in {path}, sha256={digest}")

    def test_prototype_compiled_inversion_uses_center_checkbox(self):
        self.check_package(ROOT / "HarryPotter2/Unreal/System/HGame.u")

    def test_verified_retail_compiled_inversion_uses_center_checkbox(self):
        path = os.environ.get("HP2_RETAIL_HGAME")
        if not path:
            self.skipTest("HP2_RETAIL_HGAME is not set")
        self.check_package(Path(path))


if __name__ == "__main__":
    unittest.main()
