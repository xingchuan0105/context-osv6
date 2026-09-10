#!/usr/bin/env bash
# The isolated Rust std::call_once probe crashes with the matching Win32
# libstdc++ DLL and passes with the same compiler's static runtime.
set -euo pipefail
cxx="${CXX_x86_64_pc_windows_gnu:-${TARGET_CXX:-${CXX:-x86_64-w64-mingw32-g++}}}"
[[ "$("$cxx" -dumpmachine)" == x86_64-w64-mingw32 ]] || {
    echo 'Expected x64 MinGW C++ linker.' >&2
    exit 1
}
stdlib="$("$cxx" -print-file-name=libstdc++.a)"
[[ -f "$stdlib" && "$stdlib" != libstdc++.a ]] || {
    echo 'Matching MinGW static C++ runtime is missing.' >&2
    exit 1
}
args=()
for arg in "$@"; do
    if [[ "$arg" == -lstdc++ ]]; then
        args+=("$stdlib")
    else
        args+=("$arg")
    fi
done

# PE linking performs many small random writes. On WSL's Windows mount, write
# the complete executable locally and copy it once after a successful link.
for ((i=0; i<${#args[@]}-1; i++)); do
    if [[ "${args[i]}" == -o && "${args[i+1]}" == /mnt/*.exe ]]; then
        output="${args[i+1]}"
        link_dir="$(mktemp -d "${TMPDIR:-/tmp}/context-os-link.XXXXXXXX")"
        trap 'rm -rf -- "$link_dir"' EXIT
        args[i+1]="$link_dir/$(basename "$output")"
        "$cxx" "${args[@]}"
        cp -- "${args[i+1]}" "$output"
        exit 0
    fi
done
exec "$cxx" "${args[@]}"
