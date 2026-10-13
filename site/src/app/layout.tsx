import type { Metadata } from "next";
import { DM_Sans, Inter } from "next/font/google";
import localFont from "next/font/local";
import "./globals.css";

// Paper Mono (github.com/paper-design/paper-mono), SIL OFL 1.1: see src/fonts/PaperMono-OFL.txt
const mono = localFont({
  src: "../fonts/PaperMono-Variable.woff2",
  weight: "100 800",
  variable: "--font-paper-mono",
  fallback: ["ui-monospace", "monospace"],
});
const inter = Inter({ subsets: ["latin"], weight: ["400", "500", "600", "700"], variable: "--font-inter" });
const dmSans = DM_Sans({ subsets: ["latin"], style: ["normal", "italic"], display: "swap", variable: "--font-dm-sans" });

export const metadata: Metadata = {
  metadataBase: new URL("https://feanor.shift.graphics"),
  title: "Feanor: a formatter and linter for OpenType feature files",
  description:
    "Format and lint .fea feature code. Layout-only formatting that never changes the compiled font, and diagnostics with fixes. Written in Rust, by Shift.",
};

export default function RootLayout({ children }: { children: React.ReactNode }) {
  return (
    <html lang="en" className={`${mono.variable} ${inter.variable} ${dmSans.variable}`}>
      <head>
        {/* Shift's Adobe Fonts kit, for aktiv-grotesk headings as on shift.graphics */}
        <link rel="stylesheet" href="https://use.typekit.net/xvg1cxs.css" />
      </head>
      <body className="min-h-screen antialiased">{children}</body>
    </html>
  );
}
