import setupEnv from "./setup-env";
import setupAuth from "./setup-auth";

/**
 * globalSetup 对外单入口（兼容 Playwright 的 string 类型 API）。
 * 内部按顺序调用 setupEnv → setupAuth，职责分离但对外保持单一入口。
 *
 * 失败策略：每一步失败都会打印 clear error message 并抛出异常，
 * Playwright 会以非零 exit code 退出，不会继续执行任何 spec。
 */
export default async function globalSetup() {
  try {
    await setupEnv();
  } catch (e) {
    console.error("[global-setup] setup-env failed:", e);
    throw e;
  }

  try {
    await setupAuth();
  } catch (e) {
    console.error("[global-setup] setup-auth failed:", e);
    throw e;
  }

  console.log("[global-setup] env + auth ready");
}
