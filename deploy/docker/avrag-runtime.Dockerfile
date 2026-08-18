# Minimal runtime for avrag-api / avrag-worker (host network).
# Binaries are bind-mounted from /opt/avrag-rs; this image supplies glibc,
# TLS, and document-parser CLIs the worker shells out to.
FROM ubuntu:24.04

ENV DEBIAN_FRONTEND=noninteractive

RUN apt-get update -qq \
  && apt-get install -y -qq --no-install-recommends \
    ca-certificates \
    libssl3 \
    curl \
    python3 \
    python3-pip \
    python3-venv \
  && rm -rf /var/lib/apt/lists/*

# markitdown on PATH (text/code long-tail)
RUN python3 -m pip install --break-system-packages --no-cache-dir 'markitdown[all]'

# anydoc-extract (Office/ODF/RTF/EPUB/CSV). Build context must include ./anydoc-extract.
COPY anydoc-extract /tmp/anydoc-extract
RUN python3 -m pip install --break-system-packages --no-cache-dir /tmp/anydoc-extract \
  && rm -rf /tmp/anydoc-extract

# lit (liteparse PDF CLI) + official pdfium runtime lib (scanned PDFs fall back
# to remote PaddleOCR, so the tesseract feature is stripped from this build).
COPY lit /usr/local/bin/lit
COPY libpdfium.so /usr/local/lib/libpdfium.so
RUN chmod 755 /usr/local/bin/lit && ldconfig

WORKDIR /opt/avrag-rs
