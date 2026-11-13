"use client";

import React, { useEffect, useState } from "react";
import { Sun, Moon } from "lucide-react";

export default function ThemeToggle() {
  const [theme, setTheme] = useState<"light" | "dark">("dark");
  const [mounted, setMounted] = useState(false);

  useEffect(() => {
    setMounted(true);
    const saved = localStorage.getItem("examplus_theme") as "light" | "dark" | null;
    if (saved) {
      setTheme(saved);
      if (saved === "dark") {
        document.documentElement.classList.add("dark");
        document.documentElement.setAttribute("data-theme", "dark");
      } else {
        document.documentElement.classList.remove("dark");
        document.documentElement.setAttribute("data-theme", "light");
      }
    } else {
      document.documentElement.classList.add("dark");
      document.documentElement.setAttribute("data-theme", "dark");
    }
  }, []);

  const toggleTheme = () => {
    const next = theme === "dark" ? "light" : "dark";
    setTheme(next);
    localStorage.setItem("examplus_theme", next);
    if (next === "dark") {
      document.documentElement.classList.add("dark");
      document.documentElement.setAttribute("data-theme", "dark");
    } else {
      document.documentElement.classList.remove("dark");
      document.documentElement.setAttribute("data-theme", "light");
    }
  };

  if (!mounted) {
    return (
      <div
        className="h-10 w-10 rounded-full border border-slate-200/90 dark:border-white/10 bg-white/80 dark:bg-slate-900/80 shrink-0"
        aria-hidden="true"
      />
    );
  }

  return (
    <button
      onClick={toggleTheme}
      type="button"
      className="inline-flex items-center justify-center h-10 w-10 rounded-full border border-slate-200/90 dark:border-white/10 bg-white/85 dark:bg-slate-900/85 text-slate-700 dark:text-slate-200 shadow-sm hover:border-blue-500/50 hover:bg-white dark:hover:bg-slate-800 transition-all cursor-pointer shrink-0 active:scale-95"
      title={theme === "dark" ? "Chuyển sang giao diện Sáng" : "Chuyển sang giao diện Tối"}
      aria-label={theme === "dark" ? "Chuyển sang giao diện Sáng" : "Chuyển sang giao diện Tối"}
    >
      {theme === "dark" ? (
        <Sun className="w-4 h-4 text-amber-400 transition-transform duration-200" aria-hidden="true" />
      ) : (
        <Moon className="w-4 h-4 text-blue-600 transition-transform duration-200" aria-hidden="true" />
      )}
    </button>
  );
}
