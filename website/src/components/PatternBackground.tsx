"use client";

import React from "react";

export default function PatternBackground() {
  return (
    <div
      className="fixed inset-0 pointer-events-none z-0 overflow-hidden select-none bg-vector-pattern"
      aria-hidden="true"
    >
      {/* 1. Uiverse.io sweet-dolphin-36 Dense Micro-Matrix Rain Engine */}
      <div className="uiv-rain-dense" />

      {/* 2. Top Ambient Spotlight Aura */}
      <div className="bg-ambient-spotlight" />

      {/* 3. Subtle Lower Ambient Soft Glow */}
      <div className="bg-ambient-lower" />

      {/* 4. Delicate decorative SVG accent lines */}
      <svg
        className="absolute top-0 left-1/2 -translate-x-1/2 w-full max-w-7xl h-[600px] opacity-[0.25] dark:opacity-[0.15] pointer-events-none"
        xmlns="http://www.w3.org/2000/svg"
        fill="none"
        viewBox="0 0 1200 600"
      >
        <path
          d="M0 120 C 300 180, 500 40, 600 160 C 700 280, 900 140, 1200 220"
          stroke="url(#gradient-line-1)"
          strokeWidth="1.5"
          strokeDasharray="4 4"
        />
        <path
          d="M0 240 C 350 160, 550 280, 600 200 C 650 120, 850 260, 1200 180"
          stroke="url(#gradient-line-2)"
          strokeWidth="1"
        />
        <defs>
          <linearGradient id="gradient-line-1" x1="0%" y1="0%" x2="100%" y2="0%">
            <stop offset="0%" stopColor="#2563eb" stopOpacity="0" />
            <stop offset="50%" stopColor="#3b82f6" stopOpacity="0.8" />
            <stop offset="100%" stopColor="#60a5fa" stopOpacity="0" />
          </linearGradient>
          <linearGradient id="gradient-line-2" x1="0%" y1="0%" x2="100%" y2="0%">
            <stop offset="0%" stopColor="#10b981" stopOpacity="0" />
            <stop offset="50%" stopColor="#38bdf8" stopOpacity="0.5" />
            <stop offset="100%" stopColor="#6366f1" stopOpacity="0" />
          </linearGradient>
        </defs>
      </svg>
    </div>
  );
}
