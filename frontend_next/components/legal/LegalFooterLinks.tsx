import Link from "next/link";

export default function LegalFooterLinks() {
  const currentYear = new Date().getFullYear();

  return (
    <footer className="legal-footer-links">
      <div className="legal-footer-content">
        <Link href="/legal/terms">用户协议</Link>
        <span className="legal-footer-separator">·</span>
        <Link href="/legal/privacy">隐私政策</Link>
        <span className="legal-footer-separator">·</span>
        <Link href="/legal/licenses">开源声明</Link>
      </div>
      <div className="legal-footer-copyright">
        © {currentYear} Context-OS
      </div>
    </footer>
  );
}
