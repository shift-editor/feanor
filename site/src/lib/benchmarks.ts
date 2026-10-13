// Measured with `feanor dev bench` and `feanor dev incremental` (feanor 0.1.0,
// release build) on an AMD Ryzen 5 5500U, single thread, median of repeated runs.
// Files are real feature code from open-source fonts.

export const MACHINE = "AMD Ryzen 5 5500U laptop processor, on one thread";

/** Lint and format time for whole files, by size. */
export const WHOLE_FILE = [
  { file: "Inter calt.fea", lines: 310, lintMs: 0.91, formatMs: 0.81 },
  { file: "Roboto features.fea", lines: 1027, lintMs: 3.07, formatMs: 3.37 },
  { file: "Noto Nastaliq Urdu", lines: 3087, lintMs: 3.97, formatMs: 4.87 },
  { file: "Gentium features.fea", lines: 11077, lintMs: 39.0, formatMs: 44.65 },
  { file: "Harmattan features.fea", lines: 65379, lintMs: 174.97, formatMs: 225.98 },
];

/** Re-linting after one edit (median over 726 edits) versus linting the whole file. */
export const PER_EDIT = [
  { file: "Roboto features.fea", lines: 1027, editMs: 1.49, fullMs: 2.87 },
  { file: "Noto Nastaliq Urdu", lines: 3087, editMs: 0.71, fullMs: 3.83 },
  { file: "Gentium features.fea", lines: 11077, editMs: 3.5, fullMs: 38.59 },
  { file: "Harmattan features.fea", lines: 65379, editMs: 2.05, fullMs: 172.65 },
];
