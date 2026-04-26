import type { CSSProperties } from "react";

type ContextOsMarkProps = {
  className?: string;
  style?: CSSProperties;
};

export function ContextOsMark({ className, style }: ContextOsMarkProps) {
  return (
    <svg
      aria-hidden="true"
      className={className}
      fill="none"
      height="90"
      style={style}
      viewBox="0 0 76 76"
      width="90"
      xmlns="http://www.w3.org/2000/svg"
    >
      <rect width="76" height="76" rx="18" fill="#0F1117" />
      <rect
        x="0.75"
        y="0.75"
        width="74.5"
        height="74.5"
        rx="17.25"
        stroke="white"
        strokeOpacity="0.14"
        strokeWidth="1.5"
      />
      <path
        d="M34 16C26.82 16 21 21.82 21 29V47C21 54.18 26.82 60 34 60"
        stroke="white"
        strokeLinecap="round"
        strokeLinejoin="round"
        strokeWidth="2.4"
      />
      <path
        d="M42 16C49.18 16 55 21.82 55 29V47C55 54.18 49.18 60 42 60"
        stroke="white"
        strokeLinecap="round"
        strokeLinejoin="round"
        strokeWidth="2.4"
      />
      <path d="M38 17.5V58.5" stroke="white" strokeLinecap="round" strokeWidth="2.2" />
      <path d="M29.5 31.5H38" stroke="white" strokeLinecap="round" strokeWidth="2" />
      <path d="M38 25.5H45" stroke="white" strokeLinecap="round" strokeWidth="2" />
      <path d="M38 44.5H47" stroke="white" strokeLinecap="round" strokeWidth="2" />
      <path d="M38 52H45" stroke="white" strokeLinecap="round" strokeWidth="2" />
      <path
        d="M29.5 34.1C30.936 34.1 32.1 32.936 32.1 31.5C32.1 30.064 30.936 28.9 29.5 28.9C28.064 28.9 26.9 30.064 26.9 31.5C26.9 32.936 28.064 34.1 29.5 34.1Z"
        fill="white"
      />
      <path
        d="M45 28.1C46.436 28.1 47.6 26.936 47.6 25.5C47.6 24.064 46.436 22.9 45 22.9C43.564 22.9 42.4 24.064 42.4 25.5C42.4 26.936 43.564 28.1 45 28.1Z"
        fill="white"
      />
      <path
        d="M47 47.1C48.436 47.1 49.6 45.936 49.6 44.5C49.6 43.064 48.436 41.9 47 41.9C45.564 41.9 44.4 43.064 44.4 44.5C44.4 45.936 45.564 47.1 47 47.1Z"
        fill="white"
      />
      <path
        d="M45 54.3C46.27 54.3 47.3 53.27 47.3 52C47.3 50.73 46.27 49.7 45 49.7C43.73 49.7 42.7 50.73 42.7 52C42.7 53.27 43.73 54.3 45 54.3Z"
        fill="white"
      />
    </svg>
  );
}
