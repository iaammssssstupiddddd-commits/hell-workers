import os
import random
import math
from PIL import Image, ImageFilter

ASSETS_TEXTURES_DIR = "/home/satotakumi/projects/hell-workers/assets/textures"

# Re-use the existing tileable noise generator
def generate_tileable_noise(size, blur_radius, seed):
    random.seed(seed)
    noise_data = [int(random.random() * 255) for _ in range(size * size)]
    img = Image.new('L', (size, size))
    img.putdata(noise_data)
    
    tiled_size = size * 3
    tiled = Image.new('L', (tiled_size, tiled_size))
    for i in range(3):
        for j in range(3):
            tiled.paste(img, (i * size, j * size))
            
    blurred = tiled.filter(ImageFilter.GaussianBlur(blur_radius))
    final_img = blurred.crop((size, size, size * 2, size * 2))
    
    extrema = final_img.getextrema()
    if extrema[0] != extrema[1]:
        final_img = final_img.point(lambda p: int((p - extrema[0]) / (extrema[1] - extrema[0]) * 255))
    return final_img

def generate_grass_macro_overlay():
    # Grass: large uneven paint strokes
    # We combine 2 different scale noises
    n1 = generate_tileable_noise(256, 12, 1001)
    n2 = generate_tileable_noise(256, 4, 1002)
    
    pixels1 = n1.load()
    pixels2 = n2.load()
    img = Image.new('L', (256, 256))
    out_pixels = img.load()
    
    for y in range(256):
        for x in range(256):
            v = int(pixels1[x, y] * 0.7 + pixels2[x, y] * 0.3)
            # Add some contrast for "paint strokes"
            v = int(max(0, min(255, (v - 128) * 1.8 + 128)))
            out_pixels[x, y] = v
            
    img.save(os.path.join(ASSETS_TEXTURES_DIR, "grass_macro_overlay.png"))

def generate_dirt_macro_overlay():
    # Dirt: rocky rough terrain, higher frequency
    n1 = generate_tileable_noise(256, 6, 2001)
    n2 = generate_tileable_noise(256, 2, 2002)
    
    pixels1 = n1.load()
    pixels2 = n2.load()
    img = Image.new('L', (256, 256))
    out_pixels = img.load()
    
    for y in range(256):
        for x in range(256):
            # multiply logic for darker cracks
            v1 = pixels1[x, y] / 255.0
            v2 = pixels2[x, y] / 255.0
            v = v1 * v2 * 255.0 * 1.5
            v = int(max(0, min(255, v)))
            out_pixels[x, y] = v
            
    img.save(os.path.join(ASSETS_TEXTURES_DIR, "dirt_macro_overlay.png"))

def generate_sand_macro_overlay():
    # Sand: wavy sand dunes, dry/wet difference
    # We can stretch a low frequency noise to look wavy
    n1 = generate_tileable_noise(256, 10, 3001)
    # Stretch horizontally
    n1_stretched = n1.resize((512, 256), Image.Resampling.BILINEAR).crop((0, 0, 256, 256))
    
    img = n1_stretched.point(lambda p: int(max(0, min(255, (p - 128) * 1.5 + 128))))
    img.save(os.path.join(ASSETS_TEXTURES_DIR, "sand_macro_overlay.png"))

def generate_river_normal_like():
    # River normal like: we take a flow noise and compute simple gradient
    noise = generate_tileable_noise(256, 8, 4001)
    noise = noise.resize((256, 512), Image.Resampling.BILINEAR).crop((0, 0, 256, 256))
    pixels = noise.load()
    
    img = Image.new('RGB', (256, 256))
    out = img.load()
    
    for y in range(256):
        for x in range(256):
            nx = (x + 1) % 256
            ny = (y + 1) % 256
            
            dx = (pixels[nx, y] - pixels[x, y]) / 255.0
            dy = (pixels[x, ny] - pixels[x, y]) / 255.0
            
            # small Z
            dz = 0.05
            length = math.sqrt(dx*dx + dy*dy + dz*dz)
            nx_n = dx / length
            ny_n = dy / length
            nz_n = dz / length
            
            # to RGB
            r = int((nx_n * 0.5 + 0.5) * 255)
            g = int((ny_n * 0.5 + 0.5) * 255)
            b = int((nz_n * 0.5 + 0.5) * 255)
            out[x, y] = (r, g, b)
            
    img.save(os.path.join(ASSETS_TEXTURES_DIR, "river_normal_like.png"))

def generate_terrain_blend_mask():
    # soft blend mask
    n = generate_tileable_noise(256, 15, 5001)
    # lower contrast
    img = n.point(lambda p: int(max(0, min(255, (p - 128) * 0.5 + 128))))
    img.save(os.path.join(ASSETS_TEXTURES_DIR, "terrain_blend_mask_soft.png"))

def generate_shoreline_detail():
    # shoreline detail mask, high frequency ripples
    n = generate_tileable_noise(256, 2, 6001)
    # stretch vertically for water ripples
    n2 = n.resize((256, 512), Image.Resampling.BILINEAR).crop((0, 0, 256, 256))
    img = n2.point(lambda p: int(max(0, min(255, (p - 128) * 2.0 + 128))))
    img.save(os.path.join(ASSETS_TEXTURES_DIR, "shoreline_detail.png"))

if __name__ == "__main__":
    generate_grass_macro_overlay()
    generate_dirt_macro_overlay()
    generate_sand_macro_overlay()
    generate_river_normal_like()
    generate_terrain_blend_mask()
    generate_shoreline_detail()
    print("Done generated priority B & C procedural assets.")
