"""Dibujos en caracteres con el repertorio del teleimpresor de Clementina.

Convierte un dibujo (una imagen, o un dibujo en caracteres ASCII que se
rasteriza primero) a texto en código Ferranti, eligiendo cada carácter por
su forma y no sólo por cuánta tinta tiene:

* Vectores de forma (A. Harri, "ASCII characters are not pixels", 2026):
  cada celda se muestrea con seis círculos (dos columnas por tres filas) y
  se elige el carácter cuyo vector, medido con la misma fuente, está más
  cerca. Así una diagonal sale con / y un borde de arriba con un guion alto,
  en lugar de un carácter de gris parecido.
* Realce de contraste direccional (mismo artículo): círculos externos, en
  las celdas vecinas, afilan los bordes.
* Costo de perforación: en código Ferranti cifras y letras están en dos
  registros, y pasar de uno a otro cuesta un carácter más (el espacio sólo
  existe en cifras). Cada línea se resuelve con Viterbi sobre el registro,
  sumando al error de forma un costo por cada cambio. Es lo que no tienen
  los conversores comunes y lo que más importa en una perforadora de 33
  caracteres por segundo.

Uso:
    python tools/ascii_ferranti.py DIBUJO.txt|IMAGEN.png [--ancho N]
        [--cambio COSTO] [--comparar COMPARACION.png]

Escribe el resultado en la salida estándar (UTF-8). Hace falta Pillow,
NumPy y SciPy, y la fuente Consolas (la que usa la página de --html).
"""

import argparse
import sys
from pathlib import Path

import numpy as np
from PIL import Image, ImageDraw, ImageFont
from scipy import ndimage

FUENTE = 'C:/Windows/Fonts/consola.ttf'
TAM = 40  # puntos; Consolas avanza 22 px por carácter
CELDA_W, CELDA_H = 22, 48  # la página usa line-height 1,25 sobre 16 px ≈ 2,2:1

# Repertorio (CS 225 A, Apéndice VI), sin los de control.
CIFRAS = list("12*4()78≠=-v ,0>≥3→56/×9+.n")
LETRAS = list("ABCDEFGHIJKLMNOPQRSTUVWXYZ.?£")

# Seis círculos de muestreo: (x, y) en fracciones de celda, y el radio.
# Las tres filas cubren el cuerpo de una mayúscula, que va de ARRIBA_M a
# 0,73 de la celda (lo de abajo queda para las colas de , y ;).
ARRIBA_M = 0.19
CIRCULOS = [(0.28, 0.28), (0.72, 0.28), (0.28, 0.47), (0.72, 0.47), (0.28, 0.66), (0.72, 0.66)]
RADIO = 0.24  # en anchos de celda
# Hacia dónde mira cada círculo, para el realce direccional.
AFUERA = [(-0.5, -0.25), (0.5, -0.25), (-0.6, 0.0), (0.6, 0.0), (-0.5, 0.25), (0.5, 0.25)]


def fuente():
    return ImageFont.truetype(FUENTE, TAM)


def rasterizar(lineas, f=None):
    """Dibuja texto en una imagen en escala de grises (1 = tinta)."""
    f = f or fuente()
    w = max((len(l) for l in lineas), default=1) * CELDA_W
    img = Image.new('L', (max(w, 1), len(lineas) * CELDA_H), 0)
    d = ImageDraw.Draw(img)
    # Centrar verticalmente el cuerpo de la fuente en la celda.
    arriba = round(ARRIBA_M * CELDA_H) - f.getbbox('M')[1]
    for j, l in enumerate(lineas):
        for i, ch in enumerate(l):
            if ch != ' ':
                d.text((i * CELDA_W, j * CELDA_H + arriba), ch, fill=255, font=f)
    return np.asarray(img, dtype=np.float32) / 255.0


def mascaras():
    """Máscaras de los círculos internos y externos para una celda."""
    yy, xx = np.mgrid[0:CELDA_H, 0:CELDA_W]
    r = RADIO * CELDA_W
    internas = [((xx - cx * CELDA_W) ** 2 + (yy - cy * CELDA_H) ** 2) <= r * r for cx, cy in CIRCULOS]
    return internas


def muestrear(img, cols, filas):
    """Vectores de forma de cada celda: internos (6) y externos (6)."""
    r = RADIO * CELDA_W
    # Promedio dentro de un círculo = convolución con un disco.
    k = int(np.ceil(r))
    yy, xx = np.mgrid[-k:k + 1, -k:k + 1]
    disco = ((xx * xx + yy * yy) <= r * r).astype(np.float32)
    disco /= disco.sum()
    prom = ndimage.convolve(img, disco, mode='constant')
    H, W = prom.shape

    def leer(x, y):
        xi = np.clip(np.round(x).astype(int), 0, W - 1)
        yi = np.clip(np.round(y).astype(int), 0, H - 1)
        fuera = (x < 0) | (x >= W) | (y < 0) | (y >= H)
        v = prom[yi, xi]
        v[fuera] = 0
        return v

    jj, ii = np.mgrid[0:filas, 0:cols]
    internos = np.stack([leer((ii + cx) * CELDA_W, (jj + cy) * CELDA_H) for cx, cy in CIRCULOS], -1)
    externos = np.stack([leer((ii + cx + dx) * CELDA_W, (jj + cy + dy) * CELDA_H)
                         for (cx, cy), (dx, dy) in zip(CIRCULOS, AFUERA)], -1)
    return internos, externos


def vectores_de_glifos(f):
    glifos = sorted(set(CIFRAS + LETRAS))
    img = rasterizar([''.join(glifos)], f)
    v, _ = muestrear(img, len(glifos), 1)
    return glifos, v[0]


def realzar(v, ext, exp_global=1.6, exp_dir=2.2):
    """Realce de contraste: global por celda y direccional con los externos."""
    m = v.max(-1, keepdims=True)
    seguro = np.where(m > 1e-6, m, 1)
    v = np.where(m > 1e-6, (v / seguro) ** exp_global * m, v)
    mx = np.maximum(v, ext)
    seguro = np.where(mx > 1e-6, mx, 1)
    return np.where(mx > 1e-6, (v / seguro) ** exp_dir * mx, v)


def preparar_imagen(ruta, ancho, umbral=None):
    """Imagen fuente (1 = tinta) y grilla de salida."""
    if ruta.suffix.lower() == '.txt':
        lineas = ruta.read_text(encoding='utf-8').split('\n')
        while lineas and not lineas[-1].strip():
            lineas.pop()
        return imagen_de_texto(lineas, ancho, umbral)
    img = Image.open(ruta)
    if img.mode in ('RGBA', 'LA', 'P'):
        # Lo transparente es papel: componer sobre blanco.
        img = img.convert('RGBA')
        fondo = Image.new('RGBA', img.size, (255, 255, 255, 255))
        img = Image.alpha_composite(fondo, img)
    img = img.convert('L')
    a = np.asarray(img, dtype=np.float32) / 255.0
    if a.mean() > 0.5:  # fondo claro: la tinta es lo oscuro
        a = 1 - a
    cols = ancho or 40
    filas = max(1, round(cols * CELDA_W * a.shape[0] / a.shape[1] / CELDA_H))
    img = img.resize((cols * CELDA_W, filas * CELDA_H), Image.LANCZOS)
    a = np.asarray(img, dtype=np.float32) / 255.0
    if a.mean() > 0.5:
        a = 1 - a
    if umbral is not None:
        a = ndimage.gaussian_filter((a > umbral).astype(np.float32), 1.5)
    return a, cols, filas


def imagen_de_texto(lineas, ancho=None, umbral=None):
    """Un dibujo en caracteres se toma como imagen: se rasteriza y se
    suaviza para que cada carácter cuente como mancha de tinta con su forma.
    Con `umbral`, la mancha se vuelve una figura sólida de bordes netos (los
    caracteres del original dejan rayas que no son parte del dibujo)."""
    img = rasterizar(lineas)
    sigma = CELDA_W * (0.6 if umbral is not None else 0.28)
    img = ndimage.gaussian_filter(img, sigma=sigma)
    img /= max(np.percentile(img, 99.5), 1e-6)
    img = np.clip(img, 0, 1)
    if umbral is not None:
        img = ndimage.gaussian_filter((img > umbral).astype(np.float32), 1.5)
    cols = max(len(l) for l in lineas)
    filas = len(lineas)
    if ancho and ancho != cols:
        nf = max(1, round(filas * ancho / cols))
        img = np.asarray(Image.fromarray((img * 255).astype(np.uint8)).resize((ancho * CELDA_W, nf * CELDA_H), Image.LANCZOS),
                         dtype=np.float32) / 255.0
        cols, filas = ancho, nf
    return img, cols, filas


def convertir(img, cols, filas, cambio=0.08, *, repertorio=None, ocupadas=None):
    """Devuelve las líneas en código Ferranti (Unicode) y cuántos caracteres de
    cinta cuestan (con cambios de registro y CR LF)."""
    f = fuente()
    glifos, gv = vectores_de_glifos(f)
    if repertorio is not None:
        usar = [g in repertorio for g in glifos]
        glifos = [g for g, incluido in zip(glifos, usar) if incluido]
        gv = gv[usar]
    # Normalizar cada componente por el máximo de los glifos (como Harri).
    # Tinta plena en la imagen = el glifo más cargado (no tinta 100 %, que
    # ningún carácter tiene); después, cada componente se normaliza por el
    # máximo de los glifos, como en Harri.
    lleno = gv.mean(1).max()
    escala = gv.max(0)
    escala[escala == 0] = 1
    gv = gv / escala
    v, ext = muestrear(img, cols, filas)
    v = realzar(v * lleno / escala, ext * lleno / escala)
    # Error de forma de cada celda con cada glifo.
    err = ((v[:, :, None, :] - gv[None, None, :, :]) ** 2).sum(-1)
    if ocupadas is not None:
        # En un plano de líneas, conservar también los trazos que quedan
        # fuera de los círculos interiores de una celda.
        err[ocupadas, glifos.index(' ')] = np.inf
    en = {'F': [g in CIFRAS for g in glifos], 'L': [g in LETRAS for g in glifos]}
    en = {k: np.array(x) for k, x in en.items()}
    lineas, punz = [], 0
    for j in range(filas):
        fila = err[j]
        # Viterbi sobre el registro (F cifras, L letras); se empieza y se
        # termina en cifras (el CR LF va en cifras).
        INF = 1e18
        costo = {'F': 0.0, 'L': cambio}
        atras = []
        for i in range(cols):
            nuevo, paso = {}, {}
            for s in 'FL':
                e = np.where(en[s], fila[i], INF)
                k = int(e.argmin())
                # Llegar a s desde s o desde el otro registro.
                desde = min('FL', key=lambda t: costo[t] + (0 if t == s else cambio))
                nuevo[s] = costo[desde] + (0 if desde == s else cambio) + e[k]
                paso[s] = (desde, glifos[k])
            atras.append(paso)
            costo = nuevo
        s = min('FL', key=lambda t: costo[t] + (0 if t == 'F' else cambio))
        texto = []
        for paso in reversed(atras):
            d, g = paso[s]
            texto.append(g)
            s = d
        lineas.append(''.join(reversed(texto)).rstrip())
    return lineas, punzadas(lineas)


def punzadas(lineas):
    """Caracteres de cinta de las líneas, empezando y terminando en cifras."""
    n, reg = 0, 'F'
    for l in lineas:
        for ch in l:
            if ch in CIFRAS and (reg == 'F' or ch not in LETRAS):
                if reg != 'F':
                    n, reg = n + 1, 'F'
            elif ch in LETRAS:
                if reg != 'L':
                    n, reg = n + 1, 'L'
            n += 1
        if reg != 'F':
            n, reg = n + 1, 'F'
        n += 2
    return n


def directo(lineas):
    """La conversión ingenua: cada signo al más parecido (como `clementina midi`)."""
    tabla = {':': '.', '!': '.', ';': ',', "'": ',', '"': ',', '`': ',', '_': '-', '~': '-', '^': '-',
             '|': '/', '\\': '/', '%': '/', '<': '(', '[': '(', '{': '(', ']': ')', '}': ')', '&': '+',
             '#': '≠', '$': '≠', '@': '×', '\t': ' '}
    out = []
    for l in lineas:
        s = ''
        for ch in l.upper():
            c = ch if ch in CIFRAS or ch in LETRAS else tabla.get(ch, ' ')
            s += c
        out.append(s.rstrip())
    return out


def parecido(a, b):
    """Error cuadrático medio entre dos imágenes suavizadas (menos es mejor)."""
    h = min(a.shape[0], b.shape[0])
    w = min(a.shape[1], b.shape[1])
    pa = np.zeros((h, w), np.float32)
    pb = np.zeros((h, w), np.float32)
    pa[:a.shape[0], :a.shape[1]] = a[:h, :w]
    pb[:b.shape[0], :b.shape[1]] = b[:h, :w]
    sa = ndimage.gaussian_filter(pa, CELDA_W * 0.5)
    sb = ndimage.gaussian_filter(pb, CELDA_W * 0.5)
    sa /= max(sa.max(), 1e-6)
    sb /= max(sb.max(), 1e-6)
    return float(((sa - sb) ** 2).mean())


def lado_a_lado(imagenes, ruta):
    alto = max(i.shape[0] for i in imagenes)
    ancho = sum(i.shape[1] for i in imagenes) + 40 * (len(imagenes) - 1)
    lienzo = np.ones((alto, ancho), np.float32)
    x = 0
    for i in imagenes:
        lienzo[:i.shape[0], x:x + i.shape[1]] = 1 - np.clip(i, 0, 1)
        x += i.shape[1] + 40
    Image.fromarray((lienzo * 255).astype(np.uint8)).save(ruta)


def main():
    ap = argparse.ArgumentParser(description=__doc__.split('\n\n')[0])
    ap.add_argument('dibujo', type=Path)
    ap.add_argument('--ancho', type=int, help='columnas del resultado (por omisión, las del dibujo)')
    ap.add_argument('--cambio', type=float, default=0.08, help='costo de un cambio de registro')
    ap.add_argument('--umbral', type=float, help='volver sólida la figura (0 a 1; p. ej. 0,3)')
    ap.add_argument('--comparar', type=Path, help='PNG con el original, la conversión directa y esta')
    a = ap.parse_args()
    img, cols, filas = preparar_imagen(a.dibujo, a.ancho, a.umbral)
    lineas, n = convertir(img, cols, filas, a.cambio)
    sys.stdout.reconfigure(encoding='utf-8')
    print('\n'.join(lineas))
    if a.comparar:
        partes = [img, rasterizar(lineas)]
        if a.dibujo.suffix.lower() == '.txt':
            orig = a.dibujo.read_text(encoding='utf-8').rstrip('\n').split('\n')
            d = directo(orig)
            partes.insert(1, rasterizar(d))
            print(f'directo: {punzadas(d)} caracteres de cinta, error {parecido(img, partes[1]):.4f}', file=sys.stderr)
        print(f'formas:  {n} caracteres de cinta, error {parecido(img, partes[-1]):.4f}', file=sys.stderr)
        lado_a_lado(partes, a.comparar)


if __name__ == '__main__':
    main()
