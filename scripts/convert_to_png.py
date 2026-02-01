from PIL import Image
import os
import sys

def convert_to_transparent_png(input_path, output_path, chroma_color=(255, 0, 255)):
    """
    Converts an image to PNG and makes a specific chroma color transparent.
    Default chroma color is Magenta (#FF00FF).
    """
    try:
        img = Image.open(input_path).convert("RGBA")
        datas = img.getdata()

        new_data = []
        
        # Parameters
        # Threshold for full transparency
        cutoff_threshold = 60 
        # Threshold for partial transparency / color correction
        fade_threshold = 180 
        
        for item in datas:
            # Handle both RGB and RGBA inputs
            if len(item) == 4:
                r, g, b, a = item
            else:
                r, g, b = item
                a = 255
            
            # Manhattan distance to magenta
            dist = abs(r - chroma_color[0]) + abs(g - chroma_color[1]) + abs(b - chroma_color[2])
            
            if dist < cutoff_threshold:
                new_data.append((0, 0, 0, 0))
                continue

            # Check for inner magenta gaps (high bias)
            magenta_bias = (r + b) / 2 - g
            if magenta_bias > 60:
                # Likely an inner gap -> Transparent
                new_data.append((0, 0, 0, 0))
                continue
                
            if dist < fade_threshold:
                # Edge region processing
                factor = (dist - cutoff_threshold) / (fade_threshold - cutoff_threshold)
                new_alpha = int(a * factor)
                
                # Despill
                magenta_bias = (r + b) / 2 - g
                if magenta_bias > 0:
                    correction_strength = (1.0 - factor)
                    new_r = int(r - (magenta_bias * correction_strength))
                    new_b = int(b - (magenta_bias * correction_strength))
                    new_r = max(0, new_r)
                    new_b = max(0, new_b)
                    new_data.append((new_r, g, new_b, new_alpha))
                else:
                    new_data.append((r, g, b, new_alpha))
            else:
                # Safe zone
                new_data.append((r, g, b, a))

        img.putdata(new_data)
        img.save(output_path, "PNG")
        print(f"Successfully converted {input_path} to {output_path} with refined transparency")
    except Exception as e:
        print(f"Error: {e}")

if __name__ == "__main__":
    if len(sys.argv) < 3:
        print("Usage: python convert.py <input> <output>")
    else:
        convert_to_transparent_png(sys.argv[1], sys.argv[2])
