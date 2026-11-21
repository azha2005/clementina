"""Prepara un tambor de Clementina para TINY 2.1 (WollyMonjac)."""
import argparse
import math
from pathlib import Path
import struct
import tempfile

import numpy as np
import torch

SECTOR = 128
CAPACIDAD = 512 * SECTOR


def largo(valor):
    """Constantes en el formato de mercury_core::acc::Float."""
    if not math.isfinite(valor):
        raise ValueError(f"constante no finita: {valor}")
    if valor == 0:
        return [768, 0, 0, 0]
    frac, exp = math.frexp(valor)
    fraccion = math.floor(abs(frac) * (1 << 29) + 0.5)
    if frac < 0:
        fraccion = -fraccion
    if fraccion >= 1 << 29:
        fraccion >>= 1
        exp += 1
    while -(1 << 28) <= fraccion < (1 << 28):
        fraccion <<= 1
        exp -= 1
    return [exp & 1023, *[(fraccion >> desplazamiento) & 1023 for desplazamiento in (0, 10, 20)]]


def planos(peso, alpha):
    """Grupo de cinco columnas y dos filas consecutivas en tres cortos."""
    filas, columnas = peso.shape
    niveles = (4 * (peso / alpha[:, None]).clamp(-1, 1)).floor().clamp(-4, 3) + 4
    arr = niveles.numpy().astype(np.uint16)
    arr = np.pad(arr, ((0, 0), (0, -columnas % 5)))
    arr = arr.reshape(filas, -1, 5).transpose(1, 0, 2)
    p0, p1, p2 = [
        (np.sum(((arr >> bit) & 1) << np.arange(5), axis=2)).astype(np.uint16)
        for bit in range(3)
    ]
    return np.stack(
        [(p2[:, ::2] << 5) | p1[:, ::2],
         (p0[:, ::2] << 5) | p2[:, 1::2],
         (p1[:, 1::2] << 5) | p0[:, 1::2]], axis=-1
    ).reshape(-1).tolist()


def ternas(peso, alpha):
    """Dos filas para el mismo grupo de tres columnas por corto."""
    filas, columnas = peso.shape
    trits = (1.5 * (peso / alpha[:, None]).clamp(-1, 1)).round().clamp(-1, 1) + 1
    grupos = trits.numpy().astype(np.uint16).reshape(filas, columnas // 3, 3)
    indices = (grupos @ np.array([1, 3, 9], dtype=np.uint16)).T
    return ((indices[:, ::2] << 5) | indices[:, 1::2]).reshape(-1).tolist()


def escala_entera(valores, bits):
    valores = np.asarray(valores, dtype=np.float64)
    limite = (1 << (bits - 1)) - 1
    maximo = np.abs(valores).max()
    exponente = math.ceil(math.log2(maximo / limite)) if maximo else 0
    enteros = np.rint(np.ldexp(valores, -exponente)).astype(np.int16)
    return (9 + exponente) & 1023, enteros


def compacto(pesos):
    """Datos contiguos con directorio en los sectores que el tele-input reserva."""
    tambor = [0] * (2 * SECTOR)
    campos = list(b"TINYD1") + [0, 0]

    def guardar(datos):
        campos.extend(divmod(len(tambor), SECTOR))
        tambor.extend(datos)

    # Diez componentes de nueve bits en nueve cortos, sin relleno.
    expo, emb = escala_entera(pesos["emb.weight"].numpy(), 9)
    cortos = [expo]
    for grupo in (emb & 511).reshape(-1, 10):
        bits = 0
        for entero in grupo:
            bits = (bits << 9) | int(entero)
        cortos.extend((bits >> corrimiento) & 1023 for corrimiento in range(80, -1, -10))
    guardar(cortos)
    nombres = ["loop_emb"]
    nombres += [f"pass_norms.{c}.{n}.g" for c in range(6) for n in ("n1", "sn1", "n2", "sn2")]
    nombres += ["nf.g"]
    for nombre in nombres:
        expo, enteros = escala_entera(pesos[nombre].numpy().reshape(-1), 10)
        guardar([expo, *(enteros & 1023).tolist()])
    for bloque in range(3):
        for nombre in ("q", "k", "v", "o", "f1", "f2"):
            clave = f"blocks.{bloque}.{nombre}"
            alpha = pesos[clave + ".alpha"].abs().clamp_min(1e-7)
            guardar(planos(pesos[clave + ".weight"], alpha))
            e_a, a = escala_entera((alpha / 8).numpy(), 10)
            e_b, b = escala_entera(pesos[clave + ".bias"].numpy(), 10)
            guardar([e_a, e_b, *(np.stack([a, b], axis=-1).reshape(-1) & 1023).tolist()])
    campos[6] = math.ceil(len(tambor) / SECTOR)
    tambor[:len(campos)] = campos
    return tambor, campos[6], 36


def escribir_imagen(salida, tambor, ocupados, reserva_kv):
    if ocupados + reserva_kv > CAPACIDAD // SECTOR:
        raise ValueError("los datos y la caché KV exceden los 512 sectores")
    tambor.extend([0] * (CAPACIDAD - len(tambor)))
    salida.parent.mkdir(parents=True, exist_ok=True)
    with tempfile.NamedTemporaryFile(dir=salida.parent, delete=False) as archivo:
        temporal = Path(archivo.name)
        try:
            archivo.write(struct.pack(f"<{CAPACIDAD}H", *tambor))
            archivo.close()
            temporal.replace(salida)
        finally:
            temporal.unlink(missing_ok=True)
    print(f"TINY 2.1 de WollyMonjac | {ocupados} sectores de datos y directorio, {reserva_kv} reservados para KV")


def exportar(entrada, salida):
    ck = torch.load(entrada, map_location="cpu", weights_only=True)
    cfg, pesos = ck["cfg"], ck["sd"]
    formas = {("3bit_seq", 2): (80, 256), ("tern_seq", 2): (96, 384),
              ("3bit_seq", 3): (80, 224)}
    quant, bloques = cfg.get("quant"), cfg.get("L")
    dimension, intermedia = formas.get((quant, bloques), (0, 0))
    esperado = dict(D=dimension, loops=2, H=4, HID=intermedia, KVH=1, kv_share=True,
                    norm="l1", pos="alibi", act="relu2",
                    scale_mode="row", subln=True, bias=True, vocab="lower")
    if not ck.get("retro") or any(cfg.get(k) != v for k, v in esperado.items()):
        raise ValueError("se esperaba r3_80x2L2x256, rt3_96_wd05 o r3_80x3L2x224")

    if bloques == 3:
        escribir_imagen(salida, *compacto(pesos))
        return
    ternario = cfg["quant"] == "tern_seq"
    empaquetar = ternas if ternario else planos
    tambor = [0] * SECTOR
    campos = list(b"TINYT1" if ternario else b"TINYC1")

    def anexar(datos):
        tambor.extend([0] * (-len(tambor) % SECTOR))
        sector = len(tambor) // SECTOR
        tambor.extend(datos)
        return sector

    def tensor(nombre):
        valores = pesos[nombre].numpy().reshape(-1).astype(np.float64)
        maximo = max(abs(valores))
        escala = math.ldexp(1.0, math.ceil(math.log2(maximo / 511))) if maximo else 1.0
        cortos = np.rint(valores / escala).astype(np.int16)
        assert min(cortos) >= -511 and max(cortos) <= 511
        campos.extend([anexar(largo(escala) + (cortos & 1023).tolist())])

    tensor("emb.weight")
    tensor("loop_emb")
    for capa in range(4):
        for nom in ("n1", "sn1", "n2", "sn2"):
            tensor(f"pass_norms.{capa}.{nom}.g")
    tensor("nf.g")

    for bloque in range(2):
        for nom in ("q", "k", "v", "o", "f1", "f2"):
            clave = f"blocks.{bloque}.{nom}"
            if clave + ".weight" not in pesos:
                campos.extend([0, 0])
                continue
            alpha = pesos[clave + ".alpha"].abs().clamp_min(1e-7)
            w = anexar(empaquetar(pesos[clave + ".weight"], alpha))
            # seq_tern aplica 2/3 y seq_nbit aplica 1/8 al nivel impar.
            escalas = alpha * (2.0 / 3.0) if ternario else alpha / 8
            coef = [c for a, b in zip(escalas.tolist(), pesos[clave + ".bias"].tolist())
                    for c in largo(a) + largo(b)]
            campos.extend([w, anexar(coef)])

    ocupados = math.ceil(len(tambor) / SECTOR)
    # Dos pares de K/V, 80 posiciones, dos valores de 5 bits por corto
    # y una escala FP40 para cada vector. Se reservan aunque el motor
    # inicial todavía lleve el control de la caché desde el anfitrión.
    reserva_kv = 40 if ternario else math.ceil(2 * 2 * 80 * (10 + 4) / SECTOR)
    campos.extend([ocupados, reserva_kv])
    tambor[:len(campos)] = campos
    escribir_imagen(salida, tambor, ocupados, reserva_kv)


if __name__ == "__main__":
    cli = argparse.ArgumentParser(description=__doc__)
    cli.add_argument("checkpoint", type=Path)
    cli.add_argument("tambor", type=Path)
    args = cli.parse_args()
    exportar(args.checkpoint, args.tambor)
