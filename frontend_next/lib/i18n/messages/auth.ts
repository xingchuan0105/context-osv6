import type { UiMessageDescriptor } from "./types";

export const authMessages = {
  authConfirmPasswordLabel: {
    zh: "确认密码",
    en: "Confirm password",
  },
  authContinueToLogin: {
    zh: "去登录",
    en: "Go to sign in",
  },
  authCreateAccount: {
    zh: "创建账号",
    en: "Create account",
  },
  authCreatingAccount: {
    zh: "创建中...",
    en: "Creating...",
  },
  authEmailAndCodeRequired: {
    zh: "请输入邮箱和验证码。",
    en: "Enter your email and verification code.",
  },
  authEmailAndPasswordRequired: {
    zh: "请输入邮箱和密码。",
    en: "Enter your email and password.",
  },
  authEmailLabel: {
    zh: "邮箱",
    en: "Email",
  },
  authForgotPassword: {
    zh: "忘记密码",
    en: "Forgot password",
  },
  authHasAccount: {
    zh: "已有账号？",
    en: "Already have an account?",
  },
  authLoginFailed: {
    zh: "登录失败，请稍后再试。",
    en: "Sign-in failed. Try again later.",
  },
  authLoginSubmit: {
    zh: "继续登录",
    en: "Continue",
  },
  authLoginSubmitting: {
    zh: "登录中...",
    en: "Signing in...",
  },
  authLoginSubtitle: {
    zh: "登录以继续进入您的工作区",
    en: "Sign in to continue into your workspace",
  },
  authLoginTitle: {
    zh: "欢迎回来",
    en: "Welcome back",
  },
  authNameLabel: {
    zh: "姓名",
    en: "Name",
  },
  authNeedsAccount: {
    zh: "还没有账号？",
    en: "Need an account?",
  },
  authNewPasswordLabel: {
    zh: "新密码",
    en: "New password",
  },
  authNewPasswordRequired: {
    zh: "请输入新密码。",
    en: "Enter a new password.",
  },
  authOptional: {
    zh: "可选",
    en: "Optional",
  },
  authPasswordLabel: {
    zh: "密码",
    en: "Password",
  },
  authPasswordMinLengthHint: {
    zh: "至少 8 位",
    en: "At least 8 characters",
  },
  authPasswordMinLengthRequired: {
    zh: "密码至少需要 8 位。",
    en: "Password must be at least 8 characters.",
  },
  authPasswordMismatch: {
    zh: "两次输入的密码不一致。",
    en: "Passwords do not match.",
  },
  authRegisterFailed: {
    zh: "创建账号失败，请稍后再试。",
    en: "Account creation failed. Try again later.",
  },
  authRegisterSubtitle: {
    zh: "使用邮箱创建账号，开始使用 Context OS。",
    en: "Create your account with email to get started with Context OS.",
  },
  authResetBackToLogin: {
    zh: "返回登录",
    en: "Back to sign in",
  },
  authResetBackToPrevious: {
    zh: "返回上一步",
    en: "Back",
  },
  authResetBackToStart: {
    zh: "返回找回密码",
    en: "Back to reset",
  },
  authResetCodeHint: {
    zh: "6 位验证码",
    en: "6-digit code",
  },
  authResetCodeLabel: {
    zh: "验证码",
    en: "Verification code",
  },
  authResetConfirmFailed: {
    zh: "重置密码失败，请稍后再试。",
    en: "Password reset failed. Try again later.",
  },
  authResetConfirmSubmit: {
    zh: "完成重置",
    en: "Finish reset",
  },
  authResetConfirmSubmitting: {
    zh: "提交中...",
    en: "Submitting...",
  },
  authResetConfirmSubtitle: {
    zh: "输入新密码后即可完成重置。",
    en: "Set a new password to finish the reset flow.",
  },
  authResetConfirmTitle: {
    zh: "设置新密码",
    en: "Set a new password",
  },
  authResetConfirmUnavailable: {
    zh: "请先完成验证码验证。",
    en: "Complete code verification first.",
  },
  authResetEmailRequired: {
    zh: "请输入邮箱。",
    en: "Enter your email.",
  },
  authResetRequestSubtitle: {
    zh: "输入邮箱后，我们会发送验证码到你的邮箱。",
    en: "Enter your email and we'll send a verification code.",
  },
  authResetRequestTitle: {
    zh: "找回密码",
    en: "Reset password",
  },
  authResetSendFailed: {
    zh: "发送验证码失败，请稍后再试。",
    en: "Failed to send the verification code. Try again later.",
  },
  authResetSendSubmit: {
    zh: "发送验证码",
    en: "Send code",
  },
  authResetSendSubmitting: {
    zh: "发送中...",
    en: "Sending...",
  },
  authResetUnavailable: {
    zh: "密码重置暂不可用。",
    en: "Password reset is currently unavailable.",
  },
  authResetVerifyFailed: {
    zh: "验证验证码失败，请稍后再试。",
    en: "Verification failed. Try again later.",
  },
  authResetVerifySubmit: {
    zh: "继续",
    en: "Continue",
  },
  authResetVerifySubmitting: {
    zh: "验证中...",
    en: "Verifying...",
  },
  authResetVerifySubtitle: {
    zh: "输入邮箱和验证码，继续到设置新密码。",
    en: "Enter your email and verification code to continue.",
  },
  authResetVerifyTitle: {
    zh: "验证验证码",
    en: "Verify code",
  },
  authSignIn: {
    zh: "去登录",
    en: "Sign in",
  },
  authSignUp: {
    zh: "去注册",
    en: "Sign up",
  },
  authVerificationRequired: {
    zh: "请先完成验证码验证。",
    en: "Complete verification first.",
  },
  authErrorAccountNotRegistered: {
    zh: "此账号还未注册，请先注册。",
    en: "This account is not registered yet. Sign up first.",
  },
  authErrorEmailExists: {
    zh: "该邮箱已注册，请直接登录。",
    en: "This email is already registered. Sign in instead.",
  },
  authErrorInvalidCredentials: {
    zh: "邮箱或密码错误。",
    en: "Incorrect email or password.",
  },
  authErrorInvalidPassword: {
    zh: "密码错误。",
    en: "Incorrect password.",
  },
  authErrorInvalidResetTicket: {
    zh: "重置会话无效或已过期。",
    en: "The reset session is invalid or has expired.",
  },
  authErrorPasswordResetUnavailable: {
    zh: "当前环境未启用密码找回，请联系管理员。",
    en: "Password reset is unavailable in this environment. Contact an administrator.",
  },
  authErrorServiceUnavailable: {
    zh: "服务暂时不可用，请稍后再试。",
    en: "The service is temporarily unavailable. Try again later.",
  },
  authErrorConsentRequired: {
    zh: "请先阅读并同意用户协议与隐私政策。",
    en: "Read and accept the Terms of Service and Privacy Policy first.",
  },
  authErrorInvalidTermsVersion: {
    zh: "用户协议版本已更新，请刷新页面后重试。",
    en: "The Terms of Service were updated. Refresh the page and try again.",
  },
  authErrorInvalidPrivacyVersion: {
    zh: "隐私政策版本已更新，请刷新页面后重试。",
    en: "The Privacy Policy was updated. Refresh the page and try again.",
  },
  authErrorInvalidLegalContext: {
    zh: "法律同意记录无效，请刷新页面后重试。",
    en: "The legal acceptance request was invalid. Refresh the page and try again.",
  },
} satisfies Record<string, UiMessageDescriptor>;
