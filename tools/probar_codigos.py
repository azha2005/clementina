"""Prueba todas las asignaciones de los 5 canales a bits y dos códigos
(Ferranti Mercury Apéndice VI e ITA2) para ver cuál da texto legible."""
import sys, itertools
FIG = [None,'1','2','*','4','(',')','7','8','≠','=','-','v','\n',' ',',','0','>','≥','3','→','5','6','/','×','9','+',None,'.','n','\r','#']
LET = [None,'A','B','C','D','E','F','G','H','I','J','K','L','M','N','O','P','Q','R','S','T','U','V','W','X','Y','Z',None,'.','?','£','#']
# ITA2 (bit1 = canal 1). Letras / cifras; 27 = FIGS, 31 = LTRS.
ITA_L = {3:'A',25:'B',14:'C',9:'D',1:'E',13:'F',26:'G',20:'H',6:'I',11:'J',15:'K',18:'L',28:'M',12:'N',24:'O',22:'P',23:'Q',10:'R',5:'S',16:'T',7:'U',30:'V',19:'W',29:'X',21:'Y',17:'Z',4:' ',8:'\r',2:'\n',0:''}
ITA_F = {3:'-',25:'?',14:':',9:'$',1:'3',13:'!',26:'&',20:'#',6:'8',11:"'",15:'(',18:')',28:'.',12:',',24:'9',22:'0',23:'1',10:'4',5:"'",16:'5',7:'7',30:';',19:'2',29:'/',21:'6',17:'"',4:' ',8:'\r',2:'\n',0:''}
def ferranti(vals, sh):
    o=[]
    for v in vals:
        if v==0: sh='F'; continue
        if v==27: sh='L'; continue
        c=(FIG if sh=='F' else LET)[v]; o.append(c or '')
    return ''.join(o)
def ita(vals, sh):
    o=[]
    for v in vals:
        if v==27: sh='F'; continue
        if v==31: sh='L'; continue
        o.append((ITA_F if sh=='F' else ITA_L).get(v,'?'))
    return ''.join(o)
def score(t):
    if not t: return 0
    good=sum(ch.isalpha() or ch==' ' or ch.isdigit() for ch in t)
    vow=sum(ch in 'AEIOUaeiou' for ch in t)
    return good/len(t) + 0.5*min(vow/max(1,sum(ch.isalpha() for ch in t)),0.45)
vals=[int(x) for x in sys.argv[1:]]
# posiciones: p0=canal16(lado 3, exterior) ... p4=canal1(lado 2, exterior)
pos=[[(v>>(4-k))&1 for k in range(5)] for v in vals]
res=[]
for perm in itertools.permutations(range(5)):
    w=[1<<(4-perm[k]) for k in range(5)]
    vv=[sum(b*w[k] for k,b in enumerate(p)) for p in pos]
    for dname,seq in (('dir',vv),('inv',vv[::-1])):
        for code,fn in (('FER',ferranti),('ITA',ita)):
            for sh in 'FL':
                t=fn(seq,sh).replace('\r','').replace('\n','/')
                res.append((score(t),perm,dname,code,sh,t))
res.sort(key=lambda r:-r[0])
seen=set()
for r in res[:25]:
    if r[5] in seen: continue
    seen.add(r[5]); print(f'{r[0]:.2f} {r[1]} {r[2]} {r[3]} {r[4]} | {r[5]}')
