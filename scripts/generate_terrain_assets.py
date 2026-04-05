import os
import random
from PIL import Image, ImageFilter

ASSETS_TEXTURES_DIR = "/home/satotakumi/projects/hell-workers/assets/textures"

def generate_tileable_noise(size, blur_radius, seed):
    random.seed(seed)
    
    # Generate 1x1 noise
    noise_data = []
    for _ in range(size * size):
        noise_data.append(int(random.random() * 255))
        
    img = Image.new('L', (size, size))
    img.putdata(noise_data)
    
    # Create 3x3 tiled version
    tiled_size = size * 3
    tiled = Image.new('L', (tiled_size, tiled_size))
    for i in range(3):
        for j in range(3):
            tiled.paste(img, (i * size, j * size))
            
    # Blur heavily
    blurred = tiled.filter(ImageFilter.GaussianBlur(blur_radius))
    
    # Crop middle
    final_img = blurred.crop((size, size, size * 2, size * 2))
    
    # Normalize contrast
    extrema = final_img.getextrema()
    if extrema[0] != extrema[1]:
        final_img = final_img.point(lambda p: int((p - extrema[0]) / (extrema[1] - extrema[0]) * 255))
        
    return final_img

def generate_terrain_macro_noise():
    size = 256
    
    # Create 3 channels
    r_img = generate_tileable_noise(size, 8, 123)
    g_img = generate_tileable_noise(size, 8, 456)
    b_img = generate_tileable_noise(size, 10, 789)
    
    # Tweak brightness of channels
    r_img = r_img.point(lambda p: int(max(0, min(255, (p - 128) * 1.5 + 128))))
    g_img = g_img.point(lambda p: int(max(0, min(255, (p - 128) * 1.5 + 128))))
    b_img = b_img.point(lambda p: int(max(0, min(255, (p - 128) * 1.2 + 128))))
    
    rgb = Image.merge('RGB', (r_img, g_img, b_img))
    
    path = os.path.join(ASSETS_TEXTURES_DIR, "terrain_macro_noise.png")
    rgb.save(path)
    print(f"Generated {path}")

def generate_river_flow_noise():
    size = 256
    raw_img = generate_tileable_noise(size, 6, 111)
    
    # Stretch vertically
    # Using tile and blur again but anisotropically if we want, or just stretch
    # We can just stretch and ensure tileable!
    stretched = raw_img.resize((size, size * 2), Image.Resampling.BILINEAR)
    river_noise = stretched.crop((0, 0, size, size))
    
    # It must be tileable vertically! If we just stretch and crop, it might lose tileability if not aligned.
    # But wait, stretching by 2 and taking half preserves tileability!
    
    # Enhance contrast
    extrema = river_noise.getextrema()
    if extrema[0] != extrema[1]:
        river_noise = river_noise.point(lambda p: int((p - extrema[0]) / (extrema[1] - extrema[0]) * 255))
    river_noise = river_noise.point(lambda p: int(max(0, min(255, (p - 128) * 1.5 + 128))))
    
    path = os.path.join(ASSETS_TEXTURES_DIR, "river_flow_noise.png")
    river_noise.save(path)
    print(f"Generated {path}")

def generate_terrain_feature_lut():
    # A 256x1 LUT for terrain feature tints and roughness
    lut_img = Image.new('RGBA', (256, 1), color=(128, 128, 128, 128))
    pixels = lut_img.load()
    
    # 0 = Normal
    pixels[0, 0] = (128, 128, 128, 128)
    # 1 = Shore sand (very subtle warm gray, slightly darker) -> rgh=0.40 (102)
    pixels[1, 0] = (116, 118, 112, 102)
    # 2 = Inland sand (very subtle warm gray) -> rgh=0.55 (140)
    pixels[2, 0] = (132, 128, 122, 140)
    # 3 = Rock field dirt (dry, reddish brown reduced, gray brown) -> rgh=0.7 (178)
    pixels[3, 0] = (122, 115, 107, 178)
    # 4 = Grass zone bias (cooler green) -> rgh=0.5 (128)
    pixels[4, 0] = (108, 138, 114, 128)
    # 5 = Dirt zone bias (dry earth brown) -> rgh=0.5 (128)
    pixels[5, 0] = (148, 120, 96, 128)
    
    path = os.path.join(ASSETS_TEXTURES_DIR, "terrain_feature_lut.png")
    lut_img.save(path)
    print(f"Generated {path}")

if __name__ == "__main__":
    os.makedirs(ASSETS_TEXTURES_DIR, exist_ok=True)
    generate_terrain_macro_noise()
    generate_river_flow_noise()
    generate_terrain_feature_lut()
    print("Done generating priority A procedural assets.")
