import type { Metadata } from "next";
import { Fragment_Mono, Inter } from "next/font/google";
import "./globals.css";

const mono = Fragment_Mono({ subsets: ["latin"], weight: "400", variable: "--font-fragment-mono" });
const inter = Inter({ subsets: ["latin"], variable: "--font-inter" });

export const metadata: Metadata = {
  metadataBase: new URL("https://feanor.shift.graphics"),
  title: "Feanor: a formatter and linter for OpenType feature files",
  description:
    "Format and lint .fea feature code. Layout-only formatting that never changes the compiled font, and diagnostics with fixes. Written in Rust, by Shift.",
};

export default function RootLayout({ children }: { children: React.ReactNode }) {
  return (
    <html lang="en" className={`${mono.variable} ${inter.variable}`}>
      <body className="min-h-screen">{children}</body>
    </html>
  );
}
