import sys
from PIL import Image, ImageDraw, ImageFont

def ascii_word(text, fontpath, cap=45, track=6.5, cols=150, ink_thresh=None):
    # Render at 1px per unit, then downsample to `cols` wide with max-pooling so
    # thin strokes never vanish (which is what makes a small mark unreadable).
    probe = ImageFont.truetype(fontpath, 200)
    bb = probe.getbbox('H'); cap_px = bb[3]-bb[1]
    size = round(200*cap/cap_px)
    f = ImageFont.truetype(fontpath, size)
    W = int(sum(f.getlength(c) for c in text) + track*len(text) + 40)
    im = Image.new('L', (W, size*2), 0)
    d = ImageDraw.Draw(im)
    x = 20.0
    for c in text:
        d.text((x, size), c, font=f, fill=255)
        x += f.getlength(c) + track
    bb = im.getbbox(); im = im.crop(bb)
    sx = cols/im.width
    out_h = max(1, round(im.height*sx*0.5))  # chars are ~2x tall
    sm = im.resize((cols, out_h), Image.BOX)
    px = sm.load()
    for y in range(out_h):
        print(''.join('#' if px[x,y]>45 else '.' for x in range(cols)))

if __name__ == '__main__':
    ascii_word(sys.argv[1], sys.argv[2], track=float(sys.argv[3]) if len(sys.argv)>3 else 6.5)
