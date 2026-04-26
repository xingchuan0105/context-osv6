import { NextRequest, NextResponse } from "next/server";

import { AUTH_SESSION_COOKIE_NAME } from "./lib/auth/server-session";
import { resolveMiddlewareAction } from "./lib/middleware-routing";

export function middleware(request: NextRequest) {
  const action = resolveMiddlewareAction(
    request.nextUrl.pathname,
    Boolean(request.cookies.get(AUTH_SESSION_COOKIE_NAME)),
  );

  if (action.type === "next") {
    return NextResponse.next();
  }

  const url = request.nextUrl.clone();
  url.pathname = action.destination;

  return NextResponse.redirect(url);
}

export const config = {
  matcher: ["/((?!api|_next/static|_next/image|favicon.ico).*)"],
};
