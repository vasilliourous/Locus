import sys
from PIL import Image
def show(path, cols=140, thresh=30):
    im=Image.open(path).convert('L')
    b=im.point(lambda v:255 if v>thresh else 0).getbbox()
    if not b: print('EMPTY'); return
    c=im.crop(b)
    hi=c.resize((c.width*3,c.height*3),Image.LANCZOS)
    out=hi.resize((cols,max(1,round(hi.height*cols/hi.width/2.1))),Image.BOX)
    px=out.load()
    for y in range(out.height): print(''.join('#' if px[x,y]>thresh else '.' for x in range(out.width)))
if __name__=='__main__':
    show(sys.argv[1], int(sys.argv[2]) if len(sys.argv)>2 else 140, int(sys.argv[3]) if len(sys.argv)>3 else 30)
