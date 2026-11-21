//! TINY 2.1 de WollyMonjac. Proyecciones SEQ en código Mercury.
use mercury_core::{
    acc::{self, AddMode, Float, MulMode},
    asm,
    io::{BufferIo, NullIo},
    store::signed10,
    tape, Machine, Stop,
};
use std::{
    io::{self, BufRead, Write},
    path::Path,
};

const D: usize = 80;
const HD: usize = 20;
const HID: usize = 256;
const CTX: usize = 80;
const V: usize = 55;
const CINTA: &str = " !\"'()*+,-./0123456789:;=?\\abcdefghijklmnopqrstuvwxyz";
const X: usize = 19 * 32;
const OUT: usize = 7 * 32;
const DATOS: usize = 49;
const RESERVA_KV: usize = 50;
const DESCRIPTORES: usize = 25;
const DIMENSIONES: [(usize, usize); 6] = [(D, D), (D, HD), (D, HD), (D, D), (D, HID), (HID, D)];

fn f(x: f64) -> Float {
    Float::from_f64(x)
}

fn suma(a: Float, b: Float) -> Float {
    acc::add(a, b, false, AddMode::Unrounded).unwrap().0
}

fn resta(a: Float, b: Float) -> Float {
    acc::add(a, b, true, AddMode::Unrounded).unwrap().0
}

fn producto(a: Float, b: Float) -> Float {
    acc::mul(a, b, false, MulMode::Unrounded).unwrap()
}

struct Motor {
    m: Machine,
    campos: [u16; 128],
    emb: Vec<Float>,
    pasos: Vec<Float>,
    normas: Vec<Vec<Float>>,
}

impl Motor {
    fn cargar(path: &Path) -> Result<Self, Box<dyn std::error::Error>> {
        let datos = std::fs::read(path)?;
        if datos.len() != 512 * 128 * 2 {
            return Err("el tambor debe tener exactamente 512 sectores de 128 cortos".into());
        }
        let mut m = Machine::new(Default::default());
        for (n, sector) in datos.chunks_exact(256).enumerate() {
            for (destino, corto) in m.drum.sector_mut(n).iter_mut().zip(sector.chunks_exact(2)) {
                *destino = u16::from_le_bytes([corto[0], corto[1]]);
                if *destino > 1023 {
                    return Err(format!("corto fuera de rango en sector {n}").into());
                }
            }
        }
        let campos = *m.drum.sector(0);
        if !campos[..6]
            .iter()
            .copied()
            .eq(b"TINYC1".iter().copied().map(u16::from))
        {
            return Err("firma de tambor TINY incorrecta".into());
        }
        let ocupados = campos[DATOS] as usize;
        if ocupados == 0
            || ocupados + campos[RESERVA_KV] as usize > 512
            || (campos[RESERVA_KV] as usize) * 128 < 4 * CTX * 14
        {
            return Err("límite de datos o espacio KV inválido".into());
        }
        for (i, &puntero) in campos[6..DATOS].iter().enumerate() {
            if puntero as usize >= ocupados || (puntero == 0 && !(33..=36).contains(&i)) {
                return Err(format!("descriptor de sector inválido: {i}").into());
            }
        }
        let motor_mercury = [
            asm::assemble(include_str!("../../../ports/tiny/tmac.mer"))?,
            asm::assemble(include_str!("../../../ports/tiny/vectores.mer"))?,
            asm::assemble_at(include_str!("../../mercury-autocode/runtime/exp.mer"), 64)?,
            asm::assemble_at(
                &format!(
                    "{}\n596 0",
                    include_str!("../../../input_routine/quickies/01.mer").replace("@128", "")
                ),
                148,
            )?,
        ];
        let mut memoria = [false; 2048];
        for programa in motor_mercury {
            for &(i, _) in &programa.shorts {
                if i >= 896 || std::mem::replace(&mut memoria[i], true) {
                    return Err(format!("el código de TINY se superpone en el corto {i}").into());
                }
            }
            programa.load(&mut m.store);
        }
        m.store.set_long(1022, f(-1.0).to_long()); // 31.60, para exp
        m.store.set_long(1021, f(0.0).to_long()); // 31.58
        m.store.set_long(28, Float::default().to_long()); // 56, cero de exponente 0
        let mut motor = Self {
            m,
            campos,
            emb: Vec::new(),
            pasos: Vec::new(),
            normas: Vec::new(),
        };
        motor.emb = motor.tensor(6, V * D);
        motor.pasos = motor.tensor(7, 4 * D);
        for i in 0..17 {
            let largo = if i % 4 == 3 { HID } else { D };
            motor.normas.push(motor.tensor(8 + i, largo));
        }
        Ok(motor)
    }

    fn corto(&self, direccion: usize) -> u16 {
        self.m.drum.sector(direccion / 128)[direccion % 128]
    }

    fn largo(&self, direccion: usize) -> Float {
        Float::from_long(std::array::from_fn(|i| self.corto(direccion + i)))
    }

    fn tensor(&self, campo: usize, longitud: usize) -> Vec<Float> {
        let p = self.campos[campo] as usize * 128;
        let escala = self.largo(p);
        (0..longitud)
            .map(|i| producto(escala, f(signed10(self.corto(p + 4 + i)) as f64)))
            .collect()
    }

    fn proyectar(&mut self, bloque: usize, tipo: usize, activacion: &[Float]) -> Vec<Float> {
        let campo = DESCRIPTORES + 2 * (bloque * 6 + tipo);
        let (columnas, filas) = DIMENSIONES[tipo];
        assert_eq!(activacion.len(), columnas);
        let ancho = columnas.div_ceil(5) * 5;
        for i in 0..ancho {
            self.m.store.set_long(
                X + i,
                activacion
                    .get(i)
                    .copied()
                    .unwrap_or_else(|| f(0.0))
                    .to_long(),
            );
        }
        for (i, p) in [
            ancho as u16,
            filas as u16,
            self.campos[campo],
            self.campos[campo + 1],
        ]
        .into_iter()
        .enumerate()
        {
            self.m.store.set_short(120 + i, p);
        }
        self.ejecutar(176);
        (0..filas)
            .map(|i| Float::from_long(self.m.store.long(OUT + i)))
            .collect()
    }

    fn ejecutar(&mut self, entrada: u16) {
        self.m.goto(entrada);
        let parada = self.m.run(&mut NullIo, 1_000_000);
        assert_eq!(
            parada,
            Some(Stop::Stop99),
            "rutina {entrada}, C={}",
            self.m.c
        );
        assert_eq!(
            self.m
                .store
                .medium(self.m.c.wrapping_sub(1) as usize & 1023),
            0,
            "falla en rutina {entrada}"
        );
    }

    fn preparar_x(&mut self, x: &[Float]) {
        self.m.store.set_short(121, x.len() as u16);
        for (i, v) in x.iter().enumerate() {
            self.m.store.set_long(X + i, v.to_long());
        }
    }

    fn preparar_y(&mut self, y: &[Float]) {
        for (i, v) in y.iter().enumerate() {
            self.m.store.set_long(OUT + i, v.to_long());
        }
    }

    fn extraer_x(&self, n: usize) -> Vec<Float> {
        (0..n)
            .map(|i| Float::from_long(self.m.store.long(X + i)))
            .collect()
    }

    fn normalizar(&mut self, x: &[Float], norma: usize) -> Vec<Float> {
        self.preparar_x(x);
        for (i, v) in self.normas[norma].iter().enumerate() {
            self.m.store.set_long(OUT + i, v.to_long());
        }
        self.m.store.set_long(5, f(1.0 / x.len() as f64).to_long());
        self.ejecutar(5 * 64);
        self.extraer_x(x.len())
    }

    fn sumar_vectores(&mut self, x: &[Float], y: &[Float]) -> Vec<Float> {
        self.preparar_x(x);
        self.preparar_y(y);
        self.ejecutar(5 * 64 + 48);
        self.extraer_x(x.len())
    }

    fn punto(&mut self, x: &[Float], y: &[Float]) -> Float {
        self.preparar_x(x);
        self.preparar_y(y);
        self.ejecutar(6 * 64);
        self.m.acc
    }

    fn transformar(&mut self, entrada: u16, v: Float) -> Float {
        self.m.acc = v;
        self.ejecutar(entrada);
        self.m.acc
    }

    fn direccion_kv(&self, capa: usize, posicion: usize, valor: usize) -> usize {
        self.campos[DATOS] as usize * 128 + ((capa * CTX + posicion) * 2 + valor) * 14
    }

    fn guardar_kv(&mut self, capa: usize, pos: usize, valor: usize, vec: &[Float]) {
        let p = self.direccion_kv(capa, pos, valor);
        let maximo = vec.iter().map(|x| x.to_f64().abs()).fold(0.0, f64::max);
        let escala = f(maximo.max(1e-8) / 15.0);
        let quant = |i: usize| {
            ((vec[i].to_f64() / escala.to_f64())
                .round_ties_even()
                .clamp(-15.0, 15.0) as i16 as u16)
                & 31
        };
        let cortos = escala
            .to_long()
            .into_iter()
            .chain((0..10).map(|i| (quant(2 * i) << 5) | quant(2 * i + 1)));
        for (i, corto) in cortos.enumerate() {
            let a = p + i;
            self.m.drum.sector_mut(a / 128)[a % 128] = corto;
        }
    }

    fn leer_kv(&self, capa: usize, pos: usize, valor: usize) -> [Float; HD] {
        let p = self.direccion_kv(capa, pos, valor);
        let escala = self.largo(p);
        std::array::from_fn(|i| {
            let par = self.corto(p + 4 + i / 2);
            let q = (par >> (if i % 2 == 0 { 5 } else { 0 })) & 31;
            producto(escala, f(if q >= 16 { q as f64 - 32.0 } else { q as f64 }))
        })
    }

    fn atender(&mut self, q: &[Float], capa: usize, pos: usize) -> Vec<Float> {
        let k: Vec<_> = (0..=pos).map(|j| self.leer_kv(capa, j, 0)).collect();
        let v: Vec<_> = (0..=pos).map(|j| self.leer_kv(capa, j, 1)).collect();
        let mut resultado = Vec::with_capacity(D);
        let escala = f(1.0 / (HD as f64).sqrt());
        for (cabeza, q) in q.chunks_exact(HD).enumerate() {
            let pendiente = 2.0_f64.powi(-2 * (cabeza as i32 + 1));
            let puntajes: Vec<_> = k
                .iter()
                .enumerate()
                .map(|(j, k)| {
                    let punto = self.punto(q, k);
                    resta(producto(punto, escala), f((pos - j) as f64 * pendiente)).to_f64()
                })
                .collect();
            let maximo = puntajes.iter().copied().fold(f64::NEG_INFINITY, f64::max);
            let pesos: Vec<_> = puntajes
                .into_iter()
                .map(|s| self.transformar(6 * 64 + 24, f(s - maximo)))
                .collect();
            let total = pesos.iter().copied().fold(f(0.0), suma);
            let inv = self.transformar(6 * 64 + 16, total);
            for d in 0..HD {
                let columna: Vec<_> = v.iter().map(|v| v[d]).collect();
                let punto = self.punto(&pesos, &columna);
                resultado.push(producto(punto, inv));
            }
        }
        resultado
    }

    fn paso(&mut self, token: usize, pos: usize, completo: bool) -> Option<usize> {
        let mut x = self.emb[token * D..(token + 1) * D].to_vec();
        for capa in 0..if completo { 4 } else { 3 } {
            let bloque = capa % 2;
            for (i, valor) in x.iter_mut().enumerate() {
                *valor = suma(*valor, self.pasos[capa * D + i]);
            }
            let h = self.normalizar(&x, capa * 4);
            if capa % 2 == 0 {
                let k = self.proyectar(bloque, 1, &h);
                let v = self.proyectar(bloque, 2, &h);
                self.guardar_kv(capa / 2, pos, 0, &k);
                self.guardar_kv(capa / 2, pos, 1, &v);
            }
            if !completo && capa == 2 {
                return None;
            }
            let q = self.proyectar(bloque, 0, &h);
            let a = self.atender(&q, capa / 2, pos);
            let a = self.normalizar(&a, capa * 4 + 1);
            let o = self.proyectar(bloque, 3, &a);
            x = self.sumar_vectores(&x, &o);
            let h = self.normalizar(&x, capa * 4 + 2);
            let u = self.proyectar(bloque, 4, &h);
            self.preparar_x(&u);
            self.ejecutar(5 * 64 + 32);
            let u = self.extraer_x(HID);
            let u = self.normalizar(&u, capa * 4 + 3);
            let f2 = self.proyectar(bloque, 5, &u);
            x = self.sumar_vectores(&x, &f2);
        }
        let h = self.normalizar(&x, 16);
        let mut elegido = (0, f64::NEG_INFINITY);
        for i in 0..V {
            let emb = self.emb[i * D..(i + 1) * D].to_vec();
            let score = self.punto(&h, &emb).to_f64();
            if score > elegido.1 {
                elegido = (i, score);
            }
        }
        Some(elegido.0)
    }

    fn responder(&mut self, pregunta: &str) -> Result<String, &'static str> {
        let pregunta = pregunta.to_lowercase();
        let tokens: Vec<_> = pregunta
            .trim()
            .chars()
            .map(|c| {
                CINTA
                    .find(c)
                    .map(|i| i + 2)
                    .ok_or("la pregunta contiene un carácter fuera del vocabulario")
            })
            .collect::<Result<_, _>>()?;
        if tokens.len() + 1 >= CTX {
            return Err("la pregunta deja lugar insuficiente en el contexto de 80 tokens");
        }
        let mut pos = 0;
        for tok in tokens {
            self.paso(tok, pos, false);
            pos += 1;
        }
        let mut salida = String::new();
        let mut tok = self.paso(1, pos, true).unwrap();
        pos += 1;
        while tok >= 2 && salida.len() < 48 && pos < CTX {
            salida.push(CINTA.as_bytes()[tok - 2] as char);
            if salida.len() == 48 || pos + 1 == CTX {
                break;
            }
            tok = self.paso(tok, pos, true).unwrap();
            pos += 1;
        }
        Ok(salida)
    }

    fn verificar_proyecciones(&mut self) {
        for x in [0.01_f64, 0.5, 1.0, 3.0, 7.0, 100.0] {
            let inv = self.transformar(6 * 64 + 16, f(x)).to_f64();
            assert!((x * inv - 1.0).abs() < 1e-7, "inversa {x}: {inv}");
        }
        for x in [-100.0_f64, -3.0, -1.0, -0.25, 0.0] {
            let exp = self.transformar(6 * 64 + 24, f(x)).to_f64();
            assert!((exp / x.exp() - 1.0).abs() < 1e-7, "exponencial {x}: {exp}");
        }
        let h: Vec<_> = (0..D).map(|i| f(i as f64 - 20.0)).collect();
        let normal = self.normalizar(&h, 0);
        let media = h.iter().map(|x| x.to_f64().abs()).sum::<f64>() / D as f64;
        for (i, obtenido) in normal.iter().enumerate() {
            let esperado = h[i].to_f64() / media * self.normas[0][i].to_f64();
            assert!(
                (obtenido.to_f64() - esperado).abs() < 1e-6,
                "normalización {i}: {obtenido:?} vs {esperado}"
            );
        }
        let producto_ref = h
            .iter()
            .zip(&h)
            .map(|(a, b)| a.to_f64() * b.to_f64())
            .sum::<f64>();
        assert_eq!(self.punto(&h, &h).to_f64(), producto_ref);
        let mut total = 0;
        for bloque in 0..2 {
            for (tipo, &(columnas, filas)) in DIMENSIONES.iter().enumerate() {
                let campo = DESCRIPTORES + 2 * (bloque * 6 + tipo);
                let sector = self.campos[campo] as usize;
                if sector == 0 {
                    continue;
                }
                for caso in 0..3 {
                    let x: Vec<_> = (0..columnas)
                        .map(|i| f(((i * 31 + caso * 7) % 97) as f64 / 32.0 - 1.5))
                        .collect();
                    let salida = self.proyectar(bloque, tipo, &x);
                    for (fila, &resultado) in salida.iter().enumerate() {
                        let mut suma_real = 0.0;
                        for (g, valores) in x.chunks(5).enumerate() {
                            let p = sector * 128 + g * (filas / 2) * 3 + (fila / 2) * 3;
                            let (a, b, c) = (self.corto(p), self.corto(p + 1), self.corto(p + 2));
                            let (p2, p1, p0) = if fila % 2 == 0 {
                                (a >> 5, a & 31, b >> 5)
                            } else {
                                (b & 31, c >> 5, c & 31)
                            };
                            for (i, v) in valores.iter().enumerate() {
                                let u = 4 * ((p2 >> i) & 1) + 2 * ((p1 >> i) & 1) + ((p0 >> i) & 1);
                                suma_real += (2.0 * f64::from(u) - 7.0) * v.to_f64();
                            }
                        }
                        let coef = self.campos[campo + 1] as usize * 128 + fila * 8;
                        // Las entradas de esta prueba son diádicas. Las
                        // sumas de los niveles impares son exactas en FP40.
                        let esperado = suma(
                            producto(f(suma_real), self.largo(coef)),
                            self.largo(coef + 4),
                        );
                        assert_eq!(
                            resultado.to_long(),
                            esperado.to_long(),
                            "bloque {bloque}, matriz {tipo}, fila {fila}"
                        );
                        total += 1;
                    }
                }
            }
        }
        println!(
            "{total} salidas de proyección coinciden palabra por palabra con el producto escalar."
        );
    }

    fn perforar(
        &mut self,
        pregunta: &str,
        respuesta: &str,
        path: &Path,
    ) -> Result<(), Box<dyn std::error::Error>> {
        let texto = format!("TINY 2.1 / WOLLYMONJAC / CLEMENTINA\n{pregunta}\n{respuesta}\n");
        // El texto del lanzador conserva el vocabulario del checkpoint.
        // Las grafías que no existen en el código Ferranti salen como '?'.
        let texto: String = texto
            .chars()
            .map(|c| {
                if matches!(c, '!' | '\'' | '"' | ':' | ';' | '\\') {
                    '?'
                } else {
                    c
                }
            })
            .collect();
        let filas =
            tape::encode(&texto).map_err(|c| format!("carácter inválido en la cinta: {c}"))?;
        let mut io = BufferIo::with_tape(&filas);
        self.m.store.set_short(121, filas.len() as u16);
        self.m.goto(6 * 64 + 36);
        if self.m.run(&mut io, 10_000) != Some(Stop::Stop99) || io.output[1] != filas {
            return Err("la perforación se interrumpió antes del final".into());
        }
        std::fs::write(
            path,
            tape::to_svg_with(&io.output[1], &tape::SvgOptions::default()),
        )?;
        Ok(())
    }
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut args = std::env::args().skip(1);
    let imagen = args
        .next()
        .ok_or("uso: tiny_clementina TAMBOR.bin [PREGUNTA [--svg ARCHIVO] | --verificar]")?;
    let pregunta = args.next();
    let svg = match args.next() {
        None => None,
        Some(opcion) if opcion == "--svg" && pregunta.as_deref() != Some("--verificar") => {
            Some(args.next().ok_or("falta el archivo SVG")?)
        }
        Some(opcion) => return Err(format!("opción inesperada: {opcion}").into()),
    };
    if args.next().is_some() {
        return Err("sobran argumentos".into());
    }
    let mut motor = Motor::cargar(Path::new(&imagen))?;
    println!("TINY 2.1 de WollyMonjac | Clementina | r3_80x2L2x256");
    let mut responder = |pregunta: &str| -> Result<(), Box<dyn std::error::Error>> {
        let t = motor.m.time_us;
        let pc = std::time::Instant::now();
        let respuesta = motor.responder(pregunta)?;
        println!("{respuesta}");
        eprintln!(
            "{:.2} s en PC | {:.1} s de máquina en las rutinas emuladas",
            pc.elapsed().as_secs_f64(),
            (motor.m.time_us - t) as f64 / 1e6
        );
        if let Some(path) = &svg {
            motor.perforar(pregunta.trim(), &respuesta, Path::new(path))?;
        }
        Ok(())
    };
    match pregunta {
        Some(p) if p == "--verificar" => motor.verificar_proyecciones(),
        Some(p) => responder(&p)?,
        None => {
            let mut entrada = io::stdin().lock();
            let mut pregunta = String::new();
            loop {
                print!("> ");
                io::stdout().flush()?;
                pregunta.clear();
                if entrada.read_line(&mut pregunta)? == 0 || pregunta.trim().is_empty() {
                    break;
                }
                if let Err(e) = responder(pregunta.trim()) {
                    eprintln!("{e}");
                }
            }
        }
    }
    Ok(())
}
