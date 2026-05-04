# PNG Filter System

PNG uses a set of five filters to improve the efficiency of the `DEFLATE` compression. These filters transform pixel values into differences (deltas) between a pixel and its neighbors.

In `png-glitch`, you can use these filters creatively to produce distortions by applying or removing them incorrectly.

## Filter Types

The `FilterType` enum supports all standard PNG filters:

| Filter Type | ID | Description |
| :--- | :--- | :--- |
| `None` | 0 | The raw pixel values are stored. |
| `Sub` | 1 | Stores the difference between a pixel and its left neighbor. |
| `Up` | 2 | Stores the difference between a pixel and the pixel above it. |
| `Average` | 3 | Stores the difference between a pixel and the average of its left and top neighbors. |
| `Paeth` | 4 | Uses a more complex linear predictor (the Paeth predictor) based on left, top, and top-left neighbors. |

## Glitching with Filters

### Filter Removal
Standard PNG decoding always removes filters to get the original image. `png-glitch` allows you to remove filters from a specific region or the whole image. Once removed, the filter type for those lines is set to `None`.

### Filter Application
You can "force" a specific filter type onto a scanline. For example, if you apply the `Paeth` filter to a scanline that was previously raw (`None`), the data will be transformed into deltas. When a standard PNG viewer later tries to render this, it will apply the Paeth *inverse* operation, leading to complex visual artifacts.

### Filter Mismatch
One of the most powerful glitch techniques is to change the filter type byte without actually transforming the data. 

**Example:**
1.  Remove all filters (data is now raw).
2.  Set the filter type byte to `Sub` for all lines.
3.  The PNG viewer will now interpret the raw pixel values as deltas from their neighbors, creating a horizontal "streaking" effect.

## Sequential Dependency
Note that filters like `Up`, `Average`, and `Paeth` depend on the *previous* scanline. When removing or applying these filters across a range, `png-glitch` processes the lines sequentially to maintain the mathematical relationships.
