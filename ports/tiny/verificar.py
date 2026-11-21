"""Compara respuestas del port con una referencia PyTorch de la variante desplegada."""
import argparse
import math
from pathlib import Path
import subprocess
import sys
import tempfile

import torch
from exportar import escala_entera


PREGUNTAS = (
    "what is the capital of peru",
    "what is the capital of france",
    "how many legs does a spider have",
    "2+2",
    "what is the opposite of hot",
    "what language is spoken in brazil",
)


def es_compacto(modelo):
    return modelo.cfg["L"] == 3 and modelo.cfg["quant"] == "3bit_seq" and modelo.cfg["HID"] == 224


def cache_fp20(x, bits):
    """Como la Mercury compacta: escala FP20 y redondeo de magnitud."""
    limite = 2 ** (bits - 1) - 1
    mantisa, exponente = torch.frexp(x.abs().amax(-1, keepdim=True).clamp_min(1e-8) / limite)
    escala = torch.ldexp((mantisa * 512).floor() / 512, exponente)
    niveles = ((x.abs() / escala) + 0.5).floor().clamp_max(limite)
    return niveles * x.sign() * escala


@torch.no_grad()
def preparar(modelo, empaquetado):
    compacto = es_compacto(modelo)
    modelo.cfg["ctx"] = 60 if compacto else 80
    if not empaquetado:
        return
    modelo.cfg["act_bits"] = 0
    bits_kv = 4 if compacto else 5
    modelo.cfg["kv_bits"] = bits_kv
    for bloque in modelo.blocks:
        bloque.cfg["kv_bits"] = bits_kv
        for capa in bloque.children():
            if hasattr(capa, "act_bits"):
                capa.act_bits = 0
            if compacto and getattr(capa, "alpha", None) is not None:
                alpha = capa.alpha.abs().clamp_min(1e-7)
                niveles = (4 * (capa.weight / alpha[:, None]).clamp(-1, 1)).floor().clamp(-4, 3) + 0.5
                e, enteros = escala_entera(alpha.detach().cpu().numpy(), 10)
                exponente = (e + 512) % 1024 - 512 - 9
                capa.alpha.copy_(torch.as_tensor(enteros, device=capa.alpha.device) * 2.0 ** exponente)
                capa.weight.copy_((niveles / 4) * capa.alpha[:, None])
                capa.quant = "float"
    for nombre, tensor in modelo.named_parameters():
        es_tensor = nombre in ("emb.weight", "loop_emb") or nombre.endswith(".g")
        if compacto and (es_tensor or nombre.endswith(".bias")):
            origen = tensor.detach().cpu().numpy()
            e, enteros = escala_entera(origen, 9 if nombre == "emb.weight" else 10)
            exponente = (e + 512) % 1024 - 512 - 9
            tensor.copy_(torch.as_tensor(enteros, device=tensor.device) * 2.0 ** exponente)
        elif es_tensor:
            maximo = float(tensor.abs().max())
            escala = 2 ** math.ceil(math.log2(maximo / 511)) if maximo else 1.0
            tensor.copy_(torch.round(tensor / escala) * escala)


def main():
    cli = argparse.ArgumentParser(description=__doc__)
    cli.add_argument("tiny68k", type=Path)
    cli.add_argument("checkpoint", type=Path)
    cli.add_argument("motor", type=Path)
    cli.add_argument("tambor", type=Path)
    cli.add_argument("--nativo", action="store_true",
                     help="usa clementina y la cinta de tele-input como argumentos")
    cli.add_argument("--mini", action="store_true", help="mide además ambas variantes en mini desde PyTorch")
    args = cli.parse_args()
    sys.path.insert(0, str(args.tiny68k.resolve() / "scripts"))
    import retro_qat

    torch.set_num_threads(1)
    original = retro_qat.load_retro(args.checkpoint).eval()
    compacto = es_compacto(original)
    preguntas = PREGUNTAS + ("name the capital of argentina",) if compacto else PREGUNTAS
    preparar(original, False)
    originales = retro_qat.answer_many_gpu(original, preguntas)
    if args.mini:
        retro_qat.apply_syllabus("mini")
        muestra = retro_qat.sel_set()
        original.to(retro_qat.DEV)
        base, _ = retro_qat.score(original, muestra)
    preparar(original, True)
    if compacto:
        retro_qat.kv_quant = cache_fp20
    if args.mini:
        puntuacion, _ = retro_qat.score(original, muestra)
        print(f"mini ({len(muestra)} preguntas), PyTorch: checkpoint {base:.2f}%, desplegado {puntuacion:.2f}%", flush=True)
    referencia = retro_qat.answer_many_gpu(original, preguntas)
    iguales = 0
    with tempfile.TemporaryDirectory() as directorio:
        datos = Path(directorio) / "pregunta.txt"
        for pregunta in preguntas:
            if args.nativo:
                datos.write_text(pregunta + '\n', encoding="utf-8")
                argumentos = ["tele-input", str(args.tambor.resolve()), "--cinta", str(datos)]
            else:
                argumentos = [str(args.tambor.resolve()), pregunta]
            proceso = subprocess.run(
                [str(args.motor.resolve()), *argumentos],
                capture_output=True, text=True, encoding="utf-8",
            )
            if proceso.returncode:
                raise RuntimeError(f"Clementina se detuvo con {pregunta!r}:\n{proceso.stderr}")
            salida = proceso.stdout.splitlines()[1]
            if args.nativo:
                salida = salida.lower()
            oracle = referencia[pregunta][:48]
            iguales += salida == oracle
            print(f"{pregunta!r}: Clementina={salida!r}, referencia={oracle!r}, checkpoint={originales[pregunta]!r}",
                  flush=True)
    print(f"{iguales}/{len(preguntas)} respuestas idénticas a la variante desplegada")
    return int(iguales != len(preguntas))


if __name__ == "__main__":
    raise SystemExit(main())
