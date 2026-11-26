from PIL import Image, ImageDraw, ImageFont
import glob
fonts = sorted(glob.glob('/usr/share/fonts/**/*Bold.ttf', recursive=True))
fonts += sorted(glob.glob('/usr/share/fonts/**/*SemiBold.ttf', recursive=True))
fonts += sorted(glob.glob('/usr/share/fonts/**/*Black.ttf', recursive=True))
for m in fonts:
    try:
        im=Image.new('L',(80,80),0); d=ImageDraw.Draw(im)
        d.text((4,4),'H',font=ImageFont.truetype(m,60),fill=255)
        bb = im.point(lambda v:255 if v>60 else 0).getbbox()
        print(round((bb[3]-bb[1])/60,3), round((bb[2]-bb[0])/60,3), m)
    except Exception as e:
        print('ERR',m,e)
