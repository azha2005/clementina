"""Decodifica valores de cinta con el código ITA2 (Baudot-Murray) de los
teletipos. Bit 1 = canal 1 (el del borde del lado de 2 agujeros)."""
import sys
L = {0:'',1:'E',2:'\n',3:'A',4:' ',5:'S',6:'I',7:'U',8:'',9:'D',10:'R',11:'J',12:'N',13:'F',14:'C',15:'K',
     16:'T',17:'Z',18:'L',19:'W',20:'H',21:'Y',22:'P',23:'Q',24:'O',25:'B',26:'G',28:'M',29:'X',30:'V'}
F = {0:'',1:'3',2:'\n',3:'-',4:' ',5:"'",6:'8',7:'7',8:'',9:'¿',10:'4',11:'¡',12:',',13:'!',14:':',15:'(',
     16:'5',17:'+',18:')',19:'2',20:'#',21:'6',22:'0',23:'1',24:'9',25:'?',26:'&',28:'.',29:'/',30:';'}
def dec(vals, sh='L'):
    out = []
    for v in vals:
        if v == 27: sh = 'F'; continue
        if v == 31: sh = 'L'; continue
        out.append((L if sh == 'L' else F)[v])
    return ''.join(out)
vals = [int(x) for x in sys.argv[1:]]
print('directo  :', repr(dec(vals)))
print('invertido:', repr(dec(vals[::-1])))
