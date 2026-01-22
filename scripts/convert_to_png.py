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
        # Support a small threshold for JPEG compression artifacts around the chroma color
        threshold = 60 
        
        for item in datas:
            # Check if this pixel is close to our chroma color
            dist = sum(abs(item[i] - chroma_color[i]) for i in range(3))
            
            if dist < threshold:
                # Make transparent
                new_data.append((0, 0, 0, 0))
            else:
                new_data.append(item)

        img.putdata(new_data)
        img.save(output_path, "PNG")
        print(f"Successfully converted {input_path} to {output_path} with chroma key transparency")
    except Exception as e:
        print(f"Error: {e}")

if __name__ == "__main__":
    if len(sys.argv) < 3:
        print("Usage: python convert.py <input> <output>")
    else:
        convert_to_transparent_png(sys.argv[1], sys.argv[2])
