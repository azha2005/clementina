//! Página HTML que reproduce una corrida en tiempo real: el sonido del
//! parlante (el WAV de `--wav`) y, sincronizado con él, lo que sale de la
//! perforadora tal como lo imprimiría el teleimpresor.

use mercury_core::tape::{self, Shift};

/// Lo que imprime el teleimpresor por cada carácter de cinta, siguiendo el
/// registro de cifras o letras (igual que `tape::decode`).
pub fn texto_por_caracter(cinta: &[u16]) -> Vec<String> {
    let mut shift = Shift::Figures;
    cinta
        .iter()
        .map(|&c| match c & 0x1f {
            tape::FIG_SHIFT => {
                shift = Shift::Figures;
                String::new()
            }
            tape::LET_SHIFT => {
                shift = Shift::Letters;
                String::new()
            }
            tape::ERASE | tape::CR => String::new(),
            tape::LF if shift == Shift::Figures => "\n".into(),
            v => {
                let t = if shift == Shift::Figures { &tape::FIGURES } else { &tape::LETTERS };
                t[v as usize].map(String::from).unwrap_or_default()
            }
        })
        .collect()
}

fn json_texto(s: &str) -> String {
    let mut o = String::from("\"");
    for ch in s.chars() {
        match ch {
            '"' => o.push_str("\\\""),
            '\\' => o.push_str("\\\\"),
            '\n' => o.push_str("\\n"),
            '<' => o.push_str("\\u003c"),
            c => o.push(c),
        }
    }
    o.push('"');
    o
}

fn html_texto(s: &str) -> String {
    s.replace('&', "&amp;").replace('<', "&lt;").replace('>', "&gt;").replace('"', "&quot;")
}

/// `perforado`: (instante de máquina en µs, carácter de cinta).
pub fn generar(titulo: &str, wav: Option<&str>, perforado: &[(u64, u16)], fin_us: u64) -> String {
    let codigos: Vec<u16> = perforado.iter().map(|p| p.1).collect();
    let textos = texto_por_caracter(&codigos);
    let mut eventos = String::from("[");
    for ((t, _), s) in perforado.iter().zip(&textos) {
        if s.is_empty() {
            continue;
        }
        if eventos.len() > 1 {
            eventos.push(',');
        }
        eventos.push_str(&format!("[{:.1},{}]", *t as f64 / 1000.0, json_texto(s)));
    }
    eventos.push(']');
    let audio = match wav {
        Some(w) => format!("<audio id=\"audio\" preload=\"auto\" src=\"{}\"></audio>", html_texto(w)),
        None => String::new(),
    };
    PLANTILLA
        .replace("{{TITULO}}", &html_texto(titulo))
        .replace("{{AUDIO}}", &audio)
        .replace("{{EVENTOS}}", &eventos)
        .replace("{{FIN}}", &format!("{:.1}", fin_us as f64 / 1000.0))
}

const PLANTILLA: &str = r##"<!DOCTYPE html>
<html lang="es">
<head>
<meta charset="utf-8">
<meta name="viewport" content="width=device-width, initial-scale=1">
<title>{{TITULO}} en Clementina</title>
<style>
:root {
  --papel: oklch(96% 0.012 92); --tinta: oklch(22% 0.01 80); --tenue: oklch(48% 0.01 80);
  --fondo: oklch(88% 0.006 95); --linea: oklch(80% 0.01 95); --acento: oklch(48% 0.12 152);
  --acento-tinta: oklch(98% 0.01 150);
}
@media (prefers-color-scheme: dark) {
  :root:not([data-theme="light"]) {
    --papel: oklch(25% 0.01 90); --tinta: oklch(92% 0.02 95); --tenue: oklch(70% 0.01 95);
    --fondo: oklch(17% 0.006 95); --linea: oklch(35% 0.01 95); --acento: oklch(70% 0.13 152);
    --acento-tinta: oklch(18% 0.02 150);
  }
}
* { box-sizing: border-box; }
body { margin: 0; background: var(--fondo); color: var(--tinta);
  font: 15px/1.4 system-ui, sans-serif; min-height: 100vh; display: flex; flex-direction: column; }
header { display: flex; gap: 16px; align-items: center; flex-wrap: wrap; padding: 12px 16px;
  border-bottom: 1px solid var(--linea); }
h1 { font-size: 1rem; margin: 0; font-weight: 600; }
.estado { color: var(--tenue); font-variant-numeric: tabular-nums; margin-left: auto; }
button { font: inherit; padding: 6px 14px; border-radius: 6px; border: 0; cursor: pointer;
  background: var(--acento); color: var(--acento-tinta); }
button:focus-visible { outline: 2px solid var(--tinta); outline-offset: 2px; }
main { flex: 1; overflow: auto; padding: 24px 16px; }
pre { margin: 0 auto; max-width: 80ch; min-height: 100%; padding: 24px 28px; background: var(--papel);
  font: 16px/1.25 Consolas, "Courier New", Courier, monospace; white-space: pre; overflow-x: auto;
  box-shadow: 0 1px 0 var(--linea); }
.cursor { display: inline-block; width: 0.6em; height: 1.1em; vertical-align: -0.15em; background: var(--tinta);
  animation: titila 1s steps(1) infinite; }
@keyframes titila { 50% { opacity: 0; } }
@media (prefers-reduced-motion: reduce) { .cursor { animation: none; } }
</style>
</head>
<body>
<header>
  <h1>{{TITULO}}</h1>
  <button id="boton" type="button">Tocar</button>
  <span class="estado" id="estado">0,0 s</span>
</header>
<main id="hoja"><pre id="papel"><span class="cursor"></span></pre></main>
{{AUDIO}}
<script>
const EVENTOS = {{EVENTOS}};
const FIN = {{FIN}};
const audio = document.getElementById('audio');
const papel = document.getElementById('papel');
const hoja = document.getElementById('hoja');
const boton = document.getElementById('boton');
const estado = document.getElementById('estado');
const cursor = papel.querySelector('.cursor');
let texto = papel.insertBefore(document.createTextNode(''), cursor);
let i = 0, inicio = null, tocando = false;
// Si el WAV no está o no se puede tocar, el texto sigue con el reloj.
let conAudio = !!audio;
if (audio) audio.addEventListener('error', () => { conAudio = false; });

function ahoraMs() {
  if (conAudio) return audio.currentTime * 1000;
  return inicio === null ? 0 : performance.now() - inicio;
}
function cuadro() {
  const t = ahoraMs();
  let nuevo = '';
  while (i < EVENTOS.length && EVENTOS[i][0] <= t) nuevo += EVENTOS[i++][1];
  if (nuevo) {
    texto.appendData(nuevo);
    hoja.scrollTop = hoja.scrollHeight;
  }
  estado.textContent = (t / 1000).toFixed(1).replace('.', ',') + ' s de ' + (FIN / 1000).toFixed(1).replace('.', ',');
  if (tocando && t < FIN + 500) requestAnimationFrame(cuadro);
  else { tocando = false; boton.textContent = 'Otra vez'; }
}
boton.addEventListener('click', () => {
  if (tocando) return;
  texto.data = ''; i = 0; tocando = true; boton.textContent = 'Tocando';
  inicio = performance.now();
  if (conAudio) {
    audio.currentTime = 0;
    audio.play().catch(() => { conAudio = false; inicio = performance.now(); });
  }
  requestAnimationFrame(cuadro);
});
</script>
</body>
</html>
"##;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn caracteres_con_registro() {
        let cinta = tape::encode("AB 1\nC").unwrap();
        let t = texto_por_caracter(&cinta);
        assert_eq!(t.concat(), tape::decode(&cinta));
        assert_eq!(t.len(), cinta.len());
    }
}
