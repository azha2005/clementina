"""Decodifica una secuencia de valores de cinta (0-31) con el código del
Apéndice VI de CS 225 A, en ambos sentidos y empezando en cada registro."""
import sys
FIG = [None,'1','2','*','4','(',')','7','8','≠','=','-','v','<LF>',' ',',','0','>','≥','3','→','5','6','/','×','9','+',None,'.','n','<CR>','<ER>']
LET = [None,'A','B','C','D','E','F','G','H','I','J','K','L','M','N','O','P','Q','R','S','T','U','V','W','X','Y','Z',None,'.','?','£','<ER>']
def dec(vals, start):
    sh = start; out = []
    for v in vals:
        if v == 0: sh = 'F'; out.append('[F]'); continue
        if v == 27: sh = 'L'; out.append('[L]'); continue
        out.append((FIG if sh == 'F' else LET)[v])
    return ''.join(out)
vals = [int(x) for x in sys.argv[1:]]
for name, seq in (('directo', vals), ('invertido', vals[::-1])):
    for s in 'FL':
        print(f'{name:9} desde {"cifras" if s=="F" else "letras"}: {dec(seq, s)}')
