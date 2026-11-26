import sys, re
from PIL import Image, ImageDraw

def build(svgpath, S=3):
    t = open(svgpath).read()
    vb = [float(v) for v in re.search(r'viewBox="([^"]+)"', t).group(1).split()]
    W, H = int(vb[2]*S)+8, int(vb[3]*S)+8
    img = Image.new('L', (W, H), 0)
    dr = ImageDraw.Draw(img)
    for d in re.findall(r' d="([^"]+)"', t):
        pts = []
        for m in re.finditer(r'([MLZ])|(-?\d+\.?\d*),(-?\d+\.?\d*)', d):
            if m.group(1):
                if m.group(1) == 'M':
                    pts = []
                continue
            pts.append(((float(m.group(2)))*S+4, (float(m.group(3)))*S+4))
        dr.polygon(pts, fill=255)
    return img

if __name__ == '__main__':
    img = build(sys.argv[1], int(sys.argv[2]) if len(sys.argv) > 2 else 3)
    out = img.resize((150, max(1, round(img.height*150/img.width/1.15))), Image.BOX)
    px = out.load()
    for y in range(out.height):
        print(''.join('#' if px[x, y] > 60 else '.' for x in range(out.width)))
