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
    pandoc \
  && rm -rf /var/lib/apt/lists/*

# markitdown on PATH
RUN python3 -m pip install --break-system-packages --no-cache-dir 'markitdown[all]'

# office-direct (docx/xlsx/pptx). Build context must include ./office-direct.
COPY office-direct /tmp/office-direct
RUN python3 -m pip install --break-system-packages --no-cache-dir /tmp/office-direct \
  && rm -rf /tmp/office-direct

WORKDIR /opt/avrag-rs
