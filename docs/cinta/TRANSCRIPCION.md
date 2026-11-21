# Cintas perforadas del Museo de Informática (CABA)

Dos trozos de cinta de papel de 5 canales, traídos del museo antes de su
cierre. Fotos en `cinta1/` y `cinta2/`. Se leyeron con
[`tools/leer_cinta.py`](../../tools/leer_cinta.py), que detecta los agujeros, y
se revisaron a mano sobre las imágenes de control.

## Formato

3 canales de un lado del agujero de arrastre y 2 del otro. Valor de cada fila:
16, 8 y 4 del borde del lado de 3 hacia el arrastre; 2 y 1 del arrastre hacia
el borde del lado de 2.

El texto **no** está en el código Ferranti de la Mercury (CS 225 A, Apéndice VI),
que con estos valores da basura, sino en **ITA2 (Baudot-Murray)**, el código
de los teletipos: 8 = CR, 2 = LF, 31 = letras, 27 = cifras. Leída así, la cinta
es texto en castellano y no hace falta ninguna corrección de orden de canales.

## Cinta 2 (completa)

Valores en orden de lectura (49 filas):

```
8 2 31 14 3 18 14 7 18 3 12 9 24 4 22 10 6 28 24 5 27 14 8 2
19 12 4 1 12 [4] 16 12 4 7 12 4 8 2
3 4 31 13 6 12 4 27 3 8 2
```

```
CALCULANDO PRIMOS:
2, 3, 5, 7,
- FIN -
```

La fila `[4]` (espacio) está superpuesta con la siguiente: el perforador
avanzó menos, los dos agujeros del canal 4 se juntaron en uno doble y el
arrastre también quedó doble. La lectura automática ve `3,5`; con el espacio
repuesto queda `3, 5`, igual que el resto de la línea.

## Cinta 1 (cortada)

```
18 8 2 14 3 18 14 7 18 3 12 9 24 4 22 10 6 28 24 5 27 14 8 2
19 12 4 1 12 4 8 2 3 4 31
```

```
(L|) 
CALCULANDO PRIMOS:
2, 3,
- 
```

Es otra corrida del mismo programa, con un límite menor (primos menores que 5),
cortada después del `- ` y del cambio a letras. El primer valor (18, `L` o `)`)
es el resto de algo anterior, y a diferencia de la cinta 2 no hay cambio a
letras antes de `CALCULANDO`. Antes del texto hay 12 filas de guía (sólo
arrastre).

## Procedencia

Que el texto esté en ITA2 y no en el código Ferranti indica que estas cintas
**no salieron de la perforadora estándar de la Mercury**: probablemente las
perforó un teletipo de 5 canales, quizás como demostración en el museo. No hay
forma de saberlo con certeza a partir de la cinta. El programa
[`examples/primos.mer`](../../examples/primos.mer) muestra que Clementina pudo
perforar exactamente estos agujeros: la instrucción 62 perfora los 5 bits que
se le indiquen, sin importar el código.
