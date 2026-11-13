"use client";

import React, { useState } from "react";
import Link from "next/link";
import Navbar from "@/components/Navbar";
import Footer from "@/components/Footer";
import RibbonBackground from "@/components/RibbonBackground";
import {
  Shield,
  Lock,
  CheckCircle2,
  AlertCircle,
  ExternalLink,
  HardDrive,
  UserCheck,
  EyeOff,
  Globe,
  FileText,
} from "lucide-react";

export default function PrivacyPolicyPage() {
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
              <Shield className="w-3.5 h-3.5 text-blue-500" />
              <span>
                {isVi
                  ? "Tài Liệu Pháp Lý Chính Thức • Cập Nhật 2026"
                  : "Official Legal Document • Updated 2026"}
              </span>
            </div>

            {/* Language Toggle */}
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
              ? "Chính Sách Quyền Riêng Tư (Privacy Policy)"
              : "ExamPlus Privacy Policy"}
          </h1>
          <p className="text-sm text-slate-500 dark:text-slate-400">
            {isVi
              ? "Hiệu lực từ ngày: 01/01/2026 • Áp dụng cho Ứng dụng Máy tính ExamPlus (ExamPlus Windows Desktop Application) và Dịch vụ Tích hợp Google OAuth 2.0"
              : "Effective Date: January 1, 2026 • Applies to ExamPlus Windows Desktop Application and Google OAuth 2.0 Services"}
          </p>
        </div>

        {/* Content Box */}
        <div className="rounded-3xl p-6 sm:p-10 glass-surface shadow-xl flex flex-col gap-10 leading-relaxed text-sm sm:text-base text-slate-600 dark:text-slate-300">
          {/* SECTION 1: Giới thiệu */}
          <section className="flex flex-col gap-3">
            <h2 className="text-xl sm:text-2xl font-bold text-slate-900 dark:text-white flex items-center gap-2.5">
              <span className="text-blue-400 font-mono">1.</span>
              <span>{isVi ? "Giới Thiệu & Phạm Vi Áp Dụng" : "Introduction & Scope"}</span>
            </h2>
            <p>
              {isVi ? (
                <>
                  Chào mừng bạn đến với <strong>ExamPlus</strong> ("chúng tôi", "ứng dụng"). ExamPlus là giải pháp phần mềm máy tính (Desktop Windows Application) phục vụ công tác tổ chức thi trắc nghiệm, bóc tách đề thi Word MathType SVG và chấm điểm theo chuẩn Chương trình Giáo dục Phổ thông 2018 (GDPT 2018) cho giáo viên và các cơ sở giáo dục tại Việt Nam.
                </>
              ) : (
                <>
                  Welcome to <strong>ExamPlus</strong> ("we", "our", or "the application"). ExamPlus is a native Windows desktop software designed to assist teachers and schools in administering digital examinations, parsing MathType SVG formulas, and calculating grades under Vietnam's official GDPT 2018 education curriculum.
                </>
              )}
            </p>
            <p>
              {isVi ? (
                <>
                  Chính sách Quyền riêng tư này giải thích rõ ràng và minh bạch cách ExamPlus thu thập, sử dụng, lưu trữ và bảo vệ thông tin khi bạn sử dụng phần mềm, bao gồm cả khi bạn liên kết tài khoản Google để đồng bộ Google Drive.
                </>
              ) : (
                <>
                  This Privacy Policy describes how ExamPlus collects, uses, stores, and protects information when you use our software, including when you authenticate your Google Account to synchronize with Google Drive.
                </>
              )}
            </p>
          </section>

          {/* SECTION 2: Dữ liệu thu thập */}
          <section className="flex flex-col gap-3">
            <h2 className="text-xl sm:text-2xl font-bold text-slate-900 dark:text-white flex items-center gap-2.5">
              <span className="text-blue-400 font-mono">2.</span>
              <span>{isVi ? "Dữ Liệu Chúng Tôi Thu Thập & Cách Thu Thập" : "Information We Collect"}</span>
            </h2>
            <p>
              {isVi ? (
                <>
                  ExamPlus được thiết kế theo triết lý <strong>Bảo vệ Quyền riêng tư Ngay từ Đầu (Privacy by Design)</strong> và <strong>Cục bộ Hóa (Local-First)</strong>:
                </>
              ) : (
                <>
                  ExamPlus operates under a <strong>Privacy-by-Design</strong> and <strong>Local-First</strong> philosophy:
                </>
              )}
            </p>
            <ul className="list-disc pl-5 flex flex-col gap-2 text-slate-300">
              <li>
                <strong>{isVi ? "Thông tin Tài khoản Google:" : "Google Account Information:"}</strong>{" "}
                {isVi
                  ? "Khi bạn chọn 'Đăng nhập bằng Google', chúng tôi chỉ nhận Họ tên, Địa chỉ Email và Ảnh đại diện (Avatar) từ dịch vụ Google OAuth 2.0 UserInfo để hiển thị trên thanh tiêu đề phần mềm."
                  : "When you choose 'Sign in with Google', we receive your Name, Email address, and Profile avatar from Google OAuth 2.0 UserInfo solely to display on the application titlebar."}
              </li>
              <li>
                <strong>{isVi ? "Dữ liệu Đề thi & Bài thi:" : "Exam & Candidate Data:"}</strong>{" "}
                {isVi
                  ? "Đề thi (.docx), câu hỏi trắc nghiệm, danh sách thí sinh và bảng điểm thi được lưu trữ trực tiếp trong tệp cơ sở dữ liệu SQLite cục bộ (examplus.db) trên máy tính của bạn."
                  : "Exam files (.docx), questions, candidate rosters, and score sheets are stored directly in a local SQLite database (examplus.db) on your machine."}
              </li>
              <li>
                <strong>{isVi ? "Không thu thập dữ liệu ngầm:" : "Zero Telemetry Tracking:"}</strong>{" "}
                {isVi
                  ? "ExamPlus KHÔNG thu thập lịch sử duyệt web, vị trí GPS, thông tin tài chính hay bất kỳ dữ liệu cá nhân nào ngoài phạm vi phục vụ kỳ thi."
                  : "ExamPlus does NOT track your browsing history, GPS location, financial information, or personal files outside the exam context."}
              </li>
            </ul>
          </section>

          {/* SECTION 3: GOOGLE API USER DATA POLICY (LIMITED USE) - MANDATORY */}
          <section id="google-api" className="flex flex-col gap-4 p-6 rounded-2xl bg-blue-950/40 border border-blue-500/30">
            <div className="flex items-center gap-2.5 text-blue-400 font-bold text-base sm:text-lg">
              <Lock className="w-5 h-5 flex-shrink-0" />
              <span>
                {isVi
                  ? "3. Tuân Thủ Chính Sách Google API Services User Data Policy (Limited Use Requirements)"
                  : "3. Compliance with Google API Services User Data Policy (Limited Use Requirements)"}
              </span>
            </div>

            <div className="p-4 rounded-xl bg-blue-900/30 border border-blue-400/20 text-xs sm:text-sm text-blue-200 font-mono leading-relaxed">
              <strong>Official Limited Use Disclosure:</strong>
              <br />
              "ExamPlus's use and transfer of information received from Google APIs to any other app will adhere to the{" "}
              <a
                href="https://developers.google.com/terms/api-services-user-data-policy#additional_requirements_for_specific_api_scopes"
                target="_blank"
                rel="noopener noreferrer"
                className="underline text-white font-semibold hover:text-blue-300 inline-flex items-center gap-1"
              >
                Google API Services User Data Policy
                <ExternalLink className="w-3 h-3" />
              </a>
              , including the Limited Use requirements."
            </div>

            <p>
              {isVi ? (
                <>
                  Để phục vụ tính năng lưu trữ đề thi và sao lưu bảng điểm cho giáo viên, ExamPlus sử dụng các phạm vi quyền (OAuth Scopes) tối thiểu sau:
                </>
              ) : (
                <>
                  To enable cloud exam importing and gradebook backup for teachers, ExamPlus requests the following strictly limited OAuth Scopes:
                </>
              )}
            </p>

            <div className="flex flex-col gap-2.5">
              <div className="p-3 rounded-xl bg-white/[0.04] border border-white/10 flex flex-col gap-1">
                <code className="text-xs sm:text-sm font-bold text-emerald-400 font-mono break-all">
                  https://www.googleapis.com/auth/drive.file
                </code>
                <p className="text-xs text-slate-300">
                  {isVi
                    ? "Chỉ cấp quyền tạo mới, xem và quản lý các tệp hoặc thư mục do chính ExamPlus tạo ra hoặc do bạn chủ động chọn (trong thư mục ExamPlus/). Ứng dụng TUYỆT ĐỐI KHÔNG có quyền truy cập, đọc hoặc xóa bất kỳ tệp riêng tư nào khác trên Google Drive của bạn."
                    : "Grants permission only to create, view, and manage files or folders created by ExamPlus or explicitly selected by the user (inside the ExamPlus/ folder). The app CANNOT access, read, or delete any of your other private Google Drive files."}
                </p>
              </div>

              <div className="p-3 rounded-xl bg-white/[0.04] border border-white/10 flex flex-col gap-1">
                <code className="text-xs sm:text-sm font-bold text-blue-400 font-mono break-all">
                  https://www.googleapis.com/auth/userinfo.profile & email
                </code>
                <p className="text-xs text-slate-300">
                  {isVi
                    ? "Chỉ dùng để hiển thị Họ tên, Email và Ảnh đại diện của giáo viên tại Bảng điều khiển phần mềm nhằm xác nhận danh tính ca thi."
                    : "Used solely to display the teacher's Name, Email, and Avatar on the local studio dashboard to confirm identity."}
                </p>
              </div>
            </div>

            <div className="p-4 rounded-xl bg-white/[0.02] border border-white/10 flex flex-col gap-2 text-xs text-slate-300">
              <span className="font-bold text-white uppercase tracking-wider text-[11px]">
                {isVi ? "Các Cam Kết Limited Use Bắt Buộc:" : "Our Strict Limited Use Pledges:"}
              </span>
              <div className="flex items-center gap-2">
                <CheckCircle2 className="w-4 h-4 text-emerald-400 flex-shrink-0" />
                <span>
                  {isVi
                    ? "KHÔNG BAO GIỜ bán dữ liệu người dùng Google cho bất kỳ bên thứ ba, nhà môi giới dữ liệu hoặc đối tác nào."
                    : "NEVER sell Google user data to any third party, data broker, or marketing entity."}
                </span>
              </div>
              <div className="flex items-center gap-2">
                <CheckCircle2 className="w-4 h-4 text-emerald-400 flex-shrink-0" />
                <span>
                  {isVi
                    ? "KHÔNG BAO GIỜ dùng dữ liệu người dùng để phân phối quảng cáo nhắm mục tiêu."
                    : "NEVER use Google user data for advertising, retargeting, or market research."}
                </span>
              </div>
              <div className="flex items-center gap-2">
                <CheckCircle2 className="w-4 h-4 text-emerald-400 flex-shrink-0" />
                <span>
                  {isVi
                    ? "KHÔNG BAO GIỜ dùng dữ liệu đề thi hoặc dữ liệu Google để huấn luyện các mô hình Trí Tuệ Nhân Tạo (AI/ML) tổng quát."
                    : "NEVER use exam or Google data to train general Artificial Intelligence (AI) or Machine Learning models."}
                </span>
              </div>
              <div className="flex items-center gap-2">
                <CheckCircle2 className="w-4 h-4 text-emerald-400 flex-shrink-0" />
                <span>
                  {isVi
                    ? "KHÔNG BAO GIỜ cho phép nhân viên con người đọc dữ liệu người dùng trừ khi có yêu cầu hỗ trợ kỹ thuật rõ ràng bằng văn bản."
                    : "NEVER allow humans to read user data unless explicitly requested by the user for troubleshooting."}
                </span>
              </div>
            </div>
          </section>

          {/* SECTION 4: Bảo mật & Lưu trữ */}
          <section className="flex flex-col gap-3">
            <h2 className="text-xl sm:text-2xl font-bold text-slate-900 dark:text-white flex items-center gap-2.5">
              <span className="text-blue-400 font-mono">4.</span>
              <span>{isVi ? "Lưu Trữ An Toàn & Mã Hóa Dữ Liệu" : "Data Storage & Cryptographic Security"}</span>
            </h2>
            <p>
              {isVi ? (
                <>
                  ExamPlus áp dụng tiêu chuẩn bảo mật hệ điều hành Windows:
                </>
              ) : (
                <>
                  ExamPlus enforces native operating system-level encryption standards:
                </>
              )}
            </p>
            <ul className="list-disc pl-5 flex flex-col gap-2">
              <li>
                <strong>{isVi ? "Mã Hóa Windows DPAPI:" : "Windows DPAPI Encryption:"}</strong>{" "}
                {isVi
                  ? "Mã truy cập Google OAuth (Refresh Token, Access Token) được mã hóa bằng Windows Data Protection API (DPAPI). Chỉ tài khoản người dùng Windows trên máy tính cục bộ mới có thể giải mã."
                  : "OAuth tokens are encrypted using Windows Data Protection API (DPAPI) tied to the local Windows user credentials."}
              </li>
              <li>
                <strong>{isVi ? "Giao Thức Truyền Tải An Toàn:" : "Encrypted Transit (HTTPS/TLS):"}</strong>{" "}
                {isVi
                  ? "Toàn bộ liên lạc với Google APIs và Cloudflare Tunnel đều được mã hóa bằng TLS 1.3 / HTTPS tiêu chuẩn."
                  : "All external communication with Google APIs and Cloudflare Tunnel uses TLS 1.3 / HTTPS encryption."}
              </li>
            </ul>
          </section>

          {/* SECTION 5: Quyền người dùng & Thu hồi */}
          <section id="revoke-access" className="flex flex-col gap-3">
            <h2 className="text-xl sm:text-2xl font-bold text-slate-900 dark:text-white flex items-center gap-2.5">
              <span className="text-blue-400 font-mono">5.</span>
              <span>{isVi ? "Quyền Kiểm Soát & Hướng Dẫn Thu Hồi Quyền Truy Cập" : "User Rights & Access Revocation"}</span>
            </h2>
            <p>
              {isVi ? (
                <>
                  Bạn hoàn toàn làm chủ dữ liệu của mình. Bạn có thể thu hồi quyền truy cập của ExamPlus bất kỳ lúc nào bằng 2 cách:
                </>
              ) : (
                <>
                  You have full ownership of your data and can revoke ExamPlus access anytime:
                </>
              )}
            </p>
            <ol className="list-decimal pl-5 flex flex-col gap-2">
              <li>
                <strong>{isVi ? "Trực tiếp trong ExamPlus:" : "Inside ExamPlus Desktop:"}</strong>{" "}
                {isVi
                  ? "Mở Trung Tâm Cài Đặt (biểu tượng bánh răng ⚙) -> Thẻ 'Hồ Sơ & Đám Mây' -> Nhấp 'Đăng Xuất Tài Khoản'. Toàn bộ khóa token trên máy tính sẽ được xóa sạch ngay lập tức."
                  : "Open Settings ⚙ -> 'Profile & Cloud' tab -> Click 'Log Out Account'. All encrypted tokens stored locally will be purged immediately."}
              </li>
              <li>
                <strong>{isVi ? "Qua Cổng Quản Lý Tài Khoản Google:" : "Via Google Account Security Portal:"}</strong>{" "}
                {isVi ? (
                  <>
                    Truy cập trang{" "}
                    <a
                      href="https://myaccount.google.com/permissions"
                      target="_blank"
                      rel="noopener noreferrer"
                      className="text-blue-400 underline font-semibold inline-flex items-center gap-1 hover:text-blue-300"
                    >
                      Google Account Third-Party Permissions
                      <ExternalLink className="w-3.5 h-3.5" />
                    </a>
                    , tìm <strong>ExamPlus</strong> và nhấp <strong>Xóa quyền truy cập (Remove Access)</strong>.
                  </>
                ) : (
                  <>
                    Visit{" "}
                    <a
                      href="https://myaccount.google.com/permissions"
                      target="_blank"
                      rel="noopener noreferrer"
                      className="text-blue-400 underline font-semibold inline-flex items-center gap-1 hover:text-blue-300"
                    >
                      Google Third-Party Account Permissions
                      <ExternalLink className="w-3.5 h-3.5" />
                    </a>
                    , select <strong>ExamPlus</strong>, and click <strong>Remove Access</strong>.
                  </>
                )}
              </li>
            </ol>
          </section>

          {/* SECTION 6: Thông tin liên hệ */}
          <section className="flex flex-col gap-3 pt-4 border-t border-white/10">
            <h2 className="text-xl sm:text-2xl font-bold text-slate-900 dark:text-white flex items-center gap-2.5">
              <span className="text-blue-400 font-mono">6.</span>
              <span>{isVi ? "Thông Tin Liên Hệ & Phản Hồi" : "Contact Information"}</span>
            </h2>
            <p>
              {isVi
                ? "Nếu bạn có bất kỳ câu hỏi nào về Chính sách Quyền riêng tư hoặc thực hành bảo mật của ExamPlus, xin vui lòng liên hệ với nhóm phát triển:"
                : "If you have any questions regarding this Privacy Policy or our security practices, please contact our team:"}
            </p>
            <div className="p-4 rounded-xl bg-white/[0.03] border border-white/10 text-xs sm:text-sm text-slate-300 flex flex-col gap-1.5 font-mono break-all">
              <div>
                <span className="text-slate-400">Application:</span> ExamPlus
              </div>
              <div>
                <span className="text-slate-400">Official Support Email:</span>{" "}
                <a href="mailto:examplus.edu@gmail.com" className="text-blue-400 underline">
                  examplus.edu@gmail.com
                </a>
              </div>
            </div>
          </section>
        </div>
      </main>

        <Footer lang={lang} />
      </div>
    </>
  );
}
