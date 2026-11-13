"use client";

import React, { useState } from "react";
import { Mail, CheckCircle2, ArrowRight, BellRing, Sparkles } from "lucide-react";

interface SubscriptionFormProps {
  lang?: "vi" | "en";
}

export default function SubscriptionForm({ lang = "vi" }: SubscriptionFormProps) {
  const isVi = lang === "vi";
  const [email, setEmail] = useState("");
  const [agreed, setAgreed] = useState(true);
  const [status, setStatus] = useState<"idle" | "loading" | "success" | "error">("idle");
  const [errorMessage, setErrorMessage] = useState("");

  const handleSubmit = (e: React.FormEvent) => {
    e.preventDefault();
    if (!email || !email.includes("@")) {
      setStatus("error");
      setErrorMessage(
        isVi
          ? "Vui lòng nhập địa chỉ email hợp lệ."
          : "Please enter a valid email address."
      );
      return;
    }
    if (!agreed) {
      setStatus("error");
      setErrorMessage(
        isVi
          ? "Vui lòng tích đồng ý nhận thông báo."
          : "Please check the box to receive notifications."
      );
      return;
    }

    setStatus("loading");
    setTimeout(() => {
      // Store in localStorage for persistence
      try {
        const subs = JSON.parse(localStorage.getItem("examplus_subscribers") || "[]");
        subs.push({ email, subscribedAt: new Date().toISOString() });
        localStorage.setItem("examplus_subscribers", JSON.stringify(subs));
      } catch {
        // ignore
      }
      setStatus("success");
      setEmail("");
    }, 600);
  };

  return (
    <section className="relative py-20 px-4 sm:px-6 lg:px-8">
      <div className="max-w-4xl mx-auto">
        <div className="relative rounded-3xl p-8 sm:p-12 glass-panel border border-white/10 shadow-2xl overflow-hidden">
          {/* Subtle inner background glow */}
          <div className="absolute top-0 right-0 w-80 h-80 bg-blue-500/15 rounded-full blur-3xl pointer-events-none" />
          <div className="absolute bottom-0 left-0 w-72 h-72 bg-indigo-500/15 rounded-full blur-3xl pointer-events-none" />

          <div className="relative z-10 flex flex-col items-center text-center max-w-2xl mx-auto">
            {/* Badge */}
            <div className="inline-flex items-center gap-2 px-3 py-1.5 rounded-full bg-blue-500/10 border border-blue-500/25 text-blue-300 text-xs font-semibold mb-6">
              <BellRing className="w-3.5 h-3.5 animate-pulse" />
              <span>
                {isVi
                  ? "Bản Phát Hành & Thông Báo GDPT 2018"
                  : "ExamPlus Updates & Release Notes"}
              </span>
            </div>

            {/* Title & Description */}
            <h3 className="text-2xl sm:text-4xl font-bold tracking-tight text-white mb-4">
              {isVi
                ? "Nhận Thông Báo Cập Nhật & Bản Vá Mới Nhất"
                : "Stay Updated with Future ExamPlus Releases"}
            </h3>
            <p className="text-sm sm:text-base text-slate-300 leading-relaxed mb-8">
              {isVi
                ? "Đăng ký nhận thông báo khi có bản cập nhật phần mềm, tính năng xáo đề GDPT 2018 mới và bộ đề thi mẫu môn Toán, Vật Lý, Hóa Học."
                : "Subscribe to get early notifications for new releases, enhanced MathType parsers, and official sample exam templates."}
            </p>

            {/* Form */}
            {status === "success" ? (
              <div className="w-full p-6 rounded-2xl bg-emerald-500/10 border border-emerald-500/30 flex flex-col items-center gap-3 animate-in fade-in duration-300">
                <div className="w-12 h-12 rounded-full bg-emerald-500/20 flex items-center justify-center text-emerald-400">
                  <CheckCircle2 className="w-6 h-6" />
                </div>
                <h4 className="text-lg font-bold text-white">
                  {isVi ? "Đăng Ký Thành Công!" : "Subscribed Successfully!"}
                </h4>
                <p className="text-sm text-emerald-300 max-w-md">
                  {isVi
                    ? "Cảm ơn thầy/cô! Chúng tôi sẽ gửi email thông báo khi có bản phát hành ExamPlus mới hoặc tài liệu hướng dẫn thi cử."
                    : "Thank you! You will be notified whenever a new ExamPlus build or template is available."}
                </p>
                <button
                  onClick={() => setStatus("idle")}
                  className="mt-2 text-xs text-slate-400 hover:text-white underline"
                >
                  {isVi ? "Đăng ký địa chỉ email khác" : "Subscribe another email"}
                </button>
              </div>
            ) : (
              <form onSubmit={handleSubmit} className="w-full flex flex-col gap-4">
                <div className="flex flex-col sm:flex-row gap-3 w-full">
                  <div className="relative flex-1">
                    <Mail className="absolute left-4 top-1/2 -translate-y-1/2 w-5 h-5 text-slate-400" />
                    <input
                      type="email"
                      value={email}
                      onChange={(e) => setEmail(e.target.value)}
                      placeholder={
                        isVi
                          ? "Nhập email của bạn (vd: giaovien@gmail.com)..."
                          : "Enter your email address..."
                      }
                      className="w-full pl-12 pr-4 py-3.5 rounded-xl glass-input text-sm text-white placeholder-slate-400"
                      required
                    />
                  </div>
                  <button
                    type="submit"
                    disabled={status === "loading"}
                    className="gradient-border-btn flex items-center justify-center gap-2 px-6 py-3.5 rounded-xl text-sm font-semibold text-white shadow-lg shadow-blue-600/30 whitespace-nowrap cursor-pointer disabled:opacity-50"
                  >
                    <span>
                      {status === "loading"
                        ? isVi
                          ? "Đang gửi..."
                          : "Submitting..."
                        : isVi
                        ? "Đăng Ký Nhận Tin"
                        : "Subscribe"}
                    </span>
                    <ArrowRight className="w-4 h-4" />
                  </button>
                </div>

                {/* Consent Checkbox */}
                <label className="flex items-center gap-2.5 text-xs text-slate-300 cursor-pointer self-start sm:self-center select-none pt-1">
                  <input
                    type="checkbox"
                    checked={agreed}
                    onChange={(e) => setAgreed(e.target.checked)}
                    className="w-4 h-4 rounded border-slate-600 bg-slate-900 text-blue-600 focus:ring-blue-500"
                  />
                  <span>
                    {isVi
                      ? "Tôi đồng ý nhận thông báo cập nhật và tôn trọng Chính sách Quyền riêng tư của ExamPlus."
                      : "I agree to receive release notifications in accordance with ExamPlus Privacy Policy."}
                  </span>
                </label>

                {status === "error" && (
                  <p className="text-xs text-red-400 font-medium">{errorMessage}</p>
                )}
              </form>
            )}

            {/* Privacy note */}
            <p className="text-[11px] text-slate-400 mt-4 flex items-center gap-1.5">
              <Sparkles className="w-3.5 h-3.5 text-blue-400" />
              <span>
                {isVi
                  ? "Cam kết bảo mật 100%: Tuyệt đối không gửi thư rác, không chia sẻ email với bên thứ ba."
                  : "Zero spam guarantee: We never sell or share your contact info with any third party."}
              </span>
            </p>
          </div>
        </div>
      </div>
    </section>
  );
}
