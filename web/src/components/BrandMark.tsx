/** The pool's concentric ripples, shared by the masthead and footer. */
export function BrandMark() {
  return (
    <svg className="brand-mark" viewBox="0 0 32 32" fill="none" aria-hidden="true">
      <ellipse cx="16" cy="18" rx="14" ry="9" />
      <ellipse cx="16" cy="16" rx="10" ry="6.5" />
      <ellipse cx="16" cy="14" rx="6" ry="4" />
      <path d="M16 3v5M13.5 5.5h5" />
    </svg>
  );
}
