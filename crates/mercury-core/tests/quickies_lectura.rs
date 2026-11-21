//! Pruebas de lectura de los quickies del manuscrito.
use mercury_core::{asm, io::BufferIo, tape, Machine, Stop};

#[test]
fn leer_entero() {
    for (entrada, esperado) in [("0 ",0), ("7 ",7), ("123 ",123), ("-42 ",982), ("+12 ",12), ("511 ",511)] {
        let src = include_str!("../../../input_routine/quickies/18.mer");
        let p = asm::assemble(&format!("{src}\n990 0\n")).unwrap();
        let mut m = Machine::new(Default::default());
        p.load(&mut m.store);
        asm::assemble_at("990 0", 9).unwrap().load(&mut m.store);
        m.c = 128;
        m.b[6] = 123;
        let mut io = BufferIo::with_tape(&tape::encode(entrada).unwrap());
        assert_eq!(m.run(&mut io, 10000), Some(Stop::Stop99), "{entrada}");
        assert_ne!(m.c, 10, "error para {entrada}");
        assert_eq!(m.b[7], esperado, "{entrada}");
        assert_eq!(m.b[6], 123, "{entrada}");
    }
}

#[test]
fn rechazar_entero_mal_formado() {
    for entrada in ["1+2 ", "1.2 ", "1-2 "] {
        let src = include_str!("../../../input_routine/quickies/18.mer");
        let p = asm::assemble(&format!("{src}\n990 0\n")).unwrap();
        let mut m = Machine::new(Default::default());
        p.load(&mut m.store);
        asm::assemble_at("990 0", 9).unwrap().load(&mut m.store);
        m.c = 128;
        let mut io = BufferIo::with_tape(&tape::encode(entrada).unwrap());
        assert_eq!(m.run(&mut io, 10000), Some(Stop::Stop99), "{entrada}");
        assert_eq!(m.c, 10, "se esperaba la entrada de error para {entrada}");
    }
}

#[test]
fn leer_decimal() {
    for (entrada, esperado) in [("0 ",0.0), ("7 ",7.0), ("123 ",123.0), ("-42 ",-42.0), ("+12 ",12.0), ("1.25 ",1.25), ("-3.5 ",-3.5), ("1,2 ",100.0), ("1.25,-2 ",0.0125), ("3,+2 ",300.0)] {
        let src = include_str!("../../../input_routine/quickies/19.mer");
        let p = asm::assemble(&format!("{src}\n990 0\n")).unwrap();
        let mut m = Machine::new(Default::default());
        p.load(&mut m.store);
        asm::assemble_at("990 0", 9).unwrap().load(&mut m.store);
        m.c = 128;
        m.b[5] = 222;
        m.b[6] = 123;
        let mut io = BufferIo::with_tape(&tape::encode(&format!("{entrada} ")).unwrap());
        assert_eq!(m.run(&mut io, 10000), Some(Stop::Stop99), "{entrada}");
        assert_ne!(m.c, 10, "error para {entrada}");
        assert!((m.acc.to_f64() - esperado).abs() < 1e-5, "{entrada}: {}", m.acc.to_f64());
        assert_eq!((m.b[5],m.b[6]),(222,123), "{entrada}");
    }
}

#[test]
fn errores_y_fin_de_cinta_decimal() {
    for entrada in ["1+2  ", "1.2.3  ", "1,2,3  "] {
        let src = include_str!("../../../input_routine/quickies/19.mer");
        let p = asm::assemble(&format!("{src}\n990 0\n")).unwrap();
        let mut m = Machine::new(Default::default());
        p.load(&mut m.store);
        asm::assemble_at("990 0", 9).unwrap().load(&mut m.store);
        m.c = 128;
        let mut io = BufferIo::with_tape(&tape::encode(entrada).unwrap());
        assert_eq!(m.run(&mut io, 10000), Some(Stop::Stop99), "{entrada}");
        assert_eq!(m.c, 10, "se esperaba la entrada de error para {entrada}");
    }
    let p = asm::assemble(include_str!("../../../input_routine/quickies/19.mer")).unwrap();
    let mut m = Machine::new(Default::default());
    p.load(&mut m.store);
    m.c = 128;
    let mut io = BufferIo::with_tape(&tape::encode("12 ").unwrap());
    assert_eq!(m.run(&mut io, 10000), Some(Stop::InputExhausted { channel: 1 }));
}
