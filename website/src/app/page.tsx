"use client";

import React, { useState } from "react";
import Link from "next/link";
import Navbar from "@/components/Navbar";
import Footer from "@/components/Footer";
import PatternBackground from "@/components/PatternBackground";
import TeacherStudioMockup from "@/components/TeacherStudioMockup";
import DownloadSection from "@/components/DownloadSection";
import FAQSection from "@/components/FAQSection";
import {
  FileText,
  MonitorCheck,
  GraduationCap,
  Cloud,
  CheckCircle2,
  HardDriveDownload,
  ArrowRight,
  ShieldCheck,
  Sparkles,
  Zap,
} from "lucide-react";

export default function Home() {
  const [lang, setLang] = useState<"vi" | "en">("vi");
  const isVi = lang === "vi";

  return (
    <>
      {/* 100% Pure Vector CSS/SVG Gradients & Dynamic Micro-Grid (ZERO Static Images) */}
      <PatternBackground />

      {/* Main Page Content Layer */}
      <div className="relative z-10 flex flex-col min-h-screen">
        <Navbar lang={lang} onToggleLang={(l) => setLang(l)} />

        <main className="flex-1 flex flex-col pt-24 sm:pt-28">
          {/* ===================== HERO SECTION ===================== */}
          <section className="relative w-full overflow-hidden px-4 sm:px-6 lg:px-8 pt-8 sm:pt-12 pb-16 sm:pb-20 text-center max-w-5xl mx-auto flex flex-col items-center">
            {/* Frosted Glass Hero Container for Ultimate Readability & Depth */}
            <div className="hero-frosted-card w-full max-w-4xl mx-auto flex flex-col items-center px-5 py-8 sm:px-10 sm:py-12 mb-12">
              {/* Status Pill with Orbital Indicator */}
              <div className="cir-load mb-6" role="status">
                <span className="uiv-orbit" aria-hidden="true">
                  <span className="uiv-orbit__core" />
                  <span className="uiv-orbit__ring" />
                </span>
                <span className="cir-load__text text-xs sm:text-sm">
                  {isVi
                    ? "ExamPlus v1.0 • Chuẩn Khảo Thí GDPT 2018"
                    : "ExamPlus v1.0 • Vietnam MOET Standard"}
                </span>
              </div>

              {/* Headline */}
              <h1 className="text-3xl sm:text-5xl lg:text-6xl font-extrabold tracking-tight text-slate-900 dark:text-white leading-[1.15] mb-5 max-w-3xl">
                {isVi ? (
                  <>
                    Hệ Thống Khảo Thí & Giám Sát Phòng Thi{" "}
                    <span className="text-blue-600 dark:text-blue-400">GDPT 2018</span>
                  </>
                ) : (
                  <>
                    Smart Examination & Proctored Hub for{" "}
                    <span className="text-blue-600 dark:text-blue-400">Educators</span>
                  </>
                )}
              </h1>

              {/* Clear, Teacher-Focused Subtitle */}
              <p className="text-base sm:text-lg text-slate-700 dark:text-slate-200 max-w-2xl leading-relaxed mb-8">
                {isVi
                  ? "Bóc tách đề thi từ Word chuẩn nét, tổ chức thi mượt mà trong phòng máy không lo mất mạng và tự động lưu trữ lên Google Drive an toàn."
                  : "Import exams from Word files with crisp vector formulas, run seamless classroom testing offline, and back up securely to Google Drive."}
              </p>

              {/* Dual Tactile 3D Buttons */}
              <div className="flex flex-col sm:flex-row items-center gap-4">
                <Link href="#download" className="btn-tactile-primary">
                  <HardDriveDownload className="w-4 h-4" aria-hidden="true" />
                  <span>{isVi ? "Tải Bản Windows Portable" : "Download Portable"}</span>
                  <ArrowRight className="w-4 h-4 ml-1" aria-hidden="true" />
                </Link>
                <Link href="#features" className="btn-tactile-secondary">
                  <span>{isVi ? "Khám Phá Tính Năng" : "Explore Highlights"}</span>
                </Link>
              </div>
            </div>

            {/* Centerpiece: Interactive Teacher Studio Mockup */}
            <TeacherStudioMockup lang={lang} />
          </section>

          {/* ===================== ASYMMETRIC BENTO GRID (4 CORE PILLARS) ===================== */}
          <section id="features" className="relative py-16 sm:py-20 px-4 sm:px-6 lg:px-8 max-w-6xl mx-auto w-full">
            <div className="text-center max-w-2xl mx-auto mb-12">
              <div className="inline-flex items-center gap-2 px-3 py-1 rounded-full bg-blue-500/10 border border-blue-500/20 text-blue-600 dark:text-blue-400 text-xs font-semibold mb-3">
                <Sparkles className="w-3.5 h-3.5" aria-hidden="true" />
                <span>{isVi ? "Ưu Điểm Vượt Trội" : "Core Advantages"}</span>
              </div>
              <h2 className="text-2xl sm:text-4xl font-extrabold text-slate-900 dark:text-white tracking-tight mb-3">
                {isVi ? "Thiết Kế Chuyên Biệt Cho Giáo Viên" : "Engineered Specifically for Educators"}
              </h2>
              <p className="text-sm sm:text-base text-slate-600 dark:text-slate-300">
                {isVi
                  ? "Tối ưu hóa toàn diện cho việc tổ chức thi trắc nghiệm hiện đại — trực quan, chính xác và không phụ thuộc đường truyền Internet."
                  : "Designed for seamless classroom exams — intuitive, reliable, and completely offline-capable."}
              </p>
            </div>

            <div className="grid grid-cols-1 md:grid-cols-12 gap-5">
              {/* Card 1: 8 Columns - Bóc Tách Đề Word MathType SVG */}
              <div className="md:col-span-8 rounded-3xl p-6 sm:p-8 bg-white/92 dark:bg-[#0c1224]/90 backdrop-blur-2xl border border-slate-200/90 dark:border-white/10 shadow-lg flex flex-col justify-between">
                <div>
                  <div className="w-11 h-11 rounded-2xl bg-blue-500/10 text-blue-600 dark:text-blue-400 flex items-center justify-center mb-4">
                    <FileText className="w-5 h-5" aria-hidden="true" />
                  </div>
                  <h3 className="text-xl font-bold text-slate-900 dark:text-white mb-2">
                    {isVi ? "Bóc Tách Đề Word & Công Thức Toán Vector SVG" : "Word Exam & MathType SVG Parser"}
                  </h3>
                  <p className="text-sm text-slate-600 dark:text-slate-300 leading-relaxed mb-6">
                    {isVi
                      ? "Chỉ cần nạp tệp Word (.docx) có sẵn. Phần mềm tự động nhận diện câu hỏi, các phương án A/B/C/D, bảng đáp án và chuyển đổi toàn bộ công thức MathType sang định dạng Vector SVG sắc nét, không bao giờ bị vỡ hạt khi phóng to."
                      : "Directly import your existing Word .docx files. Questions, answer choices, and MathType equations are automatically parsed into crisp vector SVGs that never blur on high-DPI screens."}
                  </p>
                </div>
                <div className="p-4 rounded-2xl bg-slate-50 dark:bg-slate-900/60 border border-slate-200/80 dark:border-white/10 flex items-center justify-between text-xs font-mono">
                  <span className="text-slate-700 dark:text-slate-300">Tệp: De_VatLy_12.docx</span>
                  <span className="text-emerald-600 dark:text-emerald-400 font-bold flex items-center gap-1.5">
                    <CheckCircle2 className="w-3.5 h-3.5" aria-hidden="true" /> 100% Vector SVG
                  </span>
                </div>
              </div>

              {/* Card 2: 4 Columns - 100% Offline Phòng Máy */}
              <div className="md:col-span-4 rounded-3xl p-6 sm:p-8 bg-white/92 dark:bg-[#0c1224]/90 backdrop-blur-2xl border border-slate-200/90 dark:border-white/10 shadow-lg flex flex-col justify-between">
                <div>
                  <div className="w-11 h-11 rounded-2xl bg-emerald-500/10 text-emerald-600 dark:text-emerald-400 flex items-center justify-center mb-4">
                    <MonitorCheck className="w-5 h-5" aria-hidden="true" />
                  </div>
                  <h3 className="text-xl font-bold text-slate-900 dark:text-white mb-2">
                    {isVi ? "Chạy 100% Offline Phòng Máy" : "100% Offline Classroom LAN"}
                  </h3>
                  <p className="text-sm text-slate-600 dark:text-slate-300 leading-relaxed mb-6">
                    {isVi
                      ? "Tổ chức thi hoàn toàn trong mạng nội bộ phòng máy (Wi-Fi hoặc mạng dây). Trường học bị rớt mạng Internet ra ngoài buổi thi vẫn diễn ra trơn tru, độ trễ cực thấp dưới 5ms."
                      : "Operates purely within your local classroom network. Exams run smoothly even when the school's external Internet goes down."}
                  </p>
                </div>
                <div className="p-3 rounded-2xl bg-emerald-50/80 dark:bg-emerald-950/30 border border-emerald-200 dark:border-emerald-500/20 text-xs font-bold text-emerald-700 dark:text-emerald-400 flex items-center justify-between">
                  <span>Độ trễ phản hồi:</span>
                  <span className="font-mono text-sm">&lt; 5ms</span>
                </div>
              </div>

              {/* Card 3: 4 Columns - Cấu Trúc Đề 3 Phần GDPT 2018 */}
              <div className="md:col-span-4 rounded-3xl p-6 sm:p-8 bg-white/92 dark:bg-[#0c1224]/90 backdrop-blur-2xl border border-slate-200/90 dark:border-white/10 shadow-lg flex flex-col justify-between">
                <div>
                  <div className="w-11 h-11 rounded-2xl bg-indigo-500/10 text-indigo-600 dark:text-indigo-400 flex items-center justify-center mb-4">
                    <GraduationCap className="w-5 h-5" aria-hidden="true" />
                  </div>
                  <h3 className="text-xl font-bold text-slate-900 dark:text-white mb-2">
                    {isVi ? "Chuẩn Cấu Trúc Đề GDPT 2018" : "MOET 2018 Standard Rubric"}
                  </h3>
                  <p className="text-sm text-slate-600 dark:text-slate-300 leading-relaxed mb-6">
                    {isVi
                      ? "Hỗ trợ đầy đủ cả 3 dạng câu hỏi mới: Trắc nghiệm 4 lựa chọn, Đúng/Sai với thang điểm bậc thang (0.1 - 0.25 - 0.5 - 1.0 điểm) và Trả lời ngắn điền số."
                      : "Fully supports all 3 new question formats: 4-option MCQs, True/False stepped rubric, and short numeric answer fill."}
                  </p>
                </div>
                <div className="p-3 rounded-2xl bg-slate-100 dark:bg-slate-900/60 border border-slate-200 dark:border-white/10 text-xs font-semibold text-slate-700 dark:text-slate-300 flex items-center justify-between">
                  <span>Thang điểm:</span>
                  <span className="text-indigo-600 dark:text-indigo-400">Tự động chấm 3 phần</span>
                </div>
              </div>

              {/* Card 4: 8 Columns - Đồng Bộ Google Drive An Toàn */}
              <div className="md:col-span-8 rounded-3xl p-6 sm:p-8 bg-white/92 dark:bg-[#0c1224]/90 backdrop-blur-2xl border border-slate-200/90 dark:border-white/10 shadow-lg flex flex-col justify-between">
                <div>
                  <div className="w-11 h-11 rounded-2xl bg-sky-500/10 text-sky-600 dark:text-sky-400 flex items-center justify-center mb-4">
                    <Cloud className="w-5 h-5" aria-hidden="true" />
                  </div>
                  <h3 className="text-xl font-bold text-slate-900 dark:text-white mb-2">
                    {isVi ? "Đồng Bộ & Sao Lưu Google Drive Tiện Lợi" : "Effortless Google Drive Backup"}
                  </h3>
                  <p className="text-sm text-slate-600 dark:text-slate-300 leading-relaxed mb-6">
                    {isVi
                      ? "Kết nối trực tiếp tài khoản Google của giáo viên với quyền truy cập hẹp an toàn. Phần mềm chỉ tạo và lưu trữ tệp trong thư mục riêng /ExamPlus/ trên Drive, không bao giờ đọc hay can thiệp tệp cá nhân của thầy cô."
                      : "Directly syncs to your Google Drive under a narrow, secure scope. ExamPlus only creates and manages files inside its dedicated /ExamPlus/ folder, keeping your personal files 100% private."}
                  </p>
                </div>
                <div className="p-4 rounded-2xl bg-sky-50/80 dark:bg-sky-950/30 border border-sky-200/80 dark:border-sky-500/20 flex items-center justify-between text-xs font-mono">
                  <span className="text-slate-700 dark:text-slate-300">Thư mục: /ExamPlus/SaoLuu/</span>
                  <span className="text-sky-600 dark:text-sky-400 font-bold flex items-center gap-1.5">
                    <ShieldCheck className="w-4 h-4" aria-hidden="true" /> Google Verified
                  </span>
                </div>
              </div>
            </div>
          </section>

          {/* ===================== WORKFLOW SECTION (3 BƯỚC ĐƠN GIẢN) ===================== */}
          <section id="workflow" className="relative py-16 sm:py-20 px-4 sm:px-6 lg:px-8 max-w-6xl mx-auto w-full">
            <div className="text-center max-w-2xl mx-auto mb-14">
              <div className="inline-flex items-center gap-2 px-3 py-1 rounded-full bg-emerald-500/10 border border-emerald-500/20 text-emerald-600 dark:text-emerald-400 text-xs font-semibold mb-3">
                <Zap className="w-3.5 h-3.5" aria-hidden="true" />
                <span>{isVi ? "Quy Trình 3 Bước" : "3-Step Workflow"}</span>
              </div>
              <h2 className="text-2xl sm:text-4xl font-extrabold text-slate-900 dark:text-white tracking-tight mb-3">
                {isVi ? "Bắt Đầu Kỳ Thi Trong 1 Phút" : "Launch Your Exam in Minutes"}
              </h2>
              <p className="text-sm sm:text-base text-slate-600 dark:text-slate-300">
                {isVi
                  ? "Không cần cài đặt phức tạp, không cần tài khoản học sinh cồng kềnh — mọi thứ diễn ra trực tiếp trên máy giáo viên."
                  : "Zero installation headaches. No student account setup required. Everything runs locally on teacher PC."}
              </p>
            </div>

            <div className="grid grid-cols-1 md:grid-cols-3 gap-6">
              {/* Step 1 */}
              <div className="rounded-3xl p-6 sm:p-7 bg-white/92 dark:bg-[#0c1224]/90 backdrop-blur-2xl border border-slate-200/90 dark:border-white/10 shadow-md flex flex-col gap-4">
                <div className="w-10 h-10 rounded-2xl bg-blue-600 text-white font-bold text-base flex items-center justify-center shadow-md">
                  1
                </div>
                <h3 className="text-lg font-bold text-slate-900 dark:text-white">
                  {isVi ? "Nạp Đề Word Sẵn Có" : "Import Word Exam"}
                </h3>
                <p className="text-sm text-slate-600 dark:text-slate-300 leading-relaxed">
                  {isVi
                    ? "Kéo thả tệp đề thi Word .docx của thầy cô vào ExamPlus. Ứng dụng tự động bóc tách 28 câu và công thức MathType trong 3 giây."
                    : "Drag and drop your .docx exam. ExamPlus instantly extracts questions and MathType equations in 3 seconds."}
                </p>
              </div>

              {/* Step 2 */}
              <div className="rounded-3xl p-6 sm:p-7 bg-white/92 dark:bg-[#0c1224]/90 backdrop-blur-2xl border border-slate-200/90 dark:border-white/10 shadow-md flex flex-col gap-4">
                <div className="w-10 h-10 rounded-2xl bg-emerald-600 text-white font-bold text-base flex items-center justify-center shadow-md">
                  2
                </div>
                <h3 className="text-lg font-bold text-slate-900 dark:text-white">
                  {isVi ? "Phát Đề Cho Phòng Máy" : "Distribute to Lab PCs"}
                </h3>
                <p className="text-sm text-slate-600 dark:text-slate-300 leading-relaxed">
                  {isVi
                    ? "Học sinh mở trình duyệt web trên máy phòng thi hoặc quét mã QR là vào ngay phòng thi. Không cần cài thêm ứng dụng gì."
                    : "Students open their browser or scan the lab QR code to enter immediately. Zero extra software required on student PCs."}
                </p>
              </div>

              {/* Step 3 */}
              <div className="rounded-3xl p-6 sm:p-7 bg-white/92 dark:bg-[#0c1224]/90 backdrop-blur-2xl border border-slate-200/90 dark:border-white/10 shadow-md flex flex-col gap-4">
                <div className="w-10 h-10 rounded-2xl bg-indigo-600 text-white font-bold text-base flex items-center justify-center shadow-md">
                  3
                </div>
                <h3 className="text-lg font-bold text-slate-900 dark:text-white">
                  {isVi ? "Tự Động Chấm & Xuất Excel" : "Instant Grading & Excel"}
                </h3>
                <p className="text-sm text-slate-600 dark:text-slate-300 leading-relaxed">
                  {isVi
                    ? "Khi học sinh nộp bài hoặc hết giờ, hệ thống tự động tính điểm theo thang GDPT 2018 và xuất ngay bảng điểm tổng kết ra Excel."
                    : "As students submit, exams are graded instantly according to MOET rubrics and ready for 1-click Excel export."}
                </p>
              </div>
            </div>
          </section>

          {/* ===================== DOWNLOAD SECTION ===================== */}
          <DownloadSection lang={lang} />

          {/* ===================== FAQ SECTION ===================== */}
          <FAQSection lang={lang} />
        </main>

        {/* Footer */}
        <Footer lang={lang} />
      </div>
    </>
  );
}
