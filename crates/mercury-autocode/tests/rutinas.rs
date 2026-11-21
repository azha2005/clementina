//! Pruebas de las rutinas de apoyo en código Mercury, sin el compilador.

use mercury_autocode::runtime::{self, Rutina};
use mercury_core::acc::Float;
use mercury_core::asm;
use mercury_core::io::BufferIo;
use mercury_core::tape;
use mercury_core::{Machine, Stop};

const ORIGEN_RUTINA: u16 = 600;

/// Carga la página 0, una rutina en 600 y un programa de prueba en 128.
fn maquina(rutina: Option<Rutina>, prueba: &str) -> Machine {
    let mut m = Machine::new(Default::default());
    runtime::pagina0().unwrap().load(&mut m.store);
    if let Some(r) = rutina {
        let p = r.ensamblar(ORIGEN_RUTINA).unwrap();
        assert!(p.shorts.iter().all(|(a, _)| *a < 1920), "la rutina se pasa de la página 14");
        p.load(&mut m.store);
    }
    let p = asm::assemble_at(prueba, 128).unwrap();
    p.load(&mut m.store);
    m.c = p.start;
    m
}

fn correr(m: &mut Machine, io: &mut BufferIo) {
    match m.run(io, 2_000_000) {
        Some(Stop::Stop99) => {}
        otro => panic!("terminó con {otro:?}"),
    }
}

fn poner(m: &mut Machine, largo: u16, v: f64) {
    m.store.set_long(largo as usize, Float::from_f64(v).to_long());
}

fn leer(m: &Machine, largo: u16) -> f64 {
    Float::from_long(m.store.long(largo as usize)).to_f64()
}

#[test]
fn pagina0_no_pisa_el_lugar_de_trabajo_ni_los_indices() {
    let p = runtime::pagina0().unwrap();
    for (a, _) in &p.shorts {
        let medio = a / 2;
        assert!(
            !(64..112).contains(&medio)
                && !(118..64 * 31).contains(&medio)
                && !(32..40).contains(&medio)
                && !(58..64).contains(&medio),
            "la página 0 escribe en el registro medio {medio}"
        );
    }
}

#[test]
fn division() {
    // A = L(600) / L(602) con B1 = largo del divisor y vuelta por B2.
    let prueba = "
        400 1000
        101 501
        102 v1
        590 4
        410 1004 (1
        990 0
    ";
    for (a, b) in [(1.0, 3.0), (-7.5, 2.5), (22.0, -7.0), (1e-10, 3e20), (355.0, 113.0), (6.0, 0.5), (1.0, 0.75)] {
        let mut m = maquina(None, prueba);
        poner(&mut m, 500, a);
        poner(&mut m, 501, b);
        correr(&mut m, &mut BufferIo::default());
        let q = leer(&m, 502);
        let esperado = a / b;
        assert!(((q - esperado) / esperado).abs() < 1e-8, "{a}/{b}: {q} en vez de {esperado}");
    }
}

/// Imprime x con m, n y devuelve el texto perforado.
fn imprimir(x: f64, m: u16, n: u16) -> String {
    let prueba = format!(
        "
        300 {m}
        210 0+
        300 {n}
        210 1
        400 1000
        102 v1
        590 {ORIGEN_RUTINA}
        990 0    (1
    "
    );
    let mut maq = maquina(Some(Rutina::Imprimir), &prueba);
    poner(&mut maq, 500, x);
    let mut io = BufferIo::default();
    correr(&mut maq, &mut io);
    tape::decode(&io.output[1])
}

#[test]
fn imprimir_punto_fijo() {
    assert_eq!(imprimir(-3.27721675, 2, 5), " -3.27722  ");
    assert_eq!(imprimir(12.0, 4, 0), "   12  ");
    assert_eq!(imprimir(0.25, 1, 3), " 0.250  ");
    assert_eq!(imprimir(0.0, 1, 2), " 0.00  ");
    assert_eq!(imprimir(123456.0, 2, 1), " 123456.0  ");
    assert_eq!(imprimir(-0.004, 3, 2), "  -0.00  ");
    assert_eq!(imprimir(99.996, 2, 2), " 100.00  ");
}

#[test]
fn imprimir_flotante() {
    assert_eq!(imprimir(1234.5, 0, 3), " 1.235,  3  ");
    assert_eq!(imprimir(-0.00025, 0, 2), "-2.50, -4  ");
    assert_eq!(imprimir(0.0, 0, 1), " 0.0,-128  ");
    assert_eq!(imprimir(6.02e23, 0, 4), " 6.0200, 23  ");
    assert_eq!(imprimir(-1.5e-12, 0, 1), "-1.5,-12  ");
}

/// Lee un número de la cinta y devuelve el valor.
fn leer_cinta(texto: &str) -> Vec<f64> {
    let prueba = format!(
        "
        102 v1   (2
        590 {ORIGEN_RUTINA}
        410 1004 (1
        990 0
    "
    );
    let cinta = tape::encode(texto).unwrap();
    let mut io = BufferIo::with_tape(&cinta);
    let mut valores = Vec::new();
    let mut maq = maquina(Some(Rutina::Leer), &prueba);
    while io.input_pos[1] < io.input[1].len() {
        maq.goto(128);
        match maq.run(&mut io, 2_000_000) {
            Some(Stop::Stop99) => valores.push(leer(&maq, 502)),
            Some(Stop::InputExhausted { .. }) => break,
            otro => panic!("{otro:?}"),
        }
    }
    valores
}

#[test]
fn leer_numeros() {
    let v = leer_cinta("2.2575\n-17\n1000,-3.0\n-2.5,-3\n0.001\n12345678\n");
    let esperado = [2.2575, -17.0, 1.0, -0.0025, 0.001, 12345678.0];
    assert_eq!(v.len(), esperado.len(), "{v:?}");
    for (a, b) in v.iter().zip(esperado) {
        assert!(((a - b) / b).abs() < 1e-8, "{a} en vez de {b}");
    }
    // Los enteros entran exactos.
    assert_eq!(v[1], -17.0);
    assert_eq!(v[5], 12345678.0);
}


/// Error de la división en unidades del último bit de la mantisa del cociente.
fn error_ulp(q: f64, exacto: f64) -> f64 {
    let e = exacto.abs().log2().floor() + 1.0; // exponente de la mantisa en [0.5, 1)
    (q - exacto).abs() / 2f64.powf(e - 29.0)
}

#[test]
fn division_barrido() {
    // Unos miles de pares seudoaleatorios y de cocientes exactos (k·b)/b.
    let prueba = "
        400 1000
        101 501
        102 v1
        590 4
        410 1004 (1
        990 0
    ";
    let mut semilla: u64 = 12345;
    let mut azar = || {
        semilla = semilla.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
        (semilla >> 11) as f64 / (1u64 << 53) as f64
    };
    let mut peor: f64 = 0.0;
    let mut enteros_por_debajo = 0;
    for k in 0..3000 {
        let (a, b) = if k % 3 == 0 {
            let b = (azar() * 1000.0).floor() + 1.0;
            let kk = (azar() * 500.0).floor() + 1.0;
            (kk * b, b)
        } else {
            let s = if azar() < 0.5 { -1.0 } else { 1.0 };
            (s * azar() * 10f64.powf(azar() * 20.0 - 10.0), (azar() + 0.01) * 10f64.powf(azar() * 20.0 - 10.0))
        };
        let mut m = maquina(None, prueba);
        poner(&mut m, 500, a);
        poner(&mut m, 501, b);
        correr(&mut m, &mut BufferIo::default());
        let pa = Float::from_long(m.store.long(500)).to_f64();
        let pb = Float::from_long(m.store.long(501)).to_f64();
        let q = leer(&m, 502);
        let exacto = pa / pb;
        peor = peor.max(error_ulp(q, exacto));
        if k % 3 == 0 && q < exacto {
            enteros_por_debajo += 1;
        }
    }
    eprintln!("división: peor error {peor:.2} unidades del último bit; {enteros_por_debajo} de 1000 cocientes enteros quedan por debajo");
    // Con el redondeo de la Mercury (forzar un 1) un cociente entero exacto
    // queda arriba o abajo más o menos la mitad de las veces: int pt(x/h)
    // puede dar uno menos, como advierte el manual.
    assert!(peor <= 3.0, "peor error {peor} unidades");
    assert!(enteros_por_debajo < 700, "{enteros_por_debajo} cocientes enteros por debajo");
}

/// Corre una función (rutina, etiqueta de entrada) sobre x (y en el largo 51).
fn funcion(r: Rutina, entrada: u32, x: f64, y: f64) -> Result<f64, Stop> {
    let e = r.ensamblar(ORIGEN_RUTINA).unwrap().labels[&entrada];
    let prueba = format!(
        "
        400 1000
        102 v1
        590 {e}
        410 1004 (1
        990 0
    "
    );
    let mut m = maquina(Some(r), &prueba);
    poner(&mut m, 500, x);
    poner(&mut m, 51, y);
    match m.run(&mut BufferIo::default(), 1_000_000) {
        Some(Stop::Stop99) => Ok(leer(&m, 502)),
        Some(s) => Err(s),
        None => panic!("no terminó"),
    }
}

/// Peor error relativo de f sobre una lista de argumentos.
fn peor(r: Rutina, entrada: u32, xs: &[f64], exacta: impl Fn(f64) -> f64) -> f64 {
    let mut peor: f64 = 0.0;
    for &x in xs {
        let xm = Float::from_f64(x).to_f64();
        let v = funcion(r, entrada, xm, 0.0).unwrap_or_else(|s| panic!("{x}: {s:?}"));
        let ex = exacta(xm);
        let err = if ex.abs() < 1e-30 { v.abs() } else { ((v - ex) / ex).abs() };
        assert!(err < 2e-8, "{} ({x}) = {v}, debía ser {ex}", r.nombre());
        peor = peor.max(err);
    }
    peor
}

fn barrido(desde: f64, hasta: f64, n: usize) -> Vec<f64> {
    (0..n).map(|k| desde + (hasta - desde) * k as f64 / (n - 1) as f64).collect()
}

#[test]
fn funciones_de_biblioteca() {
    let mut xs = barrido(0.001, 10.0, 300);
    xs.extend([1e-20, 2.0, 4.0, 0.25, 1e30, 12345.678]);
    eprintln!("sqrt: {:.1e}", peor(Rutina::Raiz, 1, &xs, f64::sqrt));
    let mut xs = barrido(-80.0, 80.0, 300);
    xs.extend([0.0, 1.0, -1.0, 1e-9, 170.0, -170.0]);
    eprintln!("exp: {:.1e}", peor(Rutina::Exp, 1, &xs, f64::exp));
    let mut xs = barrido(0.01, 50.0, 300);
    xs.extend([1.0 + 1e-6, 0.999, 1e-30, 1e30, 2.0, 0.5]);
    eprintln!("log: {:.1e}", peor(Rutina::Log, 1, &xs, f64::ln));
    let xs: Vec<f64> = barrido(-20.0, 20.0, 401).into_iter().filter(|x| x.abs() > 1e-3).collect();
    eprintln!("sin: {:.1e}", peor(Rutina::Trig, 1, &xs, f64::sin));
    eprintln!("cos: {:.1e}", peor(Rutina::Trig, 2, &xs, f64::cos));
    let xs: Vec<f64> = barrido(-1.5, 1.5, 301).into_iter().filter(|x| x.abs() > 1e-3).collect();
    eprintln!("tan: {:.1e}", peor(Rutina::Trig, 3, &xs, f64::tan));
}

#[test]
fn arcotangente() {
    let mut peor: f64 = 0.0;
    for k in 0..360 {
        let a = (k as f64 + 0.37) * std::f64::consts::PI / 180.0 - std::f64::consts::PI;
        for r in [0.001, 1.0, 1e6] {
            let (x, y) = (r * a.cos(), r * a.sin());
            let (xm, ym) = (Float::from_f64(x).to_f64(), Float::from_f64(y).to_f64());
            let v = funcion(Rutina::Arctan, 1, xm, ym).unwrap();
            let ex = ym.atan2(xm);
            let err = ((v - ex) / ex).abs();
            assert!(err < 2e-8, "arctan({x}, {y}) = {v}, debía ser {ex}");
            peor = peor.max(err);
        }
    }
    eprintln!("arctan: {peor:.1e}");
    assert_eq!(funcion(Rutina::Arctan, 1, 0.0, 0.0).unwrap(), 0.0);
    assert!((funcion(Rutina::Arctan, 1, -1.0, 0.0).unwrap() - std::f64::consts::PI).abs() < 1e-8);
}

#[test]
fn desbordes_de_exp_y_log() {
    assert_eq!(funcion(Rutina::Exp, 1, 200.0, 0.0), Err(Stop::Overflow));
    assert_eq!(funcion(Rutina::Exp, 1, 1000.0, 0.0), Err(Stop::Overflow));
    assert_eq!(funcion(Rutina::Exp, 1, -200.0, 0.0), Ok(0.0));
    assert_eq!(funcion(Rutina::Exp, 1, -1000.0, 0.0), Ok(0.0));
    assert_eq!(funcion(Rutina::Log, 1, 0.0, 0.0), Err(Stop::Overflow));
    assert_eq!(funcion(Rutina::Log, 1, -3.0, 0.0), Err(Stop::Overflow));
}

#[test]
fn doble_longitud_barrido() {
    use mercury_autocode::runtime::Superposicion;
    let programa = Superposicion::Doble.ensamblar().unwrap();
    let salida = mercury_core::asm::assemble("@961\n990 0").unwrap();
    let mut m = Machine::new(Default::default());
    let pagina0 = mercury_autocode::runtime::pagina0().unwrap();
    let parte = |v: f64| (v, Float::from_f64(v).to_f64());
    for k in -64..=64 {
        let (ah, a) = parte(1.0 + f64::from(k) / 17.0);
        let (ab, a_bajo) = parte(f64::from(k + 3) * 2f64.powi(-39));
        let (bh, b) = parte(0.5 + f64::from(k) / 23.0);
        let (bb, b_bajo) = parte(f64::from(3 - k) * 2f64.powi(-39));
        let a = a + a_bajo;
        let b = b + b_bajo;
        for (op, esperado) in [(0, a + b), (1, a - b), (2, a * b), (3, a / b)] {
            pagina0.load(&mut m.store);
            programa.load(&mut m.store);
            salida.load(&mut m.store);
            for (i, v) in [ah, ab, bh, bb].into_iter().enumerate() {
                poner(&mut m, 52 + i as u16, v);
            }
            m.store.set_short(199, op);
            m.goto(512);
            correr(&mut m, &mut BufferIo::default());
            let resultado = leer(&m, 52) + leer(&m, 53);
            let error = (resultado - esperado).abs() / esperado.abs().max(1.0);
            assert!(error < 1e-15, "op={op} k={k} error={error} valor={resultado} referencia={esperado}");
        }
    }
}

fn situar_superposicion(m: &mut Machine, rutina: runtime::Superposicion) {
    let origen = usize::from(rutina.origen()) * 2;
    for (a, palabra) in rutina.ensamblar().unwrap().shorts {
        let (pagina, corto) = ((a - origen) / 128, (a - origen) % 128);
        assert!(pagina < usize::from(rutina.paginas()));
        m.drum.sector_mut(usize::from(rutina.sector()) + pagina)[corto] = palabra;
    }
}

fn auxiliar(m: &Machine, direccion: usize) -> f64 {
    let sector = m.drum.sector(128 + direccion / 32);
    let corto = (direccion % 32) * 4;
    Float::from_long(sector[corto..corto + 4].try_into().unwrap()).to_f64()
}

fn poner_auxiliar(m: &mut Machine, direccion: usize, valor: f64) {
    let mut sector = m.drum.sector_mut(128 + direccion / 32);
    let corto = (direccion % 32) * 4;
    sector[corto..corto + 4].copy_from_slice(&Float::from_f64(valor).to_long());
}

#[test]
fn preserve_y_restore_aislados() {
    let mut m = maquina(None, "102 130\n590 960\n990 0\n@135\n102 137\n590 961\n990 0");
    runtime::Superposicion::Preservar.ensamblar().unwrap().load(&mut m.store);
    poner(&mut m, 527, 15.25);
    m.store.set_short(127, 777);
    correr(&mut m, &mut BufferIo::default());
    assert_ne!(m.drum.sector(48)[60..64], [0; 4]);
    poner(&mut m, 527, 1.0);
    m.store.set_short(127, 1);
    m.goto(135);
    correr(&mut m, &mut BufferIo::default());
    assert_eq!(leer(&m, 527), 15.25);
    assert_eq!(m.store.short(127), 777);
}

#[test]
fn auxiliar_aislado_preserva_el_buffer_y_largos_vecinos() {
    let mut m = maquina(None, "670 3\n680 15\n102 132\n590 961\n990 0");
    situar_superposicion(&mut m, runtime::Superposicion::Auxiliar);
    m.store.set_short(192, 464);
    m.drum.sector_mut(476)[0] = 456;
    m.store.set_short(195, 527);
    m.store.set_short(197, 2);
    poner(&mut m, 527, 15.25);
    poner(&mut m, 528, 0.375);
    poner_auxiliar(&mut m, 30, -9.0);
    poner_auxiliar(&mut m, 33, -8.0);
    m.acc = Float { frac: 31, exp: 29 };
    correr(&mut m, &mut BufferIo::default());
    assert_eq!([30, 31, 32, 33].map(|a| auxiliar(&m, a)), [-9.0, 15.25, 0.375, -8.0]);
    assert_eq!(m.store.short(1792), 456);
}

#[test]
fn subpaso_rk4_aislado() {
    let mut m = maquina(None, "102 130\n590 960\n990 0");
    runtime::Superposicion::IntStep.ensamblar().unwrap().load(&mut m.store);
    for (corto, base) in [(195, 512), (197, 516), (199, 520), (201, 524)] {
        m.store.set_short(corto, base);
    }
    m.store.set_short(121, 2);
    poner(&mut m, 53, 0.125);
    poner(&mut m, 513, 2.0);
    poner(&mut m, 514, 4.0);
    poner(&mut m, 517, 1.0);
    poner(&mut m, 518, 2.0);
    correr(&mut m, &mut BufferIo::default());
    let vals = [513, 514, 521, 522, 525, 526].map(|l| leer(&m, l));
    for (valor, ref_) in vals.into_iter().zip([2.125, 4.25, 2.0, 4.0, 1.0, 2.0]) {
        assert!((valor - ref_).abs() < 1e-7, "{vals:?}");
    }
}

#[test]
fn rmp_aislado_no_borra_el_almacen_numerico() {
    let mut m = maquina(None, "590 960");
    let rmp = runtime::Superposicion::Rmp.ensamblar().unwrap();
    rmp.load(&mut m.store);
    m.store.set_short(rmp.labels[&80] as usize * 2, 448);
    let fin = asm::assemble_at("990 0", 128).unwrap();
    for (a, palabra) in fin.shorts {
        m.drum.sector_mut(448)[a - 256] = palabra;
    }
    poner(&mut m, 512, 140.0);
    correr(&mut m, &mut BufferIo::default());
    assert_eq!(leer(&m, 512), 140.0);
    assert_eq!(m.store.short(203), 1);
    assert_eq!(m.store.short(192), 448);
}

#[test]
fn cargador_y_division_compleja_con_escalas_extremas() {
    use runtime::Superposicion as S;
    let mut m = maquina(None, "670 2\n680 15\n102 132\n590 960\n990 0");
    situar_superposicion(&mut m, S::Cargador);
    situar_superposicion(&mut m, S::Complejo);
    m.store.set_short(195, S::Complejo.sector());
    m.store.set_short(197, S::Complejo.paginas());
    m.store.set_short(192, 464);
    m.drum.sector_mut(470)[0] = 123;
    for (i, v) in [1.0, 1.0, 1.0, -1.0].into_iter().enumerate() {
        poner(&mut m, 52 + i as u16, v * 2f64.powi(254));
    }
    correr(&mut m, &mut BufferIo::default());
    assert!(leer(&m, 52).abs() < 1e-7);
    assert!((leer(&m, 53) - 1.0).abs() < 1e-7);
    assert_eq!(m.store.short(1024), 123);
}

#[test]
fn rutinas_matriciales_aisladas() {
    use runtime::Superposicion as S;
    for (op, rutina) in [(10, S::MatIo), (11, S::MatLin), (26, S::MatMul), (25, S::MatElim)] {
        let mut m = maquina(None, "102 130\n590 960\n990 0");
        S::Cargador.ensamblar().unwrap().load(&mut m.store);
        for sector in [rutina, S::MatMemoria, S::MatLeer] {
            situar_superposicion(&mut m, sector);
        }
        for (l, v) in [31.0, 63.0, 127.0, 2.0, 2.0, 2.0, 1.0].into_iter().enumerate() {
            poner(&mut m, 505 + l as u16, v);
        }
        poner(&mut m, 51, 10752.0);
        m.store.set_short(195, rutina.sector());
        m.store.set_short(197, rutina.paginas());
        m.store.set_short(199, op);
        for (a, v) in (63..).zip([0.0, 2.0, 3.0, 4.0]) {
            poner_auxiliar(&mut m, a, v);
        }
        for (a, v) in (127..).zip([1.0, 2.0, 3.0, 4.0]) {
            poner_auxiliar(&mut m, a, v);
        }
        poner(&mut m, 960, 104.5);
        let datos = tape::encode("8  9  ").unwrap();
        correr(&mut m, &mut BufferIo::with_tape(&datos));
        assert_eq!(leer(&m, 960), 104.5);
        match op {
            10 => assert_eq!([31, 32].map(|a| auxiliar(&m, a)), [8.0, 9.0]),
            11 => assert_eq!([31, 32].map(|a| auxiliar(&m, a)), [1.0, 4.0]),
            26 => assert_eq!([31, 32, 33, 34].map(|a| auxiliar(&m, a)), [6.0, 8.0, 15.0, 22.0]),
            25 => assert_eq!(leer(&m, 505), -6.0),
            _ => unreachable!(),
        }
    }
}
