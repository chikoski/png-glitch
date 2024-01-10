# png-glitch

A library to glitch PNG files. This library is inspired by the [pnglitch](https://github.com/ucnv/pnglitch), a Ruby library to glitch PNG images.

Please visit "[The Art of PNG Glitch](https://ucnv.github.io/pnglitch/)" for more details about glitching PNG images.

![Glitched PNG image](etc/sample00-glitched.png)

The original image: 

![The original PNG file is a photo of a media art placed in a slightly darker space.](etc/sample00.png)

# Example usage

The following snippet glitches `./a_png_file.png` by 

- Changing filter method of all scan lines 
- Setting `1` to the 4th byte of each scan line 

The glitched image is emitted to `./glitched.png`.

```Rust
use png_glitch::{FilterType, PngGlitch};

let mut png_glitch = PngGlitch::open("./a_png_file.png")?;
png_glitch.foreach_scanline(|scan_line|{
  scan_line.set_filter_type(FilterType::None);
  scan_line[4] = 1;
});
png_glitch.save("./glitched.png")?;
```

# Contribution

1. Fork the repository.
2. Create a feature branch on your forked repository with `git checkout -b feature-name` command.
3. Develop the feature.
4. Commit your changes with `git commit` command.
5. Upload the feature branch to GitHub and create a pull request.

# License

Please refer to the [LICENSE](LICENSE) file.