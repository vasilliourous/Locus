import sys, re
from PIL import Image, ImageDraw

def render(svgpath, S=4):
    t=open(svgpath).read()
    vb=[float(v) for v in re.search(r'viewBox="([^"]+)"',t).group(1).split()]
    img=Image.new('L',(int(vb[2]*S)+8,int(vb[3]*S)+8),0); dr=ImageDraw.Draw(img)
    for d in re.findall(r' d="([^"]+)"',t):
        pts=[]
        for m in re.finditer(r'([MLZ])|(-?\d+\.?\d*),(-?\d+\.?\d*)',d):
            if m.group(1): continue
            pts.append((float(m.group(2))*S+4,float(m.group(3))*S+4))
        dr.polygon(pts,fill=255)
    return img.crop(img.getbbox())

def show(img, cols=160):
    rows = max(1, round(img.height / (img.width / cols) / 2.05))
    out = img.resize((cols, rows), Image.LANCZOS)
    px=out.load()
    for y in range(rows):
        print(''.join('#' if px[x,y]>110 else '.' for x in range(cols)))
    print(img.size, '->', cols, 'x', rows)

if __name__=='__main__':
    show(render(sys.argv[1]), int(sys.argv[2]) if len(sys.argv)>2 else 160)
