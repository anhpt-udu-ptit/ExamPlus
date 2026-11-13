"use client";

import React from "react";
import Link from "next/link";
import Image from "next/image";
import { Shield, FileText, CheckCircle2, Lock, Mail } from "lucide-react";

interface FooterProps {
  lang?: "vi" | "en";
}

export default function Footer({ lang = "vi" }: FooterProps) {
  const isVi = lang === "vi";

  return (
    <footer className="relative w-full border-t border-slate-200/80 dark:border-white/[0.08] bg-slate-50/70 dark:bg-[#070b14]/70 backdrop-blur-2xl pt-14 pb-12 overflow-hidden transition-colors">
      {/* Subtle top glow */}
      <div className="absolute top-0 left-1/2 -translate-x-1/2 w-3/4 h-[1px] bg-gradient-to-r from-transparent via-blue-500/40 to-transparent" />

      <div className="max-w-7xl mx-auto px-4 sm:px-6 lg:px-8">
        <div className="grid grid-cols-1 md:grid-cols-2 lg:grid-cols-4 gap-10 lg:gap-8 pb-12 border-b border-slate-200/80 dark:border-white/[0.06]">
          {/* Col 1: Brand & Bio */}
          <div className="lg:col-span-1 flex flex-col gap-4">
            <Link href="/" className="flex items-center gap-3">
              <div className="relative w-9 h-9 rounded-xl overflow-hidden shadow-sm shadow-blue-500/20">
                <Image
                  src="/logo_blue.svg"
                  alt="ExamPlus Logo"
                  fill
                  className="object-contain"
                />
              </div>
              <span className="text-xl font-bold tracking-tight text-slate-900 dark:text-white">
                ExamPlus
              </span>
            </Link>
            <p className="text-sm text-slate-600 dark:text-slate-400 leading-relaxed">
              {isVi
                ? "Hệ thống Khảo thí & Giám sát Phòng thi dành cho giáo dục Việt Nam. Bóc tách đề Word công thức sắc nét, tổ chức thi mượt mà trong phòng máy và đồng bộ Google Drive tiện lợi."
                : "Smart Examination & Proctored Test Hub for modern educators. Word formula parsing, smooth classroom testing, and seamless Google Drive sync."}
            </p>
            <div className="flex items-center gap-2 text-xs text-slate-500 dark:text-slate-400 pt-1 flex-wrap">
              <span className="inline-flex items-center gap-1.5 px-2.5 py-1 rounded-full bg-emerald-500/10 text-emerald-600 dark:text-emerald-400 border border-emerald-500/20 font-medium">
                <span className="w-1.5 h-1.5 rounded-full bg-emerald-500 animate-pulse" />
                {isVi ? "Sẵn sàng v1.0" : "Ready v1.0"}
              </span>
              <span className="inline-flex items-center gap-1 px-2.5 py-1 rounded-full bg-blue-500/10 text-blue-600 dark:text-blue-400 border border-blue-500/20 font-medium">
                <CheckCircle2 className="w-3 h-3" />
                {isVi ? "Chuẩn GDPT 2018" : "GDPT 2018 Standard"}
              </span>
            </div>
          </div>

          {/* Col 2: Sản Phẩm */}
          <div className="flex flex-col gap-3">
            <h4 className="text-xs font-semibold text-slate-800 dark:text-slate-300 uppercase tracking-wider">
              {isVi ? "Sản Phẩm" : "Product"}
            </h4>
            <ul className="flex flex-col gap-2.5 text-sm text-slate-600 dark:text-slate-400">
              <li>
                <Link href="/#download" className="hover:text-slate-950 dark:hover:text-white transition-colors">
                  {isVi ? "Tải Bản Windows Portable" : "Download Portable Windows"}
                </Link>
              </li>
              <li>
                <Link href="/#features" className="hover:text-slate-950 dark:hover:text-white transition-colors">
                  {isVi ? "Ý Chính Nổi Bật" : "Core Highlights"}
                </Link>
              </li>
              <li>
                <Link href="/#faq" className="hover:text-slate-950 dark:hover:text-white transition-colors">
                  {isVi ? "Giải Đáp Thắc Mắc (FAQ)" : "Frequently Asked Questions"}
                </Link>
              </li>
            </ul>
          </div>

          {/* Col 3: Pháp Lý & Google Drive */}
          <div className="flex flex-col gap-3">
            <h4 className="text-xs font-semibold text-blue-600 dark:text-blue-400 uppercase tracking-wider flex items-center gap-1.5">
              <Shield className="w-3.5 h-3.5" />
              {isVi ? "Pháp Lý & Bảo Mật" : "Legal & Privacy"}
            </h4>
            <ul className="flex flex-col gap-2.5 text-sm">
              <li>
                <Link
                  href="/privacy"
                  className="text-slate-700 dark:text-slate-300 hover:text-blue-600 dark:hover:text-blue-400 transition-colors font-medium flex items-center gap-1.5"
                >
                  <Lock className="w-3 h-3 text-blue-500" />
                  {isVi ? "Chính Sách Quyền Riêng Tư" : "Privacy Policy"}
                </Link>
              </li>
              <li>
                <Link
                  href="/terms"
                  className="text-slate-700 dark:text-slate-300 hover:text-slate-950 dark:hover:text-white transition-colors flex items-center gap-1.5"
                >
                  <FileText className="w-3 h-3 text-slate-400" />
                  {isVi ? "Điều Khoản Dịch Vụ" : "Terms of Service"}
                </Link>
              </li>
              <li>
                <Link
                  href="/privacy#google-api"
                  className="text-slate-500 dark:text-slate-400 hover:text-slate-950 dark:hover:text-white transition-colors text-xs"
                >
                  {isVi ? "Google API Limited Use" : "Google API Limited Use"}
                </Link>
              </li>
              <li>
                <Link
                  href="/privacy#revoke-access"
                  className="text-slate-500 dark:text-slate-400 hover:text-slate-950 dark:hover:text-white transition-colors text-xs"
                >
                  {isVi ? "Hủy Kết Nối Google Drive" : "Revoke Google Access"}
                </Link>
              </li>
            </ul>
          </div>

          {/* Col 4: Hỗ Trợ & Liên Hệ */}
          <div className="flex flex-col gap-3">
            <h4 className="text-xs font-semibold text-slate-800 dark:text-slate-300 uppercase tracking-wider">
              {isVi ? "Hỗ Trợ Kỹ Thuật" : "Technical Support"}
            </h4>
            <ul className="flex flex-col gap-2.5 text-sm text-slate-600 dark:text-slate-400">
              <li>
                <a
                  href="mailto:examplus.edu@gmail.com"
                  className="hover:text-slate-950 dark:hover:text-white transition-colors flex items-center gap-2 text-blue-600 dark:text-blue-300"
                >
                  <Mail className="w-4 h-4 text-blue-500 shrink-0" />
                  <span className="font-mono text-xs sm:text-sm break-all">examplus.edu@gmail.com</span>
                </a>
              </li>
              <li className="text-xs text-slate-500 dark:text-slate-400 leading-relaxed pt-1">
                {isVi
                  ? "Hỗ trợ giáo viên cấu hình phòng máy, nạp đề thi và hướng dẫn sử dụng phần mềm."
                  : "Assisting teachers with lab setup, exam importing, and software guidance."}
              </li>
            </ul>
          </div>
        </div>

        {/* Bottom bar */}
        <div className="pt-8 flex flex-col sm:flex-row items-center justify-between gap-4 text-xs text-slate-500 dark:text-slate-400">
          <p>© 2026 ExamPlus. Bản quyền phần mềm phục vụ cộng đồng giáo dục Việt Nam.</p>
          <div className="flex items-center gap-4">
            <Link href="/privacy" className="hover:text-slate-800 dark:hover:text-slate-200 transition-colors">
              {isVi ? "Bảo mật & Quyền riêng tư" : "Privacy"}
            </Link>
            <span>•</span>
            <Link href="/terms" className="hover:text-slate-800 dark:hover:text-slate-200 transition-colors">
              {isVi ? "Điều khoản dịch vụ" : "Terms"}
            </Link>
            <span>•</span>
            <a
              href="mailto:examplus.edu@gmail.com"
              className="hover:text-slate-800 dark:hover:text-slate-200 transition-colors"
            >
              Liên hệ
            </a>
          </div>
        </div>
      </div>
    </footer>
  );
}
