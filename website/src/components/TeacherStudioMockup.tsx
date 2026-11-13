"use client";

import React, { useState } from "react";
import {
  FileText,
  Users,
  CheckCircle2,
  Clock,
  Sparkles,
  Download,
  AlertCircle,
  ShieldCheck,
  ChevronRight,
} from "lucide-react";

interface TeacherStudioMockupProps {
  lang?: "vi" | "en";
}

export default function TeacherStudioMockup({ lang = "vi" }: TeacherStudioMockupProps) {
  const isVi = lang === "vi";
  const [tab, setTab] = useState<"import" | "monitor" | "grading">("import");

  return (
    <div className="w-full max-w-4xl mx-auto rounded-3xl p-1 sm:p-2 bg-gradient-to-b from-blue-500/20 via-slate-200/50 to-slate-200/10 dark:from-blue-500/25 dark:via-white/10 dark:to-transparent shadow-[0_20px_50px_-15px_rgba(37,99,235,0.2)] overflow-hidden">
      <div className="rounded-2xl bg-white/95 dark:bg-[#0c1224]/95 backdrop-blur-2xl border border-slate-200/90 dark:border-white/10 overflow-hidden shadow-2xl">
        {/* Window Chrome Header */}
        <div className="flex flex-wrap items-center justify-between gap-2.5 px-3.5 sm:px-6 py-3 bg-slate-100/80 dark:bg-slate-900/80 border-b border-slate-200/80 dark:border-white/10 text-xs">
          {/* macOS / Windows Window Dots */}
          <div className="flex items-center gap-1.5 sm:gap-2">
            <span className="w-2.5 h-2.5 sm:w-3 sm:h-3 rounded-full bg-rose-500/80 inline-block" />
            <span className="w-2.5 h-2.5 sm:w-3 sm:h-3 rounded-full bg-amber-500/80 inline-block" />
            <span className="w-2.5 h-2.5 sm:w-3 sm:h-3 rounded-full bg-emerald-500/80 inline-block" />
            <span className="ml-1.5 font-mono font-semibold text-slate-700 dark:text-slate-300 text-[11px] sm:text-xs">
              Phòng 12A1
            </span>
          </div>

          {/* Live Session Telemetry Pill */}
          <div className="flex items-center gap-2">
            <span className="inline-flex items-center gap-1 px-2.5 py-1 rounded-full bg-emerald-500/10 border border-emerald-500/25 text-emerald-600 dark:text-emerald-400 font-mono text-[10.5px] sm:text-[11px] font-semibold">
              <span className="uiv-radar" aria-hidden="true">
                <span className="uiv-radar__dot" />
                <span className="uiv-radar__ping" />
              </span>
              42/42 Thí sinh
            </span>
            <span className="hidden sm:inline-flex items-center gap-1 text-[11px] text-slate-500 dark:text-slate-400 font-mono">
              <span className="uiv-equalizer mr-1" aria-hidden="true">
                <span />
                <span />
                <span />
                <span />
              </span>
              &lt; 5ms LAN
            </span>
          </div>
        </div>

        {/* Tab Controls (Scrollable on small viewports with no overflow) */}
        <div className="flex items-center justify-start sm:justify-center p-2.5 sm:p-3 border-b border-slate-100 dark:border-white/5 bg-slate-50/50 dark:bg-white/[0.02] overflow-x-auto">
          <div className="cir-tabs shrink-0" role="tablist" aria-label="Các mô phỏng tính năng">
            <button
              type="button"
              role="tab"
              aria-selected={tab === "import"}
              onClick={() => setTab("import")}
              className={`cir-tabs__t !px-3 sm:!px-4 !text-xs sm:!text-[13.5px] ${tab === "import" ? "active" : ""}`}
            >
              <FileText className="w-3.5 h-3.5 mr-1 text-blue-500 shrink-0" aria-hidden="true" />
              <span className="sm:hidden">1. Nạp Đề</span>
              <span className="hidden sm:inline">{isVi ? "1. Nạp Đề Word SVG" : "1. Word Exam Parser"}</span>
            </button>
            <button
              type="button"
              role="tab"
              aria-selected={tab === "monitor"}
              onClick={() => setTab("monitor")}
              className={`cir-tabs__t !px-3 sm:!px-4 !text-xs sm:!text-[13.5px] ${tab === "monitor" ? "active" : ""}`}
            >
              <Users className="w-3.5 h-3.5 mr-1 text-emerald-500 shrink-0" aria-hidden="true" />
              <span className="sm:hidden">2. Giám Sát</span>
              <span className="hidden sm:inline">{isVi ? "2. Giám Sát Phòng Thi" : "2. Live Lab Monitor"}</span>
            </button>
            <button
              type="button"
              role="tab"
              aria-selected={tab === "grading"}
              onClick={() => setTab("grading")}
              className={`cir-tabs__t !px-3 sm:!px-4 !text-xs sm:!text-[13.5px] ${tab === "grading" ? "active" : ""}`}
            >
              <Sparkles className="w-3.5 h-3.5 mr-1 text-amber-500 shrink-0" aria-hidden="true" />
              <span className="sm:hidden">3. Chấm Điểm</span>
              <span className="hidden sm:inline">{isVi ? "3. Chấm Điểm GDPT" : "3. MOET Grading"}</span>
            </button>
          </div>
        </div>

        {/* Tab Content Body */}
        <div className="p-3.5 sm:p-6 min-h-[320px] flex flex-col justify-center overflow-hidden">
          {/* TAB 1: NẠP ĐỀ WORD SVG */}
          {tab === "import" && (
            <div className="flex flex-col gap-4 animate-in fade-in duration-200">
              {/* File Info Bar */}
              <div className="flex flex-wrap items-center justify-between gap-2.5 p-3 rounded-2xl bg-blue-50/80 dark:bg-blue-950/30 border border-blue-200/80 dark:border-blue-500/20">
                <div className="flex items-center gap-2.5 min-w-0">
                  <div className="w-9 h-9 rounded-xl bg-blue-600 text-white flex items-center justify-center font-bold text-xs shadow-md shrink-0">
                    DOCX
                  </div>
                  <div className="min-w-0">
                    <div className="text-xs sm:text-sm font-bold text-slate-900 dark:text-white truncate">
                      De_VatLy_12_GDPT2018.docx
                    </div>
                    <div className="text-[11px] text-slate-500 dark:text-slate-400 truncate">
                      28 Câu hỏi • Công thức MathType SVG sắc nét
                    </div>
                  </div>
                </div>
                <div className="inline-flex items-center gap-1.5 px-2.5 py-1 rounded-full bg-emerald-500/10 text-emerald-600 dark:text-emerald-400 text-[11px] font-semibold shrink-0">
                  <CheckCircle2 className="w-3.5 h-3.5" aria-hidden="true" />
                  <span>{isVi ? "Đáp án đầy đủ" : "Keys parsed"}</span>
                </div>
              </div>

              {/* Sample Question Preview with Crisp Formulas */}
              <div className="p-3.5 sm:p-5 rounded-2xl bg-slate-50 dark:bg-slate-900/60 border border-slate-200/80 dark:border-white/10 flex flex-col gap-2.5">
                <div className="flex items-center justify-between text-[11px] sm:text-xs font-bold text-blue-600 dark:text-blue-400">
                  <span>CÂU 14 • PHẦN I (Trắc nghiệm)</span>
                  <span className="font-mono text-slate-500 dark:text-slate-400">0.25 Điểm</span>
                </div>
                <p className="text-xs sm:text-sm font-medium text-slate-900 dark:text-slate-100 leading-relaxed">
                  Một vật dao động điều hòa theo phương trình vận tốc và gia tốc. Chu kỳ dao động của con lắc lò xo được xác định theo hệ thức:
                </p>

                {/* Simulated MathType Vector SVG Formulas */}
                <div className="grid grid-cols-1 sm:grid-cols-2 gap-2 pt-1">
                  <div className="p-2.5 rounded-xl border border-blue-500/80 bg-blue-50/90 dark:bg-blue-900/25 flex items-center gap-2 text-xs text-blue-800 dark:text-blue-300 font-semibold shadow-sm">
                    <span className="w-4 h-4 rounded-full bg-blue-600 text-white inline-flex items-center justify-center text-[10px] shrink-0">
                      A
                    </span>
                    <span className="truncate">T = 2π √(m / k) (Đúng)</span>
                  </div>
                  <div className="p-2.5 rounded-xl border border-slate-200 dark:border-white/10 bg-white/80 dark:bg-slate-800/50 flex items-center gap-2 text-xs text-slate-700 dark:text-slate-300">
                    <span className="w-4 h-4 rounded-full bg-slate-200 dark:bg-slate-700 text-slate-700 dark:text-slate-300 inline-flex items-center justify-center text-[10px] shrink-0">
                      B
                    </span>
                    <span className="truncate">T = 2π √(k / m)</span>
                  </div>
                  <div className="p-2.5 rounded-xl border border-slate-200 dark:border-white/10 bg-white/80 dark:bg-slate-800/50 flex items-center gap-2 text-xs text-slate-700 dark:text-slate-300">
                    <span className="w-4 h-4 rounded-full bg-slate-200 dark:bg-slate-700 text-slate-700 dark:text-slate-300 inline-flex items-center justify-center text-[10px] shrink-0">
                      C
                    </span>
                    <span className="truncate">f = 1 / (2π) √(k / m)</span>
                  </div>
                  <div className="p-2.5 rounded-xl border border-slate-200 dark:border-white/10 bg-white/80 dark:bg-slate-800/50 flex items-center gap-2 text-xs text-slate-700 dark:text-slate-300">
                    <span className="w-4 h-4 rounded-full bg-slate-200 dark:bg-slate-700 text-slate-700 dark:text-slate-300 inline-flex items-center justify-center text-[10px] shrink-0">
                      D
                    </span>
                    <span className="truncate">ω = √(m / k)</span>
                  </div>
                </div>
              </div>
            </div>
          )}

          {/* TAB 2: GIÁM SÁT PHÒNG THI THỜI GIAN THỰC */}
          {tab === "monitor" && (
            <div className="flex flex-col gap-4 animate-in fade-in duration-200">
              <div className="flex items-center justify-between text-xs pb-1">
                <span className="font-semibold text-slate-700 dark:text-slate-300 flex items-center gap-1.5">
                  <Clock className="w-4 h-4 text-blue-500" aria-hidden="true" />
                  Thời gian làm bài còn lại: <strong className="font-mono text-blue-600 dark:text-blue-400">14:28</strong> / 50 phút
                </span>
                <span className="text-emerald-600 dark:text-emerald-400 font-semibold flex items-center gap-1">
                  <ShieldCheck className="w-4 h-4" aria-hidden="true" />
                  Chế độ chống mất kết nối 30s: Sẵn sàng
                </span>
              </div>

              {/* Candidates Grid */}
              <div className="grid grid-cols-2 sm:grid-cols-3 gap-2.5">
                <div className="p-3 rounded-xl bg-slate-50 dark:bg-slate-900/60 border border-slate-200/80 dark:border-white/10 flex flex-col gap-1">
                  <div className="flex items-center justify-between text-[11px]">
                    <span className="font-bold text-slate-800 dark:text-slate-200">Máy 01 • Nguyễn An</span>
                    <span className="text-emerald-600 dark:text-emerald-400 font-bold">9.25đ</span>
                  </div>
                  <div className="text-[11px] text-emerald-600 dark:text-emerald-400 font-medium">
                    ✓ Đã nộp bài (38 phút)
                  </div>
                </div>

                <div className="p-3 rounded-xl bg-slate-50 dark:bg-slate-900/60 border border-blue-500/40 dark:border-blue-500/30 flex flex-col gap-1">
                  <div className="flex items-center justify-between text-[11px]">
                    <span className="font-bold text-slate-800 dark:text-slate-200">Máy 02 • Trần Bích</span>
                    <span className="text-blue-600 dark:text-blue-400 font-bold">26/28 câu</span>
                  </div>
                  <div className="text-[11px] text-blue-600 dark:text-blue-400 font-medium">
                    Đang làm Phần III (Điền số)
                  </div>
                </div>

                <div className="p-3 rounded-xl bg-slate-50 dark:bg-slate-900/60 border border-slate-200/80 dark:border-white/10 flex flex-col gap-1">
                  <div className="flex items-center justify-between text-[11px]">
                    <span className="font-bold text-slate-800 dark:text-slate-200">Máy 03 • Lê Nam</span>
                    <span className="text-amber-600 dark:text-amber-400 font-bold">20/28 câu</span>
                  </div>
                  <div className="text-[11px] text-amber-600 dark:text-amber-400 font-medium">
                    Đang làm Phần II (Đúng/Sai)
                  </div>
                </div>

                <div className="p-3 rounded-xl bg-slate-50 dark:bg-slate-900/60 border border-slate-200/80 dark:border-white/10 flex flex-col gap-1">
                  <div className="flex items-center justify-between text-[11px]">
                    <span className="font-bold text-slate-800 dark:text-slate-200">Máy 04 • Thu Trang</span>
                    <span className="text-blue-600 dark:text-blue-400 font-bold">24/28 câu</span>
                  </div>
                  <div className="text-[11px] text-slate-500 dark:text-slate-400">
                    Đang làm Phần III
                  </div>
                </div>

                <div className="p-3 rounded-xl bg-slate-50 dark:bg-slate-900/60 border border-slate-200/80 dark:border-white/10 flex flex-col gap-1">
                  <div className="flex items-center justify-between text-[11px]">
                    <span className="font-bold text-slate-800 dark:text-slate-200">Máy 05 • Tuấn Minh</span>
                    <span className="text-blue-600 dark:text-blue-400 font-bold">28/28 câu</span>
                  </div>
                  <div className="text-[11px] text-blue-600 dark:text-blue-400 font-medium">
                    Đang rà soát lại bài
                  </div>
                </div>

                <div className="p-3 rounded-xl bg-slate-50 dark:bg-slate-900/60 border border-slate-200/80 dark:border-white/10 flex flex-col gap-1">
                  <div className="flex items-center justify-between text-[11px]">
                    <span className="font-bold text-slate-800 dark:text-slate-200">Máy 06 • Hoàng Lan</span>
                    <span className="text-emerald-600 dark:text-emerald-400 font-bold">9.75đ</span>
                  </div>
                  <div className="text-[11px] text-emerald-600 dark:text-emerald-400 font-medium">
                    ✓ Đã nộp bài (41 phút)
                  </div>
                </div>
              </div>
            </div>
          )}

          {/* TAB 3: CHẤM ĐIỂM CHUẨN GDPT 2018 & XUẤT EXCEL */}
          {tab === "grading" && (
            <div className="flex flex-col gap-4 animate-in fade-in duration-200">
              <div className="grid grid-cols-1 sm:grid-cols-3 gap-3">
                <div className="p-3.5 rounded-2xl bg-blue-50/80 dark:bg-blue-950/30 border border-blue-200/80 dark:border-blue-500/20">
                  <div className="text-xs text-blue-600 dark:text-blue-400 font-bold">PHẦN I (18 Câu)</div>
                  <div className="text-lg font-bold text-slate-900 dark:text-white mt-1">4.50 Điểm</div>
                  <div className="text-[11px] text-slate-600 dark:text-slate-400 mt-0.5">Trắc nghiệm 4 lựa chọn (0.25đ / câu)</div>
                </div>

                <div className="p-3.5 rounded-2xl bg-amber-50/80 dark:bg-amber-950/30 border border-amber-200/80 dark:border-amber-500/20">
                  <div className="text-xs text-amber-600 dark:text-amber-400 font-bold">PHẦN II (4 Câu)</div>
                  <div className="text-lg font-bold text-slate-900 dark:text-white mt-1">4.00 Điểm</div>
                  <div className="text-[11px] text-slate-600 dark:text-slate-400 mt-0.5">Đúng/Sai bậc thang (0.1 - 0.25 - 0.5 - 1.0đ)</div>
                </div>

                <div className="p-3.5 rounded-2xl bg-emerald-50/80 dark:bg-emerald-950/30 border border-emerald-200/80 dark:border-emerald-500/20">
                  <div className="text-xs text-emerald-600 dark:text-emerald-400 font-bold">PHẦN III (6 Câu)</div>
                  <div className="text-lg font-bold text-slate-900 dark:text-white mt-1">1.50 Điểm</div>
                  <div className="text-[11px] text-slate-600 dark:text-slate-400 mt-0.5">Trả lời ngắn điền số (0.25đ / câu)</div>
                </div>
              </div>

              {/* Action Bar */}
              <div className="flex flex-wrap items-center justify-between gap-3 p-4 rounded-2xl bg-slate-50 dark:bg-slate-900/60 border border-slate-200/80 dark:border-white/10 mt-1">
                <div className="flex items-center gap-2 text-xs text-slate-700 dark:text-slate-300">
                  <CheckCircle2 className="w-4 h-4 text-emerald-500" aria-hidden="true" />
                  <span>Tổng kết điểm trung bình lớp 12A1: <strong>8.45đ</strong> (100% học sinh nộp bài)</span>
                </div>
                <button
                  type="button"
                  className="btn-tactile-control !h-9 !px-4 !text-xs gap-1.5 cursor-pointer"
                  onClick={() => alert(isVi ? "Bảng điểm lớp 12A1 đã sẵn sàng tải về tệp Excel (.xlsx)" : "Excel spreadsheet ready")}
                >
                  <Download className="w-3.5 h-3.5" aria-hidden="true" />
                  <span>{isVi ? "Xuất Bảng Điểm Excel (.xlsx)" : "Export Excel Sheet"}</span>
                </button>
              </div>
            </div>
          )}
        </div>
      </div>
    </div>
  );
}
