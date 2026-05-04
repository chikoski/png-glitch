# Supported Pixel Formats

`png-glitch` supports all standard PNG bit depths and color types, ensuring that even highly compressed or high-precision images can be manipulated.

## Color Types

The library handles the following PNG color types (as defined in the `ColorType` enum in `src/png/parser/header/color_type.rs`):

| Color Type | Description | Channels |
| :--- | :--- | :--- |
| `GrayScale` | Grayscale image | 1 (Y) |
| `TrueColor` | RGB image | 3 (R, G, B) |
| `IndexColor` | Indexed color image (requires `PLTE` chunk) | 1 (Index) |
| `GrayScaleAlpha`| Grayscale image with alpha channel | 2 (Y, A) |
| `TrueColorAlpha`| RGB image with alpha channel | 4 (R, G, B, A) |

## Bit Depths

The library supports the full range of PNG bit depths. For depths lower than 8 bits, multiple pixels are packed into a single byte.

| Bit Depth | Supported Color Types | Notes |
| :--- | :--- | :--- |
| **1** | `GrayScale`, `IndexColor` | 8 pixels per byte. |
| **2** | `GrayScale`, `IndexColor` | 4 pixels per byte. |
| **4** | `GrayScale`, `IndexColor` | 2 pixels per byte. |
| **8** | All | 1 channel per byte. |
| **16** | `GrayScale`, `TrueColor`, `GrayScaleAlpha`, `TrueColorAlpha` | 1 channel per 2 bytes (Big-Endian). |

## Pixel API

The `Pixel` enum provides a unified way to interact with pixel data regardless of the underlying format:

*   `Pixel::Gray(u16)`
*   `Pixel::RGB(u16, u16, u16)`
*   `Pixel::Indexed(u8)`
*   `Pixel::GrayAlpha(u16, u16)`
*   `Pixel::RGBA(u16, u16, u16, u16)`

### Pixel Interaction Efficiency

While `get_pixel(x)` and `set_pixel(x)` are convenient for simple operations, the **`process_pixels`** API is the recommended way to perform bulk manipulations. It hoists format resolution (bit depth and color type checks) out of the inner loop, providing a massive performance speedup for large images.
