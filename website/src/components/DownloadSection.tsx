"use client";

import React from "react";
import { Download, CheckCircle2, Laptop, HardDrive, Zap } from "lucide-react";

interface DownloadSectionProps {
  lang?: "vi" | "en";
}

export default function DownloadSection({ lang = "vi" }: DownloadSectionProps) {
  const isVi = lang === "vi";

  return (
    <section id="download" className="relative py-16 sm:py-20 px-4 sm:px-6 lg:px-8">
      <div className="max-w-5xl mx-auto">
        <div className="relative rounded-3xl p-6 sm:p-10 bg-white/92 dark:bg-[#0c1224]/90 backdrop-blur-2xl border border-slate-200/90 dark:border-white/10 shadow-2xl overflow-hidden">
          <div className="grid grid-cols-1 lg:grid-cols-12 gap-8 items-center">
            {/* Left: Info */}
            <div className="lg:col-span-7 flex flex-col gap-4">
              <div className="inline-flex items-center gap-2 px-3.5 py-1.5 rounded-full bg-emerald-500/10 border border-emerald-500/20 text-emerald-600 dark:text-emerald-400 text-xs font-semibold w-max">
                <span className="uiv-radar" aria-hidden="true">
                  <span className="uiv-radar__dot" />
                  <span className="uiv-radar__ping" />
                </span>
                <span>{isVi ? "Bản Chính Thức v1.0" : "Official v1.0"}</span>
              </div>

              <h2 className="text-2xl sm:text-4xl font-extrabold text-slate-900 dark:text-white tracking-tight">
                {isVi ? "Tải ExamPlus Cho Windows" : "Download ExamPlus"}
              </h2>

              <p className="text-sm sm:text-base text-slate-700 dark:text-slate-300 leading-relaxed">
                {isVi
                  ? "Bản Portable tiện lợi, giải nén là chạy ngay trên máy tính Windows. Dễ dàng sao chép vào USB để mang đi dạy hoặc tổ chức thi tại các phòng máy."
                  : "Standalone portable package. Extract and run instantly on Windows. Easily copy to a USB drive for classroom use."}
              </p>

              {/* Specs badges */}
              <div className="grid grid-cols-2 sm:grid-cols-3 gap-2.5 pt-1">
                <div className="p-3 rounded-2xl border border-slate-200/90 dark:border-white/10 bg-slate-100/90 dark:bg-white/[0.04] backdrop-blur-md flex items-center gap-2.5 text-xs text-slate-800 dark:text-slate-200 font-medium">
                  <Laptop className="w-4 h-4 text-blue-500 shrink-0" />
                  <span>Win 10 / 11 (64-bit)</span>
                </div>
                <div className="p-3 rounded-2xl border border-slate-200/90 dark:border-white/10 bg-slate-100/90 dark:bg-white/[0.04] backdrop-blur-md flex items-center gap-2.5 text-xs text-slate-800 dark:text-slate-200 font-medium">
                  <HardDrive className="w-4 h-4 text-cyan-500 shrink-0" />
                  <span>Gọn nhẹ ~25 MB</span>
                </div>
                <div className="p-3 rounded-2xl border border-slate-200/90 dark:border-white/10 bg-slate-100/90 dark:bg-white/[0.04] backdrop-blur-md flex items-center gap-2.5 text-xs text-slate-800 dark:text-slate-200 font-medium col-span-2 sm:col-span-1">
                  <Zap className="w-4 h-4 text-amber-500 shrink-0" />
                  <span>Khởi động tức thì</span>
                </div>
              </div>
            </div>

            {/* Right: Download Card */}
            <div className="lg:col-span-5 flex flex-col gap-4 p-6 sm:p-7 rounded-2xl bg-slate-950 text-white shadow-xl border border-slate-800 backdrop-blur-xl">
              <div className="flex items-center justify-between pb-3 border-b border-slate-800 text-xs">
                <span className="font-bold text-sm">ExamPlus Portable</span>
                <span className="font-mono text-blue-400 font-semibold">v1.0.0</span>
              </div>

              <div className="flex flex-col gap-2 text-xs text-slate-400 py-1 font-mono">
                <div className="flex justify-between">
                  <span>Tập tin:</span>
                  <span className="text-slate-200">ExamPlus-v1.0.0-x64.zip</span>
                </div>
                <div className="flex justify-between">
                  <span>Hệ điều hành:</span>
                  <span className="text-slate-200">Windows 10 / 11</span>
                </div>
                <div className="flex justify-between">
                  <span>Cài đặt:</span>
                  <span className="text-emerald-400">Không cần cài đặt</span>
                </div>
              </div>

              {/* Download CTA Button */}
              <a
                href="/downloads/ExamPlus-v1.0.0-Windows-x64-Portable.zip"
                download="ExamPlus-v1.0.0-Windows-x64-Portable.zip"
                className="btn-tactile-primary w-full text-center mt-2"
              >
                <Download className="w-4 h-4 mr-1.5" aria-hidden="true" />
                <span>{isVi ? "Tải Bản Portable (.zip)" : "Download Portable (.zip)"}</span>
              </a>

              <p className="text-[11px] text-center text-slate-400 leading-relaxed">
                {isVi
                  ? "Phần mềm miễn phí phục vụ giáo viên và trường học Việt Nam."
                  : "Free for educators and schools."}
              </p>
            </div>
          </div>
        </div>
      </div>
    </section>
  );
}
