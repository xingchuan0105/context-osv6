import { cookies } from "next/headers";
import { redirect } from "next/navigation";

import { AUTH_SESSION_COOKIE_NAME } from "../lib/auth/server-session";

export default async function HomePage() {
  const cookieStore = await cookies();
  const hasAuthSessionHint = cookieStore.get(AUTH_SESSION_COOKIE_NAME)?.value === "1";

  redirect(hasAuthSessionHint ? "/dashboard" : "/login");
}
