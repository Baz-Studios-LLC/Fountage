"""Regressions for failures observed on the actual Windows release runner."""
import importlib.util
import os
from pathlib import Path
import sys
import tempfile
import unittest
import zipfile

spec = importlib.util.spec_from_file_location("package_release", Path(__file__).with_name("package-release.py"))
release = importlib.util.module_from_spec(spec)
spec.loader.exec_module(release)


class PackagingTests(unittest.TestCase):
    def test_utf8_cargo_style_output_is_not_decoded_using_the_console_code_page(self):
        output = release.run(sys.executable, "-c", "import sys; sys.stdout.reconfigure(encoding='utf-8'); print('世界 e\\u0301')")
        self.assertEqual(output.strip(), "世界 e\u0301")

    def test_old_license_timestamp_and_unicode_name_preserve_exact_archive_contents(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            package = root / "Fountage-v001-windows-x64"
            package.mkdir()
            license_file = package / "License 世界.txt"
            body = "Copyright — café 世界\r\n".encode("utf-8")
            license_file.write_bytes(body)
            os.utime(license_file, (0, 0))
            archive = root / "release.zip"
            release.archive_directory(package, archive)
            with zipfile.ZipFile(archive) as zipped:
                name = f"{package.name}/{license_file.name}"
                self.assertEqual(zipped.read(name), body)
                self.assertEqual(zipped.getinfo(name).date_time[0], 1980)
                self.assertIsNone(zipped.testzip())


if __name__ == "__main__":
    unittest.main()
