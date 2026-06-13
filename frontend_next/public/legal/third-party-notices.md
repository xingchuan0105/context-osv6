# Third-Party Notices

_Generated: 2026-06-13 via `scripts/generate-third-party-notices.sh`_

This project (Context-OS / AVRag) is licensed under the [MIT License](LICENSE).
Third-party components listed below are subject to their own licenses.

## Commercial deployment checklist

| Priority | Component | License | Action |
|----------|-----------|---------|--------|
| P0 | PyMuPDF (`pdf-visual-renderer`) | AGPL-3.0 or Artifex commercial | Do not deploy in SaaS unless licensed; leave `PDF_RENDERER_BASE_URL` unset |
| P1 | MinIO (upload / Milvus compose) | AGPL-3.0 | Prefer cloud S3/OSS via `S3_*` env vars |
| P1 | Redis server 7.4+ | RSALv2 / SSPL | Internal cache only; pin ≤7.2 or use Valkey |
| P2 | `@img/sharp-libvips-linux-x64` (Next.js web) | LGPL-3.0 | NOTICE only; desktop build uses `images.unoptimized` |
| P2 | `cssparser` / `selectors` (via `scraper`) | MPL-2.0 | NOTICE; share file changes only if you modify MPL files |
| P2 | `dompurify` | MPL-2.0 OR Apache-2.0 | Compliance: choose Apache-2.0 |

## Runtime infrastructure (not npm/cargo)

| Component | Typical license | Notes |
|-----------|-----------------|-------|
| PostgreSQL | PostgreSQL License | Permissive |
| Milvus | Apache-2.0 | Permissive |
| etcd | Apache-2.0 | Bundled with Milvus compose |
| Paddle OCR Jobs | API Terms of Service | External SaaS, not open source |
| LLM / Embedding providers | API Terms of Service | DeepSeek, DashScope, Brave, etc. |

## Rust dependencies (avrag-rs)

Total crates: **634**

### Apache-2.0 OR MIT (320 crates)

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
- block-padding
- blowfish
- bstr
- bumpalo
- bytecount
- bytes-utils
- bzip2
- bzip2-sys
- cbc
- cc
- cfg-if
- chrono
- cipher
- clap
- clap_builder
- clap_derive
- clap_lex
- cmake
- colorchoice
- concurrent-queue
- const-oid
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
- fastrand
- fdeflate
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
- hyper-tls
- iana-time-zone
- iana-time-zone-haiku
- id-arena
- ident_case
- idna
- idna_adapter
- image
- indexmap
- inherent
- inout
- ipnet
- iri-string
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
- leb128fmt
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
- num-bigint
- num-bigint-dig
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
- pin-project
- pin-project-internal
- pin-project-lite
- pin-utils
- pkcs1
- pkcs8
- pkg-config
- plain
- png
- powerfmt
- ppv-lite86
- prettyplease
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
- rand_xorshift
- rangemap
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
- unicode-width
- unicode-xid
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
- weezl
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

### MIT (152 crates)

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
- avrag-auth
- avrag-billing
- avrag-cache-redis
- avrag-chatmemory
- avrag-code-interpreter
- avrag-guardrails
- avrag-llm
- avrag-office-parser-jvm
- avrag-rag-core
- avrag-rag-core-ports
- avrag-retrieval-data-plane
- avrag-search
- avrag-share
- avrag-storage-milvus
- avrag-storage-pg
- avrag-test-kit
- avrag-worker
- axum
- axum-core
- axum-extra
- axum-macros
- base64-simd
- bcrypt
- bytes
- cfb
- cfg_aliases
- combine
- common
- console
- contracts
- core_maths
- darling
- darling_core
- darling_macro
- data-encoding
- deflate64
- derive_more
- dotenvy
- e2e-analyzer
- email_address
- evalexpr
- fancy-regex
- fs_extra
- generic-array
- h2
- headers
- headers-core
- http-body
- http-body-util
- hyper
- hyper-util
- infer
- ingestion
- ingestion-types
- jsonwebtoken
- lettre
- libm
- libredox
- libsqlite3-sys
- lopdf
- lru
- lzma-rs
- matchers
- mime_guess
- mio
- multer
- nanoid
- new_debug_unreachable
- nom
- nom_locate
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
- zip
- zmij
- zstd

### Apache-2.0 (29 crates)

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
- aws-smithy-types
- aws-smithy-xml
- aws-types
- insta
- liteparse
- liteparse-pdfium
- liteparse-pdfium-sys
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

### Apache-2.0 OR Apache-2.0 WITH LLVM-exception OR MIT (14 crates)

- linux-raw-sys
- rustix
- wasi
- wasip2
- wasip3
- wasm-encoder
- wasm-metadata
- wasmparser
- wit-bindgen
- wit-bindgen-core
- wit-bindgen-rust
- wit-bindgen-rust-macro
- wit-component
- wit-parser

### MIT OR Unlicense (8 crates)

- aho-corasick
- byteorder
- byteorder-lite
- memchr
- same-file
- termcolor
- walkdir
- winapi-util

### ISC (6 crates)

- ego-tree
- libloading
- rustls-webpki
- scraper
- simple_asn1
- untrusted

### Apache-2.0 OR ISC OR MIT (4 crates)

- hyper-rustls
- rustls
- rustls-native-certs
- sct

### Apache-2.0 OR MIT OR Zlib (5 crates)

- bytemuck
- lru-slab
- miniz_oxide
- tinyvec
- tinyvec_macros

### MPL-2.0 (4 crates)

- cssparser
- cssparser-macros
- dtoa-short
- selectors

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

### Apache-2.0 OR BSD-3-Clause (2 crates)

- moxcms
- pxfm

### Apache-2.0 OR BSL-1.0 OR MIT (2 crates)

- wasite
- whoami

### Apache-2.0 OR CC0-1.0 OR MIT-0 (2 crates)

- constant_time_eq
- dunce

### Apache-2.0 OR LGPL-2.1-or-later OR MIT (1 crates)

- r-efi

### Zlib (2 crates)

- foldhash
- zlib-rs

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

### Apache-2.0 OR BSL-1.0 (1 crates)

- ryu

### BSD-3-Clause AND MIT (1 crates)

- matchit

## Frontend dependencies (frontend_next, transitive)

Total packages: **197**

### MIT (167 packages)

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
- @next/env@16.2.4
- @next/swc-linux-x64-gnu@16.2.4
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
- @types/deep-eql@4.0.2
- @types/estree@1.0.8
- @types/trusted-types@2.0.7
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
- assertion-error@2.0.1
- bidi-js@1.0.3
- cac@6.7.14
- chai@5.3.3
- check-error@2.1.3
- client-only@0.0.1
- css-tree@3.2.1
- css.escape@1.5.1
- cssstyle@5.3.7
- csstype@3.2.3
- data-urls@6.0.1
- debug@4.4.3
- decimal.js@10.6.0
- deep-eql@5.0.2
- dequal@2.0.3
- dom-accessibility-api@0.5.16
- es-module-lexer@1.7.0
- esbuild@0.27.7
- estree-walker@3.0.3
- fast-equals@5.4.0
- fdir@6.5.0
- html-encoding-sniffer@4.0.0
- http-proxy-agent@7.0.2
- https-proxy-agent@7.0.6
- iconv-lite@0.6.3
- icu-minify@4.9.1
- indent-string@4.0.0
- is-extglob@2.1.1
- is-glob@4.0.3
- is-potential-custom-element-name@1.0.1
- js-tokens@4.0.0
- linkifyjs@4.3.2
- loupe@3.2.1
- lz-string@1.5.0
- magic-string@0.30.21
- marked@17.0.6
- min-indent@1.0.1
- ms@2.1.3
- nanoid@3.3.11
- negotiator@1.0.0
- next-intl-swc-plugin-extractor@4.9.1
- node-addon-api@7.1.1
- orderedmap@2.1.1
- parse5@7.3.0
- pathe@2.0.3
- pathval@2.0.1
- picomatch@4.0.4
- po-parser@2.1.1
- postcss@8.4.31
- pretty-format@27.5.1
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
- require-from-string@2.0.2
- rollup@4.60.1
- rope-sequence@1.3.4
- rrweb-cssom@0.8.0
- safer-buffer@2.1.2
- scheduler@0.27.0
- stackback@0.0.2
- std-env@3.10.0
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
- undici-types@7.13.0
- use-intl@4.9.1
- use-sync-external-store@1.6.0
- vite-node@3.2.4
- vite@7.3.2
- w3c-keyname@2.2.8
- w3c-xmlserializer@5.0.0
- whatwg-encoding@3.1.1
- whatwg-mimetype@4.0.0
- whatwg-url@15.1.0
- why-is-node-running@2.3.0
- ws@8.20.0
- xmlchars@2.2.0

### Apache-2.0 (13 packages)

- @img/sharp-linux-x64@0.34.5
- @swc/core@1.15.30
- @swc/counter@0.1.3
- @swc/helpers@0.5.15
- @swc/types@0.1.26
- aria-query@5.3.0
- baseline-browser-mapping@2.10.19
- detect-libc@2.1.2
- expect-type@1.3.0
- playwright-core@1.53.0
- playwright@1.53.0
- sharp@0.34.5
- xml-name-validator@5.0.0

### ISC (4 packages)

- picocolors@1.1.1
- saxes@6.0.0
- semver@7.7.4
- siginfo@2.0.0

### BSD-3-Clause (3 packages)

- intl-messageformat@11.2.1
- source-map-js@1.2.1
- tough-cookie@6.0.1

### BSD-2-Clause (2 packages)

- entities@6.0.1
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

- caniuse-lite@1.0.30001788

### CC0-1.0 (1 packages)

- mdn-data@2.27.1

### LGPL-3.0-or-later (1 packages)

- @img/sharp-libvips-linux-x64@1.2.4

## Python sidecar (optional)

`avrag-rs/services/pdf-visual-renderer/requirements.txt`:

- **PyMuPDF** — Dual Licensed: GNU Affero GPL 3.0 or Artifex Commercial License
- fastapi, uvicorn, pydantic, httpx, python-multipart — MIT / BSD / Apache-2.0

## Regeneration

```bash
./scripts/generate-third-party-notices.sh
./scripts/check-licenses.sh
```
