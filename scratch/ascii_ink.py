import sys
from PIL import Image
# Ink is any pixel brighter than the ground; the ground is the darkest pixel.
def show(path, cols=150):
    im=Image.open(path).convert('L')
    lo=min(im.getdata())
    mask=im.point(lambda v: 255 if v>lo+3 else 0)
    b=mask.getbbox()
    if not b: print('EMPTY'); return
    c=mask.crop(b)
    hi=c.resize((c.width*2,c.height*2),Image.LANCZOS)
    out=hi.resize((cols,max(1,round(hi.height*cols/hi.width/2.1))),Image.BOX)
    px=out.load()
    for y in range(out.height): print(''.join('#' if px[x,y]>60 else '.' for x in range(out.width)))
if __name__=='__main__': show(sys.argv[1], int(sys.argv[2]) if len(sys.argv)>2 else 150)
