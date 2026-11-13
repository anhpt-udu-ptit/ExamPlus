"use client";

import React, { useState } from "react";
import { ChevronDown, HelpCircle } from "lucide-react";

interface FAQSectionProps {
  lang?: "vi" | "en";
}

export default function FAQSection({ lang = "vi" }: FAQSectionProps) {
  const isVi = lang === "vi";
  const [openIndex, setOpenIndex] = useState<number | null>(0);

  const faqs = [
    {
      q: isVi
        ? "Tính năng đồng bộ Google Drive có an toàn và bảo mật không?"
        : "How secure is the Google Drive integration?",
      a: isVi
        ? "Tuyệt đối an toàn. ExamPlus chỉ lưu trữ tài liệu thi cử trong thư mục riêng ExamPlus/ trên Google Drive của bạn. Phần mềm không can thiệp, không đọc và không truy cập bất kỳ tệp dữ liệu cá nhân nào khác. Thầy cô có thể chủ động kiểm tra hoặc hủy kết nối bất kỳ lúc nào."
        : "Completely secure. ExamPlus only creates and stores exam backups within its dedicated ExamPlus/ folder on your Google Drive. It never accesses or shares any of your personal files or emails.",
    },
    {
      q: isVi
        ? "Nếu phòng máy trường học mất mạng Internet, phần mềm có hoạt động không?"
        : "Does ExamPlus work if the school network loses Internet connection?",
      a: isVi
        ? "Có, phần mềm hoạt động 100% bình thường. Toàn bộ quá trình phát đề, học sinh làm bài và thu bài diễn ra hoàn toàn trong mạng nội bộ phòng máy (qua Wi-Fi hoặc mạng dây). Dù trường học bị ngắt kết nối Internet ra ngoài, buổi thi vẫn diễn ra thông suốt."
        : "Yes, 100% functional offline. The entire exam session operates purely within the local classroom network via Wi-Fi or Ethernet. Even if external internet is disconnected, exams continue without interruption.",
    },
    {
      q: isVi
        ? "Đề thi Word chứa công thức Toán MathType có bị lỗi font hay vỡ hình không?"
        : "Are MathType equations in Word .docx files displayed accurately?",
      a: isVi
        ? "Không bao giờ bị lỗi. ExamPlus tự động xử lý và tối ưu toàn bộ công thức Toán học, phân số, ký hiệu và hình vẽ từ tệp Word, đảm bảo luôn hiển thị rõ ràng, chuẩn nét trên mọi kích thước màn hình máy tính và điện thoại."
        : "Never distorted. ExamPlus accurately decodes and optimizes mathematical formulas, fractions, and scientific symbols from Word documents, ensuring crisp display across all screen sizes.",
    },
    {
      q: isVi
        ? "Phần mềm tính điểm chuẩn GDPT 2018 như thế nào?"
        : "How does ExamPlus score exams under Vietnam's GDPT 2018 curriculum?",
      a: isVi
        ? "ExamPlus tính điểm tự động theo đúng quy chế chuẩn của Bộ Giáo dục & Đào tạo cho cả 3 phần thi: Phần I (Trắc nghiệm nhiều lựa chọn), Phần II (Trắc nghiệm Đúng/Sai với thang điểm bậc thang chính xác), và Phần III (Câu hỏi trả lời ngắn điền số)."
        : "ExamPlus precisely grades tests according to official Vietnam MOET standards: Part I (4-choice MCQs), Part II (True/False stepped grading rubric), and Part III (Short numeric responses).",
    },
    {
      q: isVi
        ? "Máy tính giáo viên có cần cài đặt phần mềm phụ trợ (như Python, Node.js) không?"
        : "Do teachers need to install additional software?",
      a: isVi
        ? "Hoàn toàn không. ExamPlus được đóng gói gọn nhẹ, chỉ cần tải về, giải nén và mở lên là sử dụng được ngay. Thầy cô cũng có thể chép vào USB để mang sang phòng máy bất kỳ mà không cần cài đặt thêm bất cứ thứ gì."
        : "Zero extra dependencies required. Simply download, unzip, and launch on any Windows 10 or 11 computer.",
    },
  ];

  return (
    <section id="faq" className="relative py-20 px-4 sm:px-6 lg:px-8">
      <div className="max-w-4xl mx-auto">
        <div className="text-center mb-10">
          <div className="inline-flex items-center gap-2 px-3.5 py-1.5 rounded-full bg-blue-500/10 border border-blue-500/20 text-blue-600 dark:text-blue-400 text-xs font-semibold mb-3">
            <HelpCircle className="w-4 h-4" />
            <span>{isVi ? "Giải Đáp Thắc Mắc" : "FAQ"}</span>
          </div>
          <h2 className="text-2xl sm:text-4xl font-extrabold text-slate-900 dark:text-white tracking-tight">
            {isVi ? "Câu Hỏi Thường Gặp" : "Frequently Asked Questions"}
          </h2>
        </div>

        <div className="flex flex-col gap-3.5">
          {faqs.map((item, idx) => {
            const isOpen = openIndex === idx;
            return (
              <div
                key={idx}
                className="rounded-2xl bg-white/92 dark:bg-[#0c1224]/90 backdrop-blur-2xl border border-slate-200/90 dark:border-white/10 shadow-sm overflow-hidden transition-all duration-200"
              >
                <button
                  onClick={() => setOpenIndex(isOpen ? null : idx)}
                  aria-expanded={isOpen}
                  aria-controls={`faq-answer-${idx}`}
                  className="w-full p-4 sm:p-5 text-left flex items-center justify-between gap-4 cursor-pointer hover:bg-slate-50/80 dark:hover:bg-white/[0.04] transition-colors focus-visible:ring-2 focus-visible:ring-blue-500"
                >
                  <span className="text-sm sm:text-base font-bold text-slate-900 dark:text-white">
                    {item.q}
                  </span>
                  <div
                    className={`w-7 h-7 rounded-lg border border-slate-200 dark:border-white/10 flex items-center justify-center text-slate-700 dark:text-slate-300 transition-transform duration-200 shrink-0 ${
                      isOpen ? "rotate-180 bg-blue-50 dark:bg-blue-600/30 text-blue-600 dark:text-blue-400" : "bg-slate-100/80 dark:bg-slate-800/80"
                    }`}
                  >
                    <ChevronDown className="w-4 h-4" aria-hidden="true" />
                  </div>
                </button>

                {isOpen && (
                  <div
                    id={`faq-answer-${idx}`}
                    role="region"
                    aria-label={item.q}
                    className="px-4 sm:px-5 pb-5 pt-2 text-xs sm:text-sm text-slate-800 dark:text-slate-100 font-normal leading-relaxed border-t border-slate-200/80 dark:border-white/10 animate-in fade-in duration-200"
                  >
                    {item.a}
                  </div>
                )}
              </div>
            );
          })}
        </div>
      </div>
    </section>
  );
}
