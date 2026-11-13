"use client";

import React, { useState } from "react";
import Link from "next/link";
import Navbar from "@/components/Navbar";
import Footer from "@/components/Footer";
import RibbonBackground from "@/components/RibbonBackground";
import {
  FileText,
  Shield,
  CheckCircle2,
  AlertTriangle,
  Scale,
  Globe,
  ExternalLink,
} from "lucide-react";

export default function TermsOfServicePage() {
  const [lang, setLang] = useState<"vi" | "en">("vi");
  const isVi = lang === "vi";

  return (
    <>
      <RibbonBackground />
      <div className="relative min-h-screen flex flex-col justify-between overflow-x-hidden w-full">
        <Navbar lang={lang} onToggleLang={(l) => setLang(l)} />

        <main className="flex-1 w-full max-w-4xl mx-auto px-4 sm:px-6 lg:px-8 pt-32 pb-24 text-slate-800 dark:text-slate-200 min-w-0">
        {/* Header */}
        <div className="pb-8 border-b border-slate-200 dark:border-white/10 mb-10">
          <div className="flex items-center justify-between flex-wrap gap-4 mb-4">
            <div className="inline-flex items-center gap-2 px-3 py-1 rounded-full bg-blue-500/10 border border-blue-500/30 text-blue-600 dark:text-blue-300 text-xs font-semibold">
              <Scale className="w-3.5 h-3.5 text-blue-500" />
              <span>
                {isVi
                  ? "Điều Khoản Sử Dụng Dịch Vụ Công Khai • 2026"
                  : "Public Terms of Service • 2026"}
              </span>
            </div>

            {/* Language Switch */}
            <button
              onClick={() => setLang(isVi ? "en" : "vi")}
              className="flex items-center gap-1.5 px-3 py-1.5 rounded-full border border-slate-200 dark:border-white/10 bg-white dark:bg-white/[0.04] text-xs font-medium text-slate-700 dark:text-slate-300 hover:bg-slate-50 dark:hover:bg-white/[0.08] transition-colors cursor-pointer"
            >
              <Globe className="w-3.5 h-3.5 text-blue-500" />
              <span>{isVi ? "Switch to English" : "Chuyển sang Tiếng Việt"}</span>
            </button>
          </div>

          <h1 className="text-3xl sm:text-5xl font-extrabold text-slate-900 dark:text-white tracking-tight mb-4">
            {isVi
              ? "Điều Khoản Dịch Vụ (Terms of Service)"
              : "ExamPlus Terms of Service"}
          </h1>
          <p className="text-sm text-slate-500 dark:text-slate-400">
            {isVi
              ? "Hiệu lực từ ngày: 01/01/2026 • Áp dụng cho Phần mềm ExamPlus, Tài liệu Hướng dẫn và các Cổng thông tin liên quan"
              : "Effective Date: January 1, 2026 • Governs the ExamPlus Application, Documentation, and Related Services"}
          </p>
        </div>

        {/* Content Box */}
        <div className="rounded-3xl p-6 sm:p-10 glass-surface shadow-xl flex flex-col gap-10 leading-relaxed text-sm sm:text-base text-slate-600 dark:text-slate-300">
          {/* SECTION 1: Chấp nhận điều khoản */}
          <section className="flex flex-col gap-3">
            <h2 className="text-xl sm:text-2xl font-bold text-slate-900 dark:text-white flex items-center gap-2.5">
              <span className="text-blue-400 font-mono">1.</span>
              <span>{isVi ? "Chấp Nhận Các Điều Khoản" : "Acceptance of Terms"}</span>
            </h2>
            <p>
              {isVi ? (
                <>
                  Bằng việc tải xuống, cài đặt hoặc sử dụng phần mềm <strong>ExamPlus</strong> ("Ứng dụng", "Phần mềm"), bạn đồng ý chịu sự ràng buộc bởi các Điều khoản Dịch vụ này. Nếu bạn không đồng ý với bất kỳ phần nào của các điều khoản, vui lòng không tải về hoặc gỡ cài đặt phần mềm khỏi máy tính.
                </>
              ) : (
                <>
                  By downloading, installing, or using the <strong>ExamPlus</strong> software ("Application", "Software"), you agree to be bound by these Terms of Service. If you do not agree to any part of these terms, please refrain from using or downloading the application.
                </>
              )}
            </p>
          </section>

          {/* SECTION 2: Giấy phép sử dụng */}
          <section className="flex flex-col gap-3">
            <h2 className="text-xl sm:text-2xl font-bold text-slate-900 dark:text-white flex items-center gap-2.5">
              <span className="text-blue-400 font-mono">2.</span>
              <span>{isVi ? "Giấy Phép Sử Dụng & Mục Đích Giáo Dục" : "Software License & Permitted Use"}</span>
            </h2>
            <p>
              {isVi ? (
                <>
                  ExamPlus được cung cấp miễn phí cho mục đích giáo dục, giảng dạy và khảo thí phi thương mại tại các trường học, trung tâm khảo thí và giáo viên tại Việt Nam:
                </>
              ) : (
                <>
                  ExamPlus is provided free of charge for non-commercial educational and proctoring purposes in schools and educational organizations:
                </>
              )}
            </p>
            <ul className="list-disc pl-5 flex flex-col gap-2">
              <li>
                {isVi
                  ? "Bạn được quyền tải về, cài đặt và sao chép kỹ thuật phần mềm trên máy tính cá nhân hoặc phòng máy trường học để tổ chức các kỳ thi trắc nghiệm hoàn toàn KHÔNG THU PHÍ."
                  : "You are granted a non-exclusive license to download, install, and run the software on personal computers and school labs for strictly NON-COMMERCIAL educational purposes."}
              </li>
              <li>
                {isVi
                  ? "NGHIÊM CẤM THƯƠNG MẠI HÓA: Tuyệt đối không được bán, cho thuê, cung cấp dịch vụ thi trả phí (SaaS), nhúng vào sản phẩm thương mại, khai thác doanh thu quảng cáo, hoặc dùng để tổ chức lớp học thêm, trung tâm luyện thi có thu học phí."
                  : "STRICT COMMERCIAL PROHIBITION: You may not sell, lease, provide paid proctoring services (SaaS), embed in commercial products, monetize via ads, or use for paid tutoring or test-prep centers."}
              </li>
              <li>
                {isVi
                  ? "Bảo lưu quyền tác giả: Mọi quyền sửa đổi, tạo bản phái sinh và phân phối bản nhị phân công khai được bảo lưu cho tác giả Phạm Tuấn Anh và Nhóm Phát Triển ExamPlus."
                  : "Reservation of Rights: Modification, derivative works, and public binary redistribution rights are reserved to the author and the ExamPlus Development Team."}
              </li>
            </ul>
          </section>

          {/* SECTION 3: Trách nhiệm người dùng & Đạo đức thi cử */}
          <section className="flex flex-col gap-3">
            <h2 className="text-xl sm:text-2xl font-bold text-slate-900 dark:text-white flex items-center gap-2.5">
              <span className="text-blue-400 font-mono">3.</span>
              <span>{isVi ? "Trách Nhiệm Của Giáo Viên & Thí Sinh" : "User Responsibilities & Integrity"}</span>
            </h2>
            <p>
              {isVi ? (
                <>
                  ExamPlus cung cấp các công cụ chống gian lận (khóa toàn màn hình, cảnh báo chuyển tab, phát hiện thiết bị đúp) nhằm bảo vệ tính công bằng học thuật:
                </>
              ) : (
                <>
                  ExamPlus provides multi-layer integrity controls (fullscreen lockdown, tab-switch detection, dual-device guard) to uphold academic integrity:
                </>
              )}
            </p>
            <ul className="list-disc pl-5 flex flex-col gap-2">
              <li>
                <strong>{isVi ? "Giáo viên / Giám thị:" : "Educators / Proctors:"}</strong>{" "}
                {isVi
                  ? "Chịu trách nhiệm bảo mật đề thi, đáp án và xác minh danh tính thí sinh tham gia phòng thi."
                  : "Responsible for exam content security, answer keys, and candidate identity verification."}
              </li>
              <li>
                <strong>{isVi ? "Thí sinh:" : "Candidates / Students:"}</strong>{" "}
                {isVi
                  ? "Có nghĩa vụ tuân thủ quy chế thi, không cố tình vô hiệu hóa tính năng khóa màn hình hoặc sử dụng các công cụ can thiệp mạng."
                  : "Obligated to maintain testing integrity without attempting to bypass proctoring controls."}
              </li>
            </ul>
          </section>

          {/* SECTION 4: Tích hợp bên thứ ba & Google Services */}
          <section className="flex flex-col gap-3">
            <h2 className="text-xl sm:text-2xl font-bold text-slate-900 dark:text-white flex items-center gap-2.5">
              <span className="text-blue-400 font-mono">4.</span>
              <span>{isVi ? "Tích Hợp Dịch Vụ Google & Nhà Cung Cấp Ngoài" : "Third-Party Services & Google Integration"}</span>
            </h2>
            <p>
              {isVi ? (
                <>
                  Khi bạn kết nối tính năng Google Drive hoặc Cloudflare Tunnel trong ExamPlus, việc sử dụng các dịch vụ đó đồng thời tuân thủ Điều khoản Dịch vụ tương ứng của Google LLC và Cloudflare Inc.
                </>
              ) : (
                <>
                  When utilizing Google Drive sync or Cloudflare Quick Tunnel, your usage is also subject to the respective terms and policies of Google LLC and Cloudflare Inc.
                </>
              )}
            </p>
            <p>
              {isVi ? (
                <>
                  ExamPlus cam kết tuân thủ đầy đủ{" "}
                  <Link href="/privacy#google-api" className="text-blue-400 underline font-semibold hover:text-blue-300">
                    Chính Sách Google API Services User Data Policy
                  </Link>{" "}
                  về việc giới hạn phạm vi truy cập dữ liệu (Limited Use Requirements).
                </>
              ) : (
                <>
                  ExamPlus strictly adheres to the{" "}
                  <Link href="/privacy#google-api" className="text-blue-400 underline font-semibold hover:text-blue-300">
                    Google API Services User Data Policy
                  </Link>{" "}
                  regarding Limited Use requirements.
                </>
              )}
            </p>
          </section>

          {/* SECTION 5: Miễn trừ trách nhiệm */}
          <section className="flex flex-col gap-3 p-5 rounded-2xl bg-amber-500/10 border border-amber-500/20 text-xs sm:text-sm">
            <div className="flex items-center gap-2 text-amber-300 font-bold">
              <AlertTriangle className="w-4 h-4 flex-shrink-0" />
              <span>{isVi ? "5. Tuyên Bố Miễn Trừ Trách Nhiệm (Disclaimer of Warranty)" : "5. Disclaimer of Warranties"}</span>
            </div>
            <p className="text-slate-300">
              {isVi
                ? "PHẦN MỀM ĐƯỢC CUNG CẤP TRÊN NGUYÊN TẮC 'NGUYÊN TRẠNG' (AS IS) VÀ 'KHI CÓ SẴN' (AS AVAILABLE), KHÔNG CÓ BẤT KỲ SỰ BẢO ĐẢM NÀO DÙ RÕ RÀNG HAY NGỤ Ý. CHÚNG TÔI KHÔNG CHỊU TRÁCH NHIỆM VỀ VIỆC MẤT MÁT DỮ LIỆU DO SỰ CỐ PHẦN CỨNG MÁY TÍNH CỦA NGƯỜI DÙNG, MẤT ĐIỆN ĐỘT NGỘT HOẶC MẠNG NỘI BỘ TRƯỜNG HỌC KHÔNG ỔN ĐỊNH."
                : "THE APPLICATION IS PROVIDED 'AS IS' AND 'AS AVAILABLE' WITHOUT WARRANTIES OF ANY KIND. WE ARE NOT LIABLE FOR DATA LOSS RESULTING FROM LOCAL HARDWARE FAILURE, POWER OUTAGES, OR LOCAL NETWORK INSTABILITY."}
            </p>
          </section>

          {/* SECTION 6: Quyền sở hữu trí tuệ */}
          <section className="flex flex-col gap-3">
            <h2 className="text-xl sm:text-2xl font-bold text-slate-900 dark:text-white flex items-center gap-2.5">
              <span className="text-blue-400 font-mono">6.</span>
              <span>{isVi ? "Sở Hữu Trí Tuệ" : "Intellectual Property"}</span>
            </h2>
            <p>
              {isVi
                ? "Thương hiệu ExamPlus, biểu tượng E+, giao diện đồ họa và thuật toán bóc tách MathType SVG thuộc quyền sở hữu trí tuệ của nhóm phát triển ExamPlus. Đề thi và nội dung câu hỏi thuộc bản quyền của giáo viên và tác giả biên soạn."
                : "The ExamPlus name, E+ emblem, interface designs, and MathType decoding algorithms are protected intellectual property. Exam questions remain the property of their respective creators."}
            </p>
          </section>

          {/* SECTION 7: Liên hệ */}
          <section className="flex flex-col gap-3 pt-4 border-t border-white/10">
            <h2 className="text-xl sm:text-2xl font-bold text-slate-900 dark:text-white flex items-center gap-2.5">
              <span className="text-blue-400 font-mono">7.</span>
              <span>{isVi ? "Liên Hệ Pháp Lý" : "Contact & Governance"}</span>
            </h2>
            <p>
              {isVi
                ? "Mọi yêu cầu hoặc thắc mắc liên quan đến Điều khoản Dịch vụ này, xin vui lòng gửi thư điện tử về:"
                : "For inquiries regarding these Terms of Service, please reach out to:"}
            </p>
            <div className="p-4 rounded-xl bg-white/[0.03] border border-white/10 text-xs sm:text-sm font-mono text-slate-300 break-all">
              Email:{" "}
              <a href="mailto:examplus.edu@gmail.com" className="text-blue-400 underline">
                examplus.edu@gmail.com
              </a>{" "}
              • Website:{" "}
              <Link href="/" className="text-blue-400 underline">
                https://examplus.vercel.app
              </Link>
            </div>
          </section>
        </div>
      </main>

        <Footer lang={lang} />
      </div>
    </>
  );
}
