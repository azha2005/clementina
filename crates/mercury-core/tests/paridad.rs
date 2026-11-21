//! Paridad par de la Mercury (CS 225 A §§1.9 y 7.5).

use mercury_core::{asm::assemble_at, io::BufferIo, Machine, Stop};

fn maquina(fuente: &str) -> Machine {
    let mut m = Machine::new(Default::default());
    for (corto, palabra) in assemble_at(fuente, 64).unwrap().shorts {
        m.store.set_short(corto, palabra);
    }
    m.goto(64);
    m
}

#[test]
fn detiene_antes_de_obedecer_y_prepulse_vuelve_a_comprobar() {
    let mut m = maquina("102 19\n990 0");
    let mut io = BufferIo::default();
    assert!(m.store.invertir_bit(128, 10));
    let stop = Stop::Paridad { sector: None, corto: 128 };
    assert_eq!(m.step(&mut io), Err(stop));
    assert_eq!((m.c, m.executed, m.b[2]), (64, 0, 0));
    assert!(m.stop_flip_flop());
    assert_eq!(m.prepulse(&mut io), Err(stop));
    io.inhibit_parities = true;
    assert_eq!(m.prepulse(&mut io), Ok(()));
    assert_eq!((m.c, m.executed, m.b[2]), (65, 1, 19));
    assert!(!m.stop_flip_flop());
}

#[test]
fn controla_el_operando_corto_y_los_cuatro_cortos_del_largo() {
    let mut m = maquina("002 10\n400 32\n990 0");
    let mut io = BufferIo::default();
    m.store.set_short(20, 17);
    assert!(m.store.invertir_bit(20, 0));
    assert_eq!(m.step(&mut io), Err(Stop::Paridad { sector: None, corto: 20 }));
    assert_eq!(m.b[2], 0);
    m.store.set_short(20, 17);
    m.prepulse(&mut io).unwrap();
    assert_eq!(m.b[2], 17);
    m.store.set_long(16, [3, 8, 15, 67]);
    assert!(m.store.invertir_bit(67, 10));
    assert_eq!(m.step(&mut io), Err(Stop::Paridad { sector: None, corto: 67 }));
    assert_eq!((m.c, m.executed), (65, 1));
}

#[test]
fn controla_una_pagina_al_grabar_y_un_sector_al_leer() {
    let mut m = maquina("670 5\n690 3\n680 4\n990 0");
    let mut io = BufferIo::default();
    m.store.page_mut(3).fill(111);
    m.drum.sector_mut(5).fill(19);
    m.store.invertir_bit(3 * 128 + 4, 0);
    m.step(&mut io).unwrap();
    assert_eq!(m.step(&mut io), Err(Stop::Paridad { sector: None, corto: 388 }));
    assert_eq!(*m.drum.sector(5), [19; 128]);
    m.store.set_short(388, 111);
    m.prepulse(&mut io).unwrap();
    assert_eq!(*m.drum.sector(5), [111; 128]);
    m.drum.invertir_bit(5, 8, 1);
    assert_eq!(m.step(&mut io), Err(Stop::Paridad { sector: Some(5), corto: 8 }));
    assert_eq!(m.store.page(4), &[0; 128]);
    io.inhibit_parities = true;
    m.prepulse(&mut io).unwrap();
    assert_eq!(m.store.page(4)[8], 109);
    io.inhibit_parities = false;
    m.goto(65);
    m.t = 6;
    m.store.page_mut(3).copy_from_slice(&[109; 128]);
    m.step(&mut io).unwrap();
}

#[test]
fn transferencia_inicial_verifica_el_sector_cero() {
    let mut m = maquina("990 0");
    let mut io = BufferIo::default();
    m.drum.sector_mut(0)[1] = 7;
    m.drum.invertir_bit(0, 1, 10);
    m.initial_transfer();
    let stop = Stop::Paridad { sector: Some(0), corto: 1 };
    assert_eq!(m.stop(), Some(stop));
    assert_eq!(m.store.short(1), 0);
    assert_eq!(m.prepulse(&mut io), Err(stop));
    m.drum.invertir_bit(0, 1, 10);
    m.prepulse(&mut io).unwrap();
    assert_eq!((m.c, m.store.short(1), m.executed), (0, 7, 0));
    assert_eq!(m.step(&mut io), Err(Stop::Stop99));
}

#[test]
fn operacion_31b_documentada() {
    let mut m = maquina("990 0");
    m.b[7] = 1020;
    m.b[4] = 37;
    m.st = 13;
    assert!(m.probar_31b(4, 5));
    assert_eq!((m.b[4], m.bt, m.sac(), m.st, m.c, m.time_us), (1, 1, 1020, 13, 64, 0));
    assert!(m.probar_31b(7, 5));
    assert_eq!((m.sac(), m.bt), (1, 1));
    assert!(!m.probar_31b(8, 0));
    assert!(!m.probar_31b(4, 1024));
}
