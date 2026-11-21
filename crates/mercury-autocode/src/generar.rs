//! Generación de código Mercury a partir de las sentencias analizadas.
//!
//! Convenciones del código generado (supuestas; el manual no las da):
//! * B1 sirve para los sufijos variables (`xi`) y para pasar la dirección del
//!   divisor; B2 lleva la dirección de vuelta de las rutinas; las
//!   expresiones de índices se calculan en el sac (B7). B3 y B4 los usan las
//!   rutinas. B5 y B6 quedan para el programador.
//! * El largo 32 guarda la suma parcial cuando hay más de un término
//!   compuesto; las rutinas usan 34, 36 y 38.
//! * Un índice pasa a número largo poniéndolo en la mitad alta de la
//!   fracción de un largo con exponente 9 (largo 20); un número pasa a
//!   índice con la instrucción 78 (exponente 29: la parte entera queda en la
//!   mitad baja) y el largo 21.

use crate::runtime::{mapa, Rutina, Superposicion};
use crate::sintaxis::*;
use mercury_core::asm;
use mercury_core::opcodes::{self, AddrType};
use std::collections::{BTreeMap, BTreeSet, HashMap};

#[derive(Debug, Clone, PartialEq)]
pub struct ErrorCompilacion {
    pub linea: usize,
    pub msg: String,
}

impl std::fmt::Display for ErrorCompilacion {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        if self.linea > 0 {
            write!(f, "línea {}: {}", self.linea, self.msg)
        } else {
            write!(f, "{}", self.msg)
        }
    }
}

type R<T> = Result<T, ErrorCompilacion>;

/// Celda corta de trabajo para sumas de índices (registro 3+).
const CELDA_TEMP: usize = 7;
const CR: i32 = 30;
const LF: i32 = 13;
const SP: i32 = 14;

/// Operando de una instrucción, antes de resolver direcciones.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Op {
    Lit(i32),
    Corto(usize),
    Largo(u16),
    Medio(u16),
    /// Etiqueta (interna o del programador) como dirección de salto.
    Etq(u32),
    /// Dirección de una etiqueta como literal (para `102 vuelta`).
    EtqLit(u32),
    /// Etiqueta `m` del capítulo `c`, como literal.
    EtqCap(u32, u32),
    /// Constante larga de la zona de constantes.
    Const(usize),
    /// Dirección (largo) de una constante, como literal.
    ConstLit(usize),
    /// Constante corta (número k de la lista de cortas del capítulo).
    ConstCorta(usize),
    /// Tabla de etiquetas, para `n) = m)`.
    Tabla,
    /// Entrada de una rutina enlazada.
    /// Rutina enlazada y etiqueta de su entrada.
    Rutina(Rutina, u32),
    /// Palabra ya armada (instrucción de máquina convencional).
    Palabra(u32),
}

#[derive(Debug, Clone)]
pub struct Ins {
    pub dec: u8,
    pub b: u8,
    pub op: Op,
    pub linea: usize,
}

/// Dirección de una variable.
#[derive(Debug, Clone, Copy)]
enum DirVar {
    Fija(u16),
    /// base + índice (el índice va a B1)
    Indexada(u16, char),
}

/// Capítulo compilado, antes de resolver direcciones.
pub struct Capitulo {
    pub numero: u32,
    pub codigo: Vec<Ins>,
    /// etiqueta → posición en `codigo`
    pub etiquetas: HashMap<u32, usize>,
    pub constantes: Vec<u64>,
    pub cortas: Vec<i32>,
    pub usa_tabla: bool,
    pub rutinas: BTreeSet<Rutina>,
    pub superposiciones: BTreeSet<Superposicion>,
    pub psa: Vec<usize>,
    pub reserva: Vec<(char, u16, u16)>,
}

/// Imagen ensamblada de un capítulo.
pub struct CapituloArmado {
    pub numero: u32,
    /// Registros cortos de las páginas 2 a 14 (índice 0 = corto 256).
    pub paginas: Vec<u16>,
    pub etiquetas: HashMap<u32, u16>,
    pub libres: usize,
    pub listado: String,
    pub psa: Vec<usize>,
}

struct Gen {
    cap: Capitulo,
    linea: usize,
    proxima_interna: u32,
    ciclos: Vec<(char, Entero, bool, Entero, u32, usize)>,
    /// letra → (base, tamaño)
    grupos: BTreeMap<char, (u16, u16)>,
    siguiente_libre: u16,
    etiquetas_pendientes: Vec<u32>,
    entradas: Entradas,
    conserva_b2: bool,
    doble_longitud: bool,
}

/// Entradas de la rutina de cambio de capítulo (página 1).
#[derive(Debug, Clone, Copy)]
pub struct Entradas {
    pub across: u16,
    pub down: u16,
    pub up: u16,
}

pub fn letra_especial(l: char, primada: bool) -> u16 {
    let k = LETRAS_VARIABLES.iter().position(|c| *c == l).unwrap() as u16;
    if l == 'π' {
        return mapa::PI;
    }
    if primada {
        mapa::ESPECIALES + k
    } else {
        mapa::ESPECIALES + 14 + k
    }
}

fn corto_indice(c: char) -> usize {
    mapa::INDICES + (c as usize - 'i' as usize)
}

impl Gen {
    fn err<T>(&self, msg: impl Into<String>) -> R<T> {
        Err(ErrorCompilacion { linea: self.linea, msg: msg.into() })
    }

    fn e(&mut self, dec: u8, b: u8, op: Op) {
        let pos = self.cap.codigo.len();
        for l in self.etiquetas_pendientes.drain(..) {
            self.cap.etiquetas.insert(l, pos);
        }
        self.cap.codigo.push(Ins { dec, b, op, linea: self.linea });
    }

    fn nueva(&mut self) -> u32 {
        self.proxima_interna += 1;
        self.proxima_interna
    }

    fn poner(&mut self, etq: u32) {
        self.etiquetas_pendientes.push(etq);
    }

    fn constante(&mut self, v: f64) -> usize {
        let bits = v.to_bits();
        if let Some(k) = self.cap.constantes.iter().position(|b| *b == bits) {
            return k;
        }
        self.cap.constantes.push(bits);
        self.cap.constantes.len() - 1
    }

    fn corta(&mut self, v: i32) -> usize {
        if let Some(k) = self.cap.cortas.iter().position(|b| *b == v) {
            return k;
        }
        self.cap.cortas.push(v);
        self.cap.cortas.len() - 1
    }

    fn reservar(&mut self, l: char, n: i32) -> R<()> {
        if n < 0 {
            return self.err("el tamaño de un grupo no puede ser negativo");
        }
        if n >= i32::from(mapa::N_PRINCIPALES - self.siguiente_libre) {
            return self.err(format!("las variables principales pasan de {}", mapa::N_PRINCIPALES));
        }
        let tam = n as u16 + 1;
        let base = mapa::PRINCIPALES + self.siguiente_libre;
        self.siguiente_libre += tam;
        self.grupos.insert(l, (base, tam));
        self.cap.reserva.push((l, base, tam));
        Ok(())
    }

    fn dir_var(&self, v: &Variable) -> R<DirVar> {
        Ok(match *v {
            Variable::Especial(l) => DirVar::Fija(letra_especial(l, false)),
            Variable::Primada(l) => DirVar::Fija(letra_especial(l, true)),
            Variable::Principal(l, suf) => {
                let Some(&(base, _)) = self.grupos.get(&l) else {
                    return self.err(format!("variable {l} sin reservar (falta «{l} → n»)"));
                };
                match suf {
                    Sufijo::Fijo(n) => DirVar::Fija((base as i32 + n).rem_euclid(1024) as u16),
                    Sufijo::Indice(c, d) => DirVar::Indexada((base as i32 + d).rem_euclid(1024) as u16, c),
                }
            }
        })
    }

    /// Instrucción de acumulador `dec` sobre una variable.
    fn op_var(&mut self, dec: u8, v: &Variable) -> R<()> {
        match self.dir_var(v)? {
            DirVar::Fija(a) => self.e(dec, 0, Op::Largo(a)),
            DirVar::Indexada(base, c) => {
                self.e(0, 1, Op::Corto(corto_indice(c)));
                self.e(dec, 1, Op::Largo(base));
            }
        }
        Ok(())
    }

    /// Índice al largo de conversión (sin estandarizar).
    fn indice_a_conv(&mut self, c: char) {
        self.e(0, 1, Op::Corto(corto_indice(c)));
        self.e(1, 1, Op::Corto(mapa::CONV_ALTO));
    }

    /// Instrucción de acumulador `dec` (40, 42–45, 50–53) sobre un factor.
    fn op_factor(&mut self, dec: u8, f: &Factor) -> R<()> {
        match f {
            Factor::Numero(x) => {
                let k = self.constante(*x);
                self.e(dec, 0, Op::Const(k));
            }
            Factor::Var(v) => self.op_var(dec, v)?,
            Factor::Indice(c) => {
                self.indice_a_conv(*c);
                self.e(dec, 0, Op::Largo(mapa::CONV));
            }
        }
        Ok(())
    }

    /// Estandariza A, que viene de convertir un índice (exponente 9) o de la
    /// instrucción 78 (exponente 28 o 29).
    fn estandarizar(&mut self) {
        self.e(44, 0, Op::Largo(mapa::CERO_EXP0));
    }

    fn negar(&mut self) {
        self.e(52, 0, Op::Largo(mapa::MENOS_UNO));
    }

    fn llamar(&mut self, entrada: Op) {
        let vuelta = self.nueva();
        if self.conserva_b2 {
            self.e(1, 2, Op::Corto(mapa::VUELTA_SUBSECUENCIA));
        }
        self.e(10, 2, Op::EtqLit(vuelta));
        self.e(59, 0, entrada);
        self.poner(vuelta);
        if self.conserva_b2 {
            self.e(0, 2, Op::Corto(mapa::VUELTA_SUBSECUENCIA));
        }
    }

    /// Calcula un término compuesto en A, negado si `negar`.
    ///
    /// Los índices y los enteros se multiplican primero y sin redondeo, lo que
    /// es exacto; el producto queda estandarizado antes de tocar las
    /// variables. Un índice recién convertido no está estandarizado, y una
    /// operación redondeada sobre él agrandaría el 1 forzado al correrlo.
    fn termino(&mut self, t: &Termino, negar: bool, redondeo: bool) -> R<()> {
        let mul = if redondeo { 50 } else { 52 };
        // Un divisor índice se prepara antes, en el largo 38.
        if let Some(Factor::Indice(c)) = &t.divisor {
            self.indice_a_conv(*c);
            self.e(40, 0, Op::Largo(mapa::CONV));
            self.estandarizar();
            self.e(41, 0, Op::Largo(mapa::TRABAJO[3]));
        }
        let entero = |f: &&Factor| matches!(f, Factor::Indice(_)) || matches!(f, Factor::Numero(x) if x.fract() == 0.0);
        let (enteros, otros): (Vec<&Factor>, Vec<&Factor>) = t.factores.iter().partition(entero);
        let orden: Vec<(&Factor, bool)> =
            enteros.iter().map(|f| (*f, true)).chain(otros.iter().map(|f| (*f, false))).collect();
        let n = orden.len();
        let mut negado = false;
        for (k, (f, es_entero)) in orden.iter().enumerate() {
            let ultimo = k == n - 1;
            if k == 0 {
                self.op_factor(40, f)?;
            } else {
                let base = if *es_entero { 52 } else { mul };
                let dec = if negar && ultimo {
                    negado = true;
                    base + 1
                } else {
                    base
                };
                self.op_factor(dec, f)?;
            }
            // Un índice solo, recién cargado: estandarizarlo.
            if k == 0 && enteros.len() == 1 && matches!(f, Factor::Indice(_)) {
                self.estandarizar();
            }
        }
        if let Some(d) = &t.divisor {
            match d {
                Factor::Numero(x) => {
                    let k = self.constante(*x);
                    self.e(10, 1, Op::ConstLit(k));
                }
                Factor::Var(v) => match self.dir_var(v)? {
                    DirVar::Fija(a) => self.e(10, 1, Op::Lit(a as i32)),
                    DirVar::Indexada(base, c) => {
                        self.e(0, 1, Op::Corto(corto_indice(c)));
                        self.e(12, 1, Op::Lit(base as i32));
                    }
                },
                Factor::Indice(_) => self.e(10, 1, Op::Lit(mapa::TRABAJO[3] as i32)),
            }
            self.llamar(Op::Medio(mapa::DIVISION));
        }
        if negar && !negado {
            self.negar();
        }
        Ok(())
    }

    /// Calcula una expresión en A. Devuelve verdadero si el resultado puede
    /// no estar estandarizado (un índice solo).
    fn expr(&mut self, e: &Expr, redondeo: bool) -> R<bool> {
        let suma = if redondeo { 42 } else { 44 };
        // Un índice solo también va como compuesto: hay que estandarizarlo en A
        // antes de sumarlo.
        let compuesto = |t: &Termino| {
            t.factores.len() > 1 || t.divisor.is_some() || matches!(t.factores[0], Factor::Indice(_))
        };
        let (comp, simples): (Vec<&Termino>, Vec<&Termino>) = e.iter().partition(|t| compuesto(t));
        let mut simples = simples.into_iter();
        let sin_estandarizar = false;
        if let Some((primero, resto)) = comp.split_first() {
            self.termino(primero, primero.negativo, redondeo)?;
            for t in resto {
                self.e(41, 0, Op::Largo(mapa::TRABAJO[0]));
                self.termino(t, t.negativo, redondeo)?;
                self.e(suma, 0, Op::Largo(mapa::TRABAJO[0]));
            }
        } else {
            let t = simples.next().unwrap();
            if t.negativo {
                self.e(40, 0, Op::Largo(mapa::CERO));
                self.op_factor(suma + 1, &t.factores[0])?;
            } else {
                self.op_factor(40, &t.factores[0])?;
            }
        }
        for t in simples {
            self.op_factor(if t.negativo { suma + 1 } else { suma }, &t.factores[0])?;
        }
        Ok(sin_estandarizar)
    }

    /// Expresión estandarizada en A.
    fn expr_std(&mut self, e: &Expr, redondeo: bool) -> R<()> {
        if self.expr(e, redondeo)? {
            self.estandarizar();
        }
        Ok(())
    }

    fn guardar_var(&mut self, v: &Variable) -> R<()> {
        self.op_var(41, v)
    }

    /// Expresión de índices en el sac.
    fn expr_entera(&mut self, e: &[TerminoEntero]) -> R<()> {
        let (comp, simples): (Vec<&TerminoEntero>, Vec<&TerminoEntero>) =
            e.iter().partition(|t| t.factores.len() > 1);
        let mut simples = simples.into_iter();
        if let Some((primero, resto)) = comp.split_first() {
            self.producto_entero(primero)?;
            for t in resto {
                self.e(21, 0, Op::Corto(CELDA_TEMP));
                self.producto_entero(t)?;
                self.e(22, 0, Op::Corto(CELDA_TEMP));
            }
        } else {
            let t = simples.next().unwrap();
            match (t.factores[0], t.negativo) {
                (Entero::Ind(c), false) => self.e(20, 0, Op::Corto(corto_indice(c))),
                (Entero::Ind(c), true) => {
                    self.e(30, 0, Op::Lit(0));
                    self.e(23, 0, Op::Corto(corto_indice(c)));
                }
                (Entero::Lit(k), neg) => self.e(30, 0, Op::Lit(if neg { -k } else { k })),
            }
        }
        for t in simples {
            match t.factores[0] {
                Entero::Ind(c) => self.e(if t.negativo { 23 } else { 22 }, 0, Op::Corto(corto_indice(c))),
                Entero::Lit(k) => self.e(if t.negativo { 33 } else { 32 }, 0, Op::Lit(k)),
            }
        }
        Ok(())
    }

    /// Producto de índices y enteros, exacto, por el acumulador; deja el
    /// resultado en el sac.
    fn producto_entero(&mut self, t: &TerminoEntero) -> R<()> {
        let n = t.factores.len();
        for (k, f) in t.factores.iter().enumerate() {
            let dec = if k == 0 { 40 } else if t.negativo && k == n - 1 { 53 } else { 52 };
            match f {
                Entero::Ind(c) => {
                    self.indice_a_conv(*c);
                    self.e(dec, 0, Op::Largo(mapa::CONV));
                }
                Entero::Lit(v) => {
                    let c = self.constante(*v as f64);
                    self.e(dec, 0, Op::Const(c));
                }
            }
        }
        self.a_entero_en_sac();
        Ok(())
    }

    /// Parte entera de A al sac (instrucción 78, exponente 29).
    fn a_entero_en_sac(&mut self) {
        self.e(78, 0, Op::Lit(28));
        self.e(41, 0, Op::Largo(mapa::EXTR));
        self.e(20, 0, Op::Corto(mapa::EXTR_BAJO));
    }

    fn entero_a_sac(&mut self, x: Entero, dec_ind: u8, dec_lit: u8) {
        match x {
            Entero::Ind(c) => self.e(dec_ind, 0, Op::Corto(corto_indice(c))),
            Entero::Lit(k) => self.e(dec_lit, 0, Op::Lit(k)),
        }
    }

    fn newline(&mut self) {
        self.e(62, 0, Op::Lit(CR));
        self.e(62, 0, Op::Lit(LF));
    }

    fn imprimir_a(&mut self, m: Entero, n: Entero) {
        self.entero_a_sac(m, 20, 30);
        self.e(21, 0, Op::Corto(mapa::CELDA_M));
        self.entero_a_sac(n, 20, 30);
        self.e(21, 0, Op::Corto(mapa::CELDA_N));
        self.rutina(Rutina::Imprimir, 1);
    }

    fn rutina(&mut self, r: Rutina, entrada: u32) {
        self.cap.rutinas.insert(r);
        self.llamar(Op::Rutina(r, entrada));
    }

    fn superposicion(&mut self, r: Superposicion, entrada: u16) {
        self.cap.superposiciones.insert(r);
        self.e(67, 0, Op::Lit(i32::from(r.sector())));
        self.e(68, 0, Op::Lit(15));
        self.llamar(Op::Medio(Superposicion::ORIGEN + entrada));
    }

    fn superposicion_multipagina(&mut self, r: Superposicion) {
        self.cap.superposiciones.insert(r);
        self.e(30, 0, Op::Lit(r.sector().into()));
        self.e(21, 0, Op::Corto(195));
        self.e(30, 0, Op::Lit(r.paginas().into()));
        self.e(21, 0, Op::Corto(197));
        self.superposicion(Superposicion::Cargador, 0);
    }

    fn preparar_matriz(&mut self, numero: u8, destino: Option<&Variable>, args: &[Expr], limite: u16) -> R<()> {
        let [a, b, c, u, v, w, x] = mapa::MATRIZ_ARGS;
        let (rutina, posiciones): (_, &[u16]) = match numero {
            8 => (Superposicion::MatIo, &[a, u, v, x, w]),
            9 => (Superposicion::MatIo, &[a, u, v, w]),
            10 => (Superposicion::MatIo, &[a, u]),
            11 | 12 => (Superposicion::MatLin, &[b, c, u]),
            13 | 14 => (Superposicion::MatLin, &[b, x, c, u]),
            15 | 17 | 18 => (Superposicion::MatLin, &[b, u]),
            16 => (Superposicion::MatMul, &[b, u, v]),
            19 | 20 => (Superposicion::MatLin, &[b, x, u]),
            21 | 22 => (Superposicion::MatLin, &[b, c, u]),
            23 | 24 => (Superposicion::MatLin, &[b, x, c, u]),
            25 => (Superposicion::MatElim, &[b, u]),
            26 | 27 => (Superposicion::MatMul, &[b, c, u, v, w]),
            28 => (Superposicion::MatElim, &[b, u, v]),
            _ => return self.err(format!("φ{numero} no es una operación matricial")),
        };
        self.cap.superposiciones.extend([Superposicion::Cargador, Superposicion::MatMemoria, rutina]);
        if numero <= 10 {
            self.cap.superposiciones.insert(if numero == 10 { Superposicion::MatLeer } else { Superposicion::MatImprimir });
        }
        // Los parámetros quedan en la parte libre de la misma página del
        // cargador. Se pone el cargador antes de escribirlos.
        self.e(67, 0, Op::Lit(i32::from(Superposicion::Cargador.sector())));
        self.e(68, 0, Op::Lit(15));
        let (fuera, preparar) = (self.nueva(), self.nueva());
        self.e(59, 0, Op::Etq(preparar));
        self.poner(fuera);
        self.e(59, 0, Op::Etq(fuera));
        self.poner(preparar);

        let max_u = match numero {
            8 | 9 | 16 => 511,
            17..=24 => 239,
            25 | 28 => 97,
            26 | 27 => 129,
            _ => i32::from(limite) + 2560,
        };
        for (e, &p) in args.iter().zip(posiciones) {
            self.expr_std(e, false)?;
            if p != x || numero <= 10 {
                self.e(78, 0, Op::Lit(28));
                self.estandarizar();
            }
            self.e(41, 0, Op::Largo(p));
        }
        if let Some(destino) = destino.filter(|_| numero != 25) {
            self.op_var(40, destino)?;
            self.e(78, 0, Op::Lit(28));
            self.estandarizar();
            self.e(41, 0, Op::Largo(a));
        }
        if numero == 9 {
            self.e(40, 0, Op::Largo(mapa::CERO));
            self.e(41, 0, Op::Largo(x));
        } else if matches!(numero, 11 | 12 | 17 | 18 | 21 | 22) {
            let uno = self.constante(1.0);
            self.e(40, 0, Op::Const(uno));
            self.e(41, 0, Op::Largo(x));
        }
        if matches!(numero, 12 | 14 | 18 | 20 | 22 | 24) {
            self.e(40, 0, Op::Largo(x));
            self.negar();
            self.e(41, 0, Op::Largo(x));
        }
        self.validar_parametro_matricial(u, 1, max_u, fuera);
        if numero <= 9 {
            self.validar_parametro_matricial(v, 1, 511, fuera);
            self.validar_parametro_matricial(x, 0, 511, fuera);
            self.validar_parametro_matricial(w, 0, 511, fuera);
        }
        match numero {
            16 => self.validar_parametro_matricial(v, 1, 479, fuera),
            26 | 27 => {
                self.validar_parametro_matricial(v, 1, 129, fuera);
                self.validar_parametro_matricial(w, 1, 129, fuera);
            }
            28 => self.validar_parametro_matricial(v, 1, 97, fuera),
            _ => (),
        }
        let superior = self.constante(f64::from(limite));
        self.e(40, 0, Op::Const(superior));
        self.e(41, 0, Op::Largo(mapa::ARG2));

        let regiones = match numero {
            8 | 9 => vec![(a, u, Some(v))],
            10 => vec![(a, u, None)],
            11..=14 => vec![(a, u, None), (b, u, None), (c, u, None)],
            15 => vec![(a, u, None), (b, u, None)],
            16 => vec![(a, u, Some(v)), (b, u, Some(v))],
            17..=20 => vec![(a, u, Some(u)), (b, u, Some(u))],
            21..=24 => vec![(a, u, Some(u)), (b, u, Some(u)), (c, u, None)],
            25 => vec![(b, u, Some(u))],
            26 | 27 => vec![(a, u, Some(v)), (b, u, Some(w)), (c, v, Some(w))],
            28 => vec![(a, u, Some(v)), (b, u, Some(u))],
            _ => unreachable!(),
        };
        for &(base, largo, ancho) in &regiones {
            let valido = self.nueva();
            let comienzo = self.constante(-2560.0);
            self.e(40, 0, Op::Largo(base));
            self.e(45, 0, Op::Const(comienzo));
            self.e(49, 0, Op::Etq(valido));
            self.e(59, 0, Op::Etq(fuera));
            self.poner(valido);
            self.e(40, 0, Op::Largo(largo));
            if let Some(ancho) = ancho {
                self.e(52, 0, Op::Largo(ancho));
            }
            self.e(44, 0, Op::Largo(base));
            self.e(45, 0, Op::Const(superior));
            let negativo = self.nueva();
            self.e(49, 0, Op::Etq(negativo));
            let siguiente = self.nueva();
            self.e(59, 0, Op::Etq(siguiente));
            self.poner(negativo);
            self.e(70, 1, Op::Lit(256));
            self.e(8, 0, Op::Etq(fuera));
            self.poner(siguiente);
        }
        if matches!(numero, 11..=24 | 26..=28) {
            self.validar_solapamiento_matricial(numero, &regiones, fuera);
        }
        for (valor, corto) in [(rutina.sector(), 195), (rutina.paginas(), 197), (u16::from(numero), 199)] {
            self.e(30, 0, Op::Lit(valor.into()));
            self.e(21, 0, Op::Corto(corto));
        }
        self.llamar(Op::Medio(Superposicion::ORIGEN));
        if numero == 25 {
            self.e(40, 0, Op::Largo(a));
            self.guardar_var(destino.unwrap())?;
        }
        Ok(())
    }

    fn validar_solapamiento_matricial(
        &mut self, numero: u8, regiones: &[(u16, u16, Option<u16>)], fuera: u32,
    ) {
        let (base, largo, ancho) = regiones[0];
        let fin = self.nueva();
        self.e(40, 0, Op::Largo(largo));
        if let Some(ancho) = ancho {
            self.e(52, 0, Op::Largo(ancho));
        }
        if matches!(numero, 16 | 26 | 27) {
            let grande = self.nueva();
            let limite = self.constante(385.0);
            self.e(45, 0, Op::Const(limite));
            self.e(49, 0, Op::Etq(grande));
            self.e(59, 0, Op::Etq(fin));
            self.poner(grande);
            self.e(44, 0, Op::Const(limite));
        }
        self.e(44, 0, Op::Largo(base));
        let extremo = mapa::PARAMETROS[0];
        self.e(41, 0, Op::Largo(extremo));
        let fuentes = if matches!(numero, 21..=24) { &regiones[1..2] } else { &regiones[1..] };
        for &(otra, filas, columnas) in fuentes {
            let siguiente = self.nueva();
            if matches!(numero, 11..=15 | 17..=24) {
                let distinta = self.nueva();
                self.e(40, 0, Op::Largo(otra));
                self.e(45, 0, Op::Largo(base));
                self.e(70, 1, Op::Lit(256));
                self.e(8, 0, Op::Etq(distinta));
                self.e(59, 0, Op::Etq(siguiente));
                self.poner(distinta);
            }
            self.e(40, 0, Op::Largo(otra));
            self.e(45, 0, Op::Largo(extremo));
            self.e(49, 0, Op::Etq(siguiente));
            self.e(40, 0, Op::Largo(filas));
            if let Some(columnas) = columnas {
                self.e(52, 0, Op::Largo(columnas));
            }
            self.e(44, 0, Op::Largo(otra));
            let extremo_otro = mapa::PARAMETROS[1];
            self.e(41, 0, Op::Largo(extremo_otro));
            self.e(40, 0, Op::Largo(base));
            self.e(45, 0, Op::Largo(extremo_otro));
            self.e(49, 0, Op::Etq(siguiente));
            self.e(59, 0, Op::Etq(fuera));
            self.poner(siguiente);
        }
        self.poner(fin);
    }

    fn validar_parametro_matricial(&mut self, direccion: u16, minimo: i32, maximo: i32, fuera: u32) {
        let acotado = self.nueva();
        let inferior = self.constante(f64::from(minimo));
        let superior = self.constante(f64::from(maximo + 1));
        self.e(40, 0, Op::Largo(direccion));
        self.e(45, 0, Op::Const(inferior));
        self.e(49, 0, Op::Etq(acotado));
        self.e(59, 0, Op::Etq(fuera));
        self.poner(acotado);
        self.e(40, 0, Op::Largo(direccion));
        self.e(45, 0, Op::Const(superior));
        self.e(49, 0, Op::Etq(fuera));
    }

    fn dividir_por(&mut self, direccion: u16) {
        self.e(10, 1, Op::Lit(direccion.into()));
        self.llamar(Op::Medio(mapa::DIVISION));
    }

    fn valor_absoluto(&mut self) {
        let positivo = self.nueva();
        self.e(49, 0, Op::Etq(positivo));
        self.negar();
        self.poner(positivo);
    }

    /// Devuelve 1 + (menor/mayor)^2 en A y mayor en el largo 54.
    fn norma_compleja(&mut self) {
        let [x, y, mayor, menor] = mapa::PARAMETROS;
        let intercambiar = self.nueva();
        let calcular = self.nueva();
        let dividir = self.nueva();
        let sumar = self.nueva();
        let uno = self.constante(1.0);
        self.e(40, 0, Op::Largo(x));
        self.valor_absoluto();
        self.e(41, 0, Op::Largo(mayor));
        self.e(40, 0, Op::Largo(y));
        self.valor_absoluto();
        self.e(41, 0, Op::Largo(menor));
        self.e(45, 0, Op::Largo(mayor));
        self.e(49, 0, Op::Etq(intercambiar));
        self.e(59, 0, Op::Etq(calcular));
        self.poner(intercambiar);
        let t = mapa::TRABAJO[0];
        self.e(40, 0, Op::Largo(mayor));
        self.e(41, 0, Op::Largo(t));
        self.e(40, 0, Op::Largo(menor));
        self.e(41, 0, Op::Largo(mayor));
        self.e(40, 0, Op::Largo(t));
        self.e(41, 0, Op::Largo(menor));
        self.poner(calcular);
        self.e(40, 0, Op::Largo(mayor));
        self.e(70, 1, Op::Lit(256));
        self.e(8, 0, Op::Etq(dividir));
        self.e(40, 0, Op::Largo(mapa::CERO));
        self.e(59, 0, Op::Etq(sumar));
        self.poner(dividir);
        self.e(40, 0, Op::Largo(menor));
        self.dividir_por(mayor);
        self.e(41, 0, Op::Largo(t));
        self.e(52, 0, Op::Largo(t));
        self.poner(sumar);
        self.e(44, 0, Op::Const(uno));
    }

    fn funcion_compleja(&mut self, funcion: FuncionCompleja) {
        let [x, y, t, q] = mapa::PARAMETROS;
        match funcion {
            FuncionCompleja::Exp => {
                self.e(40, 0, Op::Largo(x));
                self.rutina(Rutina::Exp, 1);
                self.e(41, 0, Op::Largo(t));
                self.e(40, 0, Op::Largo(y));
                self.rutina(Rutina::Trig, 2);
                self.e(52, 0, Op::Largo(t));
                self.e(41, 0, Op::Largo(x));
                self.e(40, 0, Op::Largo(y));
                self.rutina(Rutina::Trig, 1);
                self.e(52, 0, Op::Largo(t));
                self.e(41, 0, Op::Largo(y));
            }
            FuncionCompleja::Log => {
                let mitad = self.constante(0.5);
                self.norma_compleja();
                self.rutina(Rutina::Log, 1);
                self.e(52, 0, Op::Const(mitad));
                self.e(41, 0, Op::Largo(q));
                self.e(40, 0, Op::Largo(t));
                self.rutina(Rutina::Log, 1);
                self.e(44, 0, Op::Largo(q));
                self.e(41, 0, Op::Largo(t));
                self.e(40, 0, Op::Largo(y));
                self.e(41, 0, Op::Largo(mapa::ARG2));
                self.e(40, 0, Op::Largo(x));
                self.rutina(Rutina::Arctan, 1);
                self.e(41, 0, Op::Largo(y));
                self.e(40, 0, Op::Largo(t));
                self.e(41, 0, Op::Largo(x));
            }
            FuncionCompleja::Raiz => {
                let mitad = self.constante(0.5);
                let cero = self.nueva();
                let positivo = self.nueva();
                let signo = self.nueva();
                let fin = self.nueva();
                self.norma_compleja();
                self.e(41, 0, Op::Largo(q));
                self.e(40, 0, Op::Largo(t));
                self.e(70, 1, Op::Lit(256));
                self.e(8, 0, Op::Etq(positivo));
                self.e(59, 0, Op::Etq(cero));
                self.poner(positivo);
                self.e(40, 0, Op::Largo(q));
                self.rutina(Rutina::Raiz, 1);
                self.e(41, 0, Op::Largo(q));
                self.e(40, 0, Op::Largo(x));
                self.valor_absoluto();
                self.dividir_por(t);
                self.e(44, 0, Op::Largo(q));
                self.e(52, 0, Op::Const(mitad));
                self.rutina(Rutina::Raiz, 1);
                self.e(41, 0, Op::Largo(q));
                self.e(40, 0, Op::Largo(t));
                self.rutina(Rutina::Raiz, 1);
                self.e(52, 0, Op::Largo(q));
                self.e(41, 0, Op::Largo(t)); // componente de mayor módulo
                self.e(40, 0, Op::Largo(y));
                self.e(52, 0, Op::Const(mitad));
                self.dividir_por(t);
                self.e(41, 0, Op::Largo(q));
                let real_no_negativa = self.nueva();
                self.e(40, 0, Op::Largo(x));
                self.e(49, 0, Op::Etq(real_no_negativa));
                self.e(40, 0, Op::Largo(q));
                self.valor_absoluto();
                self.e(41, 0, Op::Largo(x));
                self.e(40, 0, Op::Largo(y));
                self.e(49, 0, Op::Etq(signo));
                self.e(40, 0, Op::Largo(t));
                self.negar();
                self.e(59, 0, Op::Etq(fin));
                self.poner(signo);
                self.e(40, 0, Op::Largo(t));
                self.e(59, 0, Op::Etq(fin));
                self.poner(real_no_negativa);
                self.e(40, 0, Op::Largo(t));
                self.e(41, 0, Op::Largo(x));
                self.e(40, 0, Op::Largo(q));
                self.e(59, 0, Op::Etq(fin));
                self.poner(cero);
                self.e(40, 0, Op::Largo(mapa::CERO));
                self.e(41, 0, Op::Largo(x));
                self.poner(fin);
                self.e(41, 0, Op::Largo(y));
            }
        }
    }

    fn asignar_par(&mut self, destinos: &[Variable; 2], expr: &ExpresionPar, doble: bool) -> R<()> {
        if doble && !self.doble_longitud {
            return self.err("falta la directiva double length");
        }
        if destinos[0] == destinos[1] {
            return self.err("las dos variables de destino tienen que ser distintas");
        }
        let [x, y, a, b] = mapa::PARAMETROS;
        let (primero, segundo) = match expr {
            ExpresionPar::Binaria(izq, _, der) => (izq, Some(der)),
            ExpresionPar::Copia(izq) | ExpresionPar::Funcion(_, izq) => (izq, None),
        };
        for (f, largo) in primero.iter().chain(segundo.into_iter().flatten()).zip([x, y, a, b]) {
            self.op_factor(40, f)?;
            self.e(41, 0, Op::Largo(largo));
        }
        match expr {
            ExpresionPar::Copia(_) => (),
            ExpresionPar::Funcion(funcion, _) => self.funcion_compleja(*funcion),
            ExpresionPar::Binaria(_, operacion, _) if doble => self.par_doble(*operacion),
            ExpresionPar::Binaria(_, OperacionPar::Cociente, _) => self.superposicion_multipagina(Superposicion::Complejo),
            ExpresionPar::Binaria(_, OperacionPar::Producto, _) => {
                let t = mapa::TRABAJO[0];
                let u = mapa::TRABAJO[1];
                self.e(40, 0, Op::Largo(y));
                self.e(52, 0, Op::Largo(b));
                self.e(41, 0, Op::Largo(t));
                self.e(40, 0, Op::Largo(x));
                self.e(52, 0, Op::Largo(a));
                self.e(45, 0, Op::Largo(t));
                self.e(41, 0, Op::Largo(u));
                self.e(40, 0, Op::Largo(y));
                self.e(52, 0, Op::Largo(a));
                self.e(41, 0, Op::Largo(t));
                self.e(40, 0, Op::Largo(x));
                self.e(52, 0, Op::Largo(b));
                self.e(44, 0, Op::Largo(t));
                self.e(41, 0, Op::Largo(y));
                self.e(40, 0, Op::Largo(u));
                self.e(41, 0, Op::Largo(x));
            }
            ExpresionPar::Binaria(_, op, _) => {
                let instruccion = if *op == OperacionPar::Suma { 44 } else { 45 };
                for (izq, der) in [(x, a), (y, b)] {
                    self.e(40, 0, Op::Largo(izq));
                    self.e(instruccion, 0, Op::Largo(der));
                    self.e(41, 0, Op::Largo(izq));
                }
            }
        }
        for (destino, largo) in destinos.iter().zip([x, y]) {
            self.e(40, 0, Op::Largo(largo));
            self.guardar_var(destino)?;
        }
        Ok(())
    }

    fn par_doble(&mut self, op: OperacionPar) {
        let n = match op {
            OperacionPar::Suma => 0,
            OperacionPar::Resta => 1,
            OperacionPar::Producto => 2,
            OperacionPar::Cociente => 3,
        };
        self.e(30, 0, Op::Lit(n));
        self.e(21, 0, Op::Corto(199));
        self.superposicion_multipagina(Superposicion::Doble);
    }

    fn grupo_desde_cero(&self, grupo: &Variable) -> R<(u16, u16)> {
        if let Variable::Principal(letra, Sufijo::Fijo(0)) = grupo {
            if let Some(&reserva) = self.grupos.get(letra) {
                return Ok(reserva);
            }
        }
        self.err("se esperaba la primera variable de un grupo reservado, por ejemplo x0")
    }

    fn sufijo_acotado(&mut self, indice: Entero, tam: u16) -> R<()> {
        match indice {
            Entero::Lit(k) if !(0..i32::from(tam)).contains(&k) => self.err("sufijo fuera del grupo"),
            Entero::Lit(k) => {
                self.e(30, 0, Op::Lit(k));
                Ok(())
            }
            Entero::Ind(c) => {
                let no_negativo = self.nueva();
                let fuera = self.nueva();
                self.e(20, 0, Op::Corto(corto_indice(c)));
                self.e(29, 0, Op::Etq(no_negativo));
                self.poner(fuera);
                self.e(59, 0, Op::Etq(fuera));
                self.poner(no_negativo);
                self.e(37, 0, Op::Lit(tam.into()));
                self.e(29, 0, Op::Etq(fuera));
                Ok(())
            }
        }
    }

    fn transferencia_auxiliar(
        &mut self, escribir: bool, direccion: &Expr, variable: &Variable,
        cantidad: Entero, limite_aux: u16,
    ) -> R<()> {
        let (base, tam, sufijo) = match *variable {
            Variable::Principal(l, sufijo) => {
                let Some(&(base, tam)) = self.grupos.get(&l) else {
                    return self.err(format!("φ{}: variable {l} sin reservar", if escribir { 7 } else { 6 }));
                };
                (base, tam, sufijo)
            }
            Variable::Especial(l) => (letra_especial(l, false), 1, Sufijo::Fijo(0)),
            Variable::Primada(l) => (letra_especial(l, true), 1, Sufijo::Fijo(0)),
        };
        const DIRECCION: usize = 195;
        const CANTIDAD: usize = 197;
        if let Entero::Lit(n) = cantidad {
            if !(0..=i32::from(tam)).contains(&n)
                || matches!(sufijo, Sufijo::Fijo(d) if d < 0 || d >= i32::from(tam) || d + n > i32::from(tam))
            {
                return self.err("la transferencia auxiliar excede las variables de trabajo");
            }
        }
        if matches!(sufijo, Sufijo::Fijo(d) if !(0..i32::from(tam)).contains(&d))
            || matches!(sufijo, Sufijo::Indice(_, d) if d <= -512 || d >= 512)
        {
            return self.err("sufijo fuera del grupo");
        }

        let fuera = self.nueva();
        let valido = self.nueva();
        let n_no_negativo = self.nueva();
        let sufijo_no_negativo = self.nueva();
        let espacio = self.nueva();
        let dentro = self.nueva();
        let fin = self.nueva();
        self.entero_a_sac(cantidad, 20, 30);
        self.e(29, 0, Op::Etq(n_no_negativo));
        self.poner(fuera);
        self.e(59, 0, Op::Etq(fuera));
        self.poner(n_no_negativo);
        self.e(28, 0, Op::Etq(valido));
        self.e(59, 0, Op::Etq(fin));
        self.poner(valido);
        self.e(21, 0, Op::Corto(CANTIDAD));
        match sufijo {
            Sufijo::Fijo(d) => self.e(30, 0, Op::Lit(d)),
            Sufijo::Indice(c, d) => {
                self.e(20, 0, Op::Corto(corto_indice(c)));
                self.e(32, 0, Op::Lit(d));
            }
        }
        self.e(29, 0, Op::Etq(sufijo_no_negativo));
        self.e(59, 0, Op::Etq(fuera));
        self.poner(sufijo_no_negativo);
        self.e(21, 0, Op::Corto(CELDA_TEMP));
        self.e(30, 0, Op::Lit(tam.into()));
        self.e(23, 0, Op::Corto(CELDA_TEMP));
        self.e(29, 0, Op::Etq(espacio));
        self.e(59, 0, Op::Etq(fuera));
        self.poner(espacio);
        self.e(23, 0, Op::Corto(CANTIDAD));
        self.e(29, 0, Op::Etq(dentro));
        self.e(59, 0, Op::Etq(fuera));
        self.poner(dentro);
        self.e(20, 0, Op::Corto(CELDA_TEMP));
        self.e(32, 0, Op::Lit(base.into()));
        self.e(21, 0, Op::Corto(DIRECCION));

        // Supuesto: el intervalo inválido detiene la máquina antes de transferir.
        // Se toma el piso de la expresión, sin redondeo, como para un sufijo.
        let minimo = self.constante(-3072.0);
        let maximo = self.constante(f64::from(limite_aux));
        let auxiliar = mapa::PARAMETROS[0];
        let piso = mapa::PARAMETROS[1];
        let correcto = self.nueva();
        self.expr_std(direccion, false)?;
        self.e(41, 0, Op::Largo(auxiliar));
        self.e(45, 0, Op::Const(minimo));
        self.e(49, 0, Op::Etq(correcto));
        self.e(59, 0, Op::Etq(fuera));
        self.poner(correcto);
        self.e(40, 0, Op::Largo(auxiliar));
        self.e(45, 0, Op::Const(maximo));
        self.e(49, 0, Op::Etq(fuera));
        self.e(40, 0, Op::Largo(auxiliar));
        self.e(78, 0, Op::Lit(28));
        self.e(41, 0, Op::Largo(piso));
        self.estandarizar();
        self.e(41, 0, Op::Largo(auxiliar));
        self.e(20, 0, Op::Corto(CANTIDAD));
        self.e(21, 0, Op::Corto(mapa::CONV_ALTO));
        self.e(40, 0, Op::Largo(mapa::CONV));
        self.estandarizar();
        self.e(44, 0, Op::Largo(auxiliar));
        self.e(41, 0, Op::Largo(auxiliar));
        self.e(40, 0, Op::Const(maximo));
        self.e(45, 0, Op::Largo(auxiliar));
        let transferir = self.nueva();
        self.e(49, 0, Op::Etq(transferir));
        self.e(59, 0, Op::Etq(fuera));
        self.poner(transferir);
        self.e(40, 0, Op::Largo(piso));
        self.superposicion(Superposicion::Auxiliar, u16::from(escribir));
        self.poner(fin);
        Ok(())
    }

    fn paso_runge_kutta(&mut self, subsecuencia: u32) -> R<()> {
        let mut bases = [0; 4];
        let mut tam = mapa::N_PRINCIPALES;
        for (pos, letra) in ['y', 'f', 'g', 'h'].into_iter().enumerate() {
            let Some(&(base, cantidad)) = self.grupos.get(&letra) else {
                return self.err(format!("int step requiere el grupo {letra}"));
            };
            bases[pos] = base;
            tam = tam.min(cantidad);
        }
        if tam <= 1 {
            return self.err("int step requiere por lo menos un término en los grupos y, f, g, h");
        }
        self.sufijo_acotado(Entero::Ind('n'), tam)?;
        let empezar = self.nueva();
        let fuera = self.nueva();
        self.e(28, 0, Op::Etq(empezar));
        self.poner(fuera);
        self.e(59, 0, Op::Etq(fuera));
        self.poner(empezar);
        let h = letra_especial('h', false);
        let x = letra_especial('x', false);
        let medio = self.constante(0.5);
        let sexto = self.constante(1.0 / 6.0);

        for etapa in 0..4 {
            let vuelta = self.nueva();
            self.e(10, 2, Op::EtqLit(vuelta));
            self.e(59, 0, Op::Etq(subsecuencia));
            self.poner(vuelta);
            // La subsecuencia puede utilizar las otras rutinas de biblioteca.
            // Preparar sus parámetros después de cada vuelta.
            for (base, corto) in bases.into_iter().zip([195, 197, 199, 201]) {
                self.e(30, 0, Op::Lit(base.into()));
                self.e(21, 0, Op::Corto(corto));
            }
            self.e(30, 0, Op::Lit(etapa));
            self.e(21, 0, Op::Corto(202));
            self.e(40, 0, Op::Largo(h));
            match etapa {
                0 | 1 => self.e(52, 0, Op::Const(medio)),
                3 => self.e(52, 0, Op::Const(sexto)),
                _ => (),
            }
            self.e(41, 0, Op::Largo(mapa::PARAMETROS[1]));
            self.superposicion(Superposicion::IntStep, 0);

            if etapa == 0 || etapa == 2 {
                self.e(40, 0, Op::Largo(h));
                self.e(52, 0, Op::Const(medio));
                self.e(44, 0, Op::Largo(x));
                self.e(41, 0, Op::Largo(x));
            }
        }
        Ok(())
    }

    fn polinomio(&mut self, x: &Expr, grupo: &Variable, n: Entero, redondeo: bool) -> R<()> {
        let (base, tam) = self.grupo_desde_cero(grupo)?;
        let x_guardado = mapa::PARAMETROS[0];
        self.expr_std(x, redondeo)?;
        self.e(41, 0, Op::Largo(x_guardado));
        self.sufijo_acotado(n, tam)?;
        self.e(21, 0, Op::Corto(CELDA_TEMP));
        self.e(0, 3, Op::Corto(CELDA_TEMP));
        self.e(40, 3, Op::Largo(base));
        let bucle = self.nueva();
        let fin = self.nueva();
        self.e(8, 0, Op::Etq(bucle));
        self.e(59, 0, Op::Etq(fin));
        self.poner(bucle);
        self.e(13, 3, Op::Lit(1));
        self.e(if redondeo { 50 } else { 52 }, 0, Op::Largo(x_guardado));
        self.e(if redondeo { 42 } else { 44 }, 3, Op::Largo(base));
        self.e(8, 0, Op::Etq(bucle));
        self.poner(fin);
        Ok(())
    }

    fn extremo(&mut self, maximo: bool, grupo: &Variable, desde: Entero, hasta: Entero) -> R<()> {
        let (base, tam) = self.grupo_desde_cero(grupo)?;
        if matches!((desde, hasta), (Entero::Lit(n), Entero::Lit(m)) if n > m) {
            return self.err("intervalo de φ max/min vacío");
        }
        let ultimo = mapa::PARAMETROS[0] as usize * 4;
        self.sufijo_acotado(hasta, tam)?;
        self.e(21, 0, Op::Corto(ultimo));
        self.sufijo_acotado(desde, tam)?;
        let valido = self.nueva();
        let fuera = self.nueva();
        // El rango de ambos sufijos ya está acotado a 0..479.
        self.e(21, 0, Op::Corto(CELDA_TEMP));
        self.e(20, 0, Op::Corto(ultimo));
        self.e(23, 0, Op::Corto(CELDA_TEMP));
        self.e(29, 0, Op::Etq(valido));
        self.poner(fuera);
        self.e(59, 0, Op::Etq(fuera));
        self.poner(valido);
        self.e(0, 3, Op::Corto(CELDA_TEMP)); // cursor
        self.e(0, 4, Op::Corto(CELDA_TEMP)); // mejor sufijo
        let bucle = self.nueva();
        let vigente = self.nueva();
        let fin = self.nueva();
        self.poner(bucle);
        self.e(7, 3, Op::Corto(ultimo));
        let comparar = self.nueva();
        self.e(8, 0, Op::Etq(comparar));
        self.e(59, 0, Op::Etq(fin));
        self.poner(comparar);
        self.e(12, 3, Op::Lit(1));
        let (izq, der) = if maximo { (4, 3) } else { (3, 4) };
        self.e(40, izq, Op::Largo(base));
        self.e(45, der, Op::Largo(base));
        self.e(49, 0, Op::Etq(vigente));
        self.e(1, 3, Op::Corto(CELDA_TEMP));
        self.e(0, 4, Op::Corto(CELDA_TEMP));
        self.poner(vigente);
        self.e(59, 0, Op::Etq(bucle));
        self.poner(fin);
        self.e(1, 4, Op::Corto(CELDA_TEMP));
        self.e(20, 0, Op::Corto(CELDA_TEMP));
        Ok(())
    }

    /// `?`: nueva línea e impresión con m = 0, n = 10 (variables) o
    /// m = 1, n = 0 (índices). El valor está en A.
    fn interrogacion(&mut self, indice: bool) {
        self.newline();
        if indice {
            self.imprimir_a(Entero::Lit(1), Entero::Lit(0));
        } else {
            self.imprimir_a(Entero::Lit(0), Entero::Lit(10));
        }
    }

    fn indice_en_a(&mut self, c: char) {
        self.indice_a_conv(c);
        self.e(40, 0, Op::Largo(mapa::CONV));
        self.estandarizar();
    }

    fn funcion(&mut self, f: &Funcion, redondeo: bool) -> R<()> {
        match f {
            Funcion::Una(nombre, x) => match nombre.as_str() {
                "intpt" => {
                    self.expr(x, redondeo)?;
                    self.e(78, 0, Op::Lit(28));
                    self.estandarizar();
                }
                "frpt" => {
                    self.expr(x, redondeo)?;
                    self.e(41, 0, Op::Largo(mapa::TRABAJO[1]));
                    self.e(78, 0, Op::Lit(28));
                    self.estandarizar();
                    self.negar();
                    self.e(44, 0, Op::Largo(mapa::TRABAJO[1]));
                }
                "mod" => {
                    self.expr_std(x, redondeo)?;
                    let fin = self.nueva();
                    self.e(49, 0, Op::Etq(fin));
                    self.negar();
                    self.poner(fin);
                }
                "sign" => {
                    self.expr(x, redondeo)?;
                    let pos = self.nueva();
                    let fin = self.nueva();
                    self.e(49, 0, Op::Etq(pos));
                    self.e(40, 0, Op::Largo(mapa::MENOS_UNO));
                    self.e(59, 0, Op::Etq(fin));
                    let uno = self.constante(1.0);
                    self.poner(pos);
                    self.e(40, 0, Op::Const(uno));
                    self.poner(fin);
                }
                otra => {
                    let (r, entrada) = match otra {
                        "sqrt" => (Rutina::Raiz, 1),
                        "exp" => (Rutina::Exp, 1),
                        "log" => (Rutina::Log, 1),
                        "sin" => (Rutina::Trig, 1),
                        "cos" => (Rutina::Trig, 2),
                        "tan" => (Rutina::Trig, 3),
                        _ => return self.err(format!("φ {otra} desconocida")),
                    };
                    self.expr_std(x, redondeo)?;
                    self.rutina(r, entrada);
                }
            },
            Funcion::Dos(nombre, a, b) => {
                // El segundo argumento espera en un largo de la página 1 que no
                // usa ninguna rutina: el primero puede necesitar la división.
                self.expr_std(b, redondeo)?;
                self.e(41, 0, Op::Largo(mapa::ARG2));
                self.expr_std(a, redondeo)?;
                match nombre.as_str() {
                    "divide" => {
                        self.e(10, 1, Op::Lit(mapa::ARG2 as i32));
                        self.llamar(Op::Medio(mapa::DIVISION));
                    }
                    "arctan" => self.rutina(Rutina::Arctan, 1),
                    "radius" => {
                        let t = mapa::TRABAJO[0];
                        self.e(41, 0, Op::Largo(t));
                        self.e(50, 0, Op::Largo(t));
                        self.e(41, 0, Op::Largo(t));
                        self.e(40, 0, Op::Largo(mapa::ARG2));
                        self.e(50, 0, Op::Largo(mapa::ARG2));
                        self.e(42, 0, Op::Largo(t));
                        self.rutina(Rutina::Raiz, 1);
                    }
                    otra => return self.err(format!("φ {otra} desconocida")),
                }
            }
            Funcion::Paridad(e) => {
                self.expr_entera(e)?;
                self.e(35, 0, Op::Lit(1));
                let impar = self.nueva();
                let fin = self.nueva();
                self.e(28, 0, Op::Etq(impar));
                let uno = self.constante(1.0);
                self.e(40, 0, Op::Const(uno));
                self.e(59, 0, Op::Etq(fin));
                self.poner(impar);
                self.e(40, 0, Op::Largo(mapa::MENOS_UNO));
                self.poner(fin);
            }
            Funcion::Polinomio(x, grupo, n) => self.polinomio(x, grupo, *n, redondeo)?,
        }
        Ok(())
    }

    fn saltar_si(&mut self, destino: u32, c: &Comparacion) -> R<()> {
        let saltar_si_no = |g: &mut Gen, dec_salto: u8| {
            let sigue = g.nueva();
            g.e(dec_salto, 0, Op::Etq(sigue));
            g.e(59, 0, Op::Etq(destino));
            g.poner(sigue);
        };
        match c {
            Comparacion::Enteros(a, rel, b) => {
                self.entero_a_sac(*a, 20, 30);
                self.entero_a_sac(*b, 23, 33);
                match rel {
                    Relacion::MayorIgual => self.e(29, 0, Op::Etq(destino)),
                    Relacion::Mayor => {
                        self.e(33, 0, Op::Lit(1));
                        self.e(29, 0, Op::Etq(destino));
                    }
                    Relacion::Distinto => self.e(28, 0, Op::Etq(destino)),
                    Relacion::Igual => saltar_si_no(self, 28),
                }
            }
            Comparacion::Variables(a, rel, b) => {
                // A = a - b sin redondeo (o b - a para '>'), y se mira el signo o el cero.
                let (x, y) = if *rel == Relacion::Mayor { (b, a) } else { (a, b) };
                self.op_factor(40, x)?;
                self.op_factor(45, y)?;
                match rel {
                    Relacion::MayorIgual => self.e(49, 0, Op::Etq(destino)),
                    Relacion::Mayor => saltar_si_no(self, 49),
                    Relacion::Igual | Relacion::Distinto => {
                        // B1 = exponente + 256: cero sólo para el cero de la máquina.
                        self.e(70, 1, Op::Lit(256));
                        if *rel == Relacion::Distinto {
                            self.e(8, 0, Op::Etq(destino));
                        } else {
                            saltar_si_no(self, 8);
                        }
                    }
                }
            }
        }
        Ok(())
    }

    fn maquina(&mut self, funcion: u8, b: u8, op: &OperandoMaquina) -> R<()> {
        match op {
            OperandoMaquina::Convencional(texto) => {
                let src = format!("{funcion:02}{b} {texto}");
                let p = asm::assemble_at(&src, 0).or_else(|e| self.err(format!("instrucción de máquina: {}", e.msg)))?;
                let get = |a: usize| p.shorts.iter().find(|(x, _)| *x == a).map(|x| x.1).unwrap_or(0) as u32;
                self.e(funcion, b, Op::Palabra((get(0) << 10) | get(1)));
            }
            OperandoMaquina::Simbolico(s) => {
                let s = s.clone();
                let es_salto = matches!(funcion, 8 | 9 | 18 | 28 | 29 | 38 | 48 | 49 | 59);
                if es_salto {
                    let n: u32 = s.parse().or_else(|_| self.err(format!("se esperaba una etiqueta: ({s})")))?;
                    self.e(funcion, b, Op::Etq(n));
                    return Ok(());
                }
                match opcodes::addr_type(funcion) {
                    AddrType::Long => {
                        let (neg, cuerpo) = match s.strip_prefix('-') {
                            Some(r) => (true, r.to_string()),
                            None => (false, s.trim_start_matches('+').to_string()),
                        };
                        let primero = cuerpo.chars().next().unwrap_or(' ');
                        if primero.is_ascii_digit() || primero == '.' {
                            let v: f64 = cuerpo.parse().or_else(|_| self.err(format!("constante inválida: {s}")))?;
                            let k = self.constante(if neg { -v } else { v });
                            self.e(funcion, b, Op::Const(k));
                        } else if es_letra_variable(primero) && !neg {
                            let lineas = analizar(&format!("π = {cuerpo}")).or_else(|e| self.err(e.msg))?;
                            let Some(Sentencia::AsignarVar { expr, .. }) = &lineas[0].sentencia else {
                                return self.err(format!("operando inválido: ({s})"));
                            };
                            let [Termino { factores, divisor: None, negativo: false }] = expr.as_slice() else {
                                return self.err(format!("operando inválido: ({s})"));
                            };
                            let [Factor::Var(v)] = factores.as_slice() else {
                                return self.err(format!("operando inválido: ({s})"));
                            };
                            match self.dir_var(v)? {
                                DirVar::Fija(a) => self.e(funcion, b, Op::Largo(a)),
                                DirVar::Indexada(base, c) => {
                                    if b != 0 {
                                        return self.err("con un sufijo variable el dígito B debe ser 0");
                                    }
                                    self.e(0, 1, Op::Corto(corto_indice(c)));
                                    self.e(funcion, 1, Op::Largo(base));
                                }
                            }
                        } else {
                            return self.err(format!("operando inválido: ({s})"));
                        }
                    }
                    AddrType::Short | AddrType::ShortFirstQuarter => {
                        let c = s.chars().next().unwrap_or(' ');
                        if s.chars().count() == 1 && es_indice(c) {
                            self.e(funcion, b, Op::Corto(corto_indice(c)));
                        } else {
                            let v: i32 = s.parse().or_else(|_| self.err(format!("se esperaba un índice o un entero: ({s})")))?;
                            let k = self.corta(v);
                            self.e(funcion, b, Op::ConstCorta(k));
                        }
                    }
                    _ => return self.err(format!("la función {funcion:02} no admite dirección simbólica; escribirla con coma")),
                }
            }
        }
        Ok(())
    }

    fn sentencia(&mut self, s: &Sentencia, capitulos: &BTreeSet<u32>) -> R<()> {
        match s {
            Sentencia::AsignarVar { destino, expr, redondeo, imprimir } => {
                self.expr_std(expr, *redondeo)?;
                self.guardar_var(destino)?;
                if *imprimir {
                    self.interrogacion(false);
                }
            }
            Sentencia::AsignarFuncion { destino, f, redondeo, imprimir } => {
                self.funcion(f, *redondeo)?;
                self.guardar_var(destino)?;
                if *imprimir {
                    self.interrogacion(false);
                }
            }
            Sentencia::AsignarIndice { destino, expr, imprimir } => {
                self.expr_entera(expr)?;
                self.e(21, 0, Op::Corto(corto_indice(*destino)));
                if *imprimir {
                    self.indice_en_a(*destino);
                    self.interrogacion(true);
                }
            }
            Sentencia::IndiceParteEntera { destino, expr, imprimir } => {
                self.expr(expr, true)?;
                self.a_entero_en_sac();
                self.e(21, 0, Op::Corto(corto_indice(*destino)));
                if *imprimir {
                    self.indice_en_a(*destino);
                    self.interrogacion(true);
                }
            }
            Sentencia::IndiceExtremo { destino, maximo, grupo, desde, hasta, imprimir } => {
                self.extremo(*maximo, grupo, *desde, *hasta)?;
                self.e(21, 0, Op::Corto(corto_indice(*destino)));
                if *imprimir {
                    self.indice_en_a(*destino);
                    self.interrogacion(true);
                }
            }
            Sentencia::Ciclo { indice, desde, paso, paso_negativo, hasta } => {
                if self.ciclos.len() >= 8 {
                    return self.err("más de 8 ciclos anidados");
                }
                // i = p ∓ q; arriba: i = i ± q.
                self.entero_a_sac(*desde, 20, 30);
                if *paso_negativo {
                    self.entero_a_sac(*paso, 22, 32);
                } else {
                    self.entero_a_sac(*paso, 23, 33);
                }
                let ci = corto_indice(*indice);
                self.e(21, 0, Op::Corto(ci));
                let arriba = self.nueva();
                self.poner(arriba);
                self.e(20, 0, Op::Corto(ci));
                if *paso_negativo {
                    self.entero_a_sac(*paso, 23, 33);
                } else {
                    self.entero_a_sac(*paso, 22, 32);
                }
                self.e(21, 0, Op::Corto(ci));
                self.ciclos.push((*indice, *paso, *paso_negativo, *hasta, arriba, self.linea));
            }
            Sentencia::Repeat => {
                let Some((indice, _, _, hasta, arriba, _)) = self.ciclos.pop() else {
                    return self.err("repeat sin ciclo");
                };
                self.e(20, 0, Op::Corto(corto_indice(indice)));
                self.entero_a_sac(hasta, 27, 37);
                self.e(28, 0, Op::Etq(arriba));
            }
            Sentencia::Saltar(n) => self.e(59, 0, Op::Etq(*n)),
            Sentencia::SaltarSi(n, c) => self.saltar_si(*n, c)?,
            Sentencia::Check(x, y, e, destino) => {
                let [primero, diferencia, ..] = mapa::PARAMETROS;
                self.expr_std(y, false)?;
                self.e(41, 0, Op::Largo(primero));
                self.expr_std(x, false)?;
                self.e(45, 0, Op::Largo(primero));
                let positivo = self.nueva();
                self.e(49, 0, Op::Etq(positivo));
                self.negar();
                self.poner(positivo);
                self.e(41, 0, Op::Largo(diferencia));
                self.expr_std(e, false)?;
                self.e(41, 0, Op::Largo(primero));
                self.e(40, 0, Op::Largo(diferencia));
                self.e(45, 0, Op::Largo(primero));
                let sigue = self.nueva();
                self.e(49, 0, Op::Etq(sigue));
                self.e(59, 0, Op::Etq(*destino));
                self.poner(sigue);
            }
            Sentencia::SaltarIndice(c) => {
                self.e(0, 1, Op::Corto(corto_indice(*c)));
                self.e(59, 1, Op::Lit(0));
            }
            Sentencia::MarcaFija(n, k) => {
                self.e(30, 0, Op::EtqLit(*k));
                self.e(21, 0, Op::Corto(corto_indice(*n)));
            }
            Sentencia::MarcaVariable(n, m) => {
                self.cap.usa_tabla = true;
                self.e(0, 1, Op::Corto(corto_indice(*m)));
                self.e(20, 1, Op::Tabla);
                self.e(21, 0, Op::Corto(corto_indice(*n)));
            }
            Sentencia::Leer(d) => {
                self.rutina(Rutina::Leer, 1);
                match d {
                    LeerDestino::Var(v) => self.guardar_var(v)?,
                    LeerDestino::Indice(c) => {
                        self.a_entero_en_sac();
                        self.e(21, 0, Op::Corto(corto_indice(*c)));
                    }
                }
            }
            Sentencia::Imprimir { expr, m, n } => {
                self.entero_a_sac(*m, 20, 30);
                self.e(21, 0, Op::Corto(mapa::CELDA_M));
                self.entero_a_sac(*n, 20, 30);
                self.e(21, 0, Op::Corto(mapa::CELDA_N));
                self.expr_std(expr, true)?;
                self.rutina(Rutina::Imprimir, 1);
            }
            Sentencia::Newline => self.newline(),
            Sentencia::Space => self.e(62, 0, Op::Lit(SP)),
            Sentencia::End | Sentencia::Halt => self.e(99, 0, Op::Lit(0)),
            Sentencia::Hoot => {
                // Un segundo de nota de unos 1000 ciclos por segundo: 1000 vueltas
                // de 16 instrucciones de 60 µs.
                self.e(30, 0, Op::Lit(-999)); // 38 salta mientras St ≠ 0: 1000 vueltas
                let vuelta = self.nueva();
                self.poner(vuelta);
                self.e(58, 0, Op::Lit(0));
                for _ in 0..14 {
                    self.e(57, 0, Op::Lit(0));
                }
                self.e(38, 0, Op::Etq(vuelta));
            }
            Sentencia::Maquina { funcion, b, operando } => self.maquina(*funcion, *b, operando)?,
            Sentencia::Across(m, c) | Sentencia::Down(m, c) => {
                if !capitulos.contains(c) {
                    return self.err(format!("no hay capítulo {c}"));
                }
                self.e(30, 0, Op::Lit(primer_sector(*c) as i32));
                self.e(10, 1, Op::EtqCap(*c, *m));
                if matches!(s, Sentencia::Across(..)) {
                    self.e(59, 0, Op::Medio(self.entradas.across));
                } else {
                    self.llamar(Op::Medio(self.entradas.down));
                }
            }
            Sentencia::Up => self.e(59, 0, Op::Medio(self.entradas.up)),
            Sentencia::Preserve => self.superposicion(Superposicion::Preservar, 0),
            Sentencia::Restore => self.superposicion(Superposicion::Preservar, 1),
            Sentencia::Rmp => {
                self.cap.superposiciones.insert(Superposicion::Rmp);
                self.e(67, 0, Op::Lit(i32::from(Superposicion::Rmp.sector())));
                self.e(68, 0, Op::Lit(15));
                self.e(59, 0, Op::Medio(Superposicion::ORIGEN));
            }
            Sentencia::Auxiliar { escribir, direccion, variable, cantidad } => {
                let ultimo = *capitulos.last().unwrap_or(&0);
                let limite = (primer_sector(ultimo) - 128) * 32;
                self.transferencia_auxiliar(*escribir, direccion, variable, *cantidad, limite)?;
            }
            Sentencia::IntStep(etiqueta) => self.paso_runge_kutta(*etiqueta)?,
            Sentencia::Matriz { numero, destino, args } => {
                let ultimo = *capitulos.last().unwrap_or(&0);
                self.preparar_matriz(*numero, destino.as_ref(), args, (primer_sector(ultimo) - 128) * 32)?;
            }
            Sentencia::Par { destinos, expr, doble } => self.asignar_par(destinos, expr, *doble)?,
            Sentencia::DobleLongitud => {
                if self.doble_longitud || !self.cap.codigo.is_empty() {
                    return self.err("double length va una vez, después de las reservas y antes de las instrucciones");
                }
                self.doble_longitud = true;
            }
            Sentencia::Psa => self.cap.psa.push(self.cap.codigo.len()),
            Sentencia::Reservar(l, n) => self.reservar(*l, *n)?,
            Sentencia::Quickie(_) | Sentencia::Titulo => {}
            Sentencia::Capitulo(_) | Sentencia::Close | Sentencia::VariablesDe(_) | Sentencia::Programa(_)
            | Sentencia::AcrossPrograma(..) | Sentencia::DownPrograma(..) => unreachable!(),
        }
        Ok(())
    }
}

/// Primer sector del tambor de un capítulo (García Camarero, Apéndice 1:
/// capítulo 0 en 464–479, 1 en 448–463, ..., 21 en 128–143).
pub fn primer_sector(c: u32) -> u16 {
    464 - 16 * c as u16
}

/// Compila las líneas de un capítulo.
pub fn compilar_capitulo(
    numero: u32,
    lineas: &[Linea],
    reserva_previa: Option<&[(char, u16, u16)]>,
    capitulos: &BTreeSet<u32>,
    entradas: Entradas,
) -> R<Capitulo> {
    let mut g = Gen {
        cap: Capitulo {
            numero,
            codigo: Vec::new(),
            etiquetas: HashMap::new(),
            constantes: Vec::new(),
            cortas: Vec::new(),
            usa_tabla: false,
            rutinas: BTreeSet::new(),
            superposiciones: BTreeSet::new(),
            psa: Vec::new(),
            reserva: Vec::new(),
        },
        linea: 0,
        proxima_interna: 1000,
        ciclos: Vec::new(),
        grupos: BTreeMap::new(),
        siguiente_libre: 0,
        etiquetas_pendientes: Vec::new(),
        entradas,
        conserva_b2: lineas.iter().any(|l| matches!(l.sentencia, Some(Sentencia::IntStep(_)))),
        doble_longitud: false,
    };
    if let Some(r) = reserva_previa {
        for &(l, base, tam) in r {
            g.grupos.insert(l, (base, tam));
            g.cap.reserva.push((l, base, tam));
            g.siguiente_libre = g.siguiente_libre.max(base - mapa::PRINCIPALES + tam);
        }
    }
    for l in lineas {
        g.linea = l.numero;
        if let Some(e) = l.etiqueta {
            if g.cap.etiquetas.contains_key(&e) || g.etiquetas_pendientes.contains(&e) {
                return g.err(format!("etiqueta {e} repetida"));
            }
            g.poner(e);
        }
        if let Some(s) = &l.sentencia {
            g.sentencia(s, capitulos)?;
        }
    }
    if let Some(&(_, _, _, _, _, linea)) = g.ciclos.last() {
        g.linea = linea;
        return g.err("ciclo sin repeat");
    }
    // Etiquetas al final del capítulo: apuntan a una parada.
    if !g.etiquetas_pendientes.is_empty() {
        g.e(99, 0, Op::Lit(0));
    }
    Ok(g.cap)
}

/// Codifica una instrucción con su dirección de máquina ya calculada.
fn armar(dec: u8, b: u8, dir: i64) -> Result<u32, String> {
    let (bin, n) = match opcodes::addr_type(dec) {
        AddrType::Short | AddrType::ShortFirstQuarter => {
            let s = dir.rem_euclid(2048);
            if s >= 1024 {
                if opcodes::addr_type(dec) == AddrType::ShortFirstQuarter {
                    return Err(format!("la función {dec:02} sólo llega a la página 0"));
                }
                (opcodes::encode(dec, true).ok_or("sin segundo cuarto")?, (s - 1024) as u16)
            } else {
                (opcodes::encode(dec, false).ok_or("función inexistente")?, s as u16)
            }
        }
        _ => (opcodes::encode(dec, false).ok_or(format!("función inexistente: {dec:02}"))?, dir.rem_euclid(1024) as u16),
    };
    Ok(opcodes::make_instruction(bin, b, n))
}

/// Forma escrita de una dirección para el listado.
fn escrita(dec: u8, dir: i64) -> String {
    match opcodes::addr_type(dec) {
        AddrType::Short | AddrType::ShortFirstQuarter => {
            let s = dir.rem_euclid(2048);
            format!("{}{}", s / 2, if s % 2 == 1 { "+" } else { "" })
        }
        AddrType::Long => format!("{}", dir.rem_euclid(1024) * 2),
        AddrType::Literal => {
            let v = dir.rem_euclid(1024);
            if v >= 512 { format!("{}", v - 1024) } else { format!("{v}") }
        }
        AddrType::Medium => format!("{}", dir.rem_euclid(1024)),
    }
}

/// Asigna direcciones a un capítulo: código desde el registro medio 128,
/// rutinas a continuación, cortas y tabla de etiquetas, y las constantes
/// largas desde el largo 479 hacia abajo.
pub fn armar_capitulo(cap: &Capitulo, todas: &HashMap<u32, HashMap<u32, u16>>) -> R<CapituloArmado> {
    let err = |linea: usize, msg: String| ErrorCompilacion { linea, msg };
    let inicio = mapa::CAPITULO_INICIO;
    let mut medio = inicio + cap.codigo.len() as u16;
    // Rutinas enlazadas.
    let mut rutinas: HashMap<Rutina, (u16, asm::Program)> = HashMap::new();
    for r in &cap.rutinas {
        let p = r.ensamblar(medio).map_err(|e| err(0, format!("rutina {}: {e}", r.nombre())))?;
        let fin = p.shorts.iter().map(|(a, _)| a / 2 + 1).max().unwrap_or(medio as usize) as u16;
        rutinas.insert(*r, (0, p));
        medio = fin;
    }
    // Cortas y tabla de etiquetas.
    let corto_cortas = medio as usize * 2;
    let mut corto_fin = corto_cortas + cap.cortas.len();
    let corto_tabla = corto_fin;
    if cap.usa_tabla {
        corto_fin += 128;
    }
    medio = corto_fin.div_ceil(2) as u16;
    // Constantes largas.
    let largo_min = mapa::CAPITULO_FIN_LARGO as i64 - cap.constantes.len() as i64 + 1;
    let libres = largo_min * 2 - medio as i64;
    if libres < 0 {
        return Err(err(0, format!("el capítulo {} no cabe: faltan {} registros", cap.numero, -libres)));
    }

    // Direcciones de las etiquetas.
    let mut etiquetas: HashMap<u32, u16> = HashMap::new();
    for (e, pos) in &cap.etiquetas {
        etiquetas.insert(*e, inicio + *pos as u16);
    }

    let mut cortos = vec![0u16; 13 * 128];
    let mut poner = |a: usize, v: u16| {
        cortos[a - 256] = v;
    };
    let mut listado = String::new();
    for (k, ins) in cap.codigo.iter().enumerate() {
        let m = inicio as usize + k;
        let etq = |e: u32| -> Result<i64, ErrorCompilacion> {
            etiquetas
                .get(&e)
                .map(|a| *a as i64)
                .ok_or_else(|| err(ins.linea, format!("etiqueta {e} sin definir")))
        };
        let dir: i64 = match ins.op {
            Op::Lit(v) => v as i64,
            Op::Corto(a) => a as i64,
            Op::Largo(a) => a as i64,
            Op::Medio(a) => a as i64,
            Op::Etq(e) | Op::EtqLit(e) => etq(e)?,
            Op::EtqCap(c, e) => todas
                .get(&c)
                .and_then(|t| t.get(&e))
                .map(|a| *a as i64)
                .ok_or_else(|| err(ins.linea, format!("etiqueta {e} sin definir en el capítulo {c}")))?,
            Op::Const(k) | Op::ConstLit(k) => mapa::CAPITULO_FIN_LARGO as i64 - k as i64,
            Op::ConstCorta(k) => (corto_cortas + k) as i64,
            Op::Tabla => corto_tabla as i64,
            Op::Rutina(r, e) => rutinas[&r].1.labels[&e] as i64,
            Op::Palabra(_) => 0,
        };
        let palabra = match ins.op {
            Op::Palabra(w) => w,
            _ => armar(ins.dec, ins.b, dir).map_err(|m| err(ins.linea, m))?,
        };
        poner(2 * m, (palabra >> 10) as u16);
        poner(2 * m + 1, (palabra & 0x3ff) as u16);
        let texto = match ins.op {
            Op::Palabra(_) => format!("palabra {:07o}", palabra),
            _ => format!("{:02}{} {}", ins.dec, ins.b, escrita(ins.dec, dir)),
        };
        let marcas: Vec<String> = cap
            .etiquetas
            .iter()
            .filter(|(e, p)| **p == k && **e < 1000)
            .map(|(e, _)| format!("{e})"))
            .collect();
        listado.push_str(&format!("{:>4}  {:<14} {:<6} línea {}\n", m, texto, marcas.join(" "), ins.linea));
    }
    for (r, (_, p)) in &rutinas {
        for &(a, v) in &p.shorts {
            poner(a, v);
        }
        listado.push_str(&format!("{:>4}  rutina {}\n", p.labels[&1], r.nombre()));
    }
    for (k, v) in cap.cortas.iter().enumerate() {
        poner(corto_cortas + k, (*v).rem_euclid(1024) as u16);
    }
    if cap.usa_tabla {
        for (e, a) in &etiquetas {
            if *e < 128 {
                poner(corto_tabla + *e as usize, *a);
            }
        }
    }
    for (k, bits) in cap.constantes.iter().enumerate() {
        let l = mapa::CAPITULO_FIN_LARGO as usize - k;
        let f = mercury_core::acc::Float::from_f64(f64::from_bits(*bits));
        for (j, s) in f.to_long().iter().enumerate() {
            poner(4 * l + j, *s);
        }
    }
    // psa: registros libres si el capítulo terminara en ese punto.
    let psa = cap.psa.iter().map(|p| libres as usize + cap.codigo.len() - p).collect();
    Ok(CapituloArmado {
        numero: cap.numero,
        paginas: cortos,
        etiquetas: etiquetas.into_iter().filter(|(e, _)| *e < 1000).collect(),
        libres: libres as usize,
        listado,
        psa,
    })
}
