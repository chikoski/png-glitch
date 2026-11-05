---
name: Bug report
about: Report a crash, corrupt output, or unexpected behavior
title: "[Bug] "
labels: bug
assignees: ''
---

## Describe the bug
A clear and concise description of what went wrong.

## Reproduction steps
1. Command or Rust snippet executed:
   ```sh
   png-glitch input.png --seed 12345 ...
   ```
2. Seed value printed or passed (`--seed`):
3. Input image details:
   - Format: (PNG / WebP)
   - Dimensions (`width x height`):
   - Color type (`Grayscale`, `RGB`, `Indexed`, `GrayscaleAlpha`, `RGBA`):
   - Bit depth (`1`, `2`, `4`, `8`, `16`):

## Expected behavior
What you expected to happen.

## Actual behavior / Error output
Paste any error message or backtrace (`RUST_BACKTRACE=1`).

## Environment
- OS: (e.g. macOS, Linux, Windows)
- `png-glitch --version` / crate version:
- `rustc --version`:
