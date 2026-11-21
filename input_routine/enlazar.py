#!/usr/bin/env python3
"""Resuelve direcciones simbólicas de listados manuscritos del Input Routine.

Uso: python input_routine/enlazar.py FUENTE... --salida DIRECTORIO
El formato acepta C <capítulo> P <página>, R <rutina>, @página.línea,
instrucciones, cortos =n y largos; ';' introduce comentario. Las fuentes de
capítulos interdependientes deben entregarse juntas. Sin --salida sólo audita.
No genera sectores si quedan referencias o expresiones sin resolver.
"""
from __future__ import annotations
import argparse
import re
from pathlib import Path


def ubicacion(t):
    t = t.strip()
    plus = t.endswith('+')
    if plus:
        t = t[:-1]
    signo = -1 if t.startswith('-') else 1
    t = t.lstrip('-')
    if '.' in t:
        a, b = t.split('.')
        n = int(a) * 64 + int(b)
    else:
        n = int(t)
    return signo * (2 * n + int(plus))


def vista(n):
    if n < 0:
        return '-' + vista(-n)
    return str(n // 2) + ('+' if n % 2 else '')


def leer(paths):
    etiquetas, rutinas, elementos, errores = {}, {}, [], []
    cap = rutina = None
    pos = None
    for path in paths:
        for num, raw in enumerate(path.read_text(encoding='utf-8-sig').splitlines(), 1):
            texto = raw.split(';', 1)[0].strip()
            if not texto:
                continue
            origen = f'{path}:{num}'
            try:
                if m := re.fullmatch(r'C\s+(\d+)\s+P\s+(\d+)', texto):
                    cap = int(m[1]); pos = int(m[2]) * 128; rutina = None
                    continue
                if m := re.fullmatch(r'D\s+R(\d+)\s+(\S+)', texto):
                    rutinas[int(m[1])] = ubicacion(m[2])
                    continue
                if texto.startswith('@') or texto.startswith('L '):
                    pos = ubicacion(texto[1:].strip())
                    continue
                if m := re.fullmatch(r'R\s+(\d+)', texto):
                    rutina = int(m[1])
                    if rutina in rutinas:
                        raise ValueError(f'rutina R{rutina} definida dos veces')
                    rutinas[rutina] = pos
                    continue
                if pos is None or rutina is None:
                    raise ValueError('falta capítulo, página o rutina')
                if texto.startswith('+)'):
                    pos = (pos + 1) // 2 * 2
                    if pos % 4:
                        elementos.append((cap, rutina, pos, 'instruccion', ('570', '0'), origen))
                    pos = (pos + 3) // 4 * 4
                    texto = texto[2:].strip()
                    if not texto:
                        continue
                if texto[0] in '+-':
                    pos = (pos + 1) // 2 * 2
                    if pos % 4:
                        elementos.append((cap, rutina, pos, 'instruccion', ('570', '0'), origen))
                    pos = (pos + 3) // 4 * 4
                    elementos.append((cap, rutina, pos, 'largo', texto, origen))
                    pos += 4
                    continue
                if texto[0] in '=>#≠':
                    for parte in texto.split(','):
                        parte = parte.strip()
                        m = re.fullmatch(r'(.+?)(?:\s*\((\d+))?', parte)
                        valor, etiqueta = m.groups()
                        if etiqueta:
                            clave = (rutina, int(etiqueta))
                            if clave in etiquetas: raise ValueError(f'etiqueta duplicada {clave}')
                            etiquetas[clave] = pos, 'corto'
                        elementos.append((cap, rutina, pos, 'corto', valor, origen))
                        pos += 1
                    continue
                pos = (pos + 1) // 2 * 2
                m = re.fullmatch(r'(?:\((\d{3})\)|(\d{3}))\s+(.+?)(?:\s+\((\d+))?', texto)
                if not m:
                    raise ValueError(f'instrucción inválida: {texto}')
                fun = m[1] or m[2]; operando = m[3]; etiqueta = m[4]
                if etiqueta:
                    clave = (rutina, int(etiqueta))
                    if clave in etiquetas: raise ValueError(f'etiqueta duplicada {clave}')
                    etiquetas[clave] = pos, 'instruccion'
                elementos.append((cap, rutina, pos, 'instruccion', (fun, operando), origen))
                pos += 2
            except (ValueError, TypeError) as e:
                errores.append(f'{origen}: {e}')
    return rutinas, etiquetas, elementos, errores


def evaluar(texto, rutina, posicion, rutinas, etiquetas, literal=False):
    t = texto.replace(' ', '').replace('−', '-')
    # En el listado, los paréntesis señalan un operando modificable.
    if m := re.fullmatch(r'\(([^()]+)\)', t):
        t = m[1]
    mitad = t.startswith(('#', '≠'))
    if mitad:
        t = t[1:]
    # La parte fija admite página.línea. Los términos v/n suman/restan
    # direcciones simbólicas y * aporta la dirección de la línea actual.
    t = re.sub(r'-v', 'n', t)
    prefijo = re.match(r'-?\d+(?:\.\d+)?\+?', t)
    total = ubicacion(prefijo[0]) if prefijo else 0
    resto = t[prefijo.end():] if prefijo else t
    while resto:
        m = re.match(r'(?:([vn])(\d*)(?:/(\d+))?|\*)', resto)
        if not m:
            raise ValueError(f'expresión pendiente de cotejo: {texto}')
        resto = resto[m.end():]
        if m[0] == '*':
            total += posicion
            continue
        destino = int(m[3]) if m[3] else rutina
        if m[2]:
            numero = int(m[2])
            if (destino, numero) not in etiquetas:
                raise ValueError(f'v{numero}/R{destino} sin definir')
            direccion, clase = etiquetas[destino, numero]
            # Tipo 3 (PDF pág. 7): la parte simbólica se expresa en la
            # unidad del ítem etiquetado. La parte fija es un entero.
            if literal and clase == 'corto':
                direccion *= 2
        else:
            if destino not in rutinas:
                raise ValueError(f'R{destino} sin definir')
            direccion = rutinas[destino]
        total += direccion if m[1] == 'v' else -direccion
    return total // (2 if mitad else 1)


# Página de carga, primer sector y cantidad de sectores del manuscrito.
CAPITULOS = {1: (2, 13, 13), 2: (11, 26, 3), 3: (11, 29, 3),
             4: (11, 32, 2), 5: (11, 34, 2), 6: (11, 36, 1),
             7: (11, 37, 1), 8: (11, 39, 1), 9: (11, 40, 1),
             15: (11, 41, 2), 27: (0, 43, 1), 77: (0, 44, 1),
             28: (15, 46, 1), 30: (11, 45, 1)}


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('fuentes', nargs='+', type=Path)
    parser.add_argument('--salida', type=Path)
    args = parser.parse_args()
    rutinas, etiquetas, elementos, errores = leer(args.fuentes)
    salida = {}
    ocupados = {}
    for cap, rutina, pos, clase, dato, origen in elementos:
        try:
            if cap not in CAPITULOS:
                raise ValueError(f'capítulo {cap} sin mapa de sectores')
            pagina, primero, cantidad = CAPITULOS[cap]
            indice = pos // 128 - pagina
            if not 0 <= indice < cantidad:
                raise ValueError(f'elemento fuera de los sectores del capítulo {cap}: {vista(pos)}')
            sector = primero + indice
            longitud = {'corto': 1, 'instruccion': 2, 'largo': 4}[clase]
            if pos % 128 + longitud > 128:
                raise ValueError('elemento cruza el límite de sector')
            for desplazamiento in range(longitud):
                clave = (sector, pos % 128 + desplazamiento)
                if clave in ocupados:
                    raise ValueError(f'posición superpuesta con {ocupados[clave]}')
                ocupados[clave] = origen
            if clase == 'instruccion':
                fun, operand = dato
                dec = int(fun[:2])
                literal = 10 <= dec <= 17 or 30 <= dec <= 37
                value = evaluar(operand, rutina, pos, rutinas, etiquetas, literal)
                # Supuesto: las referencias simbólicas a un corto se truncan
                # a medio o largo cuando la función direcciona esa unidad,
                # como en R27: 440 v1/8 apunta al largo que contiene v1.
                corto = dec < 8 or 20 <= dec <= 27 or dec in (60, 61, 63)
                largo = 40 <= dec <= 47 or 50 <= dec <= 56 or dec == 64
                if re.search(r'[vn*]', operand) and not corto:
                    unidad = 4 if largo else 2
                    value = value // unidad * unidad
                item = f'{fun} {vista(value)}'
            elif clase == 'corto':
                value = evaluar(dato[1:], rutina, pos, rutinas, etiquetas)
                if dato[0] in '=≠#' and re.search(r'[vn*]', dato):
                    value = value // 2 * 2
                item = dato[0] + vista(value)
            else:
                item = dato
            salida.setdefault(sector, []).append((pos % 128, item, origen))
        except (ValueError, KeyError) as exc:
            errores.append(f'{origen}: {exc}')
    for e in errores:
        print(e)
    print(f'{len(rutinas)} rutinas, {len(etiquetas)} etiquetas, {len(elementos)} elementos; {len(errores)} errores')
    if errores:
        return 1
    if args.salida:
        args.salida.mkdir(parents=True, exist_ok=True)
        for sector, items in sorted(salida.items()):
            lines = [f'; Sector {sector}. Generado desde el manuscrito por enlazar.py.']
            lines += [f'{vista(pos):5} {item} ; {Path(origen.rsplit(":", 1)[0]).name}:{origen.rsplit(":", 1)[1]}'
                      for pos, item, origen in sorted(items)]
            (args.salida / f'sector{sector:02}.txt').write_text('\n'.join(lines) + '\n', encoding='utf8')
    return 0


if __name__ == '__main__':
    raise SystemExit(main())

