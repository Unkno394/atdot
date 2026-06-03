import type { Metadata } from "next";
import { Geist, Geist_Mono } from "next/font/google";
import Script from "next/script";
import "./globals.css";

const SDK_KEY = process.env.NEXT_PUBLIC_ATDOT_KEY
const API_URL = process.env.NEXT_PUBLIC_API_URL ?? 'http://localhost:8080'

const geistSans = Geist({
  variable: "--font-geist-sans",
  subsets: ["latin"],
});

const geistMono = Geist_Mono({
  variable: "--font-geist-mono",
  subsets: ["latin"],
});

export const metadata: Metadata = {
  title: "ATdot — антифрод платформа",
  description: "Анализ поведения пользователей и обнаружение фрода",
};

export default function RootLayout({
  children,
}: Readonly<{
  children: React.ReactNode;
}>) {
  return (
    <html
      lang="en"
      className={`${geistSans.variable} ${geistMono.variable} h-full antialiased`}
    >
      <body className="min-h-full flex flex-col">
        {children}
        {SDK_KEY && (
          <Script
            src="/sdk/atdot.js"
            data-key={SDK_KEY}
            data-endpoint={API_URL}
            strategy="afterInteractive"
          />
        )}
      </body>
    </html>
  );
}
