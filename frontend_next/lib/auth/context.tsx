"use client";

import {
  createContext,
  type ReactNode,
  useCallback,
  useContext,
  useEffect,
  useMemo,
  useState,
} from "react";

import {
  authRuntimeCapabilities,
  type AuthPayload,
  type AuthUser,
  logout as logoutRequest,
  me,
} from "./client";
import {
  clearAuthSessionHint,
  clearPersistedAuthCookie,
  readPersistedAuthCookie,
  setAuthSessionHint,
  writePersistedAuthCookie,
} from "./server-session";

const AUTH_STORAGE_KEY = "avrag.auth.v1";
const AUTH_BOOTSTRAP_TIMEOUT_MS = 3000;

type PersistedAuth = {
  token: string;
  user: AuthUser;
};

type AuthContextValue = {
  initialized: boolean;
  isAuthenticated: boolean;
  token: string | null;
  user: AuthUser | null;
  passwordResetEnabled: boolean;
  completeAuth: (payload: AuthPayload) => void;
  updateUser: (user: AuthUser) => void;
  clearAuth: () => void;
  logout: () => Promise<void>;
};

const AuthContext = createContext<AuthContextValue | null>(null);

function readPersistedAuth(): PersistedAuth | null {
  if (typeof window === "undefined") {
    return null;
  }

  try {
    const raw = window.localStorage.getItem(AUTH_STORAGE_KEY);

    if (raw) {
      return JSON.parse(raw) as PersistedAuth;
    }
  } catch {
    // Fall through to cookie-backed auth recovery.
  }

  return readPersistedAuthCookie();
}

function writePersistedAuth(token: string, user: AuthUser) {
  if (typeof window !== "undefined") {
    try {
      window.localStorage.setItem(AUTH_STORAGE_KEY, JSON.stringify({ token, user }));
    } catch {
      // Some embedded browsers block storage writes. Keep the cookie fallback.
    }
  }

  writePersistedAuthCookie(token, user);
}

function clearPersistedAuth() {
  if (typeof window !== "undefined") {
    try {
      window.localStorage.removeItem(AUTH_STORAGE_KEY);
    } catch {
      // Ignore storage cleanup failures and still clear the cookie fallback.
    }
  }

  clearPersistedAuthCookie();
}

function withTimeout<T>(promise: Promise<T>, timeoutMs: number) {
  return new Promise<T>((resolve, reject) => {
    const timeoutId = window.setTimeout(() => {
      reject(new Error("Auth bootstrap timed out"));
    }, timeoutMs);

    promise.then(
      (value) => {
        window.clearTimeout(timeoutId);
        resolve(value);
      },
      (error) => {
        window.clearTimeout(timeoutId);
        reject(error);
      },
    );
  });
}

export function AuthProvider({ children }: { children: ReactNode }) {
  const [initialized, setInitialized] = useState(false);
  const [token, setToken] = useState<string | null>(null);
  const [user, setUser] = useState<AuthUser | null>(null);
  const [passwordResetEnabled, setPasswordResetEnabled] = useState(false);

  const clearAuth = useCallback(() => {
    setToken(null);
    setUser(null);
    clearPersistedAuth();
    clearAuthSessionHint();
  }, []);

  const completeAuth = useCallback((payload: AuthPayload) => {
    setToken(payload.token);
    setUser(payload.user);
    writePersistedAuth(payload.token, payload.user);
    setAuthSessionHint();
  }, []);

  const updateUser = useCallback(
    (nextUser: AuthUser) => {
      setUser(nextUser);

      if (token) {
        writePersistedAuth(token, nextUser);
      }
    },
    [token],
  );

  const logout = useCallback(async () => {
    const activeToken = token;

    if (activeToken) {
      try {
        await logoutRequest(activeToken);
      } catch {
        // Keep logout best-effort and clear local session anyway.
      }
    }

    clearAuth();
  }, [clearAuth, token]);

  useEffect(() => {
    let cancelled = false;

    async function loadCapabilities() {
      try {
        const response = await authRuntimeCapabilities();

        if (!cancelled) {
          setPasswordResetEnabled(response.password_reset_enabled);
        }
      } catch {
        if (!cancelled) {
          setPasswordResetEnabled(false);
        }
      }
    }

    loadCapabilities();

    return () => {
      cancelled = true;
    };
  }, []);

  useEffect(() => {
    let cancelled = false;

    async function bootstrap() {
      const persisted = readPersistedAuth();

      if (!persisted) {
        if (!cancelled) {
          setInitialized(true);
        }

        return;
      }

      setToken(persisted.token);
      setUser(persisted.user);

      try {
        const response = await withTimeout(me(persisted.token), AUTH_BOOTSTRAP_TIMEOUT_MS);

        if (!cancelled && response.success && response.data) {
          writePersistedAuth(persisted.token, response.data.user);
          setUser(response.data.user);
          setAuthSessionHint();
        }

        if (!cancelled && (!response.success || !response.data)) {
          clearAuth();
        }
      } catch {
        if (!cancelled) {
          clearAuth();
        }
      } finally {
        if (!cancelled) {
          setInitialized(true);
        }
      }
    }

    bootstrap();

    return () => {
      cancelled = true;
    };
  }, [clearAuth]);

  const value = useMemo<AuthContextValue>(
    () => ({
      initialized,
      isAuthenticated: Boolean(token),
      token,
      user,
      passwordResetEnabled,
      completeAuth,
      updateUser,
      clearAuth,
      logout,
    }),
    [clearAuth, completeAuth, initialized, logout, passwordResetEnabled, token, updateUser, user],
  );

  return <AuthContext.Provider value={value}>{children}</AuthContext.Provider>;
}

export function useAuth() {
  const context = useContext(AuthContext);

  if (!context) {
    throw new Error("useAuth must be used inside AuthProvider");
  }

  return context;
}
