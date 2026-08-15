"""anydoc-lite: stdlib-only stand-in for the `anydoc` CLI.

Protocol (mirrors what the ingestion host expects from ANYDOC_BIN):
  argv[1] = input file
  argv[2] = output file the host reads back
  exit 0 on success, non-zero on failure

Coverage:
  - docx: zipfile + xml.etree walk of word/document.xml, one line per w:p
  - csv:  passed through as-is (UTF-8)
  - legacy binary Office formats (doc/xls/ppt/odt/...): not supported,
    exit 1 -- same failure mode as a missing parser today, no regression.
"""
import sys
import zipfile
import xml.etree.ElementTree as ET

_W = "{http://schemas.openxmlformats.org/wordprocessingml/2006/main}"


def _docx_text(path: str) -> str:
    with zipfile.ZipFile(path) as z:
        xml_bytes = z.read("word/document.xml")
    root = ET.fromstring(xml_bytes)
    lines = []
    for para in root.iter(_W + "p"):
        parts = [node.text or "" for node in para.iter(_W + "t")]
        line = "".join(parts).strip()
        if line:
            lines.append(line)
    return "\n".join(lines)


def main() -> int:
    if len(sys.argv) < 3:
        sys.stderr.write("usage: anydoc_lite.py <in> <out>\n")
        return 2
    src, dst = sys.argv[1], sys.argv[2]
    lower = src.lower()
    try:
        if lower.endswith(".docx"):
            text = _docx_text(src)
        elif lower.endswith(".csv"):
            with open(src, "rb") as f:
                text = f.read().decode("utf-8", errors="replace")
        else:
            sys.stderr.write("unsupported format: %s\n" % src)
            return 1
        with open(dst, "w", encoding="utf-8") as f:
            f.write(text)
            if text and not text.endswith("\n"):
                f.write("\n")
    except (OSError, zipfile.BadZipFile, ET.ParseError, KeyError) as e:
        sys.stderr.write("convert failed: %s\n" % e)
        return 1
    return 0


if __name__ == "__main__":
    sys.exit(main())
