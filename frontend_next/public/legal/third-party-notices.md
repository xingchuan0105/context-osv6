# Third-Party Notices

_Generated: 2026-08-05 via `scripts/generate-third-party-notices.sh`_

This project (Context-OS / AVRag) is licensed under the [MIT License](LICENSE).
Third-party components listed below are subject to their own licenses.

## Commercial deployment checklist

| Priority | Component | License | Action |
|----------|-----------|---------|--------|
| P1 | MinIO (upload / Milvus compose) | AGPL-3.0 | Prefer cloud S3/OSS via `S3_*` env vars |
| P1 | Redis **server 7.4+** (Linux SaaS/docker) | RSALv2 / SSPL | Internal cache only; pin ≤7.2 **or** use Valkey. **Not** the desktop Windows pin (see below). |
| P2 | `@img/sharp-libvips-linux-x64` (Next.js web) | LGPL-3.0 | NOTICE only; desktop build uses `images.unoptimized` |
| P2 | `cssparser` / `selectors` (via `scraper`) | MPL-2.0 | NOTICE; share file changes only if you modify MPL files |
| P2 | `dompurify` | MPL-2.0 OR Apache-2.0 | Compliance: choose Apache-2.0 |
| P2 | `markitdown[all]` transitive extras | varies | Worker image installs Microsoft markitdown (MIT core). Review extras on upgrade; do **not** reintroduce AGPL PDF stacks (e.g. PyMuPDF) without a separate legal review. |

## Runtime infrastructure (not npm/cargo)

| Component | Typical license | Notes |
|-----------|-----------------|-------|
| PostgreSQL | PostgreSQL License | Server DB (SaaS/local) and **desktop bundled** EDB Windows binaries |
| pgvector | PostgreSQL License | Extension for `RETRIEVAL_BACKEND=pgvector`; also in desktop portable runtime |
| Milvus | Apache-2.0 | Optional dense backend (`RETRIEVAL_BACKEND=milvus`) |
| etcd | Apache-2.0 | Bundled with Milvus compose |
| Redis (Linux server) | See checklist | Prefer Valkey or pre-SSPL pin for SaaS |
| Paddle OCR Jobs | API Terms of Service | External SaaS, not open source |
| LLM / Embedding / Search providers | API Terms of Service | DeepSeek, DashScope/SiliconFlow, Brave, etc. |

## Document parsers (worker / avrag-runtime image)

Baked into `avrag-runtime` (see `deploy/docker/avrag-runtime.Dockerfile`). Not Rust/npm crates.

| Component | License | Role | Source |
|-----------|---------|------|--------|
| **markitdown** (Microsoft) | MIT | PDF / text / long-tail convert → markdown | https://github.com/microsoft/markitdown · PyPI `markitdown` |
| **firecrawl-anydoc** | MIT | Office/ODF/RTF/EPUB/CSV → markdown (product non-PDF path) | https://github.com/firecrawl/anydoc · PyPI `firecrawl-anydoc` |
| **anydoc-extract** | MIT (this repo) | Thin CLI wrapper over firecrawl-anydoc for the worker | `avrag-rs/scripts/anydoc-extract/` |

## Desktop client — shell and bundled data-plane

### Shell (Tauri 2)

| Component | License | Notes |
|-----------|---------|-------|
| Tauri 2 + plugins (`desktop/src-tauri`) | MIT OR Apache-2.0 (upstream) | Desktop shell only; product logic is MIT Context-OS code |
| WebView2 (Windows) | Microsoft software license | System/runtime dependency of Tauri on Windows |

### Bundled portable runtime (optional NSIS `runtime/`)

Pins: `desktop/runtime/bundled/pins.env`. Stage/pack writes `runtime/THIRD_PARTY.txt` via
`scripts/stage-desktop-bundled-runtime.sh`. End-user setup may embed this tree under `$INSTDIR/runtime/`.

| Component | Version pin (see pins.env) | License | Distribution notes |
|-----------|----------------------------|---------|---------------------|
| PostgreSQL Windows binaries | PG 16.x (EDB zip) | PostgreSQL License | Retain notices under `runtime/pgsql/` |
| pgvector | 0.8.x matching PG 16 | PostgreSQL License | Upstream https://github.com/pgvector/pgvector ; Windows DLL may come from unofficial prebuild (andreiramani `pgvector_pgsql_windows`) — same license family |
| Redis for Windows | **5.0.14.1** (tporadowski port) | **BSD-3-Clause** (historical Redis COPYING) | **Chosen to avoid SSPL/RSALv2**. Do not silently upgrade to Redis 7.4+ Windows builds without license review. Source: https://github.com/tporadowski/redis |

Redis **desktop pin ≠ SaaS Redis**: commercial checklist P1 applies to Linux server Redis 7.4+; the client ships the BSD-era Windows port above.

## Rust dependencies (avrag-rs)

Total crates: **678**

### Apache-2.0 OR MIT (338 crates)

- aes
- ahash
- allocator-api2
- android_system_properties
- anstream
- anstyle
- anstyle-parse
- anstyle-query
- anstyle-wincon
- anyhow
- arbitrary
- arc-swap
- as-any
- async-trait
- atomic-waker
- auto_enums
- autocfg
- base16ct
- base64
- base64ct
- bit-set
- bit-vec
- bitflags
- block-buffer
- blowfish
- bstr
- bumpalo
- bytes-utils
- bzip2
- bzip2-sys
- cast
- cc
- cfg-if
- chacha20
- chrono
- cipher
- clap
- clap_builder
- clap_derive
- clap_lex
- cmake
- cmov
- colorchoice
- concurrent-queue
- const-oid
- const-random
- const-random-macro
- core-foundation
- core-foundation-sys
- cpufeatures
- crc
- crc-catalog
- crc-fast
- crc32fast
- crossbeam-deque
- crossbeam-epoch
- crossbeam-queue
- crossbeam-utils
- crypto-bigint
- crypto-common
- ctutils
- dary_heap
- der
- deranged
- derive_arbitrary
- derive_utils
- digest
- displaydoc
- dtoa
- dyn-clone
- ecdsa
- either
- elliptic-curve
- email-encoding
- encode_unicode
- equivalent
- errno
- etcetera
- event-listener
- eventsource-stream
- fallible-iterator
- fallible-streaming-iterator
- fastrand
- ff
- filetime
- find-msvc-tools
- flate2
- flume
- fnv
- foreign-types
- foreign-types-shared
- form_urlencoded
- futf
- futures
- futures-channel
- futures-core
- futures-executor
- futures-intrusive
- futures-io
- futures-macro
- futures-sink
- futures-task
- futures-timer
- futures-util
- fxhash
- getopts
- getrandom
- glob
- group
- half
- hashbrown
- hashlink
- heck
- hex
- hkdf
- hmac
- home
- html5ever
- http
- httparse
- httpdate
- hybrid-array
- hyper-tls
- iana-time-zone
- iana-time-zone-haiku
- ident_case
- idna
- idna_adapter
- indexmap
- inherent
- inout
- ipnet
- ipnetwork
- is_terminal_polyfill
- itertools
- itoa
- jni
- jni-macros
- jni-sys
- jni-sys-macros
- jobserver
- js-sys
- lazy_static
- lexical-core
- lexical-parse-float
- lexical-parse-integer
- lexical-util
- lexical-write-float
- lexical-write-integer
- libc
- lock_api
- log
- lzma-sys
- mac
- markup5ever
- match_token
- md-5
- mime
- minimal-lexical
- native-tls
- no_std_io2
- num-bigint
- num-bigint-dig
- num-complex
- num-conv
- num-integer
- num-iter
- num-traits
- once_cell
- once_cell_polyfill
- openssl-macros
- openssl-probe
- p256
- parking
- parking_lot
- parking_lot_core
- pbkdf2
- pem-rfc7468
- percent-encoding
- pgvector
- pin-project
- pin-project-internal
- pin-project-lite
- pin-utils
- pkcs1
- pkcs8
- pkg-config
- plain
- powerfmt
- ppv-lite86
- primeorder
- proc-macro-error-attr2
- proc-macro-error2
- proc-macro2
- prometheus-client
- prometheus-client-derive-encode
- proptest
- quick-error
- quinn
- quinn-proto
- quinn-udp
- quote
- rand
- rand_chacha
- rand_core
- rand_pcg
- rand_xorshift
- rayon
- rayon-core
- ref-cast
- ref-cast-impl
- regex
- regex-automata
- regex-lite
- regex-syntax
- reqwest
- rfc6979
- rle-decode-fast
- rsa
- rustc-hash
- rustc_version
- rustls-pki-types
- rustls-platform-verifier
- rustls-platform-verifier-android
- rustversion
- rusty-fork
- scopeguard
- sea-query
- sea-query-derive
- sec1
- security-framework
- security-framework-sys
- semver
- serde
- serde_core
- serde_derive
- serde_derive_internals
- serde_json
- serde_path_to_error
- serde_urlencoded
- serde_yaml
- servo_arc
- sha1
- sha2
- shlex
- signal-hook-registry
- signature
- simd_cesu8
- simdutf8
- siphasher
- smallvec
- socket2
- spki
- sqlx
- sqlx-core
- sqlx-macros
- sqlx-macros-core
- sqlx-mysql
- sqlx-postgres
- sqlx-sqlite
- stable_deref_trait
- streaming-iterator
- string_cache
- string_cache_codegen
- stringprep
- syn
- system-configuration
- system-configuration-sys
- tar
- tempfile
- tendril
- thiserror
- thiserror-impl
- thread_local
- time
- time-core
- time-macros
- tokio-rustls
- tungstenite
- typenum
- typeshare
- typeshare-annotation
- unarray
- unicase
- unicode-bidi
- unicode-normalization
- unicode-properties
- unicode-segmentation
- unicode-width
- ureq
- ureq-proto
- url
- utf-8
- utf8-zero
- utf8_iter
- utf8parse
- utoipa
- utoipa-gen
- utoipa-swagger-ui
- uuid
- vcpkg
- version_check
- wait-timeout
- wasm-bindgen
- wasm-bindgen-futures
- wasm-bindgen-macro
- wasm-bindgen-macro-support
- wasm-bindgen-shared
- wasm-streams
- web-sys
- web-time
- winapi
- winapi-i686-pc-windows-gnu
- winapi-x86_64-pc-windows-gnu
- windows-core
- windows-implement
- windows-interface
- windows-link
- windows-registry
- windows-result
- windows-strings
- windows-sys
- windows-targets
- windows_aarch64_gnullvm
- windows_aarch64_msvc
- windows_i686_gnu
- windows_i686_gnullvm
- windows_i686_msvc
- windows_x86_64_gnu
- windows_x86_64_gnullvm
- windows_x86_64_msvc
- xattr
- xmlparser
- xz2
- zeroize
- zeroize_derive
- zstd-safe
- zstd-sys

### MIT (163 crates)

- agent-loop
- agent-tools
- analytics
- any_spawner
- app
- app-admin
- app-billing
- app-bootstrap
- app-chat
- app-core
- app-documents
- async-stream
- async-stream-impl
- atoi
- avrag-api
- avrag-billing
- avrag-cache-redis
- avrag-chatmemory
- avrag-code-interpreter
- avrag-guardrails
- avrag-llm
- avrag-rag-core
- avrag-rag-core-ports
- avrag-retrieval-data-plane
- avrag-search
- avrag-share
- avrag-storage-milvus
- avrag-storage-pg
- avrag-storage-pgvector
- avrag-struct-supervision
- avrag-worker
- axum
- axum-core
- axum-extra
- axum-macros
- base64-simd
- bcrypt
- bytes
- cfg_aliases
- combine
- comfy-table
- common
- console
- contracts
- core_maths
- crossterm
- crossterm_winapi
- crunchy
- darling
- darling_core
- darling_macro
- data-encoding
- deflate64
- derive_more
- dotenvy
- duckdb
- e2e-analyzer
- email_address
- evalexpr
- evidence-form
- fancy-regex
- fs_extra
- generic-array
- h2
- headers
- headers-core
- heavytail
- http-body
- http-body-util
- hyper
- hyper-util
- ingestion
- ingestion-types
- jieba-macros
- jieba-rs
- jsonwebtoken
- lettre
- libduckdb-sys
- libflate
- libflate_lz77
- libm
- libredox
- libsqlite3-sys
- lru
- lzma-rs
- matchers
- mime_guess
- mio
- multer
- nanoid
- new_debug_unreachable
- nom
- nu-ansi-term
- openssl-sys
- ordered-float
- outref
- pem
- phf
- phf_codegen
- phf_generator
- phf_macros
- phf_shared
- precomputed-hash
- pulldown-cmark
- rag_quality
- redox_syscall
- rig-core
- rust-embed
- rust-embed-impl
- rust-embed-utils
- schannel
- schemars
- schemars_derive
- sharded-slab
- simd-adler32
- slab
- spin
- storage-local
- strsim
- strum
- strum_macros
- synstructure
- telemetry
- text-splitter
- tiktoken-rs
- tokio
- tokio-macros
- tokio-native-tls
- tokio-stream
- tokio-tungstenite
- tokio-util
- tower
- tower-http
- tower-layer
- tower-service
- tracing
- tracing-attributes
- tracing-core
- tracing-futures
- tracing-log
- tracing-subscriber
- transport-http
- tree-sitter
- tree-sitter-go
- tree-sitter-java
- tree-sitter-javascript
- tree-sitter-language
- tree-sitter-python
- tree-sitter-rust
- tree-sitter-typescript
- try-lock
- ts-rs
- ts-rs-macros
- unsafe-libyaml
- urlencoding
- valuable
- vsimd
- want
- write-core
- zip
- zmij
- zstd

### Apache-2.0 (41 crates)

- arrow
- arrow-arith
- arrow-buffer
- arrow-cast
- arrow-data
- arrow-ord
- arrow-row
- arrow-schema
- arrow-select
- arrow-string
- aws-config
- aws-credential-types
- aws-runtime
- aws-sdk-s3
- aws-sdk-sso
- aws-sdk-ssooidc
- aws-sdk-sts
- aws-sigv4
- aws-smithy-async
- aws-smithy-checksums
- aws-smithy-eventstream
- aws-smithy-http
- aws-smithy-http-client
- aws-smithy-json
- aws-smithy-observability
- aws-smithy-query
- aws-smithy-runtime
- aws-smithy-runtime-api
- aws-smithy-runtime-api-macros
- aws-smithy-schema
- aws-smithy-types
- aws-smithy-xml
- aws-types
- include-flate
- include-flate-codegen
- include-flate-compress
- insta
- openssl
- similar
- sync_wrapper
- zopfli

### Unicode-3.0 (22 crates)

- icu_collections
- icu_locale
- icu_locale_core
- icu_locale_data
- icu_normalizer
- icu_normalizer_data
- icu_properties
- icu_properties_data
- icu_provider
- icu_segmenter
- icu_segmenter_data
- litemap
- potential_utf
- tinystr
- writeable
- yoke
- yoke-derive
- zerofrom
- zerofrom-derive
- zerotrie
- zerovec
- zerovec-derive

### MIT OR Unlicense (9 crates)

- aho-corasick
- byteorder
- csv
- csv-core
- memchr
- same-file
- termcolor
- walkdir
- winapi-util

### Apache-2.0 OR Apache-2.0 WITH LLVM-exception OR MIT (5 crates)

- linux-raw-sys
- rustix
- wasi
- wasip2
- wit-bindgen

### ISC (6 crates)

- ego-tree
- maxminddb
- rustls-webpki
- scraper
- simple_asn1
- untrusted

### Apache-2.0 OR ISC OR MIT (4 crates)

- hyper-rustls
- rustls
- rustls-native-certs
- sct

### Apache-2.0 OR MIT OR Zlib (4 crates)

- lru-slab
- miniz_oxide
- tinyvec
- tinyvec_macros

### MPL-2.0 (4 crates)

- cssparser
- cssparser-macros
- dtoa-short
- selectors

### Zlib (3 crates)

- adler32
- foldhash
- zlib-rs

### BSD-3-Clause (3 crates)

- redis
- sha1_smol
- subtle

### CDLA-Permissive-2.0 (2 crates)

- webpki-root-certs
- webpki-roots

### Apache-2.0 OR BSD-2-Clause OR MIT (2 crates)

- zerocopy
- zerocopy-derive

### Apache-2.0 OR BSL-1.0 OR MIT (2 crates)

- wasite
- whoami

### Apache-2.0 OR CC0-1.0 OR MIT-0 (2 crates)

- constant_time_eq
- dunce

### Apache-2.0 OR LGPL-2.1-or-later OR MIT (1 crates)

- r-efi

### (Apache-2.0 OR ISC OR MIT) AND (Apache-2.0 OR ISC OR MIT-0) AND (Apache-2.0 OR ISC) AND Apache-2.0 AND BSD-3-Clause AND ISC AND MIT (1 crates)

- aws-lc-sys

### (Apache-2.0 OR ISC) AND ISC (1 crates)

- aws-lc-rs

### (Apache-2.0 OR MIT) AND BSD-3-Clause (1 crates)

- encoding_rs

### (Apache-2.0 OR MIT) AND Unicode-3.0 (1 crates)

- unicode-ident

### 0BSD (1 crates)

- quoted_printable

### 0BSD OR Apache-2.0 OR MIT (1 crates)

- adler2

### Apache-2.0 AND ISC (1 crates)

- ring

### Apache-2.0 AND MIT (1 crates)

- arrow-array

### Apache-2.0 OR BSL-1.0 (1 crates)

- ryu

### BSD-2-Clause (1 crates)

- cedarwood

### BSD-3-Clause AND MIT (1 crates)

- matchit

### CC0-1.0 (1 crates)

- tiny-keccak

## Frontend dependencies (frontend_next, transitive)

Total packages: **312**

### MIT (271 packages)

- @adobe/css-tools@4.5.0
- @asamuzakjp/css-color@4.1.2
- @asamuzakjp/dom-selector@6.8.1
- @asamuzakjp/nwsapi@2.3.9
- @babel/code-frame@7.29.0
- @babel/helper-validator-identifier@7.28.5
- @babel/runtime@7.29.2
- @csstools/css-calc@3.2.0
- @csstools/css-color-parser@4.1.0
- @csstools/css-parser-algorithms@4.0.0
- @csstools/css-tokenizer@4.0.0
- @esbuild/linux-x64@0.27.7
- @floating-ui/core@1.7.5
- @floating-ui/dom@1.7.6
- @floating-ui/utils@0.2.11
- @formatjs/fast-memoize@3.1.2
- @formatjs/icu-messageformat-parser@3.5.4
- @formatjs/icu-skeleton-parser@2.1.4
- @formatjs/intl-localematcher@0.8.3
- @img/colour@1.1.0
- @jridgewell/sourcemap-codec@1.5.5
- @next/env@16.2.10
- @next/swc-linux-x64-gnu@16.2.10
- @parcel/watcher-linux-x64-glibc@2.5.6
- @parcel/watcher@2.5.6
- @rollup/rollup-linux-x64-gnu@4.60.1
- @schummar/icu-type-parser@1.21.5
- @tanstack/query-core@5.99.2
- @tiptap/core@3.22.4
- @tiptap/extension-blockquote@3.22.4
- @tiptap/extension-bold@3.22.4
- @tiptap/extension-bubble-menu@3.22.4
- @tiptap/extension-bullet-list@3.22.4
- @tiptap/extension-code-block@3.22.4
- @tiptap/extension-code@3.22.4
- @tiptap/extension-document@3.22.4
- @tiptap/extension-dropcursor@3.22.4
- @tiptap/extension-floating-menu@3.22.4
- @tiptap/extension-gapcursor@3.22.4
- @tiptap/extension-hard-break@3.22.4
- @tiptap/extension-heading@3.22.4
- @tiptap/extension-horizontal-rule@3.22.4
- @tiptap/extension-italic@3.22.4
- @tiptap/extension-link@3.22.4
- @tiptap/extension-list-item@3.22.4
- @tiptap/extension-list-keymap@3.22.4
- @tiptap/extension-list@3.22.4
- @tiptap/extension-ordered-list@3.22.4
- @tiptap/extension-paragraph@3.22.4
- @tiptap/extension-strike@3.22.4
- @tiptap/extension-text@3.22.4
- @tiptap/extension-underline@3.22.4
- @types/aria-query@5.0.4
- @types/chai@5.2.3
- @types/debug@4.1.13
- @types/deep-eql@4.0.2
- @types/estree@1.0.8
- @types/hast@3.0.4
- @types/mdast@4.0.4
- @types/ms@2.1.0
- @types/trusted-types@2.0.7
- @types/unist@3.0.3
- @types/use-sync-external-store@0.0.6
- @vitest/expect@3.2.4
- @vitest/mocker@3.2.4
- @vitest/pretty-format@3.2.4
- @vitest/runner@3.2.4
- @vitest/snapshot@3.2.4
- @vitest/spy@3.2.4
- @vitest/utils@3.2.4
- agent-base@7.1.4
- ansi-regex@5.0.1
- ansi-styles@5.2.0
- argparse@1.0.10
- assertion-error@2.0.1
- bail@2.0.2
- bidi-js@1.0.3
- cac@6.7.14
- camelcase@5.3.1
- ccount@2.0.1
- chai@5.3.3
- character-entities-html4@2.1.0
- character-entities-legacy@3.0.0
- character-entities@2.0.2
- check-error@2.1.3
- client-only@0.0.1
- color-convert@2.0.1
- color-name@1.1.4
- comma-separated-tokens@2.0.3
- css-tree@3.2.1
- css.escape@1.5.1
- cssstyle@5.3.7
- csstype@3.2.3
- data-urls@6.0.1
- debug@4.4.3
- decamelize@1.2.0
- decimal.js@10.6.0
- decode-named-character-reference@1.3.0
- deep-eql@5.0.2
- dequal@2.0.3
- devlop@1.1.0
- dijkstrajs@1.0.3
- dom-accessibility-api@0.5.16
- emoji-regex@8.0.0
- es-module-lexer@1.7.0
- esbuild@0.27.7
- escape-string-regexp@5.0.0
- estree-walker@3.0.3
- extend-shallow@2.0.1
- extend@3.0.2
- fast-equals@5.4.0
- fdir@6.5.0
- find-up@4.1.0
- hast-util-heading-rank@3.0.0
- hast-util-is-element@3.0.0
- hast-util-to-html@9.0.5
- hast-util-to-string@3.0.1
- hast-util-whitespace@3.0.0
- html-encoding-sniffer@4.0.0
- html-void-elements@3.0.0
- http-proxy-agent@7.0.2
- https-proxy-agent@7.0.6
- iconv-lite@0.6.3
- icu-minify@4.9.1
- indent-string@4.0.0
- is-extendable@0.1.1
- is-extglob@2.1.1
- is-fullwidth-code-point@3.0.0
- is-glob@4.0.3
- is-plain-obj@4.1.0
- is-potential-custom-element-name@1.0.1
- js-tokens@4.0.0
- js-yaml@3.14.2
- kind-of@6.0.3
- linkifyjs@4.3.2
- locate-path@5.0.0
- longest-streak@3.1.0
- loupe@3.2.1
- lz-string@1.5.0
- magic-string@0.30.21
- markdown-table@3.0.4
- marked@17.0.6
- mdast-util-find-and-replace@3.0.2
- mdast-util-from-markdown@2.0.3
- mdast-util-gfm-autolink-literal@2.0.1
- mdast-util-gfm-footnote@2.1.0
- mdast-util-gfm-strikethrough@2.0.0
- mdast-util-gfm-table@2.0.0
- mdast-util-gfm-task-list-item@2.0.0
- mdast-util-gfm@3.1.0
- mdast-util-phrasing@4.1.0
- mdast-util-to-hast@13.2.1
- mdast-util-to-markdown@2.1.2
- mdast-util-to-string@4.0.0
- micromark-core-commonmark@2.0.3
- micromark-extension-gfm-autolink-literal@2.1.0
- micromark-extension-gfm-footnote@2.1.0
- micromark-extension-gfm-strikethrough@2.1.0
- micromark-extension-gfm-table@2.1.1
- micromark-extension-gfm-tagfilter@2.0.0
- micromark-extension-gfm-task-list-item@2.1.0
- micromark-extension-gfm@3.0.0
- micromark-factory-destination@2.0.1
- micromark-factory-label@2.0.1
- micromark-factory-space@2.0.1
- micromark-factory-title@2.0.1
- micromark-factory-whitespace@2.0.1
- micromark-util-character@2.1.1
- micromark-util-chunked@2.0.1
- micromark-util-classify-character@2.0.1
- micromark-util-combine-extensions@2.0.1
- micromark-util-decode-numeric-character-reference@2.0.2
- micromark-util-decode-string@2.0.1
- micromark-util-encode@2.0.1
- micromark-util-html-tag-name@2.0.1
- micromark-util-normalize-identifier@2.0.1
- micromark-util-resolve-all@2.0.1
- micromark-util-sanitize-uri@2.0.1
- micromark-util-subtokenize@2.1.0
- micromark-util-symbol@2.0.1
- micromark-util-types@2.0.2
- micromark@4.0.2
- min-indent@1.0.1
- ms@2.1.3
- nanoid@3.3.15
- negotiator@1.0.0
- next-intl-swc-plugin-extractor@4.9.1
- node-addon-api@7.1.1
- orderedmap@2.1.1
- p-limit@2.3.0
- p-locate@4.1.0
- p-try@2.2.0
- parse5@7.3.0
- path-exists@4.0.0
- pathe@2.0.3
- pathval@2.0.1
- picomatch@4.0.4
- pngjs@5.0.0
- po-parser@2.1.1
- postcss@8.4.31
- pretty-format@27.5.1
- property-information@7.2.0
- prosemirror-changeset@2.4.1
- prosemirror-commands@1.7.1
- prosemirror-dropcursor@1.8.2
- prosemirror-gapcursor@1.4.1
- prosemirror-history@1.5.0
- prosemirror-keymap@1.2.3
- prosemirror-model@1.25.4
- prosemirror-schema-list@1.5.1
- prosemirror-state@1.4.4
- prosemirror-tables@1.8.5
- prosemirror-transform@1.12.0
- prosemirror-view@1.41.8
- punycode@2.3.1
- react-is@17.0.2
- redent@3.0.0
- remark-stringify@11.0.0
- require-directory@2.1.1
- require-from-string@2.0.2
- rollup@4.60.1
- rope-sequence@1.3.4
- rrweb-cssom@0.8.0
- safer-buffer@2.1.2
- scheduler@0.27.0
- section-matter@1.0.0
- space-separated-tokens@2.0.2
- stackback@0.0.2
- std-env@3.10.0
- string-width@4.2.3
- stringify-entities@4.0.4
- strip-ansi@6.0.1
- strip-bom-string@1.0.0
- strip-indent@3.0.0
- strip-literal@3.1.0
- styled-jsx@5.1.6
- symbol-tree@3.2.4
- tinybench@2.9.0
- tinyexec@0.3.2
- tinyglobby@0.2.16
- tinypool@1.1.1
- tinyrainbow@2.0.0
- tinyspy@4.0.4
- tldts-core@7.0.28
- tldts@7.0.28
- tr46@6.0.0
- trim-lines@3.0.1
- trough@2.2.0
- undici-types@7.13.0
- unist-util-is@6.0.1
- unist-util-position@5.0.0
- unist-util-stringify-position@4.0.0
- unist-util-visit-parents@6.0.2
- unist-util-visit@5.1.0
- use-intl@4.9.1
- use-sync-external-store@1.6.0
- vfile-message@4.0.3
- vfile@6.0.3
- vite-node@3.2.4
- vite@7.3.2
- w3c-keyname@2.2.8
- w3c-xmlserializer@5.0.0
- whatwg-encoding@3.1.1
- whatwg-mimetype@4.0.0
- whatwg-url@15.1.0
- why-is-node-running@2.3.0
- wrap-ansi@6.2.0
- ws@8.20.0
- xmlchars@2.2.0
- yargs@15.4.1
- zwitch@2.0.4

### Apache-2.0 (13 packages)

- @img/sharp-linux-x64@0.34.5
- @swc/core@1.15.30
- @swc/counter@0.1.3
- @swc/helpers@0.5.15
- @swc/types@0.1.26
- aria-query@5.3.0
- baseline-browser-mapping@2.10.42
- detect-libc@2.1.2
- expect-type@1.3.0
- playwright-core@1.53.0
- playwright@1.53.0
- sharp@0.34.5
- xml-name-validator@5.0.0

### ISC (13 packages)

- @ungap/structured-clone@1.3.1
- cliui@6.0.0
- get-caller-file@2.0.5
- github-slugger@2.0.0
- picocolors@1.1.1
- require-main-filename@2.0.0
- saxes@6.0.0
- semver@7.8.5
- set-blocking@2.0.0
- siginfo@2.0.0
- which-module@2.0.1
- y18n@4.0.3
- yargs-parser@18.1.3

### BSD-3-Clause (4 packages)

- intl-messageformat@11.2.1
- source-map-js@1.2.1
- sprintf-js@1.0.3
- tough-cookie@6.0.1

### BSD-2-Clause (3 packages)

- entities@6.0.1
- esprima@4.0.1
- webidl-conversions@8.0.1

### MIT-0 (2 packages)

- @csstools/color-helpers@6.0.2
- @csstools/css-syntax-patches-for-csstree@1.1.3

### 0BSD (1 packages)

- tslib@2.8.1

### Apache-2.0 AND MIT (1 packages)

- @swc/core-linux-x64-gnu@1.15.30

### BlueOak-1.0.0 (1 packages)

- lru-cache@11.3.5

### CC-BY-4.0 (1 packages)

- caniuse-lite@1.0.30001803

### CC0-1.0 (1 packages)

- mdn-data@2.27.1

### LGPL-3.0-or-later (1 packages)

- @img/sharp-libvips-linux-x64@1.2.4

## Regeneration

```bash
./scripts/generate-third-party-notices.sh
./scripts/check-licenses.sh
```
