import type { Metadata } from "next";
import { DM_Sans, Fragment_Mono, Inter } from "next/font/google";
import "./globals.css";

const mono = Fragment_Mono({ subsets: ["latin"], weight: "400", variable: "--font-fragment-mono" });
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
