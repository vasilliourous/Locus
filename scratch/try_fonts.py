from PIL import Image, ImageDraw, ImageFont, ImageFilter
import sys
fonts = [
 ('LiberationSans-Bold','/usr/share/fonts/liberation/LiberationSans-Bold.ttf'),
 ('DejaVuSans-Bold','/usr/share/fonts/TTF/DejaVuSans-Bold.ttf'),
 ('Roboto-Bold','/usr/share/fonts/TTF/Roboto-Bold.ttf'),
 ('NotoSans-Bold','/usr/share/fonts/noto/NotoSans-Bold.ttf'),
 ('Roboto-Medium','/usr/share/fonts/TTF/Roboto-Medium.ttf'),
]
W=1400
im=Image.new('RGB',(W,60*len(fonts)+40),(26,27,38))
d=ImageDraw.Draw(im)
for i,(n,p) in enumerate(fonts):
    d.text((30,20+i*60),'CHRYSALIS',font=ImageFont.truetype(p,40),fill=(122,162,247))
# letterspaced version of the best candidate
for i,(n,p) in enumerate(fonts):
    pass
im.save('fonts.png')
print(im.size)
