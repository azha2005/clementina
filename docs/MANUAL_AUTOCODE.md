# Manualcito de Mercury Autocode para Clementina

Guía práctica para escribir programas en **Mercury Autocode** y correrlos en el emulador de **Clementina** (tanto en la consola web como con `clementina autocode`). Resume el lenguaje de los manuales históricos —el *Mercury Autocode Manual* (R. A. Brooker, 1961) y el *AUTOCODE* del Instituto de Cálculo (E. García Camarero, UBA, 1961)— junto con las convenciones y extensiones que acepta este compilador (`crates/mercury-autocode`).

> [!NOTE]
> Este manualcito cubre lo que efectivamente traduce y ejecuta el emulador hoy. Donde este compilador difiere del original (por ejemplo, rutinas enlazadas dentro de cada capítulo o límites de algunas funciones), se aclara explícitamente.

---

## 1. Primer programa y dónde correrlo

Un programa mínimo de Autocode se ve así:

```autocode
a = 2 + 3
print (a) 2, 0
newline
end
```

### En la consola web

1. En el panel derecho (**Programa propio**), elegí **Mercury Autocode** en *Formato del programa*.
2. Escribí el código (o abrí un `.mer` / `.txt`).
3. Si el programa usa `read`, poné los datos debajo de una línea con `->` (o cargalos con **Poner cinta…**).
4. En **Modo fácil**, apretá **Correr en Clementina** (o **Acelerar carga · arrancar a ×1**). En **Como en 1961**, usá **Compilar y poner cinta en el lector**, subí la llave **INPUT 2**, activá **WRITE CURRENT**, pasá a **AUTO** y **CONTINUOUS**, pulsá **INITIAL TRANSFER** y, al detenerse la carga, **PREPULSE**.

### En la terminal

```powershell
.\target\release\clementina autocode programa.mer
```

Opciones útiles:

* `--cinta datos.txt`: toma los datos de entrada desde un archivo aparte (si no, lee lo que venga después de `->` dentro del mismo `.mer`).
* `--listado`: muestra el código Mercury generado, las direcciones y los registros libres de cada capítulo sin ejecutar el programa.
* `--max N`: cambia el tope de instrucciones (por omisión 50 000 000).
* `--interactivo`, `--traza`, `--clementina-1966`: opciones del emulador de la máquina.

---

## 2. Reglas de escritura y equivalencias ASCII

* **Una sentencia por línea.** Los espacios y tabulaciones se ignoran al analizar (salvo en el texto que sigue a `title`/`caption`), así que `print(a)2,0` y `print (a) 2, 0` son lo mismo.
* **Comentarios:** todo lo que sigue a `//` en una línea se ignora (convención de los programas de Edimburgo).
* **Mayúsculas y minúsculas:** las variables e instrucciones van en **minúscula**, pero `F` y `X` mayúsculas representan el símbolo de función `φ`.

Como los teclados actuales no tienen los símbolos originales de la cinta Ferranti de la Mercury, el compilador acepta equivalentes ASCII:

| Símbolo del manual | Equivalentes aceptados | Uso |
|---|---|---|
| `φ` | `F`, `X`, `Æ` | Prefijo de funciones (`φ sin(x)`, `F cos(x)`) y operaciones matriciales (`φ10(...)`) |
| `→` | `->`, `®` | Reserva de vectores (`x -> 10`) y línea separadora de datos (`->`) |
| `»` | `~`, `≈` | Asignación **sin redondeo** (`a ~ b + c`) |
| `≠` | `#`, `!=` | Distinto en saltos condicionales (`jump 5, i != 0`) |
| `≥` | `>=` | Mayor o igual en saltos (`jump 3, x >= 0`) |
| `π` | `$` | Constante $\pi$ (también es un nombre de variable especial válido) |

---

## 3. Variables e índices

Autocode distingue dos tipos de datos completamente distintos: **variables** (punto flotante) e **índices** (enteros chicos).

### Variables de trabajo (punto flotante)

Las letras válidas para variables son **15**:

```
a  b  c  d  e  f  g  h  u  v  w  x  y  z  π
```

(Recordá que podés escribir `$` en lugar de `π`.) Con ellas se arman tres clases de variables:

1. **Especiales:** `a`, `b`, …, `z`, `π` (15 variables escalares disponibles sin declarar nada).
2. **Especiales primadas:** `a'`, `b'`, …, `z'` (14 variables escalares extra; **no existe** `π'`).
3. **Principales (vectores / grupos):** se declaran al comienzo del capítulo con `letra -> n`:

```autocode
x -> 10
```

Esto reserva **n + 1** variables consecutivas: `x0, x1, ..., x10`. El total de variables principales en memoria rápida es de **480 largos** entre todos los grupos del capítulo.

Los elementos de un grupo se indexan con:
* sufijo numérico fijo: `x0`, `x3` o `x(3)`;
* un índice: `xi` o `x(i)`;
* un índice con desplazamiento constante: `x(i+1)`, `x(i-50)`.

### Índices (enteros de 10 bits)

Las letras reservadas para índices son **12**:

```
i  j  k  l  m  n  o  p  q  r  s  t
```

Sirven como contadores de ciclo, subíndices de vectores y direcciones de salto calculado.

> [!WARNING]
> **Rango de −512 a 511.** En la Mercury cada índice es un registro corto de 10 bits con signo. Si una cuenta pasa de `511` o baja de `−512`, **da la vuelta módulo 1024** sin avisar (es exactamente lo que pasa con `fib2.mer` a partir del término 14). Para enteros grandes usá variables comunes (`a..z`).

---

## 4. Expresiones, redondeo y asignaciones

### Expresiones aritméticas (variables)

Una expresión es una suma/resta de términos: `± término ± término ...`.

* **Multiplicación implícita:** en expresiones escalares **no se escribe `*`**; los factores van pegados: `2mn x`, `4fs`, `0.5 ab`.
* **Una sola división por término, al final:** un término puede tener `/ factor` al final, con **un único factor** como divisor (`x/4`, `3/x`, `hf/3`). Si necesitás dividir por un producto, como $a / (bc)$, guardá antes `d = bc` y escribí `a/d` (o usá `φ divide(a, bc)`).
* **Sin paréntesis algebraicos generales:** los paréntesis sólo se usan en sufijos `a(m+1)`, argumentos de funciones `φ sin(x)` y pares complejos/dobles `(a, b)`.
* **Ojo con los sufijos pegados:** después de una letra de variable principal, el **primer** índice o número pegado es su sufijo. Por ejemplo:
  * `amn` significa $a_m \cdot n$,
  * `man` significa $m \cdot a_n$,
  * `a(m+1)` significa $a_{m+1}$.

### Redondeo: `=` contra `»` (`~`)

La Ferranti Mercury tenía dos juegos de operaciones aritméticas:

* **`y = expr`** (con redondeo): fuerza un `1` en el bit menos significativo **antes** de estandarizar el resultado. Como consecuencia, ni `a = 7 + 1` da `8` exacto bit a bit (aunque sí dentro de la precisión de ~29 bits, unas 8–9 cifras decimales), y una resta con cancelación (`m·h - 1`) agranda ese bit de error.
* **`y » expr`** o **`y ~ expr`** (sin redondeo): usa las órdenes sin redondeo. Conviene usar `~` cuando operás con enteros exactos, cuando asignás constantes/direcciones enteras (por ejemplo `a' ~ 0`) o cuando hay restas casi iguales.

### Expresiones de índices

Un índice se asigna con una expresión puramente entera (sin variables flotantes, puntos decimales ni `/`):

```autocode
i = 2mn + m + n + 1
j = 10i - 7
k = -j + 5
```

También podés pasar de punto flotante a índice con parte entera (piso) o buscar extremos en un grupo:

```autocode
i = φ int pt(x)        // piso de x (para el entero más cercano: φ int pt(x + 0.5))
j = φ max(x0, 0, n)    // sufijo del mayor entre x0 y xn
k = φ min(x0, i, 10)   // sufijo del menor entre xi y x10
```

### Impresión rápida `?`

Poniendo `?` al principio o al final de una asignación (`? x = 2a + 1` o `x = 2a + 1 ?`, también en asignaciones a índices), el programa salta a una línea nueva e imprime el valor recién calculado (en flotante con 10 decimales para variables, o entero para índices). No se aplica a ciclos ni a operaciones matriciales.

---

## 5. Funciones incorporadas (`φ`, `F`, `X`)

| Sintaxis | Qué calcula | Notas y límites en este emulador |
|---|---|---|
| `φ sqrt(x)` | $\sqrt{\|x\|}$ | Si $x < 0$, toma la raíz de $\|x\|$ |
| `φ sin(x)` | $\sin(x)$ ($x$ en radianes) | Requiere $\|x\| < 800$ |
| `φ cos(x)` | $\cos(x)$ ($x$ en radianes) | Requiere $\|x\| < 800$ |
| `φ tan(x)` | $\tan(x)$ ($x$ en radianes) | Requiere $\|x\| < 800$ |
| `φ exp(x)` | $e^x$ | Desborda y detiene la máquina si $x > 177$ |
| `φ log(x)` | $\ln(x)$ | Desborda y detiene la máquina si $x \le 0$ |
| `φ mod(x)` | $\|x\|$ (valor absoluto) | — |
| `φ int pt(x)` | Parte entera (piso $\lfloor x \rfloor$) | Vale también en `i = φ int pt(x)` |
| `φ fr pt(x)` | Parte fraccionaria ($x - \lfloor x \rfloor$) | Siempre en $[0, 1)$ |
| `φ sign(x)` | $+1$ si $x \ge 0$, $-1$ si $x < 0$ | — |
| `φ divide(a, b)` | $a / b$ | Útil cuando el divisor es una expresión |
| `φ arctan(x, y)` | Ángulo de $(x, y)$ en radianes (`atan2(y, x)`) | Devuelve $0$ en $(0, 0)$ |
| `φ radius(x, y)` | $\sqrt{x^2 + y^2}$ | — |
| `φ parity(e)` | $+1$ si la expresión entera `e` es par, $-1$ si es impar | — |
| `φ poly(x) a0, n` | Polinomio $a_0 + a_1 x + \dots + a_n x^n$ | `a0` es el primer elemento del grupo; `n` es entero o índice |

Las funciones trascendentes corren íntegramente en código Mercury con un error relativo máximo del orden de $10^{-8}$ (unas 8 cifras significativas).

---

## 6. Etiquetas, saltos, ciclos y fin

### Etiquetas

Cualquier línea puede llevar una etiqueta numérica de `1)` a `127)` al comienzo (sola o antes de una sentencia):

```autocode
 1) x = x + 1
    jump 1, 10 > x
```

### Saltos condicionales e incondicionales

* `jump N`: salta a la etiqueta `N`.
* `jump N, a rel b`: salta a `N` si se cumple la comparación. Las únicas relaciones admitidas son:
  * `=` (igual)
  * `≠`, `#` o `!=` (distinto)
  * `>` (mayor estricto)
  * `≥` o `>=` (mayor o igual)

Para escribir `<` o `≤`, invertí los lados: en lugar de `x < 10`, escribí `10 > x`; en lugar de `x ≤ 10`, escribí `10 ≥ x`. Además:
* un signo `-` sólo puede ir delante de una constante (`x >= -1` vale, `-x >= 1` no);
* no se puede comparar una variable flotante con un índice.

### Saltos calculados (marcas)

```autocode
n) = 3)      // guarda la dirección de la etiqueta 3 en el índice n
n) = m)      // guarda la dirección de la etiqueta cuyo número está en m
jump (n)     // salta a la etiqueta guardada en n
```

### Ciclos (`repeat`)

```autocode
i = 1(1)10
    ...
repeat
```

* La cabecera es `indice = inicio(paso)fin` (o `inicio(-paso)fin` para contar hacia atrás, por ejemplo `r = 9(-1)0`).
* Los tres valores pueden ser enteros o índices (`i = 1(1)n`).
* Admite hasta **8 ciclos anidados**.

> [!IMPORTANT]
> **El ciclo siempre entra al menos una vez** y la condición de corte en `repeat` es `indice ≠ fin` (después de sumar el paso). Por lo tanto, `(fin - inicio)` tiene que ser múltiplo exacto del paso; si te pasás de `fin`, el índice sigue de largo hasta dar la vuelta en 10 bits.

### `check` y fin de programa

* `check(x, y, e, N)`: salta a la etiqueta `N` si $|x - y| < e$; si no, sigue en la línea siguiente.
* `end` o `halt`: detienen la máquina (`990 0`).

---

## 7. Entrada, salida y parlante

### Lectura desde cinta de datos (`read`)

```autocode
read (x)    // lee un número en punto flotante
read (n)    // lee un entero (piso) en el índice n
```

En la cinta de datos los números van separados por espacios o saltos de línea, en formato decimal con signo `-` opcional, punto decimal opcional y exponente decimal opcional después de una coma (por ejemplo `-2.5,-3` equivale a $-0{,}0025$).

### Impresión en el teleimpresor (`print`, `newline`, `space`)

```autocode
print (x) m, n
print (x, m, n)   // variante de Edimburgo, también aceptada
```

* Si `m > 0` y $|x| < 2^{28}$: imprime en **punto fijo**, reservando `m` lugares enteros (los ceros a la izquierda salen como espacios; si el número es más ancho, se imprimen todas sus cifras) y `n` decimales.
* Si `m = 0` o $|x| \ge 2^{28}$: imprime en **punto flotante** (`mantisa, exponente`) con `n` decimales.
* Toda llamada a `print` agrega **dos espacios** al final.
* `newline` perfora un salto de línea (CR LF) y `space` perfora un espacio.
* `hoot` hace sonar el parlante de la consola alrededor de 1 segundo (~1000 Hz).

### Poner datos en el mismo archivo

Si no cargás una cinta aparte, podés poner los datos debajo del programa separados por una línea que contenga solo `->` (o `→`):

```autocode
read (n)
i = 1(1)n
    read (x)
    print (x) 3, 2
    newline
repeat
end
->
3
1.5  -2.25  4.0
```

---

## 8. Capítulos y programas grandes

Un programa chico no necesita `chapter`/`close` (el compilador lo mete todo en el capítulo 0). Cuando un capítulo supera el espacio de instrucciones/constantes de las páginas 2 a 14 (832 medios registros compartidos con las rutinas de impresión, lectura y funciones que use ese capítulo), se divide en capítulos numerados de `0` a `21`:

```autocode
chapter 1
x -> 20
1) ...
up
close

chapter 0
variables 1        // hereda las reservas x -> 20 del capítulo 1
...
down 1/1
end
close
```

Reglas de capítulos:
* El **último capítulo** del archivo tiene que ser el `chapter 0`, que es donde arranca la ejecución.
* `variables k` toma las reservas (`x -> n`) de un capítulo `k` definido **antes** en el archivo (`variables 0` toma las del capítulo 0).
* `across m/c`: salta a la etiqueta `m` del capítulo `c` sin retorno.
* `down m/c` y `up`: llama a la etiqueta `m` del capítulo `c` y vuelve con `up` a la instrucción siguiente al `down`.
* `preserve` / `restore`: guardan y restauran las variables especiales e índices en el tambor secreto al hacer llamadas anidadas entre capítulos.
* `title` (o `caption`): imprime la línea siguiente durante la traducción (no en tiempo de ejecución).
* `psa`: informa durante la traducción cuántos registros libres quedan en el capítulo hasta ese punto.
* `programme -N`, `across m/c - N`, `down m/c - N` y `rmp`: permiten empaquetar subprogramas numerados y pasar al siguiente `chapter 0` conservando el almacén numérico.
* Las declaraciones de *quickies* (`φ sqrt`, `φ cos`, etc. antes de `close`) se aceptan por compatibilidad; en este compilador todas las rutinas usadas en un capítulo se enlazan directamente dentro de él.

---

## 9. Facilidades avanzadas (capítulos 2 a 4 del manual)

### Memoria auxiliar en el tambor (`φ6` y `φ7`)

Permite mover bloques entre variables de trabajo y el tambor auxiliar:

```autocode
φ7 (dir) x0, n    // escribe n variables desde x0.. hacia la dirección auxiliar dir
φ6 (dir) x0, n    // lee n variables desde dir hacia x0..
```

* Si el capítulo físico más alto del programa es `N`, las direcciones válidas van de `0` a `10751 - 512·N` (más la zona negativa `-3072` a `-1`, que corresponde al depósito secreto).
* Una dirección fuera de rango detiene la máquina en un bucle.

### Pares complejos

Las operaciones sobre pares de variables o constantes trabajan componente a componente como números complejos:

```autocode
(c, d) = (a, b) + (1, -2)
(e, f) = (c, d) * (2, -1)
(g, h) = (e, f) / (2, -1)
(u, v) = φ sqrt(-3, -4)
(w, x) = φ exp(0, 1)
(y, z) = φ log(-3, 4)
```

Se admiten `+`, `-`, `*`, `/`, copia `(c, d) = (a, b)`, y las funciones `φ sqrt`, `φ exp` y `φ log`. Los destinos pueden coincidir con los operandos (por ejemplo `(b, a) = (a, b)`).

### Doble longitud (`double length`)

Con la directiva `double length` al inicio del capítulo (después de las reservas de variables y antes de las instrucciones), se habilitan pares con doble paréntesis de ~16 cifras decimales:

```autocode
double length
((a, b)) = 1 / ((3, 0))
((c, d)) = ((a, b)) * ((3, 0))
((e, f)) = ((c, d)) - ((1, 0))
end
```

Admite `+`, `-`, `*`, `/`, copia y `1 / ((x, y))`.

### Ecuaciones diferenciales (`int step`)

Integra un sistema de $n$ ecuaciones diferenciales de primer orden $y'_k = f_k(x, y_1, \dots, y_n)$ un paso `h` con el método de Runge-Kutta de 4.º orden:

```autocode
f -> 2
g -> 2
h -> 2
y -> 2
n = 2
h = 0.02
x = 0
y1 = 0
y2 = 1
11) int step (10)
    jump 11, 0.19 > x
end
10) f1 = ...          // subsecuencia que calcula f1..fn con x e y1..yn
    f2 = ...
    592, 0            // retorno obligatorio de la subsecuencia
```

### Operaciones matriciales (`φ8` a `φ28`)

Las matrices viven en la memoria auxiliar del tambor, guardadas por filas consecutivas a partir de una dirección base (por ejemplo `a' ~ 0`): el elemento $(i, j)$ de una matriz de $u \times v$ está en `a' + i·v + j` (con $0 \le i < u$, $0 \le j < v$).

| Operación | Qué hace |
|---|---|
| `φ8(a', u, v, m, n)` | Imprime matriz `u × v` en punto fijo (`m, n`), una fila por bloque |
| `φ9(a', u, v, n)` | Imprime matriz `u × v` en punto flotante (`n` decimales) |
| `φ10(a', u)` | Lee `u` elementos desde la cinta hacia `a'` |
| `a' = φ11(b', c', u)` | $A = B + C$ (vectores/matrices de $u$ elementos) |
| `a' = φ12(b', c', u)` | $A = B - C$ |
| `a' = φ13(b', x, c', u)` | $A = B + xC$ |
| `a' = φ14(b', x, c', u)` | $A = B - xC$ |
| `a' = φ15(b', u)` | $A = B$ (copia $u$ elementos) |
| `a' = φ16(b', u, v)` | $A = B^T$ ($B$ de $u \times v$ pasa a $A$ de $v \times u$; admite solapamiento hasta 384 elementos) |
| `a' = φ17(b', u)` / `φ18` | $A = B + I$ / $A = B - I$ ($u < 240$) |
| `a' = φ19(b', x, u)` / `φ20` | $A = B + xI$ / $A = B - xI$ ($u < 240$) |
| `a' = φ21(b', d', u)` / `φ22` | $A = B + D$ / $A = B - D$ ($D$ diagonal en vector de $u$ elementos) |
| `a' = φ23(b', x, d', u)` / `φ24` | $A = B + xD$ / $A = B - xD$ |
| `x = φ25(a', u)` | $x = \det(A)$ ($1 < u < 98$); **destruye `A`** |
| `a' = φ26(b', c', u, v, w)` | $A = B \cdot C$ ($B$ es $u \times w$, $C$ es $w \times v$, $A$ es $u \times v$; $0 < u,v,w < 130$) |
| `a' = φ27(b', c', u, v, w)` | $A = B \cdot C^T$ ($B$ es $u \times w$, $C$ es $v \times w$, $A$ es $u \times v$) |
| `a' = φ28(b', m, n)` | Resuelve $B X = A$ ($B$ es $m \times m$, $A$ es $m \times n$); deja $B^{-1} A$ en `A` y **destruye `B`**. Aquí `m, n` son índices o enteros |

---

## 10. Instrucciones de máquina dentro de Autocode

Podés intercalar instrucciones en código de máquina Mercury (formato `FFB`, donde `FF` es la función decimal de 2 cifras y `B` el dígito de registro B `0..7`):

* **Con operando simbólico entre paréntesis:**
  * `400 (x)` / `410 (a0)` / `420 (2.5)` para direcciones de largo;
  * `200 (i)` / `300 (10)` para direcciones cortas o literales;
  * `080 (1)` para saltos a etiquetas Autocode.
* **En forma escrita convencional después de coma:**
  * `592, 0` (retorno por `B2`);
  * `400, 58` (lee el largo formado por los índices `i, j, k, l`);
  * `620, 27` / `620, 0` (cambia la perforadora a registro de letras o de cifras y perfora códigos Ferranti directos, por ejemplo para imprimir títulos).

> [!TIP]
> El compilador usa `B1` (sufijos y divisor), `B2` (retorno de subrutinas), `B3`, `B4` y `B7` (`sac`). Los registros **`B5` y `B6`** quedan libres para código de máquina propio.

---

## 11. Ejemplos del repositorio para leer y probar

* [`samples/autocode/fib2.mer`](../samples/autocode/fib2.mer): sucesión de Fibonacci (muestra el desborde real de los índices de 10 bits a partir del término 14).
* [`samples/autocode/int.mer`](../samples/autocode/int.mer): integral de Sievert del cap. 1 del manual inglés.
* [`samples/autocode/contour.mer`](../samples/autocode/contour.mer): mapa de curvas de nivel de H. Whitfield (usa datos embebidos tras `->`).
* [`examples/elecciones/elecciones.mer`](../examples/elecciones/elecciones.mer): programa multi-capítulo completo con `chapter`/`close`, `down`/`up` e instrucciones `60`/`62`/`63` para leer y perforar texto.
