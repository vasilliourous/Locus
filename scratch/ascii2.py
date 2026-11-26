import sys
from PIL import Image, ImageDraw, ImageFont

def render(text, fontpath, cap=45, track=6.5, cols=140, weight=0.0):
    probe = ImageFont.truetype(fontpath, 300)
    bb = probe.getbbox('H'); cap_px = bb[3]-bb[1]
    seg = round(300*cap/cap_px)
    f = ImageFont.truetype(fontpath, seg)
    pad = 40
    W = int(sum(f.getlength(c) for c in text) + track*len(text)) + 2*pad
    im = Image.new('L', (W, seg*3), 0)
    d = ImageDraw.Draw(im)
    x = float(pad)
    for c in text:
        d.text((x, seg*1.2), c, font=f, fill=255)
        x += f.getlength(c) + track
    bbox = im.getbbox(); im = im.crop(bbox)
    # upscale then box-downsample in two stages so stems survive
    hi = im.resize((im.width*6, im.height*6), Image.LANCZOS)
    sx = cols/hi.width
    lines = max(1, round(hi.height*sx/2.1))
    return hi.resize((cols, lines), Image.BOX)

fonts = {
 'Liberation Sans Bold':'/usr/share/fonts/liberation/LiberationSans-Bold.ttf',
 'DejaVu Sans Bold':'/usr/share/fonts/TTF/DejaVuSans-Bold.ttf',
 'Roboto Bold':'/usr/share/fonts/TTF/Roboto-Bold.ttf',
 'Roboto Medium':'/usr/share/fonts/TTF/Roboto-Medium.ttf',
 'Noto Sans Bold':'/usr/share/fonts/noto/NotoSans-Bold.ttf',
}
for n,p in fonts.items():
    im = render('CHRYSALIS', p)
    px = im.load()
    print('==', n, im.size)
    for y in range(im.height):
        print(''.join('#' if px[x,y]>40 else '.' for x in range(im.width)))
