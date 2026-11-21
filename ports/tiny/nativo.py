"""Arma los bancos y el guion de TINY 2.1 para grabar en cinta Ferranti."""
import argparse
import json
import math
from pathlib import Path
import re
import struct
from cache_kv import fragmentos

VOCABULARIO = " !\"'()*+,-./0123456789:;=?\\abcdefghijklmnopqrstuvwxyz"
FIGURAS = " 12*4()78 =-v  ,0> 3 56/ 9+ .n  "
LETRAS = " abcdefghijklmnopqrstuvwxyz .?  "
PAGINA = 128


def ensamblador(origen, limite, fuente):
    """Resuelve nombres de saltos dentro de un banco de instrucciones."""
    linea = origen
    instrucciones = []
    nombres = {}
    for texto in fuente.splitlines():
        texto = texto.split(';', 1)[0].strip()
        if not texto:
            continue
        if texto.startswith('@'):
            linea = int(texto[1:])
        elif texto.endswith(':'):
            nombre = texto[:-1]
            if nombre in nombres:
                raise ValueError(f"etiqueta repetida: {nombre}")
            nombres[nombre] = linea
        else:
            if not origen <= linea < limite:
                raise ValueError(f"el código desborda el banco en el medio {linea}")
            instrucciones.append((linea, texto))
            linea += 1
    salida = [f"@{origen}", f"!{origen}"]
    siguiente = origen
    for linea, texto in instrucciones:
        if linea != siguiente:
            salida.append(f"@{linea}")
        salida.append(re.sub(r"\$([a-z_0-9]+)", lambda m: str(nombres[m[1]]), texto))
        siguiente = linea + 1
    return '\n'.join(salida) + '\n'


def tabla_ternaria():
    """Las ternas se numeran con el primer trit en el dígito menos significativo."""
    instrucciones = []
    for z in range(3):
        for y in range(3):
            terminos = [(digito, fuente) for digito, fuente in ((z, 24), (y, 22))
                        if digito != 1]
            if terminos and terminos[0][0] == 2:
                instrucciones.append(f"400 {terminos.pop(0)[1]}")
            else:
                instrucciones.append("400 18")
            for digito, fuente in terminos:
                instrucciones.append(f"{450 if digito == 0 else 440} {fuente}")
            medio = 2 * (9 * z + 3 * y)
            # Partir de la suma de y,z para obtener las tres variantes de x.
            instrucciones.extend([f"410 6.{medio + 2}", "450 20", f"410 6.{medio}",
                                  f"400 6.{medio + 2}", "440 20", f"410 6.{medio + 4}"])
    return "\n".join(instrucciones)


def tmac_compacto(ruta, fuente):
    """El banco compacto comparte el código de F1 con las otras proyecciones."""
    inicio, final = fuente.split("; Una multiplicación", 1)
    entrada = """590 v40
590 v41
300 224 (40
210 47
300 608
210 47+
590 v42
300 608 (41
210 47
300 224
210 47+
200 47 (42
220 60+
210 45
200 47+
220 60
210 45+
"""
    inicio = inicio.replace("!2.48\n101 0", "!2.48\n" + entrada + "001 47", 1)
    inicio = inicio.replace("103 0", "003 62\n200 62+\n210 44", 1)
    inicio = inicio.replace("105 0", "005 47+").replace("405 19.0", "405 0")
    inicio = inicio.replace("075 60", "075 45+")
    inicio = inicio.replace("101 0\n401 7.0 (6", "001 47\n401 7.0 (6")
    coef = (ruta / "coeficientes_compactos.mer").read_text(encoding="utf-8")
    nucleo = re.sub(r"(?m) 7\.0(?= |$)", " 0", inicio + coef).replace("071 60+", "071 45")
    return nucleo + "\n; Siguiente corto" + final.split("; Siguiente corto", 1)[1]


def preparar(imagen, destino, salida_maxima=48, temperatura=0, semilla=1):
    destino.mkdir(parents=True, exist_ok=True)
    ruta = Path(__file__).parent
    origen = struct.unpack("<65536H", imagen.read_bytes())
    firma = bytes(origen[:6])
    if firma not in (b"TINYC1", b"TINYT1", b"TINYD1"):
        raise ValueError("imagen TINY incompatible con el mapa de tambor")
    if not 1 <= salida_maxima <= 48:
        raise ValueError("la cantidad máxima de caracteres debe estar entre 1 y 48")
    if temperatura != 0 and not 0.01 <= temperatura <= 10:
        raise ValueError("la temperatura tiene que ser 0 o estar entre 0,01 y 10")
    if not 1 <= semilla <= 1023:
        raise ValueError("la semilla tiene que estar entre 1 y 1023")
    ternario = firma == b"TINYT1"
    compacto = firma == b"TINYD1"
    dimension, intermedia = (96, 384) if ternario else (80, 224 if compacto else 256)
    bloques = 3 if compacto else 2
    contexto = 60 if compacto else 80
    sectores_vector = 6 if compacto else 10
    sectores_kv = 2 * bloques * sectores_vector
    entrada_aux = 173 if compacto else 192
    cabeza = dimension // 4
    residual, kv_buffer = (16, 22) if ternario else (28, 16)
    paginas_mat = 4 if ternario or compacto else 3
    ctrl = origen[6] if compacto else origen[49] + 2
    aux = ctrl + (3 if compacto else 4)
    atn, mat_sector = aux + 6, aux + 12
    f1_sector = mat_sector if compacto else mat_sector + paginas_mat
    tabla = f1_sector + paginas_mat
    constantes, ordenes = tabla + 1, tabla + 2
    kv = ordenes + (6 if compacto else 4)
    if kv + sectores_kv > 512:
        raise ValueError("los datos, el programa y la caché exceden el tambor")
    datos = [0] * 65536
    if compacto:
        datos[2 * PAGINA:ctrl * PAGINA] = origen[2 * PAGINA:ctrl * PAGINA]
    else:
        datos[3 * PAGINA:ctrl * PAGINA] = origen[PAGINA:origen[49] * PAGINA]

    def escribir_sector(n, cortos):
        datos[n * PAGINA:n * PAGINA + len(cortos)] = cortos

    mapa = [0] * 64
    for i, c in enumerate(FIGURAS + LETRAS):
        if c in VOCABULARIO and c != ' ':
            mapa[i] = VOCABULARIO.index(c) + 2
    mapa[14] = 2
    salida = [0, 0]
    for c in VOCABULARIO:
        if c == ' ':
            salida.append(14)
        elif c in LETRAS:
            salida.append(32 + LETRAS.index(c))
        elif c in FIGURAS:
            salida.append(FIGURAS.index(c))
        else:
            salida.append(32 + LETRAS.index('?'))
    escribir_sector(tabla, mapa + salida)

    guion = []

    def tarea(banco, entrada, *parametros):
        guion.extend([banco, entrada, *parametros, *([0] * (6 - len(parametros)))])

    def matriz(bloque, tipo, filas, columnas=dimension):
        i = 6 * bloque + tipo
        if compacto:
            campo = 8 + 2 * (27 + 2 * i)
            sec_p, off_p, sec_c, off_c = origen[campo:campo + 4]
        else:
            campo = 25 + 2 * i
            sec_p, sec_c = (x + 2 for x in origen[campo:campo + 2])
            off_p = off_c = 0
        entrada = 177 if compacto and tipo == 4 else 176
        tarea(f1_sector if tipo == 4 else mat_sector, entrada, columnas, filas, sec_p, sec_c, off_p, off_c)

    def tensor(campo, entrada, n, fila=0, fuente=0, destino=0):
        if compacto:
            i = 8 + 2 * campo
            sector, offset = origen[i:i + 2]
            if campo == 0:
                offset |= 128
        else:
            sector, offset = origen[6 + campo] + 2, 0
        tarea(aux, entrada_aux + entrada, n, fila, sector, fuente, destino, offset)

    def norma(campo, entrada, fuente, destino, n=dimension):
        tensor(campo, entrada, n, fuente=fuente, destino=destino)

    x, y, r = 19 * 32, 7 * 32, residual * 32
    # La Quicky 1 y su retorno ocupan los medios 148–172.
    tensor(0, 0, dimension, destino=r)
    for capa in range(2 * bloques):
        bloque = capa % bloques
        base = kv + (capa // 2) * 2 * sectores_vector
        tensor(1, 1, dimension, fila=capa, destino=r)
        norma(2 + 4 * capa, 2, r, x)
        if capa % 2 == 0:
            matriz(bloque, 1, cabeza)
            tarea(atn, 174, base)
            matriz(bloque, 2, cabeza)
            tarea(atn, 174, base + sectores_vector)
        if capa == 2 * bloques - 2:
            tarea(0, 0)  # en el prefijo ya se llenaron todas las cachés
        matriz(bloque, 0, dimension)
        tarea(atn, 175, base)
        norma(3 + 4 * capa, 2, x, x)
        matriz(bloque, 3, dimension)
        tarea(aux, entrada_aux + 3, dimension)
        norma(4 + 4 * capa, 2, r, y)
        matriz(bloque, 4, intermedia)
        norma(5 + 4 * capa, 4, x, x, intermedia)
        matriz(bloque, 5, dimension, intermedia if ternario else math.ceil(intermedia / 5) * 5)
        tarea(aux, entrada_aux + 3, dimension)
    norma(2 + 8 * bloques, 2, r, x)
    tensor(0, 5, dimension)
    tarea(0, 1)
    if len(guion) > (kv - ordenes) * PAGINA:
        raise ValueError("el guion excede su reserva en el tambor")
    escribir_sector(ordenes, guion)
    (destino / "datos.bin").write_bytes(struct.pack("<65536H", *datos))
    bancos = [ctrl, "control.mer", aux, "auxiliares.mer", atn, "atencion.mer",
              mat_sector, "matriz.mer", constantes, "constantes.mer"]
    if not compacto:
        bancos.extend([f1_sector, "f1.mer"])
    (destino / "bancos.json").write_text(json.dumps(bancos), encoding="utf-8")

    raiz = ruta.parent.parent
    exp = (raiz / "crates/mercury-autocode/runtime/exp.mer").read_text(encoding="utf-8")
    inversa = (raiz / "input_routine/quickies/01.mer").read_text(encoding="utf-8")
    inversa = re.sub(r"([v(])([12])\b", r"\g<1>30\2", inversa)
    comunes = "@64\n!64\n" + exp + "\n" + inversa.replace("@128", "@148") + "\n596 0\n"
    for nombre, inicio, limite in (("control", 640, 814),
                                   ("auxiliares", entrada_aux, 448),
                                   ("atencion", 174, 448)):
        asm = (ruta / f"{nombre}.mer").read_text(encoding="utf-8")
        if compacto and nombre == "auxiliares":
            asm = asm.split("\ntensor:", 1)[0] + "\n" + (ruta / "tensores_compactos.mer").read_text(encoding="utf-8")
        asm = asm.format(
            guion=ordenes, matriz=mat_sector, f1=f1_sector, salida=salida_maxima,
            residual=residual, dimension=dimension, cabeza=cabeza, kv_buffer=kv_buffer,
            prefijo_maximo=contexto - 2, contexto_tope=contexto - 1,
            guardar_logit="003 60+\n413 25.0" if temperatura else "",
            sorteo="590 $muestrear" if temperatura else "",
            pagina_matriz="320 1\n677 0\n680 5" if paginas_mat == 4 else "",
            **fragmentos(compacto))
        if nombre == "auxiliares" and temperatura:
            asm += "\n" + (ruta / "muestreo.mer").read_text(encoding="utf-8")
        compilado = ensamblador(inicio, limite, asm)
        if nombre != "control":
            compilado = comunes + compilado.replace(f"!{inicio}\n", "", 1)
        else:
            cartel = "TINY 2.1 / WOLLYMONJAC"
            filas = [0]
            registro = 0
            for c in cartel.lower():
                if c in LETRAS and c != ' ':
                    cambio, valor = 27, LETRAS.index(c)
                else:
                    cambio, valor = 0, 14 if c == ' ' else FIGURAS.index(c)
                if cambio != registro:
                    filas.append(cambio)
                    registro = cambio
                filas.append(valor)
            filas.extend([0, 30, 13, 31])
            if len(filas) > 2 * (64 - 46):
                raise ValueError("el cartel excede la página 12")
            compilado += "@12.46\n" + ''.join(f"={c}\n" for c in filas)
        (destino / f"{nombre}.mer").write_text(compilado, encoding="utf-8")
    if ternario:
        fuente_ternaria = (ruta / "ternario.mer").read_text(encoding="utf-8")
        tabla_fuente = tabla_ternaria()
        mat = ensamblador(176, 384, fuente_ternaria.format(x="19.0", y="7.0", tabla_ternaria=tabla_fuente))
        f1 = ensamblador(176, 384, fuente_ternaria.format(x="7.0", y="19.0", tabla_ternaria=tabla_fuente))
    else:
        mat = (ruta / "tmac.mer").read_text(encoding="utf-8").split("@2.48", 1)[1]
        mat = "@2.48" + mat.replace("990 0", "006 50\n596 0")
        if compacto:
            mat = tmac_compacto(ruta, mat)
        else:
            f1 = mat.replace("405 19.0", "405 7.0")
            for funcion in ("411", "401", "521"):
                f1 = f1.replace(f"{funcion} 7.0", f"{funcion} 19.0")
    (destino / "matriz.mer").write_text(mat, encoding="utf-8")
    if not compacto:
        (destino / "f1.mer").write_text(f1, encoding="utf-8")
    iniciar_sorteo = f"300 {semilla}\n210 13.60\n" if temperatura else ""
    constantes_sorteo = (
        f"@31.26\n+0.0009765625\n+{1 / temperatura:.17f}\n+512\n" if temperatura else ""
    )
    (destino / "arranque.mer").write_text(
        "@1.0\n!1.0\n"
        + "".join(f"670 {ctrl + i}\n680 {10 + i}\n" for i in range(3))
        + f"670 {tabla}\n680 13\n{iniciar_sorteo}"
        + f"670 {constantes}\n680 31\n590 10.0\n",
        encoding="utf-8",
    )
    (destino / "constantes.mer").write_text(
        f"@31.0\n!31.0\n{constantes_sorteo}@31.32\n"
        f"+0.00001\n+{1 / dimension:.17f}\n+{1 / intermedia:.17f}\n+{1 / (7 if compacto else 15):.17f}\n"
        f"+0.00000001\n+0.5\n+{1 / math.sqrt(cabeza):.17f}\n"
        "+0.25\n+0.0625\n+0.015625\n+0.00390625\n-1000000\n"
        "=9\n=0\n=0\n=0\n+0\n-1\n=0\n=0\n=0\n=0\n",
        encoding="utf-8",
    )
    print(f"TINY 2.1 de WollyMonjac | {len(guion) // 8} órdenes de cálculo | {kv + sectores_kv}/512 sectores con KV")


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("imagen", type=Path)
    parser.add_argument("destino", type=Path)
    parser.add_argument("--salida-maxima", type=int, default=48)
    parser.add_argument("--temperatura", type=float, default=0)
    parser.add_argument("--semilla", type=int, default=1)
    args = parser.parse_args()
    preparar(args.imagen, args.destino, args.salida_maxima, args.temperatura, args.semilla)
