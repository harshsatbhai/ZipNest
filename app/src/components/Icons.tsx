import type { SVGProps } from "react";

type P = SVGProps<SVGSVGElement> & { size?: number };

function Svg({ size = 18, children, ...rest }: P) {
  return (
    <svg
      width={size}
      height={size}
      viewBox="0 0 24 24"
      fill="none"
      stroke="currentColor"
      strokeWidth={1.8}
      strokeLinecap="round"
      strokeLinejoin="round"
      aria-hidden="true"
      {...rest}
    >
      {children}
    </svg>
  );
}

export const IconCompress = (p: P) => (
  <Svg {...p}>
    <path d="M21 8v12a1 1 0 0 1-1 1H4a1 1 0 0 1-1-1V8" />
    <rect x="2" y="3" width="20" height="5" rx="1" />
    <path d="M12 11v6M9.5 14.5 12 17l2.5-2.5" />
  </Svg>
);

export const IconExtract = (p: P) => (
  <Svg {...p}>
    <path d="M21 8v12a1 1 0 0 1-1 1H4a1 1 0 0 1-1-1V8" />
    <rect x="2" y="3" width="20" height="5" rx="1" />
    <path d="M12 17v-6M9.5 13.5 12 11l2.5 2.5" />
  </Svg>
);

export const IconFolder = (p: P) => (
  <Svg {...p}>
    <path d="M3 7a2 2 0 0 1 2-2h4l2 2h8a2 2 0 0 1 2 2v8a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2Z" />
  </Svg>
);

export const IconFile = (p: P) => (
  <Svg {...p}>
    <path d="M14 3H7a2 2 0 0 0-2 2v14a2 2 0 0 0 2 2h10a2 2 0 0 0 2-2V8Z" />
    <path d="M14 3v5h5" />
  </Svg>
);

export const IconZip = (p: P) => (
  <Svg {...p}>
    <path d="M14 3H7a2 2 0 0 0-2 2v14a2 2 0 0 0 2 2h10a2 2 0 0 0 2-2V8Z" />
    <path d="M14 3v5h5" />
    <path d="M10 4v1M10 7v1M10 10v1M10 13v1" />
    <rect x="8.5" y="15" width="3" height="3.5" rx="0.8" />
  </Svg>
);

export const IconX = (p: P) => (
  <Svg {...p}>
    <path d="M18 6 6 18M6 6l12 12" />
  </Svg>
);

export const IconPlus = (p: P) => (
  <Svg {...p}>
    <path d="M12 5v14M5 12h14" />
  </Svg>
);

export const IconLock = (p: P) => (
  <Svg {...p}>
    <rect x="4" y="11" width="16" height="10" rx="2" />
    <path d="M8 11V7a4 4 0 0 1 8 0v4" />
  </Svg>
);

export const IconShield = (p: P) => (
  <Svg {...p}>
    <path d="M12 22s8-4 8-10V5l-8-3-8 3v7c0 6 8 10 8 10Z" />
    <path d="m9 12 2 2 4-4" />
  </Svg>
);

export const IconCheck = (p: P) => (
  <Svg {...p}>
    <path d="M20 6 9 17l-5-5" />
  </Svg>
);

export const IconAlert = (p: P) => (
  <Svg {...p}>
    <path d="M10.3 3.9 1.8 18a2 2 0 0 0 1.7 3h17a2 2 0 0 0 1.7-3L13.7 3.9a2 2 0 0 0-3.4 0Z" />
    <path d="M12 9v4M12 17h.01" />
  </Svg>
);

export const IconWifiOff = (p: P) => (
  <Svg {...p}>
    <path d="M2 2l20 20" />
    <path d="M8.5 16.5a5 5 0 0 1 7 0M5 13a10 10 0 0 1 5.2-2.8M19 13a10 10 0 0 0-2-1.5M2 8.8a15 15 0 0 1 4.2-2.7M22 8.8A15 15 0 0 0 10.7 5" />
    <path d="M12 20h.01" />
  </Svg>
);

export const IconEye = (p: P) => (
  <Svg {...p}>
    <path d="M2 12s3.5-7 10-7 10 7 10 7-3.5 7-10 7S2 12 2 12Z" />
    <circle cx="12" cy="12" r="3" />
  </Svg>
);

export const IconEyeOff = (p: P) => (
  <Svg {...p}>
    <path d="M9.9 4.2A10 10 0 0 1 12 4c6.5 0 10 8 10 8a17 17 0 0 1-2.2 3.2M6.6 6.6C3.7 8.6 2 12 2 12s3.5 7 10 7a9.7 9.7 0 0 0 5.4-1.6" />
    <path d="M2 2l20 20M9.9 9.9a3 3 0 0 0 4.2 4.2" />
  </Svg>
);

export const Logo = ({ size = 28 }: { size?: number }) => (
  <svg width={size} height={size} viewBox="0 0 64 64" aria-hidden="true">
    <defs>
      <linearGradient id="zn-g" x1="0" y1="0" x2="1" y2="1">
        <stop offset="0" stopColor="#FFB547" />
        <stop offset="1" stopColor="#F2622E" />
      </linearGradient>
    </defs>
    <rect width="64" height="64" rx="15" fill="url(#zn-g)" />
    <path d="M14 30h36l-3.5 17.5A4 4 0 0 1 42.6 51H21.4a4 4 0 0 1-3.9-3.5Z" fill="#fff" />
    <path d="M17 30c1-9 7.5-15 15-15s14 6 15 15" fill="none" stroke="#fff" strokeWidth="4" strokeLinecap="round" />
    <g fill="#F2622E">
      <rect x="29" y="33" width="6" height="3" rx="1" />
      <rect x="29" y="38" width="6" height="3" rx="1" />
      <rect x="29" y="43" width="6" height="3" rx="1" />
    </g>
  </svg>
);
