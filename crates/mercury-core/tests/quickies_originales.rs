//! Quickies originales del Input Routine: manuscrito, páginas 101–102 del PDF.

use mercury_core::acc::Float;
use mercury_core::asm;
use mercury_core::io::BufferIo;
use mercury_core::{Machine, Stop};

fn ejecutar(src: &str, valor: f64) -> (Machine, u16) {
    // La parada agregada permite observar la salida por caída. La parada en
    // la línea 9 representa la entrada a la impresión de error del sistema.
    let texto = format!("{src}\n990 0\n");
    let programa = asm::assemble(&texto).unwrap();
    let ultima = programa.shorts.iter().map(|(a, _)| a / 2).max().unwrap() as u16;
    let mut m = Machine::new(Default::default());
    m.store.set_long(1, Float::from_f64(-1.0).to_long());
    asm::assemble_at("990 0", 9).unwrap().load(&mut m.store);
    programa.load(&mut m.store);
    m.acc = Float::from_f64(valor);
    m.c = 128;
    assert_eq!(m.run(&mut BufferIo::default(), 10000), Some(Stop::Stop99));
    (m, ultima + 1)
}

#[test]
fn reciproco() {
    for x in [0.125, 0.5, 1.0, 1.5, 3.0, 10.0, 1e-20, 1e20, -0.125, -1.0, -3.0, -1e20] {
        let (m, salida) = ejecutar(include_str!("../../../input_routine/quickies/01.mer"), x);
        assert_eq!(m.c, salida, "entrada {x}");
        let esperado = 1.0 / x;
        let obtenido = m.acc.to_f64();
        assert!(((obtenido - esperado) / esperado).abs() < 1e-7, "entrada {x}: {obtenido} en vez de {esperado}");
    }
    let (m, _) = ejecutar(include_str!("../../../input_routine/quickies/01.mer"), 0.0);
    assert_eq!(m.c, 10, "el cero debe ir a la entrada de error");
}

#[test]
fn reciproco_de_raiz() {
    for x in [0.125_f64, 0.5, 1.0, 1.5, 2.0, 3.0, 10.0, 1e-20, 1e20] {
        let (m, salida) = ejecutar(include_str!("../../../input_routine/quickies/02.mer"), x);
        assert_eq!(m.c, salida, "entrada {x}");
        let esperado = 1.0 / x.sqrt();
        let obtenido = m.acc.to_f64();
        assert!(((obtenido - esperado) / esperado).abs() < 1e-7, "entrada {x}: {obtenido} en vez de {esperado}");
    }
    for x in [0.0, -0.125, -1.0, -10.0] {
        let (m, _) = ejecutar(include_str!("../../../input_routine/quickies/02.mer"), x);
        assert_eq!(m.c, 10, "entrada {x}: debe ir a la entrada de error");
    }
}

#[test]
fn raiz_cuadrada() {
    let fuente = include_str!("../../../input_routine/quickies/12.mer");
    for x in [0.0_f64, 0.125, 0.5, 1.0, 1.5, 2.0, 3.0, 10.0, 1e-20, 1e20] {
        let (m, salida) = ejecutar(fuente, x);
        assert_eq!(m.c, salida, "entrada {x}");
        let esperado = x.sqrt();
        let obtenido = m.acc.to_f64();
        if x == 0.0 {
            assert_eq!(obtenido, 0.0);
        } else {
            assert!(((obtenido - esperado) / esperado).abs() < 1e-7, "entrada {x}: {obtenido} en vez de {esperado}");
        }
    }
    for x in [-0.125, -1.0, -10.0] {
        let (m, _) = ejecutar(fuente, x);
        assert_eq!(m.c, 10, "entrada {x}: debe ir a la entrada de error");
    }
}

#[test]
fn imprimir_sac_con_y_sin_signo() {
    use mercury_core::tape;
    let fuentes = [include_str!("../../../input_routine/quickies/08.mer"), include_str!("../../../input_routine/quickies/11.mer")];
    for (quicky, fuente) in fuentes.iter().enumerate() {
        for valor in [0_u16, 1, 7, 42, 511, 512, 999, 1000, 1023] {
            let programa = asm::assemble(&format!("{fuente}\n990 0\n")).unwrap();
            let mut m = Machine::new(Default::default());
            programa.load(&mut m.store);
            m.c = 128;
            m.b[5] = 173;
            m.b[6] = 947;
            m.b[7] = valor;
            let mut io = BufferIo::default();
            assert_eq!(m.run(&mut io, 10000), Some(Stop::Stop99));
            let esperado = if quicky == 0 {
                let firmado = if valor >= 512 { valor as i32 - 1024 } else { valor as i32 };
                if firmado >= 0 { format!("\n {firmado}  ") } else { format!("\n{firmado}  ") }
            } else {
                format!("\n{valor}  ")
            };
            assert_eq!(tape::decode(&io.output[1]), esperado, "quicky {}, Sac {valor}", if quicky == 0 { 8 } else { 11 });
            assert_eq!((m.b[5], m.b[6]), (173, 947));
        }
    }
}

#[test]
fn exponencial() {
    let fuente = include_str!("../../../input_routine/quickies/04.mer");
    for x in [-100.0_f64, -20.0, -10.0, -2.0, -1.0, -0.1, 0.0, 0.1, 1.0, 2.0, 10.0, 20.0, 100.0] {
        let (m, salida) = ejecutar(fuente, x);
        assert_eq!(m.c, salida, "entrada {x}");
        let esperado = x.exp();
        let obtenido = m.acc.to_f64();
        assert!(((obtenido - esperado) / esperado).abs() < 1e-7, "entrada {x}: {obtenido} en vez de {esperado}");
    }
    let (subdesborde, salida) = ejecutar(fuente, -200.0);
    assert_eq!(subdesborde.c, salida);
    assert_eq!(subdesborde.acc.to_f64(), 0.0);
    let (exceso, _) = ejecutar(fuente, 200.0);
    assert_eq!(exceso.c, 10);
}

#[test]
fn seno_y_coseno() {
    let fuentes = [include_str!("../../../input_routine/quickies/06.mer"), include_str!("../../../input_routine/quickies/07.mer")];
    for (quicky, fuente) in fuentes.iter().enumerate() {
        for x in [-100.0_f64, -10.0, -3.0, -1.0, -0.1, 0.0, 0.1, 1.0, 3.0, 10.0, 100.0] {
            let (m, salida) = ejecutar(fuente, x);
            assert_eq!(m.c, salida, "entrada {x}");
            let esperado = if quicky == 0 { x.sin() } else { x.cos() };
            let obtenido = m.acc.to_f64();
            assert!((obtenido - esperado).abs() < 5e-7, "quicky {}, entrada {x}: {obtenido} en vez de {esperado}", quicky + 6);
        }
    }
}

#[test]
fn arcoseno() {
    let fuente = include_str!("../../../input_routine/quickies/16.mer");
    for x in [-1.0_f64, -0.9, -0.5, -0.1, 0.0, 0.1, 0.5, 0.9, 1.0] {
        let (m, salida) = ejecutar(fuente, x);
        assert_eq!(m.c, salida, "entrada {x}");
        let obtenido = m.acc.to_f64();
        assert!((obtenido - x.asin()).abs() < 1e-7, "entrada {x}: {obtenido} en vez de {}", x.asin());
    }
    for x in [-1.1, 1.1] {
        let (m, _) = ejecutar(fuente, x);
        assert_eq!(m.c, 10, "entrada {x}: debe ir a la entrada de error");
    }
}
