//! Quickies 9 y 10: impresión del acumulador, manuscrito PDF páginas 109 a 113.
use mercury_core::{acc::Float, asm, io::BufferIo, tape, Machine, Stop};

fn ejecutar(fuente: &str, valor: f64) -> (String, Machine) {
    let programa = asm::assemble(&format!("{fuente}\n990 0\n")).unwrap();
    let mut m = Machine::new(Default::default());
    programa.load(&mut m.store);
    // La rutina usa el -1 de la página cero. B1 selecciona una entrada de
    // la tabla de formatos, con m = 4 posiciones enteras y n = 2 decimales.
    m.store.set_long(1, Float::from_f64(-1.0).to_long());
    m.store.set_short(516, 4);
    m.store.set_short(517, 2);
    m.acc = Float::from_f64(valor);
    m.c = 128;
    m.b[1] = 256;
    m.b[5] = 173;
    m.b[6] = 419;
    m.b[7] = 6;
    let mut io = BufferIo::default();
    assert_eq!(m.run(&mut io, 100_000), Some(Stop::Stop99));
    (tape::decode(&io.output[1]), m)
}

#[test]
fn punto_fijo() {
    let fuente = include_str!("../../../input_routine/quickies/09.mer");
    for (valor, esperado) in [
        (0.0, "\n    0.00  "), (1.0, "\n    1.00  "),
        (-1.0, "\n   -1.00  "), (12.34, "\n   12.34  "),
        (0.125, "\n    0.12  "), (1000.0, "\n 1000.00  "),
    ] {
        let (salida, m) = ejecutar(fuente, valor);
        assert_eq!(salida, esperado, "entrada {valor}");
        assert_eq!(m.b[6], 419);
    }
}

#[test]
fn punto_flotante() {
    let fuente = include_str!("../../../input_routine/quickies/10.mer");
    for (valor, esperado) in [
        (0.0, "\n 0.00000, +0  "), (1.0, "\n 1.00000, +0  "),
        (-1.0, "\n-1.00000, +0  "), (12.34, "\n 1.23400, +1  "),
        (0.125, "\n 1.25000, -1  "), (1000.0, "\n 1.00000, +3  "),
    ] {
        let (salida, m) = ejecutar(fuente, valor);
        assert_eq!(salida, esperado, "entrada {valor}");
        assert_eq!((m.b[5], m.b[6]), (173, 419));
    }
}
