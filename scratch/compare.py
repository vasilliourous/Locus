from PIL import Image
def rows(path, cols=150, thresh=40):
    im=Image.open(path).convert('L'); m=im.point(lambda v:255 if v>thresh else 0)
    b=m.getbbox(); c=m.crop(b)
    out=c.resize((cols, max(1, round(c.height*cols/c.width/1.15))), Image.BOX)
    px=out.load()
    return [''.join('#' if px[x,y]>60 else '.' for x in range(cols)) for y in range(out.height)]
a=rows('locus.ppm')      # the shipped Locus wordmark, rasterised
b=rows('final.ppm')      # the new Chrysalis wordmark, rasterised
print('ORIGINAL (LIVE BAND, cap 45 units)')
for l in a: print(l)
print()
print('NEW')
for l in b: print(l)
