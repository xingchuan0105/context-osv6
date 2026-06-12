#!/bin/bash

# 桌面客户端构建脚本
# 用法: ./scripts/build-desktop.sh [platform]
# platform: macos | windows | linux | all (默认 all)

set -e

PLATFORM=${1:-all}

echo "=== AVRag Desktop Build Script ==="
echo "Platform: $PLATFORM"
echo ""

# 检查依赖
echo "检查依赖..."

if ! command -v pnpm &> /dev/null; then
    echo "错误: pnpm 未安装"
    exit 1
fi

if ! command -v cargo &> /dev/null; then
    echo "错误: cargo 未安装"
    exit 1
fi

# 安装前端依赖
echo "安装前端依赖..."
cd frontend_next
pnpm install
cd ..

# 安装桌面端依赖
echo "安装桌面端依赖..."
cd desktop
pnpm install
cd ..

# 构建前端静态资源
echo "构建前端静态资源..."
cd frontend_next
pnpm build:desktop
cd ..

# 构建桌面应用
echo "构建桌面应用..."
cd desktop

case $PLATFORM in
    macos)
        pnpm tauri build --target universal-apple-darwin
        ;;
    windows)
        pnpm tauri build --target x86_64-pc-windows-msvc
        ;;
    linux)
        pnpm tauri build --target x86_64-unknown-linux-gnu
        ;;
    all)
        pnpm tauri build
        ;;
    *)
        echo "错误: 未知平台 $PLATFORM"
        echo "支持的平台: macos, windows, linux, all"
        exit 1
        ;;
esac

cd ..

echo ""
echo "=== 构建完成 ==="
echo "输出目录: desktop/src-tauri/target/release/bundle/"
