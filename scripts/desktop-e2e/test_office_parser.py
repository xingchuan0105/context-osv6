"""Opt-in packaged Windows Office parser gate, without desktop or model calls."""
import os
from pathlib import Path
import subprocess
import unittest

ROOT = Path(os.environ["GPUI_OFFICE_REPORT_DIR"])
FIXTURES = ROOT / "fixtures"


class PackagedOfficeParser(unittest.TestCase):
    def convert(self, filename, markers=(), *, installed=False, fail=False):
        layout = ROOT / ("installed" if installed else "staged")
        command = layout / "runtime/parsers/anydoc-extract.cmd"
        output = ROOT / "output" / (self._testMethodName + ".md")
        output.parent.mkdir(exist_ok=True)
        # Pass argv to the actual shipped .cmd contract, with console creation
        # disabled. No Tauri/GPUI window, system input, API or provider is used.
        result = subprocess.run(
            [str(command), str(FIXTURES / filename), str(output)],
            capture_output=True, timeout=30,
            creationflags=subprocess.CREATE_NO_WINDOW,
        )
        (ROOT / "output" / (self._testMethodName + ".stderr")).write_bytes(result.stderr)
        if fail:
            self.assertNotEqual(result.returncode, 0)
            self.assertFalse(output.exists(), "failed parser produced a success artifact")
            return
        self.assertEqual(result.returncode, 0, result.stderr.decode("utf-8", errors="replace"))
        text = output.read_text(encoding="utf-8")
        for marker in markers:
            self.assertIn(marker, text)
        return text

    def test_docx(self):
        self.convert("phase0-mini.docx", ["Context-OS phase0 mini docx fixture"])

    def test_xlsx_keeps_row_values(self):
        self.convert("inventory.xlsx", ["Cedar", "137"])

    def test_pptx(self):
        self.convert("delivery.pptx", ["Violet Harbor"])

    def test_unicode_csv_and_spaces(self):
        text = self.convert("中文 资料.csv", ["货品", "余量", "杉木", "137"])
        self.assertNotIn("\ufffd", text)

    def test_corrupt_office_fails(self):
        self.convert("broken.docx", fail=True)

    def test_missing_input_fails(self):
        self.convert("missing.xlsx", fail=True)

    def test_installed_docx(self):
        self.convert("phase0-mini.docx", ["Context-OS phase0 mini docx fixture"], installed=True)

    def test_installed_xlsx(self):
        self.convert("inventory.xlsx", ["Cedar", "137"], installed=True)

    def test_installed_pptx(self):
        self.convert("delivery.pptx", ["Violet Harbor"], installed=True)


if __name__ == "__main__":
    if os.name != "nt" or not (ROOT / "acceptance.marker").is_file():
        raise SystemExit("requires accept-office-parser.ps1 isolated Windows package")
    unittest.main(verbosity=2)
