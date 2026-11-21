"""Fragmentos de ensamblador para las dos disposiciones de la caché KV."""

REDONDEO_PAR = """210 6
350 1
280 $ver_empate
590 $poner_signo
ver_empate:
200 6
210 41+
400 40
440 56
450 31.42
450 16
707 0
370 768
280 $poner_signo
200 6
330 1
210 6"""

DIRECCION_16 = """200 4
340 0
340 0
340 0
220 7+
210 6+
677 0
680 15
200 4
350 7
210 7
220 7
210 7
220 7
210 7
002 7
220 7
210 7
220 7
210 7
003 7
596 0"""

# Diez posiciones por sector. Cada posición tiene exponente, mantisa
# alta y diez pares con signo, en una ranura de doce cortos.
DIRECCION_12 = """004 7+
200 4
dividir_por_diez:
330 10
290 $al_sector_siguiente
320 10
210 7
220 7
210 7
220 7
210 7
220 7
220 7
210 7
003 7
014 6+
674 0
680 15
596 0
al_sector_siguiente:
124 1
590 $dividir_por_diez"""

# Truncar también antes de cuantizar, así se usa la misma escala al leer.
GUARDAR_20 = """410 10
010 10+
010 11
200 10
213 15.0
200 11+
213 15.0+
400 10"""

LEER_20 = """203 15.0
210 8
010 8+
010 9
203 15.0+
210 9+"""


def fragmentos(compacto):
    return dict(
        guardar_escala=GUARDAR_20 if compacto else "412 15.0",
        preparar_cuantizador="" if compacto else "400 31.56\n410 40",
        cortos_escala=2 if compacto else 4,
        guardar_magnitud="" if compacto else "410 16",
        ajustar_redondeo="210 6" if compacto else REDONDEO_PAR,
        sectores_vector=6 if compacto else 10,
        leer_escala=LEER_20 if compacto else "402 15.0\n410 8",
        direccion_vector=DIRECCION_12 if compacto else DIRECCION_16,
    )
