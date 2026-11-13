"use client";

import React, { useState } from "react";
import Image from "next/image";
import { Monitor, Smartphone, CheckCircle, ShieldAlert, Sparkles, Activity, Clock, Users } from "lucide-react";

interface AppShowcaseProps {
  lang?: "vi" | "en";
}

export default function AppShowcase({ lang = "vi" }: AppShowcaseProps) {
  const isVi = lang === "vi";
  const [activeTab, setActiveTab] = useState<"teacher" | "student">("teacher");

  return (
    <section className="relative py-20 px-4 sm:px-6 lg:px-8 overflow-hidden">
      <div className="max-w-7xl mx-auto">
        {/* Section title */}
        <div className="text-center max-w-2xl mx-auto mb-10">
          <div className="inline-flex items-center gap-2 px-3 py-1.5 rounded-full bg-indigo-500/10 border border-indigo-500/25 text-indigo-300 text-xs font-semibold mb-3">
            <Sparkles className="w-3.5 h-3.5" />
            <span>{isVi ? "Trải Nghiệm Thực Tế" : "Interactive Product Showcase"}</span>
          </div>
          <h2 className="text-3xl sm:text-4xl font-extrabold text-white tracking-tight">
            {isVi
              ? "Giao Diện Trực Quan Cho Cả Giáo Viên & Thí Sinh"
              : "Intuitive Interface for Teachers & Students"}
          </h2>
        </div>

        {/* Switcher Tabs */}
        <div className="flex justify-center mb-10">
          <div className="inline-flex p-1.5 rounded-2xl glass-panel border border-white/10 shadow-xl">
            <button
              onClick={() => setActiveTab("teacher")}
              className={`flex items-center gap-2.5 px-6 py-2.5 rounded-xl text-sm font-semibold transition-all duration-200 cursor-pointer ${
                activeTab === "teacher"
                  ? "bg-blue-600 text-white shadow-lg shadow-blue-600/30"
                  : "text-slate-400 hover:text-white hover:bg-white/[0.04]"
              }`}
            >
              <Monitor className="w-4 h-4" />
              <span>{isVi ? "Bảng Điều Khiển Giáo Viên" : "Teacher Studio"}</span>
            </button>
            <button
              onClick={() => setActiveTab("student")}
              className={`flex items-center gap-2.5 px-6 py-2.5 rounded-xl text-sm font-semibold transition-all duration-200 cursor-pointer ${
                activeTab === "student"
                  ? "bg-blue-600 text-white shadow-lg shadow-blue-600/30"
                  : "text-slate-400 hover:text-white hover:bg-white/[0.04]"
              }`}
            >
              <Smartphone className="w-4 h-4" />
              <span>{isVi ? "Giao Diện Làm Bài Thí Sinh" : "Student Web Client"}</span>
            </button>
          </div>
        </div>

        {/* Mockup Frame */}
        <div className="relative rounded-3xl p-4 sm:p-6 glass-panel border border-white/15 shadow-2xl">
          {/* Window Titlebar */}
          <div className="flex items-center justify-between pb-4 mb-4 border-b border-white/10 px-2">
            <div className="flex items-center gap-2">
              <span className="w-3 h-3 rounded-full bg-red-500/80 inline-block" />
              <span className="w-3 h-3 rounded-full bg-amber-500/80 inline-block" />
              <span className="w-3 h-3 rounded-full bg-emerald-500/80 inline-block" />
              <span className="text-xs font-mono text-slate-400 ml-3">
                {activeTab === "teacher"
                  ? "ExamPlus — Windows Desktop Native Studio"
                  : "ExamPlus Student Portal — https://examplus.local:8080"}
              </span>
            </div>
            <div className="flex items-center gap-2">
              <span className="text-[11px] font-medium px-2 py-0.5 rounded bg-emerald-500/20 text-emerald-400 border border-emerald-500/30">
                {activeTab === "teacher" ? "Live Telemetry Active" : "Anti-Cheat Fullscreen"}
              </span>
            </div>
          </div>

          {/* Tab Content 1: Teacher Studio */}
          {activeTab === "teacher" && (
            <div className="grid grid-cols-1 lg:grid-cols-12 gap-6 p-4 sm:p-6 bg-[#0a1020]/90 rounded-2xl border border-white/5 animate-in fade-in duration-300">
              {/* Left sidebar mock */}
              <div className="lg:col-span-3 flex flex-col gap-4 border-r border-white/10 pr-4">
                <div className="p-3 rounded-xl bg-blue-500/10 border border-blue-500/20 flex items-center gap-3">
                  <div className="w-10 h-10 rounded-full bg-blue-600 flex items-center justify-center font-bold text-white text-sm">
                    TN
                  </div>
                  <div className="flex flex-col">
                    <span className="text-sm font-semibold text-white">Thầy Tuấn Anh</span>
                    <span className="text-xs text-emerald-400 flex items-center gap-1">
                      <span className="w-1.5 h-1.5 rounded-full bg-emerald-400" /> Google Drive Connected
                    </span>
                  </div>
                </div>

                <div className="flex flex-col gap-1 text-xs text-slate-300">
                  <div className="p-2.5 rounded-lg bg-white/[0.06] text-white font-medium flex items-center justify-between">
                    <span>Phòng Thi 12A1 (Vật Lý)</span>
                    <span className="text-[10px] px-1.5 py-0.5 rounded bg-blue-500/30 text-blue-300 font-mono">42 HS</span>
                  </div>
                  <div className="p-2.5 rounded-lg text-slate-400 hover:bg-white/[0.03] flex items-center justify-between">
                    <span>Phòng Thi Khối 11 (Toán)</span>
                    <span className="text-[10px] px-1.5 py-0.5 rounded bg-slate-800 text-slate-400 font-mono">Đã đóng</span>
                  </div>
                </div>

                <div className="mt-auto p-3 rounded-xl bg-slate-900/60 border border-white/5 text-xs text-slate-400">
                  <div className="flex items-center justify-between mb-1">
                    <span>Mạng LAN:</span>
                    <span className="font-mono text-white">192.168.1.105:8080</span>
                  </div>
                  <div className="flex items-center justify-between">
                    <span>Cloudflare:</span>
                    <span className="font-mono text-emerald-400">Online Quick Tunnel</span>
                  </div>
                </div>
              </div>

              {/* Main dashboard mock */}
              <div className="lg:col-span-9 flex flex-col gap-5">
                {/* Metrics header */}
                <div className="grid grid-cols-2 sm:grid-cols-4 gap-3">
                  <div className="p-3.5 rounded-xl bg-white/[0.03] border border-white/10">
                    <span className="text-[11px] text-slate-400 block mb-1">Thí Sinh Đang Thi</span>
                    <span className="text-xl font-bold text-white flex items-center gap-1.5">
                      <Users className="w-4 h-4 text-blue-400" /> 42/42
                    </span>
                  </div>
                  <div className="p-3.5 rounded-xl bg-white/[0.03] border border-white/10">
                    <span className="text-[11px] text-slate-400 block mb-1">Đã Nộp Bài</span>
                    <span className="text-xl font-bold text-emerald-400 flex items-center gap-1.5">
                      <CheckCircle className="w-4 h-4" /> 18
                    </span>
                  </div>
                  <div className="p-3.5 rounded-xl bg-white/[0.03] border border-white/10">
                    <span className="text-[11px] text-slate-400 block mb-1">Cảnh Báo Chuyển Tab</span>
                    <span className="text-xl font-bold text-amber-400 flex items-center gap-1.5">
                      <ShieldAlert className="w-4 h-4" /> 2
                    </span>
                  </div>
                  <div className="p-3.5 rounded-xl bg-white/[0.03] border border-white/10">
                    <span className="text-[11px] text-slate-400 block mb-1">Thời Gian Còn Lại</span>
                    <span className="text-xl font-bold text-white flex items-center gap-1.5 font-mono">
                      <Clock className="w-4 h-4 text-cyan-400" /> 28:45
                    </span>
                  </div>
                </div>

                {/* Candidate mock table */}
                <div className="overflow-x-auto rounded-xl border border-white/10">
                  <table className="w-full text-left text-xs text-slate-300">
                    <thead className="bg-white/[0.05] text-slate-400 uppercase font-semibold">
                      <tr>
                        <th className="py-2.5 px-3">SBD</th>
                        <th className="py-2.5 px-3">Họ và Tên</th>
                        <th className="py-2.5 px-3">Mã Đề</th>
                        <th className="py-2.5 px-3">Tiến Độ Làm Bài</th>
                        <th className="py-2.5 px-3">Trạng Thái</th>
                        <th className="py-2.5 px-3">Điểm Tạm Tính</th>
                      </tr>
                    </thead>
                    <tbody className="divide-y divide-white/5">
                      <tr className="hover:bg-white/[0.02]">
                        <td className="py-2.5 px-3 font-mono font-bold text-blue-400">1201</td>
                        <td className="py-2.5 px-3 font-medium text-white">Nguyễn Văn An</td>
                        <td className="py-2.5 px-3 font-mono">101</td>
                        <td className="py-2.5 px-3">
                          <div className="w-32 bg-slate-800 rounded-full h-2 overflow-hidden">
                            <div className="bg-emerald-500 h-2 rounded-full w-[92%]" />
                          </div>
                        </td>
                        <td className="py-2.5 px-3">
                          <span className="px-2 py-0.5 rounded-full bg-emerald-500/10 text-emerald-400 text-[10px] font-semibold">
                            Đã Nộp Bài
                          </span>
                        </td>
                        <td className="py-2.5 px-3 font-bold text-emerald-400 font-mono">9.25 / 10.0</td>
                      </tr>
                      <tr className="hover:bg-white/[0.02]">
                        <td className="py-2.5 px-3 font-mono font-bold text-blue-400">1202</td>
                        <td className="py-2.5 px-3 font-medium text-white">Trần Thị Bích</td>
                        <td className="py-2.5 px-3 font-mono">102</td>
                        <td className="py-2.5 px-3">
                          <div className="w-32 bg-slate-800 rounded-full h-2 overflow-hidden">
                            <div className="bg-blue-500 h-2 rounded-full w-[75%]" />
                          </div>
                        </td>
                        <td className="py-2.5 px-3">
                          <span className="px-2 py-0.5 rounded-full bg-blue-500/10 text-blue-400 text-[10px] font-semibold">
                            Đang Làm (21/28)
                          </span>
                        </td>
                        <td className="py-2.5 px-3 text-slate-400 font-mono">Đang tính...</td>
                      </tr>
                      <tr className="hover:bg-white/[0.02]">
                        <td className="py-2.5 px-3 font-mono font-bold text-blue-400">1203</td>
                        <td className="py-2.5 px-3 font-medium text-white">Lê Hoàng Nam</td>
                        <td className="py-2.5 px-3 font-mono">103</td>
                        <td className="py-2.5 px-3">
                          <div className="w-32 bg-slate-800 rounded-full h-2 overflow-hidden">
                            <div className="bg-amber-500 h-2 rounded-full w-[60%]" />
                          </div>
                        </td>
                        <td className="py-2.5 px-3">
                          <span className="px-2 py-0.5 rounded-full bg-amber-500/10 text-amber-400 text-[10px] font-semibold flex items-center gap-1 w-max">
                            <ShieldAlert className="w-3 h-3" /> Đổi tab 1 lần
                          </span>
                        </td>
                        <td className="py-2.5 px-3 text-slate-400 font-mono">Đang tính...</td>
                      </tr>
                    </tbody>
                  </table>
                </div>
              </div>
            </div>
          )}

          {/* Tab Content 2: Student Exam Web */}
          {activeTab === "student" && (
            <div className="p-4 sm:p-8 bg-[#0a1020]/90 rounded-2xl border border-white/5 animate-in fade-in duration-300 flex flex-col gap-6">
              {/* Exam Question Card Mock */}
              <div className="p-6 rounded-2xl bg-white/[0.03] border border-white/10 flex flex-col gap-4">
                <div className="flex items-center justify-between">
                  <span className="px-3 py-1 rounded-lg bg-blue-500/20 text-blue-300 text-xs font-bold border border-blue-500/30">
                    Câu 12 • Phần I (Trắc Nghiệm 4 Phương Án)
                  </span>
                  <span className="text-xs text-slate-400 font-mono">Điểm: 0.25 điểm</span>
                </div>

                <div className="text-sm sm:text-base text-white leading-relaxed font-medium">
                  Một vật dao động điều hòa theo phương trình <code className="px-1.5 py-0.5 rounded bg-blue-900/40 text-blue-300 font-mono">x = 5cos(4πt - π/3) (cm)</code>. Chu kỳ dao động của vật là:
                </div>

                {/* 4 Choices */}
                <div className="grid grid-cols-1 sm:grid-cols-2 gap-3 pt-2">
                  <div className="p-3.5 rounded-xl border border-white/10 bg-white/[0.02] hover:bg-white/[0.05] flex items-center gap-3 cursor-pointer">
                    <span className="w-7 h-7 rounded-full border border-slate-500 flex items-center justify-center font-bold text-xs text-slate-300">
                      A
                    </span>
                    <span className="text-sm text-slate-200 font-mono">T = 2,0 s</span>
                  </div>
                  <div className="p-3.5 rounded-xl border border-blue-500/40 bg-blue-500/10 flex items-center gap-3 cursor-pointer shadow-md shadow-blue-500/10">
                    <span className="w-7 h-7 rounded-full bg-blue-600 text-white flex items-center justify-center font-bold text-xs">
                      B
                    </span>
                    <span className="text-sm text-white font-mono font-semibold">T = 0,5 s (Đã chọn)</span>
                  </div>
                  <div className="p-3.5 rounded-xl border border-white/10 bg-white/[0.02] hover:bg-white/[0.05] flex items-center gap-3 cursor-pointer">
                    <span className="w-7 h-7 rounded-full border border-slate-500 flex items-center justify-center font-bold text-xs text-slate-300">
                      C
                    </span>
                    <span className="text-sm text-slate-200 font-mono">T = 4,0 s</span>
                  </div>
                  <div className="p-3.5 rounded-xl border border-white/10 bg-white/[0.02] hover:bg-white/[0.05] flex items-center gap-3 cursor-pointer">
                    <span className="w-7 h-7 rounded-full border border-slate-500 flex items-center justify-center font-bold text-xs text-slate-300">
                      D
                    </span>
                    <span className="text-sm text-slate-200 font-mono">T = 0,25 s</span>
                  </div>
                </div>
              </div>

              {/* Question Navigation Palette Mock */}
              <div className="p-4 rounded-xl bg-white/[0.02] border border-white/10 flex flex-col gap-2">
                <span className="text-xs font-semibold text-slate-400">Bảng Điều Hướng Câu Hỏi:</span>
                <div className="flex flex-wrap gap-2">
                  {[1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11].map((q) => (
                    <span key={q} className="w-8 h-8 rounded-lg bg-emerald-500/20 text-emerald-400 border border-emerald-500/30 flex items-center justify-center text-xs font-bold font-mono">
                      {q}
                    </span>
                  ))}
                  <span className="w-8 h-8 rounded-lg bg-blue-600 text-white flex items-center justify-center text-xs font-bold font-mono shadow-md shadow-blue-500/40">
                    12
                  </span>
                  {[13, 14, 15, 16, 17, 18].map((q) => (
                    <span key={q} className="w-8 h-8 rounded-lg bg-white/[0.05] text-slate-400 border border-white/10 flex items-center justify-center text-xs font-bold font-mono">
                      {q}
                    </span>
                  ))}
                </div>
              </div>
            </div>
          )}
        </div>
      </div>
    </section>
  );
}
