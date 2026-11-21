// Genera la cinta de datos de elecciones.mer a partir de los atlas de 257arg
// («Atlas electorales de Argentina 1904-2025»,
// https://github.com/azha2005/257arg). Lee los HTML con el mismo cargador del
// visor de terminal del proyecto (herramientas/atlas_terminal/atlas.mjs), así
// que las cifras son las del cartel nacional (mode.national) y las fichas
// provinciales (mode.prov) del atlas, sin recalcular nada.
//
// Uso (Node 18 o posterior, desde cualquier carpeta):
//   node examples/elecciones/generar_cinta.mjs [RAIZ_257ARG] [--todas] [--esperado ARCHIVO]
// RAIZ_257ARG es la carpeta del repositorio 257arg (por omisión, la variable
// de entorno ARG257 o E:\dataeleccproyecto). Escribe examples/elecciones/cinta.txt
// y examples/elecciones/catalogo.txt, y muestra el control de porcentajes.
//
// Formato de la cinta (todo número termina en espacio o fin de línea; un
// texto va al final de la línea, después de un solo espacio, y llega hasta
// el CR):
//   E TÍTULO                 número de elección (1, 2, …) y título
//   N SUBTÍTULO              cantidad de distritos y fuente
//   registro nacional y luego N registros de distrito:
//     L M D I V W NOMBRE     L filas de partido, M métricas, D denominador
//                            de los porcentajes, I inscriptos, V votantes
//                            (-1 si no hay dato; L = 0 es «sin dato»), W
//                            símbolo del ganador en el mapa
//     L filas: VOTOS NOMBRE
//     M filas: VALOR RÓTULO
//   mapa:
//     F                      filas del mapa
//     F filas: T L1 V1 … LT VT   T tramos de L celdas de valor V: 0 afuera,
//                            1..N relleno del distrito, 100 + código un
//                            carácter fijo (borde o número del distrito)
//     Z                      líneas de la leyenda
//     Z líneas: CÓDIGO NOMBRE    símbolo y su nombre
//   Los códigos son Ferranti; 32 + código es el mismo carácter en registro
//   de letras.
//   0                        fin de la cinta
// La consulta (elección, vista, distrito) se perfora delante; ver LEEME.md.

import { writeFileSync } from 'node:fs';
import { dirname, resolve } from 'node:path';
import { fileURLToPath, pathToFileURL } from 'node:url';

const AQUI = dirname(fileURLToPath(import.meta.url));
const args = process.argv.slice(2);
const iEsp = args.indexOf('--esperado');
// --esperado ARCHIVO.json guarda los porcentajes del atlas de cada registro,
// para comparar con lo que perfora la máquina (ver LEEME.md).
const ESPERADO = iEsp >= 0 ? args.splice(iEsp, 2)[1] : null;
const iTodas = args.indexOf('--todas');
const TODAS = iTodas >= 0 && args.splice(iTodas, 1).length > 0;
const RAIZ = resolve(args[0] || process.env.ARG257 || 'E:/dataeleccproyecto');
const A = await import(pathToFileURL(resolve(RAIZ, 'herramientas/atlas_terminal/atlas.mjs')));

// Atlas que entran: de 1904 a 1962, en el orden del catálogo de 257arg.
// Se deja afuera el atlas ficticio de 1916 (1916fic).
const EXCLUIDOS = new Set(['1916fic']);

// Filas de las fichas que no son partidos: van como métricas.
const NO_PARTIDO = /(^|_)(BLA|NUL|ANU|IMP|DIF)$/;
// «Otros» cambia de sentido según el atlas: a veces son otros partidos (su
// porcentaje cierra con el mismo denominador) y a veces blancos y otros sobre
// los votantes. Lo decide el control de porcentajes.
const OTROS = /(^|_)OTR$/;

// Teleimpresor Ferranti (CS 225 A, Apéndice VI): mayúsculas, cifras y unos
// pocos signos. Sin tildes ni eñes.
const PERMITIDOS = /^[A-Z0-9 .,()\-/+=?]*$/;
function texto(s) {
  let t = String(s ?? '')
    .normalize('NFD').replace(/[\u0300-\u036f]/g, '')
    .toUpperCase()
    .replace(/[«»"“”'’`]/g, '')
    .replace(/[·;:]/g, ',')
    .replace(/&/g, 'Y')
    .replace(/%/g, ' POR CIENTO')
    .replace(/[^A-Z0-9 .,()\-/+=?]/g, ' ')
    .replace(/ +/g, ' ')
    .replace(/ ,/g, ',')
    .trim();
  if (!PERMITIDOS.test(t)) throw new Error(`texto no perforable: ${s}`);
  return t;
}

// «10.002.327» → 10002327. Lo que no es un entero se omite (los porcentajes
// los calcula la máquina).
function entero(v) {
  if (typeof v === 'number') return Number.isInteger(v) ? v : null;
  const t = String(v).trim();
  return /^\d{1,3}(\.\d{3})*$|^\d+$/.test(t) ? Number(t.replaceAll('.', '')) : null;
}


// Elige el denominador con el que los votos reproducen los porcentajes del
// atlas: una métrica de votos positivos o válidos, o la suma de las filas.
// Devuelve también qué filas «otros» cierran con ese denominador (quedan como
// partido) y cuáles no (pasan a las métricas).
function denominador(filas, metricas) {
  const fijas = filas.filter(f => !f.otros), otros = filas.filter(f => f.otros);
  const conPct = fijas.filter(f => typeof f.pct === 'number');
  const suma = l => l.reduce((a, f) => a + f.votos, 0);
  const candidatos = [];
  const preferida = /positivos|v[aá]lidos|a candidatos|publicados/i;
  for (const [rot, val] of metricas) if (preferida.test(rot) && entero(val)) candidatos.push([entero(val), rot]);
  candidatos.push([suma(fijas), 'suma de las filas']);
  if (otros.length) candidatos.push([suma(filas), 'suma de las filas con otros']);
  for (const [rot, val] of metricas) if (!preferida.test(rot) && entero(val)) candidatos.push([entero(val), rot]);
  // El atlas publica el porcentaje redondeado a dos decimales (o sin redondear).
  const cierra = d => f => typeof f.pct === 'number' && Math.abs(100 * f.votos / d - f.pct) <= 0.00501;
  for (const [d, origen] of candidatos) {
    if (d > 0 && conPct.every(cierra(d))) {
      return { d, origen, ok: true, partidos: filas.filter(f => !f.otros || cierra(d)(f)), aparte: otros.filter(f => !cierra(d)(f)) };
    }
  }
  // Ninguno reproduce todo: se usa el primero y se informa.
  const [d, origen] = candidatos[0];
  const malas = conPct.filter(f => !cierra(d)(f));
  return { d, origen, ok: false, malas, partidos: fijas, aparte: otros };
}

function registro(nombre, filasAtlas, metricasAtlas, cand, nacional) {
  const todas = [], extra = [];
  for (const [code, votos, pct] of filasAtlas) {
    const c = cand[code] || {};
    let n = c.n || code;
    if (nacional && c.sub) n += ` (${c.sub})`;
    if (NO_PARTIDO.test(code)) extra.push([votos, n]);
    else todas.push({ votos, pct, nombre: texto(n), otros: OTROS.test(code) });
  }
  const metricas = metricasAtlas || [];
  const den = todas.length ? denominador(todas, metricas) : { d: -1, origen: '', ok: true, partidos: [], aparte: [] };
  // Los «otros» van al final, para que la primera fila sea siempre el ganador.
  const filas = [...den.partidos.filter(f => !f.otros), ...den.partidos.filter(f => f.otros)];
  for (const f of den.aparte) extra.push([f.votos, f.nombre]);
  const buscar = re => entero(metricas.find(m => re.test(m[0]))?.[1]);
  const ins = buscar(/^inscriptos$/i) ?? -1;
  const vot = buscar(/^votantes$/i) ?? -1;
  const lineasMet = [];
  for (const [rot, val] of metricas) {
    if (/^participaci/i.test(rot)) continue; // la calcula la máquina
    const v = entero(val);
    if (v == null) continue;
    lineasMet.push([v, texto(rot)]);
  }
  for (const [v, n] of extra) {
    if (!lineasMet.some(([, r]) => r === texto(n))) lineasMet.push([v, texto(n)]);
  }
  const part = metricas.find(m => /^participaci/i.test(m[0]))?.[1];
  const participacion = part == null ? null : Number(String(part).replace(/[^\d,]/g, '').replace(',', '.'));
  return { nombre: texto(nombre), filas, metricas: lineasMet, den, ins, vot, participacion };
}

// Código Ferranti (CS 225 A, Apéndice VI). En el mapa, un símbolo en
// registro de letras se anota como 32 + su código; la máquina perfora el
// cambio de registro sólo cuando hace falta.
const CIFRA = [16, 1, 2, 19, 4, 21, 22, 7, 8, 25];
const LETRAS = 32;
const letra = l => LETRAS + l.charCodeAt(0) - 64;
const SIN_DATO = 11;          // «-»
const OTRA = 3;               // «*»
const BORDE = LETRAS + 28;    // «.», que también existe en letras

// Inicial con la que se pinta cada fuerza del total del país: la primera
// letra de la última palabra larga del nombre (FRENTE JUSTICIALISTA → J,
// UCR DEL PUEBLO → P); si ya está tomada, la de otra palabra o la siguiente
// letra libre del nombre.
function iniciales(nombres) {
  const usadas = new Set();
  return nombres.map(n => {
    const t = texto(n);
    const palabras = t.split(/[^A-Z]+/).filter(w => w.length > 3).reverse();
    const opciones = [...palabras.map(w => w[0]), ...t.replace(/[^A-Z]/g, ''), ...'ABCDEFGHIJKLMNOPQRSTUVWXYZ'];
    const l = opciones.find(x => !usadas.has(x));
    usadas.add(l);
    return l;
  });
}

// Mapa por provincia, como la vista inicial del visor de terminal: cada
// departamento del atlas toma el distrito de su provincia. Las celdas donde
// cambia el distrito (a la derecha o abajo) son borde, y cada distrito lleva
// su número de la vista 2 en la celda interior más cercana a su centro.
// Supuesto: celdas el doble de altas que anchas, la relación por omisión del
// visor; 56 columnas entran en el renglón del teleimpresor.
const ANCHO = 56, ASPECTO = 2;
function mapa(at, modo, nombresProv, simbolo, nacCands, cand, letrasNac) {
  const formas = at.shapes.filter(s => !/^ant[aá]rtida/i.test(at.data.territories[s.id].n));
  const [, , bw, bh] = at.box;
  const alto = Math.round(ANCHO * bh / bw / ASPECTO);
  const grilla = A.rasterize(formas, at.box, ANCHO, alto);
  const dist = (f, c) => {
    if (f < 0 || f >= alto || c < 0 || c >= ANCHO) return 0;
    const id = grilla[f * ANCHO + c];
    return id < 0 ? 0 : nombresProv.indexOf(at.data.territories[id].p) + 1;
  };
  // Valor de cada celda en la cinta: 0 afuera, 1..N relleno del distrito,
  // 100 + código para un carácter fijo (borde o número).
  const celda = [];
  for (let f = 0; f < alto; f++) {
    celda.push([]);
    for (let c = 0; c < ANCHO; c++) {
      const d = dist(f, c);
      const borde = d && ((dist(f, c + 1) && dist(f, c + 1) !== d) || (dist(f + 1, c) && dist(f + 1, c) !== d));
      celda[f].push(borde ? 100 + BORDE : d);
    }
  }
  nombresProv.forEach((_, k) => {
    const d = k + 1, rotulo = String(d);
    // La parte conexa más grande del interior (Tierra del Fuego e islas,
    // por ejemplo, no se promedian).
    let interior = [];
    const visto = new Set();
    for (let f0 = 0; f0 < alto; f0++) for (let c0 = 0; c0 < ANCHO; c0++) {
      if (celda[f0][c0] !== d || visto.has(f0 * ANCHO + c0)) continue;
      const parte = [], pila = [[f0, c0]];
      visto.add(f0 * ANCHO + c0);
      while (pila.length) {
        const [f, c] = pila.pop();
        parte.push([f, c]);
        for (const [g, h] of [[f + 1, c], [f - 1, c], [f, c + 1], [f, c - 1]]) {
          if (g >= 0 && g < alto && h >= 0 && h < ANCHO && celda[g][h] === d && !visto.has(g * ANCHO + h)) {
            visto.add(g * ANCHO + h);
            pila.push([g, h]);
          }
        }
      }
      if (parte.length > interior.length) interior = parte;
    }
    if (!interior.length) return;
    const fm = interior.reduce((a, [f]) => a + f, 0) / interior.length;
    const cm = interior.reduce((a, [, c]) => a + c, 0) / interior.length;
    let mejor = null;
    for (const [f, c] of interior) {
      if (![...rotulo].every((_, i) => celda[f][c + i] === d)) continue;
      const dd = (ASPECTO * (f - fm)) ** 2 + (c + (rotulo.length - 1) / 2 - cm) ** 2;
      if (!mejor || dd < mejor[0]) mejor = [dd, f, c];
    }
    if (mejor) [...rotulo].forEach((ch, i) => { celda[mejor[1]][mejor[2] + i] = 100 + CIFRA[+ch]; });
  });
  const filas = celda.map(fila => {
    const v = [...fila];
    while (v.length && v.at(-1) === 0) v.pop();
    // Tramos: LARGO VALOR.
    const tramos = [];
    for (const x of v) {
      if (tramos.length && tramos.at(-1)[1] === x) tramos.at(-1)[0]++;
      else tramos.push([1, x]);
    }
    return `${tramos.length} ${tramos.flat().join(' ')}`.trim();
  });
  const usados = new Set(simbolo);
  const leyenda = [];
  nacCands.forEach((c, n) => {
    const l = letra(letrasNac[n]);
    if (usados.has(l)) leyenda.push(`${l} ${texto(cand[c.code]?.n || c.code)}`);
  });
  if (usados.has(OTRA)) leyenda.push(`${OTRA} OTRAS FUERZAS`);
  if (usados.has(SIN_DATO)) leyenda.push(`${SIN_DATO} SIN DATO`);
  leyenda.push(`${BORDE} LIMITE ENTRE DISTRITOS`);
  leyenda.push(`0 LOS NUMEROS SON LOS DE LA VISTA 2`);
  return [String(alto), ...filas, String(leyenda.length), ...leyenda];
}

// Cinta corta por omisión: 1958, 1962 y 1965. Con --todas, el catálogo completo.
// El número de cada elección es su lugar en el catálogo completo, así no
// cambia al sumar años.
const CLAVES = TODAS ? null : new Set(['1958', '1962', '1965']);
const entradas = A.catalog(RAIZ).filter(e => +e.year >= 1900 && +e.year <= 1965 && !EXCLUIDOS.has(e.key));
const cinta = [], catalogo = [], control = [], esperado = [];
let numero = 0;
for (const e of entradas) {
  numero++;
  if (CLAVES && !CLAVES.has(e.key)) continue;
  const at = A.loadAtlas(e, RAIZ);
  const modo = A.modeOf(at, 'gan');
  const cand = at.data.candidates;
  const set = Object.values(at.data.sets || {})[0] || {};
  const titulo = texto(`${e.year} ${e.title}`);
  const subtitulo = texto(set.subtitle || '');
  const nacCands = modo.national?.cands || [];
  const nac = registro('Total del pais', nacCands.map(c => [c.code, c.votes, c.pct]), modo.national?.metrics, cand, true);
  const nombresProv = Object.keys(modo.prov || {});
  const provs = nombresProv.map(p => registro(p, modo.prov[p].lines || [], modo.prov[p].metrics, cand, false));
  // Símbolo del ganador de cada distrito en el mapa: la inicial de su fuerza
  // en el total del país, «*» si ganó otra fuerza y «-» si no hay dato. Va
  // en la cinta en código Ferranti.
  const letrasNac = iniciales(nacCands.map(c => cand[c.code]?.n || c.code));
  const simbolo = nombresProv.map((p, k) => {
    if (!provs[k].filas.length) return SIN_DATO;
    const n = nacCands.findIndex(c => c.code === modo.prov[p].w);
    return n >= 0 ? letra(letrasNac[n]) : OTRA;
  });
  cinta.push(`${numero} ${titulo}`, `${provs.length} ${subtitulo}`);
  [nac, ...provs].forEach((r, s) => {
    cinta.push(`${r.filas.length} ${r.metricas.length} ${r.den.d} ${r.ins} ${r.vot} ${s ? simbolo[s - 1] : 0} ${r.nombre}`);
    for (const f of r.filas) cinta.push(`${f.votos} ${f.nombre}`);
    for (const [v, rot] of r.metricas) cinta.push(`${v} ${rot}`);
    if (!r.den.ok) control.push(`${e.key} ${r.nombre}: ningún denominador reproduce todos los porcentajes (${r.den.origen}): ${r.den.malas.map(f => `${f.nombre} ${f.pct}`).join('; ')}`);
  });
  cinta.push(...mapa(at, modo, nombresProv, simbolo, nacCands, cand, letrasNac));
  esperado.push({ numero, clave: e.key, registros: [nac, ...provs].map(r => ({ nombre: r.nombre, pct: r.filas.map(f => f.pct), participacion: r.participacion })) });
  catalogo.push(`${String(numero).padStart(2)}  ${e.key.padEnd(8)} ${titulo}`);
  const sinDato = provs.filter(r => !r.filas.length).length;
  console.log(`${String(numero).padStart(2)} ${e.key.padEnd(8)} ${provs.length} distritos (${sinDato} sin dato), denominador nacional: ${nac.den.origen}`);
}
cinta.push('0');
writeFileSync(resolve(AQUI, 'cinta.txt'), cinta.join('\n') + '\n');
writeFileSync(resolve(AQUI, 'catalogo.txt'), catalogo.join('\n') + '\n');
if (ESPERADO) writeFileSync(resolve(ESPERADO), JSON.stringify(esperado));
console.log(`\ncinta.txt: ${cinta.length} líneas, ${cinta.join('\n').length} caracteres`);
if (control.length) console.log('\nControl de porcentajes:\n' + control.join('\n'));
else console.log('Control de porcentajes: todos los porcentajes del atlas salen de votos / denominador.');
