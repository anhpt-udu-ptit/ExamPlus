import type { Metadata } from "next";
import { Analytics } from "@vercel/analytics/next";
import "./globals.css";

export const metadata: Metadata = {
  metadataBase: new URL("https://examplus-edu.vercel.app"),
  title: "ExamPlus — Hệ Thống Khảo Thí & Giám Sát Phòng Thi GDPT 2018",
  description:
    "Phần mềm tổ chức thi trắc nghiệm và quản lý phòng thi mượt mà cho giáo viên: Bóc tách đề Word chuẩn nét, tự động chấm điểm theo cấu trúc GDPT 2018, hoạt động 100% trong phòng máy và đồng bộ Google Drive tiện lợi.",
  keywords: [
    "ExamPlus",
    "phần mềm khảo thí",
    "thi trắc nghiệm",
    "GDPT 2018",
    "bóc tách đề thi word",
    "MathType SVG",
    "giám sát phòng thi",
    "Google Drive sync",
  ],
  authors: [{ name: "ExamPlus Team" }],
  icons: {
    icon: [
      { url: "/favicon.ico", sizes: "any" },
      { url: "/logo_blue.png", type: "image/png", sizes: "512x512" },
    ],
    shortcut: "/favicon.ico",
    apple: "/logo_blue.png",
  },
  verification: {
    google: "A8RBJg36uHLbG4MLx3L5go12vDJ5E-C4fAeaPl-oHNQ",
  },
  openGraph: {
    title: "ExamPlus — Hệ Thống Khảo Thí & Giám Sát Phòng Thi GDPT 2018",
    description:
      "Giải pháp toàn diện cho giáo viên: Nạp đề Word công thức rõ nét, tổ chức thi trong phòng máy mượt mà, tự động chấm điểm và đồng bộ Google Drive tiện lợi.",
    url: "https://examplus-edu.vercel.app",
    siteName: "ExamPlus",
    images: [
      {
        url: "/logo_google_brand_512.png",
        width: 512,
        height: 512,
        alt: "ExamPlus Logo",
      },
    ],
    locale: "vi_VN",
    type: "website",
  },
};

export default function RootLayout({
  children,
}: Readonly<{
  children: React.ReactNode;
}>) {
  return (
    <html lang="vi" className="dark scroll-smooth" suppressHydrationWarning>
      <head>
        <meta name="google-site-verification" content="A8RBJg36uHLbG4MLx3L5go12vDJ5E-C4fAeaPl-oHNQ" />
        <link rel="icon" href="/favicon.ico" sizes="any" />
        {/* Anti-flicker theme initialization script */}
        <script
          dangerouslySetInnerHTML={{
            __html: `
              (function() {
                try {
                  var saved = localStorage.getItem('examplus_theme');
                  if (saved === 'light') {
                    document.documentElement.classList.remove('dark');
                    document.documentElement.setAttribute('data-theme', 'light');
                  } else {
                    document.documentElement.classList.add('dark');
                    document.documentElement.setAttribute('data-theme', 'dark');
                  }
                } catch(e) {}
              })();
            `,
          }}
        />
      </head>
      <body className="min-h-screen text-slate-900 dark:text-slate-100 flex flex-col antialiased selection:bg-blue-600 selection:text-white">
        {children}
        <Analytics />
      </body>
    </html>
  );
}
