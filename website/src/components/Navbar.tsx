"use client";

import React, { useState, useEffect } from "react";
import Link from "next/link";
import Image from "next/image";
import { usePathname } from "next/navigation";
import ThemeToggle from "@/components/ThemeToggle";
import {
  Sparkles,
  Workflow,
  Download,
  Shield,
  FileText,
  Menu,
  X,
  Globe,
  ArrowUpRight,
} from "lucide-react";

interface NavbarProps {
  lang?: "vi" | "en";
  onToggleLang?: (lang: "vi" | "en") => void;
}

export default function Navbar({ lang = "vi", onToggleLang }: NavbarProps) {
  const pathname = usePathname();
  const [scrolled, setScrolled] = useState(false);
  const [mobileMenuOpen, setMobileMenuOpen] = useState(false);
  const [activeNav, setActiveNav] = useState<string>("features");

  useEffect(() => {
    const handleScroll = () => {
      setScrolled(window.scrollY > 20);
    };
    window.addEventListener("scroll", handleScroll);
    return () => window.removeEventListener("scroll", handleScroll);
  }, []);

  // Handle ESC key for mobile drawer
  useEffect(() => {
    const handleKeyDown = (e: KeyboardEvent) => {
      if (e.key === "Escape" && mobileMenuOpen) {
        setMobileMenuOpen(false);
      }
    };
    window.addEventListener("keydown", handleKeyDown);
    return () => window.removeEventListener("keydown", handleKeyDown);
  }, [mobileMenuOpen]);

  // Sync active navigation item with current pathname and hash / scroll position
  useEffect(() => {
    if (pathname === "/privacy") {
      setActiveNav("privacy");
      return;
    }
    if (pathname === "/terms") {
      setActiveNav("terms");
      return;
    }
    if (pathname === "/") {
      const handleHashChange = () => {
        const hash = window.location.hash.replace("#", "");
        if (hash && ["features", "workflow", "download"].includes(hash)) {
          setActiveNav(hash);
        }
      };

      const handleScrollSpy = () => {
        const scrollPos = window.scrollY + 250;
        const sections = ["download", "workflow", "features"];
        for (const secId of sections) {
          const el = document.getElementById(secId);
          if (el && el.offsetTop <= scrollPos) {
            setActiveNav(secId);
            return;
          }
        }
        if (window.scrollY < 400) {
          setActiveNav("features");
        }
      };

      handleHashChange();
      window.addEventListener("hashchange", handleHashChange);
      window.addEventListener("scroll", handleScrollSpy, { passive: true });
      return () => {
        window.removeEventListener("hashchange", handleHashChange);
        window.removeEventListener("scroll", handleScrollSpy);
      };
    }
  }, [pathname]);

  const navItems = [
    {
      key: "features",
      href: "/#features",
      label: lang === "vi" ? "Tính Năng" : "Features",
      icon: Sparkles,
    },
    {
      key: "workflow",
      href: "/#workflow",
      label: lang === "vi" ? "Quy Trình" : "Workflow",
      icon: Workflow,
    },
    {
      key: "download",
      href: "/#download",
      label: lang === "vi" ? "Tải Về" : "Download",
      icon: Download,
    },
    {
      key: "privacy",
      href: "/privacy",
      label: lang === "vi" ? "Quyền Riêng Tư" : "Privacy Policy",
      icon: Shield,
    },
    {
      key: "terms",
      href: "/terms",
      label: lang === "vi" ? "Điều Khoản" : "Terms",
      icon: FileText,
    },
  ];

  const t = {
    downloadBtn: lang === "vi" ? "Tải ExamPlus" : "Get ExamPlus",
  };

  return (
    <header
      className={`fixed top-0 left-0 right-0 z-50 transition-all duration-300 ${
        scrolled
          ? "bg-white/90 dark:bg-[#070a13]/90 backdrop-blur-2xl border-b border-slate-200/90 dark:border-white/[0.08] shadow-sm py-2.5"
          : "bg-transparent py-4 sm:py-5"
      }`}
    >
      <div className="max-w-7xl mx-auto px-4 sm:px-6 lg:px-8">
        <div className="flex items-center justify-between">
          {/* Brand Logo */}
          <Link href="/" className="flex items-center gap-3 group" aria-label="ExamPlus Trang Chủ">
            <div className="relative w-9 h-9 rounded-xl overflow-hidden shadow-sm shadow-blue-500/20 group-hover:scale-105 transition-transform duration-200">
              <Image
                src="/logo_blue.svg"
                alt="ExamPlus Logo"
                fill
                className="object-contain"
                priority
              />
            </div>
            <div className="flex flex-col">
              <span className="text-lg font-bold tracking-tight text-slate-900 dark:text-white flex items-center gap-1.5 leading-none">
                ExamPlus
                <span className="text-[10px] uppercase font-semibold tracking-wider px-1.5 py-0.5 rounded-full bg-blue-500/10 text-blue-600 dark:text-blue-400 border border-blue-500/20">
                  v1.0
                </span>
              </span>
              <span className="text-[11px] text-slate-500 dark:text-slate-400 font-medium tracking-normal mt-0.5">
                Teacher Studio
              </span>
            </div>
          </Link>

          {/* Desktop Nav */}
          <nav className="hidden md:flex items-center gap-6 lg:gap-8" aria-label="Thanh điều hướng chính">
            {navItems.map((item, idx) => {
              const isActive = activeNav === item.key;
              const IconComp = item.icon;
              return (
                <React.Fragment key={item.key}>
                  {idx === 3 && (
                    <div className="h-4 w-[1px] bg-slate-200 dark:bg-white/15" aria-hidden="true" />
                  )}
                  <Link
                    href={item.href}
                    onClick={() => setActiveNav(item.key)}
                    className={`text-sm font-medium transition-all duration-200 flex items-center gap-1.5 ${
                      isActive
                        ? "text-blue-600 dark:text-blue-400 font-semibold"
                        : "text-slate-700 hover:text-blue-600 dark:text-white/80 dark:hover:text-white"
                    }`}
                  >
                    {isActive && (
                      <IconComp
                        className="w-3.5 h-3.5 text-blue-500 shrink-0 animate-in fade-in zoom-in-75 duration-150"
                        aria-hidden="true"
                      />
                    )}
                    <span>{item.label}</span>
                  </Link>
                </React.Fragment>
              );
            })}
          </nav>

          {/* Actions: Symmetrical Controls Cluster */}
          <div className="hidden sm:flex items-center gap-2.5">
            {/* Button 1: Theme Toggle (Height 40px) */}
            <ThemeToggle />

            {/* Button 2: Language Toggle (Height 40px) */}
            {onToggleLang && (
              <button
                onClick={() => onToggleLang(lang === "vi" ? "en" : "vi")}
                className="inline-flex items-center justify-center h-10 px-3.5 rounded-full border border-slate-200/90 dark:border-white/10 bg-white/85 dark:bg-slate-900/85 text-xs font-semibold text-slate-700 dark:text-slate-200 shadow-sm hover:border-blue-500/50 hover:bg-white dark:hover:bg-slate-800 transition-all cursor-pointer shrink-0 gap-1.5 active:scale-95"
                title="Đổi ngôn ngữ / Switch language"
                aria-label="Đổi ngôn ngữ hiển thị"
              >
                <Globe className="w-3.5 h-3.5 text-blue-500" aria-hidden="true" />
                <span>{lang === "vi" ? "VI" : "EN"}</span>
              </button>
            )}

            {/* Button 3: Tactile Primary Download Button (Height 40px with depth) */}
            <Link
              href="#download"
              onClick={() => setActiveNav("download")}
              className="btn-tactile-control gap-1.5 cursor-pointer"
            >
              <span>{t.downloadBtn}</span>
              <ArrowUpRight className="w-4 h-4" aria-hidden="true" />
            </Link>
          </div>

          {/* Mobile Menu Toggle Button */}
          <div className="md:hidden flex items-center gap-2">
            <ThemeToggle />
            <button
              onClick={() => setMobileMenuOpen(!mobileMenuOpen)}
              className="p-2 min-h-[44px] min-w-[44px] inline-flex items-center justify-center rounded-xl text-slate-700 dark:text-slate-200 hover:bg-slate-100 dark:hover:bg-white/[0.06] transition-colors"
              aria-label={mobileMenuOpen ? "Đóng menu điều hướng" : "Mở menu điều hướng"}
              aria-expanded={mobileMenuOpen}
            >
              {mobileMenuOpen ? <X className="w-6 h-6" aria-hidden="true" /> : <Menu className="w-6 h-6" aria-hidden="true" />}
            </button>
          </div>
        </div>
      </div>

      {/* Mobile Menu Drawer */}
      {mobileMenuOpen && (
        <div
          className="md:hidden bg-white/95 dark:bg-[#070a13]/95 backdrop-blur-2xl border-b border-slate-200 dark:border-white/10 px-5 py-5 mt-3 shadow-2xl animate-in fade-in slide-in-from-top-2 duration-200"
          role="dialog"
          aria-label="Menu điều hướng mobile"
        >
          <div className="flex flex-col gap-2">
            {navItems.map((item) => {
              const isActive = activeNav === item.key;
              const IconComp = item.icon;
              return (
                <Link
                  key={item.key}
                  href={item.href}
                  onClick={() => {
                    setActiveNav(item.key);
                    setMobileMenuOpen(false);
                  }}
                  className={`text-base font-medium py-2.5 px-3 rounded-xl transition-all duration-200 flex items-center gap-2.5 ${
                    isActive
                      ? "text-blue-600 dark:text-blue-400 font-semibold bg-blue-500/10 border border-blue-500/20"
                      : "text-slate-700 dark:text-white hover:text-blue-600 dark:hover:text-blue-400 hover:bg-slate-100/50 dark:hover:bg-white/[0.04]"
                  }`}
                >
                  {isActive && (
                    <IconComp
                      className="w-4 h-4 text-blue-500 shrink-0 animate-in fade-in zoom-in-75 duration-150"
                      aria-hidden="true"
                    />
                  )}
                  <span>{item.label}</span>
                </Link>
              );
            })}

            <div className="pt-3 mt-1 flex items-center justify-between border-t border-slate-100 dark:border-white/10">
              {onToggleLang && (
                <button
                  onClick={() => onToggleLang(lang === "vi" ? "en" : "vi")}
                  className="inline-flex items-center gap-2 py-2 px-1 text-sm font-semibold text-slate-700 dark:text-slate-300"
                >
                  <Globe className="w-4 h-4 text-blue-500" aria-hidden="true" />
                  <span>Ngôn ngữ: {lang === "vi" ? "Tiếng Việt (VI)" : "English (EN)"}</span>
                </button>
              )}
            </div>

            <Link
              href="#download"
              onClick={() => {
                setActiveNav("download");
                setMobileMenuOpen(false);
              }}
              className="btn-tactile-primary w-full text-center mt-2"
            >
              <span>{t.downloadBtn}</span>
              <ArrowUpRight className="w-4 h-4" aria-hidden="true" />
            </Link>
          </div>
        </div>
      )}
    </header>
  );
}
