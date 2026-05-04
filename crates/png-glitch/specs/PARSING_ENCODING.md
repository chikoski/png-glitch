# Parsing and Encoding

The `png-glitch` library implements a specialized PNG parser and encoder focused on preserving chunk order and handling raw decompressed data for glitching.

## PNG Structure Handling

### Chunk Preservation
The library parses a PNG file into a sequence of chunks. It distinguishes between:
*   **Header (`IHDR`):** Mandatory first chunk containing image metadata.
*   **Terminator (`IEND`):** Mandatory final chunk.
*   **Image Data (`IDAT`):** One or more chunks containing compressed pixel data.
*   **Misc Chunks:** All other ancillary chunks (e.g., `PLTE`, `gAMA`, `tEXt`).

During parsing, all `IDAT` chunks are concatenated and decompressed into a single contiguous buffer. Ancillary chunks are stored in a `Vec<Chunk>` to be re-emitted during encoding in their original relative order.

## Decompression and Deflation

The library uses the `fdeflate` crate for high-performance `DEFLATE` operations.

*   **Parsing:** The concatenated `IDAT` data is decompressed into a `DecodedData` buffer (a `Vec<u8>`). This buffer represents the "filtered" data (each scanline begins with a filter type byte).
*   **Encoding:** The `DecodedData` buffer is re-compressed (deflated) and wrapped into new `IDAT` chunks.

## Error Handling

The library uses the `anyhow` crate for flexible error propagation and `thiserror` for the core `PngError` enum.

Common errors handled by the `Parser`:
*   `InvalidSignature`: The file does not start with the 8-byte PNG signature.
*   `NoIHDRFound`: The file is missing the mandatory header chunk.
*   `IncompleteDecompression`: The decompressed `IDAT` stream is shorter than the image dimensions require.
*   `UnsupportedInterlacing`: Interlaced PNGs (Adam7) are currently not supported for glitching.
*   `UnsupportedCompressionMethod` / `UnsupportedFilterMethod`: Only standard compression (0) and adaptive filtering (0) are supported.

## Security Considerations

As the library parses raw byte streams, it includes a **fuzzing infrastructure** (`fuzz/` directory) using `cargo-fuzz`. This helps ensure that malicious or malformed PNG files do not cause panics or memory safety issues within the `Parser`.
