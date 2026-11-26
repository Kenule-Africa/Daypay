import type { Metadata, Viewport } from "next";
import localFont from "next/font/local";
import { connection } from "next/server";
import "./globals.css";

// Self-hosted variable fonts under the SIL Open Font License (Fontsource packages).
const body = localFont({
  src: "../node_modules/@fontsource-variable/instrument-sans/files/instrument-sans-latin-wght-normal.woff2",
  variable: "--font-body",
  weight: "400 700",
  display: "swap",
});

const heading = localFont({
  src: "../node_modules/@fontsource-variable/bricolage-grotesque/files/bricolage-grotesque-latin-wght-normal.woff2",
  variable: "--font-heading",
  weight: "200 800",
  display: "swap",
});

export const metadata: Metadata = {
  title: { default: "DEYPAY — Digital dollars for everyday Nigerian money", template: "%s · DEYPAY" },
  description:
    "DEYPAY is a stablecoin-first account built for Nigeria. Create your account today; wallets and payments are in development.",
  applicationName: "DEYPAY",
  formatDetection: { telephone: false, email: false, address: false },
};

export const viewport: Viewport = {
  themeColor: "#f6f4ef",
  colorScheme: "light",
  width: "device-width",
  initialScale: 1,
  viewportFit: "cover",
};

export default async function RootLayout({ children }: Readonly<{ children: React.ReactNode }>) {
  // Every page is rendered per request so Next.js can attach the middleware's
  // CSP nonce to its scripts. No page is prerendered with private data.
  await connection();
  return (
    <html lang="en" className={`${body.variable} ${heading.variable}`}>
      <body className="min-h-dvh">{children}</body>
    </html>
  );
}
