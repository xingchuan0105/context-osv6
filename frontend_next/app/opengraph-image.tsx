// 桌面端静态导出：使用静态图标文件替代动态生成
// ImageResponse 在静态导出时有兼容性问题

export const dynamic = "force-static";

// 返回一个简单的空响应，桌面端会使用 public/ 目录下的静态图标
export default function OpenGraphImage() {
  return new Response(null, {
    status: 301,
    headers: {
      Location: "/opengraph-image.png",
    },
  });
}
