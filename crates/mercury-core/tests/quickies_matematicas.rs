use mercury_core::{acc::Float, asm, io::BufferIo, Machine, Stop};

fn ejecutar(fuente: &str, x: f64, y: f64) -> (Machine, u16) {
    let programa = asm::assemble(&format!("{fuente}\n990 0\n")).unwrap();
    let salida = programa.shorts.iter().map(|(a, _)| a / 2).max().unwrap() as u16 + 1;
    let mut m = Machine::new(Default::default());
    m.store.set_long(1, Float::from_f64(-1.0).to_long());
    m.store.set_long(16, Float::from_f64(y).to_long());
    m.store.set_long(17, Float::from_f64(x).to_long());
    asm::assemble_at("990 0", 9).unwrap().load(&mut m.store);
    programa.load(&mut m.store);
    m.c = 128;
    assert_eq!(m.run(&mut BufferIo::default(), 10000), Some(Stop::Stop99));
    (m, salida)
}

#[test]
fn arcotangente() {
    for (x, y) in [(1.0_f64, 1.0_f64), (1.0, -1.0), (-1.0, 1.0), (-1.0, -1.0), (0.0, 1.0), (0.0,-1.0), (1.0,0.0),(-1.0,0.0),(2.0,3.0),(-3.0,-2.0)] {
        let (m, salida) = ejecutar(include_str!("../../../input_routine/quickies/15.mer"), x, y);
        assert_eq!(m.c, salida, "x={x} y={y}");
        let esperado = y.atan2(x).rem_euclid(std::f64::consts::TAU);
        assert!((m.acc.to_f64() - esperado).abs() < 1e-6, "x={x} y={y} obtenido={} esperado={esperado}", m.acc.to_f64());
    }
    let (m, _) = ejecutar(include_str!("../../../input_routine/quickies/15.mer"), 0.0, 0.0);
    assert_eq!(m.c, 10);
}

#[test]
fn logaritmo() {
    let fuente = include_str!("../../../input_routine/quickies/14.mer");
    for x in [1e-20_f64, 0.125, 0.5, 0.999, 1.0, 1.5, 2.0, 10.0, 1e20] {
        let programa = asm::assemble(&format!("{fuente}\n990 0\n")).unwrap();
        let salida = programa.shorts.iter().map(|(a, _)| a / 2).max().unwrap() as u16 + 1;
        let mut m = Machine::new(Default::default());
        m.store.set_long(1, Float::from_f64(-1.0).to_long());
        asm::assemble_at("990 0", 9).unwrap().load(&mut m.store);
        programa.load(&mut m.store);
        m.acc = Float::from_f64(x); m.c = 128;
        assert_eq!(m.run(&mut BufferIo::default(), 10000), Some(Stop::Stop99));
        assert_eq!(m.c, salida);
        assert!((m.acc.to_f64() - x.ln()).abs() < 1e-6, "x={x} obtenido={}", m.acc.to_f64());
    }
    for x in [0.0, -0.1, -1.0, -10.0] {
        let programa = asm::assemble(fuente).unwrap();
        let mut m = Machine::new(Default::default());
        m.store.set_long(1, Float::from_f64(-1.0).to_long());
        asm::assemble_at("990 0", 9).unwrap().load(&mut m.store);
        programa.load(&mut m.store);
        m.acc = Float::from_f64(x);
        m.c = 128;
        assert_eq!(m.run(&mut BufferIo::default(), 10000), Some(Stop::Stop99));
        assert_eq!(m.c, 10, "entrada {x}: debe ir a la entrada de error");
    }

}


#[test]
fn tangente() {
    let fuente = include_str!("../../../input_routine/quickies/05.mer");
    for x in (-1000..=1000).map(|n| n as f64 / 37.0).filter(|x| x.cos().abs() > 0.05).chain([-100.0, 100.0]) {
        let programa = asm::assemble(&format!("{fuente}\n990 0\n")).unwrap();
        let salida = programa.shorts.iter().map(|(a, _)| a / 2).max().unwrap() as u16 + 1;
        let mut m = Machine::new(Default::default());
        m.store.set_long(1, Float::from_f64(-1.0).to_long());
        programa.load(&mut m.store);
        m.acc = Float::from_f64(x); m.c = 128;
        assert_eq!(m.run(&mut BufferIo::default(), 10000), Some(Stop::Stop99));
        assert_eq!(m.c, salida);
        assert!((m.acc.to_f64() - x.tan()).abs() < 1e-5 * x.tan().abs().max(1.0), "x={x} obtenido={} esperado={}", m.acc.to_f64(), x.tan());
    }
}
