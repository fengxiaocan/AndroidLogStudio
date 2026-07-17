import React from 'react';

interface IconProps {
  className?: string;
  size?: number;
}

export function LogcatIcon({ className = '', size = 18 }: IconProps) {
  return (
    <svg
      width={size}
      height={size}
      viewBox="0 0 24 24"
      fill="none"
      xmlns="http://www.w3.org/2000/svg"
      className={className}
      aria-hidden="true"
    >
      {/* Background panel */}
      <rect
        x="2"
        y="3"
        width="20"
        height="18"
        rx="3"
        ry="3"
        fill="#1a1f26"
        stroke="#263140"
        strokeWidth="1.25"
      />

      {/* Top accent bar (like terminal header) */}
      <rect x="2" y="3" width="20" height="3.5" rx="3" ry="3" fill="#222932" />

      {/* Log lines - different levels */}
      {/* Info - green */}
      <rect x="5" y="8.5" width="14" height="1.6" rx="0.8" fill="#3ddc84" opacity="0.95" />
      {/* Debug - cyan-ish */}
      <rect x="5" y="11.2" width="11" height="1.6" rx="0.8" fill="#58d1e8" opacity="0.9" />
      {/* Warning - amber */}
      <rect x="5" y="13.9" width="15.5" height="1.6" rx="0.8" fill="#f0b429" opacity="0.92" />
      {/* Error - red */}
      <rect x="5" y="16.6" width="9" height="1.6" rx="0.8" fill="#f25c5c" opacity="0.95" />

      {/* Subtle left gutter accent (Android green) */}
      <rect x="2" y="6.5" width="1.5" height="14.5" rx="0.75" fill="#3ddc84" opacity="0.6" />
    </svg>
  );
}

export default LogcatIcon;
