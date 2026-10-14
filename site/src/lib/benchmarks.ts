// Measured with `feanor dev bench` and `feanor dev incremental` (feanor 0.3.0,
// release build) on an AMD Ryzen 5 5500U, single thread, median of repeated runs.
// Files are real feature code from open-source fonts.

export const MACHINE = "AMD Ryzen 5 5500U laptop processor, on one thread";

/** Lint and format time for whole files, by size. */
export const WHOLE_FILE = [
  { file: "Inter calt.fea", lines: 310, lintMs: 1.29, formatMs: 0.91 },
  { file: "Roboto features.fea", lines: 1027, lintMs: 4.55, formatMs: 4.0 },
  { file: "Noto Nastaliq Urdu", lines: 3087, lintMs: 5.98, formatMs: 5.67 },
  { file: "Gentium features.fea", lines: 11077, lintMs: 54.51, formatMs: 51.75 },
  { file: "Harmattan features.fea", lines: 65379, lintMs: 245.24, formatMs: 252.58 },
];

/** Re-linting after one edit (median over 726 edits) versus linting the whole file. */
export const PER_EDIT = [
  { file: "Roboto features.fea", lines: 1027, editMs: 2.92, fullMs: 4.56 },
  { file: "Noto Nastaliq Urdu", lines: 3087, editMs: 1.8, fullMs: 6.03 },
  { file: "Gentium features.fea", lines: 11077, editMs: 7.91, fullMs: 54.08 },
  { file: "Harmattan features.fea", lines: 65379, editMs: 9.52, fullMs: 244.98 },
];
