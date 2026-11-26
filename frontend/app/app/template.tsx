/** Re-mounted on each account navigation, giving every page a gentle entrance. */
export default function AccountTemplate({ children }: { children: React.ReactNode }) {
  return <div className="animate-rise">{children}</div>;
}
