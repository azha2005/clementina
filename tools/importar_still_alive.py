"""Importa la canción del reproductor de consola de CATDIMA
(https://github.com/CATDIMA/StillAlive, GPL-3) para tocarla en Clementina.

Lee song.h, notes_defenitions.h y ASCII_images.h de ese repositorio (si no
se da una carpeta, lo clona en target/StillAlive) y escribe dos archivos
para `clementina midi`:

* NOMBRE.mid: la melodía, una nota por vez, con las duraciones del original;
* NOMBRE.txt: un guion con la letra y los dibujos, cada uno con su tiempo.

Uso:
    python tools/importar_still_alive.py SALIDA [--repo CARPETA] [--dibujos MODO]

MODO dice qué dibujos se perforan: `portada` (por omisión: el logo de
Aperture al principio, y la música espera a que termine de salir), `todos`,
`primero` (sólo la primera vez que aparece cada uno) o `ninguno`. La
perforadora saca a lo sumo 33 caracteres por segundo y cada dibujo tiene
cerca de 800: con `todos`, el texto se atrasa varios minutos respecto de la
música (`clementina midi` informa cuánto).

Lo generado lleva la letra de la canción, que no es de este proyecto:
conviene escribirlo en target/, que no entra en el repositorio.

El original muestra la letra y el dibujo en dos ventanas y borra la letra
entre estrofas. El teleimpresor sólo avanza el papel: el dibujo sale antes
de la línea que lo trae, y el borrado se reemplaza por una línea de guiones.
"""

import argparse
import ast
import math
import operator
import re
import struct
import subprocess
import sys
from pathlib import Path

CADENA = r'"((?:[^"\\]|\\.)*)"'


def cadena_c(s):
    """Interpreta los escapes de una cadena de C++."""
    return re.sub(r'\\(.)', lambda m: {'n': '\n', 't': '\t'}.get(m.group(1), m.group(1)), s)


def evaluar(expr, nombres):
    """Evalúa una expresión entera con +, -, *, / y nombres conocidos."""
    ops = {ast.Add: operator.add, ast.Sub: operator.sub, ast.Mult: operator.mul, ast.Div: operator.truediv}

    def ev(n):
        if isinstance(n, ast.Expression):
            return ev(n.body)
        if isinstance(n, ast.Constant) and isinstance(n.value, (int, float)):
            return n.value
        if isinstance(n, ast.Name):
            if n.id not in nombres:
                raise ValueError(f'nombre desconocido: {n.id}')
            return nombres[n.id]
        if isinstance(n, ast.BinOp) and type(n.op) in ops:
            return ops[type(n.op)](ev(n.left), ev(n.right))
        raise ValueError(f'expresión no admitida: {expr}')

    return ev(ast.parse(expr.strip(), mode='eval'))


def constantes(src_notas):
    nombres = {}
    for nombre, valor in re.findall(r'constexpr\s+int\s+(\w+)\s*=\s*(\d+)\s*;', src_notas):
        nombres[nombre] = int(valor)
    bpm = nombres.get('BPM', 120)
    # Igual que en el original: BPS = BPM / 60 es una división entera.
    bps = float(bpm // 60)
    n1 = int(1 / (bps / 4) * 1000)
    n2 = int(1 / (bps / 2) * 1000)
    n4 = int(1 / bps * 1000)
    n8 = n4 // 2
    nombres.update(note1_ms=n1, note2_ms=n2, note4_ms=n4, note8_ms=n8,
                   note4dot_ms=n8 + n4, note16_ms=n8 // 2, barDuration_ms=n1)
    return nombres


def lista(src, nombre):
    """El cuerpo de `static std::list<...> nombre { ... };`."""
    m = re.search(r'static\s+std::list<\w+>\s+' + nombre + r'\s*\{(.*?)\n\};', src, re.S)
    if not m:
        sys.exit(f'no encontré la lista {nombre}')
    return m.group(1)


def leer_notas(src, nombres):
    notas = []
    for frec, dur in re.findall(r'Note\(\s*(\w+)\s*,\s*([^)]*?)\s*\)', lista(src, 'notes')):
        f = evaluar(frec, nombres)
        notas.append((f, int(evaluar(dur, nombres))))
    return notas


def leer_letra(src, nombres, dibujos_enum):
    """Lista de (texto, duración en ms, dibujo o None, borrar)."""
    cuerpo = lista(src, 'lyrics')
    cuerpo = re.sub(r'/\*.*?\*/', '', cuerpo, flags=re.S)
    salida = []
    for m in re.finditer(r'Text\((.*?),\s*([^,()]*(?:\([^()]*\)[^,()]*)*)\)\s*,?\s*(?=Text\(|$)', cuerpo, re.S):
        parte, dur = m.group(1), m.group(2)
        dibujo = None
        borrar = 'COMMAND::CLEAR' in parte
        md = re.search(r'ASCII_IMAGES::(\w+)', parte)
        if 'COMMAND::CHANGE_IMAGE' in parte and md:
            dibujo = md.group(1)
        texto = ''.join(cadena_c(s) for s in re.findall(CADENA, parte))
        salida.append((texto, int(evaluar(dur, nombres)), dibujo, borrar))
    return salida


def leer_dibujos(src):
    enum = re.search(r'enum\s+class\s+ASCII_IMAGES\s*\{(.*?)\}', src, re.S).group(1)
    nombres = [n.strip() for n in enum.split(',') if n.strip()]
    dibujos = {}
    for nombre, cuerpo in re.findall(r'const\s+std::string\s+(\w+)\s*\[\]\s*=?\s*\{(.*?)\};', src, re.S):
        lineas = [cadena_c(s).rstrip() for s in re.findall(CADENA, cuerpo)]
        while lineas and not lineas[-1]:
            lineas.pop()
        while lineas and not lineas[0]:
            lineas.pop(0)
        margen = min((len(l) - len(l.lstrip()) for l in lineas if l.strip()), default=0)
        dibujos[nombre.upper()] = [l[margen:] for l in lineas]
    return nombres, dibujos


def punzadas(texto):
    """Caracteres de cinta aproximados: letras en un registro, el resto en el
    otro (el espacio sólo existe en cifras), CR LF por línea."""
    n, letras = 0, False
    for ch in texto.upper():
        if ch == '\n':
            n += 2 + letras
            letras = False
            continue
        quiere = 'A' <= ch <= 'Z'
        if ch != '.' and quiere != letras:
            n += 1
            letras = quiere
        n += 1
    return n


def midi(notas, ruta, antes_ms=0):
    """Formato 0, 1 pulso = 1 ms (negra de 1000 pulsos a 60 negras por minuto).
    `antes_ms`: silencio antes de la primera nota."""
    pista = bytearray(b'\x00\xff\x51\x03' + (1_000_000).to_bytes(3, 'big'))

    def vlq(v):
        b = [v & 0x7f]
        v >>= 7
        while v:
            b.append((v & 0x7f) | 0x80)
            v >>= 7
        return bytes(reversed(b))

    espera = antes_ms
    for f, dur in notas:
        if f <= 0:
            espera += dur
            continue
        altura = round(69 + 12 * math.log2(f / 440))
        pista += vlq(espera) + bytes([0x90, altura, 100]) + vlq(dur) + bytes([0x80, altura, 0])
        espera = 0
    pista += vlq(espera) + b'\xff\x2f\x00'
    datos = b'MThd' + struct.pack('>IHHH', 6, 0, 1, 1000) + b'MTrk' + struct.pack('>I', len(pista)) + pista
    Path(ruta).write_bytes(datos)


def guion(letra, dibujos, modo, ruta):
    """Escribe el guion; devuelve cuántos ms tiene que esperar la música."""
    t = 0
    vistos = set()
    out = []
    if modo == 'portada' and 'APERTURE' in dibujos:
        logo = '\n'.join(dibujos['APERTURE']) + '\n\n'
        out += ['@0', logo.rstrip('\n'), '']
        # Unos 31 ms por carácter en silencio, más un margen.
        t = int(punzadas(logo) * 31 * 1.1) + 1000
    antes = t
    for texto, dur, dibujo, borrar in letra:
        partes = []
        if borrar:
            partes.append('\n' + '-' * 40 + '\n\n')
        if dibujo and dibujo in dibujos:
            poner = (modo == 'todos'
                     or (modo == 'primero' and dibujo not in vistos)
)
            vistos.add(dibujo)
            if poner:
                partes.append('\n' + '\n'.join(dibujos[dibujo]) + '\n\n')
        partes.append(texto)
        bloque = ''.join(partes)
        if bloque:
            out.append(f'@{t / 1000:.3f}')
            # Una línea que empiece con @ se escribe @@ en el guion.
            out.append('\n'.join('@' + l if l.startswith('@') else l for l in bloque.split('\n')).rstrip('\n'))
        t += dur
    Path(ruta).write_text('\n'.join(out) + '\n', encoding='utf-8')
    return antes


def main():
    ap = argparse.ArgumentParser(description=__doc__.split('\n\n')[0])
    ap.add_argument('salida', help='nombre base de los archivos a escribir')
    ap.add_argument('--repo', help='copia local del repositorio (si no, se clona en target/StillAlive)')
    ap.add_argument('--dibujos', choices=['portada', 'todos', 'primero', 'ninguno'], default='portada')
    ap.add_argument('--logo', type=Path, help='imagen del logo: reemplaza al dibujo del repositorio, '
                    'convertida con tools/ascii_ferranti.py')
    ap.add_argument('--logo-ancho', type=int, default=36, help='columnas del logo convertido (36)')
    a = ap.parse_args()
    if a.repo:
        repo = Path(a.repo)
    else:
        repo = Path(__file__).resolve().parent.parent / 'target' / 'StillAlive'
        if not (repo / 'song.h').exists():
            subprocess.run(['git', 'clone', '--depth', '1', 'https://github.com/CATDIMA/StillAlive', str(repo)], check=True)
    leer = lambda n: (repo / n).read_text(encoding='utf-8', errors='replace')
    nombres = constantes(leer('notes_defenitions.h'))
    song = leer('song.h')
    enum, dibujos = leer_dibujos(leer('ASCII_images.h'))
    notas = leer_notas(song, nombres)
    letra = leer_letra(song, nombres, enum)
    Path(a.salida).parent.mkdir(parents=True, exist_ok=True)
    if a.logo:
        sys.path.insert(0, str(Path(__file__).resolve().parent))
        import ascii_ferranti
        img, cols, filas = ascii_ferranti.preparar_imagen(a.logo, a.logo_ancho)
        dibujos['APERTURE'], _ = ascii_ferranti.convertir(img, cols, filas)
    antes = guion(letra, dibujos, a.dibujos, a.salida + '.txt')
    midi(notas, a.salida + '.mid', antes)
    total_notas = sum(d for _, d in notas) / 1000
    total_letra = sum(x[1] for x in letra) / 1000
    print(f'{len(notas)} notas ({total_notas:.1f} s), {len(letra)} textos ({total_letra:.1f} s), '
          f'dibujos: {", ".join(sorted(dibujos))}')
    print(f'-> {a.salida}.mid\n-> {a.salida}.txt')


if __name__ == '__main__':
    main()
