from PIL import Image, ImageDraw, ImageFont
rows = [
 ('Liberation Sans Bold','/usr/share/fonts/liberation/LiberationSans-Bold.ttf'),
 ('DejaVu Sans Bold','/usr/share/fonts/TTF/DejaVuSans-Bold.ttf'),
 ('Roboto Bold','/usr/share/fonts/TTF/Roboto-Bold.ttf'),
 ('Roboto Medium','/usr/share/fonts/TTF/Roboto-Medium.ttf'),
]
W, RH = 760, 64
im = Image.new('RGB',(W, RH*len(rows)),(26,27,38))
d = ImageDraw.Draw(im)
for i,(n,p) in enumerate(rows):
    # letter-spaced like the original's tracking
    x = 20.0
    f = ImageFont.truetype(p, 45)
    for ch in 'CHRYSALIS':
        d.text((x, 8+i*RH), ch, font=f, fill=(122,162,247))
        x += d.textlength(ch, font=f) + 6.5
im.save('fonts.png')
