/** Baidu webmaster file verification — public/ is not served on standalone. */
export const dynamic = "force-static";

const BODY = "c3054d0735912577ce4407383c2a7965";

export function GET() {
  return new Response(BODY, {
    headers: { "Content-Type": "text/plain; charset=utf-8" },
  });
}
