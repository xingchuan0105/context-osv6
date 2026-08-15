"""markitdown-lite: stdlib-only stand-in for the `markitdown` CLI.

Protocol (mirrors what the ingestion host expects from MARKITDOWN_BIN):
  argv[1] = path to the source file
  stdout  = extracted markdown/text, UTF-8 encoded
  exit 0 on success, non-zero on failure

Coverage matches what the ingestion router sends to Markitdown:
  - text-like files (txt/md/rst/tsv/json/toml/yaml + code extensions):
    passed through as-is
  - html/htm: tags stripped via html.parser, script/style dropped,
    remaining text emitted one block per line
"""
import sys
from html.parser import HTMLParser


class _TextExtractor(HTMLParser):
    def __init__(self):
        super().__init__()
        self._skip_depth = 0
        self.chunks = []

    def handle_starttag(self, tag, attrs):
        if tag in ("script", "style"):
            self._skip_depth += 1
        elif self._skip_depth == 0 and tag in ("p", "br", "div", "li", "tr",
                                               "h1", "h2", "h3", "h4", "h5", "h6"):
            self.chunks.append("\n")

    def handle_endtag(self, tag):
        if tag in ("script", "style") and self._skip_depth > 0:
            self._skip_depth -= 1
        elif self._skip_depth == 0 and tag in ("p", "div", "li", "tr",
                                               "h1", "h2", "h3", "h4", "h5", "h6"):
            self.chunks.append("\n")

    def handle_data(self, data):
        if self._skip_depth == 0:
            self.chunks.append(data)


def main() -> int:
    if len(sys.argv) < 2:
        sys.stderr.write("usage: markitdown_lite.py <file>\n")
        return 2
    path = sys.argv[1]
    try:
        with open(path, "rb") as f:
            raw = f.read()
    except OSError as e:
        sys.stderr.write("read failed: %s\n" % e)
        return 1

    text = raw.decode("utf-8", errors="replace")
    lower = path.lower()
    if lower.endswith((".html", ".htm")):
        parser = _TextExtractor()
        try:
            parser.feed(text)
        except Exception:
            pass  # emit whatever was collected before the malformed bit
        lines = [" ".join(line.split()) for line in "".join(parser.chunks).split("\n")]
        text = "\n".join(line for line in lines if line)

    sys.stdout.reconfigure(encoding="utf-8")
    sys.stdout.write(text)
    if text and not text.endswith("\n"):
        sys.stdout.write("\n")
    return 0


if __name__ == "__main__":
    sys.exit(main())
