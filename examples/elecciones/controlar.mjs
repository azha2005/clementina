// Control completo de elecciones.mer: corre en el emulador la ficha de cada
// registro de la cinta (el total del pais con la vista 1 y cada distrito con
// la vista 3) y compara los porcentajes y la participacion que perfora la
// maquina con los que publica el atlas de 257arg.
//
// Uso, desde la raiz del repositorio y con el binario compilado en release:
//   node examples/elecciones/generar_cinta.mjs --esperado esperado.json
//   node examples/elecciones/controlar.mjs esperado.json

import { execFileSync } from 'node:child_process';
import { mkdtempSync, readFileSync, writeFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { dirname, join, resolve } from 'node:path';
import { fileURLToPath } from 'node:url';

const AQUI = dirname(fileURLToPath(import.meta.url));
const REPO = resolve(AQUI, '../..');
const esperado = JSON.parse(readFileSync(process.argv[2], 'utf8'));
const cinta = readFileSync(join(AQUI, 'cinta.txt'), 'utf8');
const exe = join(REPO, 'target/release/clementina' + (process.platform === 'win32' ? '.exe' : ''));
const tmp = join(mkdtempSync(join(tmpdir(), 'elecciones-')), 'cinta.txt');

function correr(eleccion, vista, distrito) {
  writeFileSync(tmp, `${eleccion} ${vista} ${distrito}\n${cinta}`);
  return execFileSync(exe, ['autocode', join(AQUI, 'elecciones.mer'), '--cinta', tmp], { encoding: 'utf8', stdio: ['ignore', 'pipe', 'pipe'] });
}

let filas = 0, partic = 0;
const errores = [];
for (const e of esperado) {
  e.registros.forEach((r, s) => {
    const salida = correr(e.numero, s === 0 ? 1 : 3, s);
    const lineas = salida.split(/\r?\n/);
    const pct = lineas.map(l => l.match(/^ +\d+ +(\d+\.\d\d)  \S/)).filter(Boolean).map(m => Number(m[1]));
    const donde = `${e.clave} ${r.nombre}`;
    if (!lineas.some(l => l.trim() === r.nombre)) errores.push(`${donde}: no aparece el nombre`);
    if (pct.length !== r.pct.length) errores.push(`${donde}: ${pct.length} filas con porcentaje, el atlas tiene ${r.pct.length}`);
    r.pct.forEach((p, k) => {
      filas++;
      // El atlas redondea a dos decimales (algunos no redondean).
      if (pct[k] === undefined || Math.abs(pct[k] - p) > 0.0051) errores.push(`${donde}, fila ${k + 1}: maquina ${pct[k]}, atlas ${p}`);
    });
    const m = salida.match(/(\d+\.\d\d)  PARTICIPACION/);
    if (r.participacion != null && m) {
      partic++;
      // El atlas publica la participacion con uno o dos decimales.
      if (Math.abs(Number(m[1]) - r.participacion) > 0.051) errores.push(`${donde}: participacion ${m[1]}, atlas ${r.participacion}`);
    }
  });
}
console.log(`${esperado.length} elecciones, ${esperado.reduce((a, e) => a + e.registros.length, 0)} registros, ${filas} porcentajes y ${partic} participaciones comparados.`);
console.log(errores.length ? `Diferencias:\n${errores.join('\n')}` : 'Sin diferencias con el atlas.');
process.exitCode = errores.length ? 1 : 0;
