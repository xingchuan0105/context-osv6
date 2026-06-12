#!/bin/bash

# Windows 桌面客户端构建脚本
# 需要在 Windows 环境或安装了 mingw-w64 的 Linux 环境中运行

set -e

echo "=== AVRag Desktop - Windows Build ==="

# 检查依赖
if ! command -v x86_64-w64-mingw32-gcc &> /dev/null; then
    echo "错误: mingw-w64 未安装"
    echo "请运行: sudo apt-get install -y mingw-w64"
    exit 1
fi

# 配置 Cargo
mkdir -p ~/.cargo
cat >> ~/.cargo/config.toml << 'EOF'

[target.x86_64-pc-windows-gnu]
linker = "x86_64-w64-mingw32-gcc"
EOF

# 添加 Rust 目标
rustup target add x86_64-pc-windows-gnu

# 构建前端
echo "构建前端静态资源..."
cd frontend_next
BUILD_TARGET=desktop pnpm build
cd ..

# 构建桌面应用
echo "构建 Windows 桌面应用..."
cd desktop
pnpm tauri build --target x86_64-pc-windows-gnu

echo ""
echo "=== 构建完成 ==="
echo "输出目录: desktop/src-tauri/target/x86_64-pc-windows-gnu/release/bundle/"
