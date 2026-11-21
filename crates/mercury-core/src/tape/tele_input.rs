//! Cintas binarias para el tele-input del Input Routine de 1959.
//! Formato transcripto en `input_routine/sector01.txt`.

use crate::asm::{self, Program};
use crate::store::{Drum, Store, SHORTS_PER_PAGE};

fn par(filas: &mut Vec<u16>, corto: u16) {
    filas.extend([corto >> 5, corto & 31]);
}

pub fn grabar_pagina(filas: &mut Vec<u16>, pagina: u16, datos: &[u16]) {
    assert!(pagina < 8 && datos.len() == SHORTS_PER_PAGE);
    filas.push(28);
    par(filas, pagina * SHORTS_PER_PAGE as u16);
    par(filas, SHORTS_PER_PAGE as u16);
    for &corto in datos {
        par(filas, corto);
    }
    let control = datos.iter().fold(0u16, |suma, &dato| suma.wrapping_sub(dato)) & 1023;
    par(filas, control);
}

pub fn grabar_sector(filas: &mut Vec<u16>, sector: u16, datos: &[u16]) {
    grabar_pagina(filas, 2, datos);
    filas.push(26);
    par(filas, sector);
    filas.push(2);
}

/// Empaqueta código escrito y tambor con un byte por fila.
pub fn cinta_programa(programa: &Program, tambor: Option<&Drum>) -> Result<CintaPrograma, String> {
    cinta_imagen(&programa.shorts, programa.start, tambor)
}

pub struct CintaPrograma {
    pub filas: Vec<u8>,
    /// Contador de órdenes al quedar en la parada de carga.
    pub vuelta: u16,
}

/// El tele-input recibe las páginas del programa en sectores auxiliares.
/// Una transferencia final las ubica en núcleos y para antes de ejecutar
/// el primer orden, para que el operador pueda ajustar las llaves.
pub fn cinta_imagen(cortos: &[(usize, u16)], arranque: u16, tambor: Option<&Drum>) -> Result<CintaPrograma, String> {
    let mut paginas = [false; 32];
    // Quitar el tele-input y limpiar las dos páginas que usó para la carga.
    paginas[..3].fill(true);
    if arranque >= 1024 {
        return Err("la dirección de arranque está fuera de las páginas ejecutables".into());
    }
    paginas[usize::from(arranque) / 64] = true;
    let mut memoria = Store::default();
    for &(corto, valor) in cortos {
        let pagina = corto / SHORTS_PER_PAGE;
        if pagina >= paginas.len() || valor > 1023 {
            return Err("la imagen contiene un corto fuera de rango".into());
        }
        paginas[pagina] = true;
        memoria.set_short(corto, valor);
    }
    let auxiliar = (3..16).rev().find(|&p| !paginas[p])
        .ok_or("falta una página ejecutable para la transferencia final")?;
    let mut libres = (500..512).chain(2..32).filter(|&s| {
        tambor.and_then(|t| t.get(s)).is_none_or(|p| p.iter().all(|&v| v == 0))
    });
    let mut tomar_sector = || libres.next().ok_or("faltan sectores reservados para la transferencia final");

    let mut filas = vec![0; 100];
    if let Some(tambor) = tambor {
        if tambor.len() > 1024 {
            return Err("la cinta sólo admite 1024 sectores de tambor".into());
        }
        if tambor.get(0).is_some_and(|p| p.iter().any(|&x| x != 0)) {
            return Err("el sector 0 del tambor se reserva para INITIAL TRANSFER".into());
        }
        // Se puede escribir el sector 1 porque su lector ya está en núcleos.
        let sectores = (2..tambor.len()).chain(1..tambor.len().min(2));
        for sector in sectores {
            let datos = tambor.sector(sector);
            if datos.iter().any(|&x| x != 0) {
                grabar_sector(&mut filas, sector as u16, datos);
            }
        }
    }
    let mut transferir = String::new();
    for (pagina, usada) in paginas.into_iter().enumerate() {
        if usada {
            let sector = tomar_sector()?;
            grabar_sector(&mut filas, sector as u16, memoria.page(pagina));
            transferir.push_str(&format!("670 {sector}\n680 {pagina}\n"));
        }
    }
    transferir.push_str(&format!(
        "101 0\n102 0\n103 0\n104 0\n105 0\n106 0\n300 0\n990 0\n590 {arranque} (1\n"
    ));
    let direccion = (auxiliar * 64) as u16;
    let final_ = asm::assemble_at(&transferir, direccion).map_err(|e| e.to_string())?;
    if final_.shorts.iter().any(|&(c, _)| c / SHORTS_PER_PAGE != auxiliar) {
        return Err("la transferencia final no cabe en su página".into());
    }
    final_.load(&mut memoria);
    let sector = tomar_sector()?;
    grabar_sector(&mut filas, sector as u16, memoria.page(auxiliar));

    memoria.page_mut(1).fill(0);
    asm::assemble(&format!("670 {sector}\n680 {auxiliar}\n590 {direccion}\n"))
        .map_err(|e| e.to_string())?
        .load(&mut memoria);
    grabar_pagina(&mut filas, 1, memoria.page(1));
    filas.push(29);
    par(&mut filas, 64);
    Ok(CintaPrograma { filas: filas.into_iter().map(|x| x as u8).collect(), vuelta: final_.labels[&1] })
}

#[cfg(test)]
mod pruebas {
    use super::*;
    use crate::{io::BufferIo, Machine, Stop};

    #[test]
    fn carga_paginas_altas_y_un_sector_que_ocupa_el_tele_input() {
        let programa = asm::assemble("!8.0\n@8.0\n670 1\n680 3\n630 3.0\n990 0\n@31.0\n=37\n@0.0\n=42").unwrap();
        let mut tambor = Drum::new(512);
        tambor.sector_mut(1)[0] = 7;
        let cinta = cinta_programa(&programa, Some(&tambor)).unwrap();

        let mut m = Machine::new(Default::default());
        for (i, fuente) in [
            include_str!("../../../../input_routine/sector00.txt"),
            include_str!("../../../../input_routine/sector01.txt"),
        ].into_iter().enumerate() {
            *m.drum.sector_mut(i) = asm::assemble_sector(fuente).unwrap();
        }
        let filas: Vec<_> = cinta.filas.iter().copied().map(u16::from).collect();
        let mut io = BufferIo::with_tape(&filas);
        io.switches = 4;
        m.initial_transfer();
        assert_eq!(m.run(&mut io, 100_000), Some(Stop::Stop99));
        assert_eq!(m.c, cinta.vuelta);
        assert_eq!(io.input_pos[1], cinta.filas.len());
        assert!(io.output[1].is_empty());
        assert_eq!(m.drum.sector(1)[0], 7);
        assert_eq!(m.store.page(0)[0], 42);
        assert_eq!(m.store.page(31)[0], 37);
        assert_eq!(m.prepulse(&mut io), Ok(()));
        assert_eq!(m.run(&mut io, 10), Some(Stop::Stop99));
        assert_eq!(io.output[1], [7]);
    }
}
