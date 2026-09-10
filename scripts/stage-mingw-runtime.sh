#!/usr/bin/env bash
# Stage the runtime belonging to the same compiler used by cc-rs for windows-gnu.
set -euo pipefail
[[ $# -gt 0 ]] || { echo 'Usage: stage-mingw-runtime.sh DEST [DEST...]' >&2; exit 1; }
cxx="${CXX_x86_64_pc_windows_gnu:-${TARGET_CXX:-${CXX:-x86_64-w64-mingw32-g++}}}"
[[ "$("$cxx" -dumpmachine)" == x86_64-w64-mingw32 ]] || { echo 'Expected x64 MinGW C++ compiler.' >&2; exit 1; }
# Resolve every file before writing. Never borrow DLLs from an installed product
# or prefer a different GCC version/thread model because its path exists.
names=(libstdc++-6.dll libgcc_s_seh-1.dll libwinpthread-1.dll)
sources=()
for name in "${names[@]}"; do
    file="$("$cxx" "-print-file-name=$name")"
    [[ -f "$file" && "$file" != "$name" ]] || { echo "Compiler runtime missing: $name" >&2; exit 1; }
    sources+=("$(realpath "$file")")
done
for dest in "$@"; do
    mkdir -p "$dest"
    for i in "${!names[@]}"; do cp -f "${sources[$i]}" "$dest/${names[$i]}"; done
    { "$cxx" -v 2>&1; sha256sum "${sources[@]}"; } > "$dest/mingw-runtime.txt"
done
