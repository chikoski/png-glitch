# glitch-context

A library to manage and apply glitch filters to PNG images, built on top of `png-glitch`.

## Installation

Add this to your `Cargo.toml`:

```toml
[dependencies]
glitch-context = "0.1.0"
```

## Usage

`glitch-context` provides a `GlitchContext` struct that allows you to chain multiple glitch filters and apply them to a PNG image.

### Example

```rust
use glitch_context::{GlitchContext, FilterType, SubFilter};

fn main() -> anyhow::Result<()> {
    // Open a PNG file
    let mut context = GlitchContext::open("input.png")?;

    // Add filters
    // Change filter type of all scan lines to Sub
    context.add_filter(SubFilter);
    
    // You can add more filters here...

    // Execute all added filters
    context.execute();

    // Save the result
    context.save("output.png")?;

    Ok(())
}
```

## Available Filters

The crate provides several built-in filters:

- **Filter Type Modifiers:**
  - `RemoveFilter`: Changes filter type of all scan lines to `None`.
  - `SubFilter`: Changes filter type of all scan lines to `Sub`.
  - `UpFilter`: Changes filter type of all scan lines to `Up`.
  - `AverageFilter`: Changes filter type of all scan lines to `Average`.
  - `PaethFilter`: Changes filter type of all scan lines to `Paeth`.
  - `ChangeFilterType { magnitude }`: Randomly changes filter type based on magnitude.

- **Glitch Effects:**
  - `Replace { magnitude }`: Replaces pixel data with random noise.
  - `Transpose { magnitude }`: Swaps scanlines randomly.
  - `SetZero { magnitude }`: Sets random pixels to zero.

## License

MIT License.
