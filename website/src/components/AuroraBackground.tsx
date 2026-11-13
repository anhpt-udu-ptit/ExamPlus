"use client";

import React from "react";

export default function AuroraBackground() {
  return (
    <div className="fixed inset-0 pointer-events-none -z-10 overflow-hidden">
      {/* Background base */}
      <div className="absolute inset-0 bg-[#070b14]" />

      {/* Figma Dot Grid */}
      <div className="absolute inset-0 dot-grid-pattern opacity-40" />

      {/* Aurora Blob 1: Royal Blue */}
      <div
        className="absolute top-[-10%] left-[15%] w-[680px] h-[680px] rounded-full animate-aurora-1 opacity-45 mix-blend-screen"
        style={{
          background: "radial-gradient(circle, rgba(29, 78, 216, 0.85) 0%, rgba(37, 99, 235, 0.25) 50%, transparent 75%)",
          filter: "blur(110px)",
        }}
      />

      {/* Aurora Blob 2: Indigo / Violet */}
      <div
        className="absolute top-[20%] right-[10%] w-[620px] h-[620px] rounded-full animate-aurora-2 opacity-40 mix-blend-screen"
        style={{
          background: "radial-gradient(circle, rgba(99, 102, 241, 0.75) 0%, rgba(79, 70, 229, 0.2) 50%, transparent 75%)",
          filter: "blur(120px)",
        }}
      />

      {/* Aurora Blob 3: Cyan Blue */}
      <div
        className="absolute bottom-[-10%] left-[30%] w-[720px] h-[720px] rounded-full animate-aurora-3 opacity-30 mix-blend-screen"
        style={{
          background: "radial-gradient(circle, rgba(2, 132, 199, 0.7) 0%, rgba(6, 182, 212, 0.18) 55%, transparent 80%)",
          filter: "blur(130px)",
        }}
      />

      {/* Vignette border */}
      <div className="absolute inset-0 bg-radial-gradient from-transparent via-[#070b14]/40 to-[#070b14] pointer-events-none" />
    </div>
  );
}
