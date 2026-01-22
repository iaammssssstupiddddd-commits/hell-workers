from PIL import Image, ImageDraw

def create_bubble():
    # 64x32 の画像を作成
    width, height = 64, 32
    # マゼンタ背景
    img = Image.new("RGB", (width, height), color=(255, 0, 255))
    draw = ImageDraw.Draw(img)
    
    # 吹き出しの本体（丸みを帯びた矩形風）
    # (x0, y0, x1, y1)
    body_shape = [4, 4, 60, 24]
    # 白い背景
    draw.rectangle(body_shape, fill=(255, 255, 255), outline=(0, 0, 0), width=1)
    
    # 吹き出しの尻尾（三角形）
    tail_shape = [(10, 24), (16, 24), (8, 30)]
    draw.polygon(tail_shape, fill=(255, 255, 255), outline=(0, 0, 0))
    # 尻尾の付け根の線を消す
    draw.line([(11, 24), (15, 24)], fill=(255, 255, 255), width=1)

    img.save("temp_speech_bubble.png")
    print("Generated temp_speech_bubble.png")

if __name__ == "__main__":
    create_bubble()
