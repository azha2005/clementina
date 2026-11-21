//! Graba sectores y una secuencia de entrada en cinta Ferranti de tele-input.
use mercury_core::{asm, store::Store, tape};
use mercury_core::tape::tele_input::{grabar_pagina, grabar_sector};
use std::{fs, path::Path};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut args = std::env::args().skip(1);
    let imagen = args.next().ok_or("falta la imagen de tambor")?;
    let arranque = args.next().ok_or("falta el programa de arranque")?;
    let salida = args.next().ok_or("falta el archivo de salida")?;
    let mut datos = fs::read(&imagen)?;
    if datos.len() != 512 * 256 {
        return Err("se esperaba una imagen de tambor de 512 sectores".into());
    }
    let mut sectores_usados = [false; 512];
    for (i, bytes) in datos.chunks_exact(256).enumerate() {
        sectores_usados[i] = i < 2 || bytes.iter().any(|&b| b != 0);
    }
    // Cada pareja agrega un banco ensamblado al tambor. El primer registro
    // escrito en el fuente indica la página inicial del banco en núcleos.
    let mut pregunta = None;
    while let Some(sector) = args.next() {
        if sector == "--cinta" {
            let path = args
                .next()
                .ok_or("falta el archivo de texto para la cinta")?;
            let texto = fs::read_to_string(path)?;
            pregunta = Some(
                tape::encode(&format!("{}\n", texto.trim_end_matches(['\r', '\n'])))
                    .map_err(|c| format!("el carácter {c:?} no existe en el código Ferranti"))?,
            );
            continue;
        }
        let sector: usize = sector.parse()?;
        let fuente = args.next().ok_or("falta el fuente del banco")?;
        let banco = asm::assemble(&fs::read_to_string(&fuente)?)?;
        let primero = banco
            .shorts
            .iter()
            .map(|&(corto, _)| corto)
            .min()
            .ok_or("banco vacío")?;
        let base = primero / 128 * 128;
        let ultimo = banco.shorts.iter().map(|&(corto, _)| corto).max().unwrap();
        let paginas = (ultimo - base) / 128 + 1;
        let rango = sectores_usados
            .get_mut(
                sector
                    ..sector
                        .checked_add(paginas)
                        .ok_or("banco fuera del tambor")?,
            )
            .ok_or("banco fuera del tambor")?;
        if rango.iter().any(|&b| b) {
            return Err(format!("el banco {fuente} pisa datos u otro banco").into());
        }
        rango.fill(true);
        for (corto, valor) in banco.shorts {
            let i = usize::from(corto)
                .checked_sub(base)
                .ok_or("corto anterior al banco")?;
            let p = (sector * 128 + i) * 2;
            datos
                .get_mut(p..p + 2)
                .ok_or("banco fuera del tambor")?
                .copy_from_slice(&valor.to_le_bytes());
        }
    }
    let programa = asm::assemble_at(&fs::read_to_string(arranque)?, 64)?;
    if programa.start != 64
        || !programa
            .shorts
            .iter()
            .all(|&(n, _)| (128..256).contains(&n))
    {
        return Err("el arranque tiene que empezar en 1.0 y caber en la página 1".into());
    }
    let mut memoria = Store::default();
    programa.load(&mut memoria);

    let mut filas = vec![0; 100];
    // Se conservan los sectores 0 y 1 del Input Routine.
    for (sector, bytes) in datos.chunks_exact(256).enumerate().skip(2) {
        if bytes.iter().all(|&x| x == 0) {
            continue;
        }
        let datos: Vec<_> = bytes
            .chunks_exact(2)
            .map(|b| u16::from_le_bytes([b[0], b[1]]))
            .collect();
        if datos.iter().any(|&c| c > 1023) {
            return Err(format!("corto fuera de rango en el sector {sector}").into());
        }
        grabar_sector(&mut filas, sector as u16, &datos);
    }
    // El tramo final deja la entrada en núcleos. El salto se ejecuta desde
    // la página 0 del tele-input, sin pisar el cargador durante la lectura.
    grabar_pagina(&mut filas, 1, memoria.page(1));
    filas.extend([29, 2, 0]); // saltar a 1.0
    if let Some(pregunta) = pregunta {
        filas.extend(pregunta);
    }

    let salida = Path::new(&salida);
    fs::write(
        salida.with_extension("bin"),
        filas.iter().map(|&x| x as u8).collect::<Vec<_>>(),
    )?;
    fs::write(
        salida.with_extension("svg"),
        tape::to_svg_with(&filas, &Default::default()),
    )?;
    fs::write(
        salida.with_extension("vista.svg"),
        tape::to_svg_with(&filas[92..156.min(filas.len())], &Default::default()),
    )?;
    println!("{} filas", filas.len());
    Ok(())
}

#[cfg(test)]
mod pruebas {
    use super::*;
    use mercury_core::{io::BufferIo, Machine, Stop};

    #[test]
    fn tele_input_carga_el_sector_y_ejecuta_la_entrada() {
        let mut m = Machine::new(Default::default());
        let sectores = [
            include_str!("../../../input_routine/sector00.txt"),
            include_str!("../../../input_routine/sector01.txt"),
        ];
        for (i, fuente) in sectores.into_iter().enumerate() {
            *m.drum.sector_mut(i) = asm::assemble_sector(fuente).unwrap();
        }
        m.initial_transfer();

        let datos = std::array::from_fn::<_, 128, _>(|i| (7 * i as u16 + 9) & 1023);
        let mut memoria = Store::default();
        asm::assemble("670 400\n680 3\n630 3.0\n990 0")
            .unwrap()
            .load(&mut memoria);
        let mut filas = vec![0; 100];
        grabar_sector(&mut filas, 400, &datos);
        grabar_pagina(&mut filas, 1, memoria.page(1));
        filas.extend([29, 2, 0]);

        let mut io = BufferIo::with_tape(&filas);
        io.switches = 4;
        assert_eq!(m.run(&mut io, 100_000), Some(Stop::Stop99));
        assert_eq!(m.c, 68);
        assert_eq!(*m.drum.sector(400), datos);
        assert_eq!(io.output[1], [9]);
    }
}
