//! A tela X antes de abrir. Os quatro lançadores forçam `SDL_VIDEO_DRIVER=x11`, e sem X o jogo
//! morre no boot com um stack trace ("x11 not available"); isto avisa antes, numa linha.

use std::env;
use std::os::linux::net::SocketAddrExt;
use std::os::unix::ffi::OsStrExt;
use std::os::unix::net::{SocketAddr, UnixStream};
use std::path::Path;

/// O número do display local em `valor` (`:0`, `:1.0`, `unix:0`). `Ok(None)` é display remoto
/// (`localhost:10.0`, o do `ssh -X`), que daqui não se confere; `Err` é valor que não é display.
fn display_local(valor: &str) -> Result<Option<u32>, ()> {
    let (maquina, resto) = valor.rsplit_once(':').ok_or(())?;
    let numero = resto
        .split_once('.')
        .map_or(resto, |(numero, _tela)| numero);
    let numero: u32 = numero.parse().map_err(|_| ())?;
    match maquina {
        "" | "unix" => Ok(Some(numero)),
        _ => Ok(None),
    }
}

/// Se alguém atende no socket `caminho`: o de arquivo ou, no Linux, o abstrato de mesmo nome.
/// Não fala o protocolo do X, só vê se a conexão é aceita.
fn atende(caminho: &Path) -> bool {
    UnixStream::connect(caminho).is_ok()
        || SocketAddr::from_abstract_name(caminho.as_os_str().as_bytes())
            .and_then(|endereco| UnixStream::connect_addr(&endereco))
            .is_ok()
}

/// Confere se há um X atendendo no `display`, procurando o socket dele em `sockets` (no sistema,
/// `/tmp/.X11-unix`).
pub fn tela_x(display: Option<&str>, sockets: &Path) -> Result<(), String> {
    let valor = display.unwrap_or("");
    if valor.is_empty() {
        return Err("sem tela X: o DISPLAY está vazio".to_string());
    }
    let numero = match display_local(valor) {
        Ok(Some(numero)) => numero,
        Ok(None) => return Ok(()),
        Err(()) => return Err(format!("sem tela X: DISPLAY={valor:?} não é um display")),
    };
    let socket = sockets.join(format!("X{numero}"));
    if atende(&socket) {
        return Ok(());
    }
    // O XWayland do Hyprland atende pelo `X<n>_`; sem o `X<n>`, nenhum cliente novo conecta, e o
    // link físico conserta (28/10: um `Xvfb -displayfd` tomou o `:0` e apagou o `X0`).
    let sublinhado = sockets.join(format!("X{numero}_"));
    if UnixStream::connect(&sublinhado).is_ok() {
        return Err(format!(
            "sem tela X: o socket {} sumiu, mas o X{numero}_ atende; refaça com: ln {} {}",
            socket.display(),
            sublinhado.display(),
            socket.display()
        ));
    }
    Err(format!("sem tela X: ninguém atende no DISPLAY={valor:?}"))
}

/// A `tela_x` deste processo.
pub fn tela() -> Result<(), String> {
    tela_x(
        env::var("DISPLAY").ok().as_deref(),
        Path::new("/tmp/.X11-unix"),
    )
}

#[cfg(test)]
mod testes {
    use super::*;
    use crate::apoio_de_teste::pasta_de_teste;
    use std::os::unix::net::UnixListener;

    #[test]
    fn display_local_le_o_numero_e_deixa_o_remoto_de_fora() {
        assert_eq!(display_local(":0"), Ok(Some(0)));
        assert_eq!(display_local(":1.0"), Ok(Some(1)));
        assert_eq!(display_local("unix:2"), Ok(Some(2)));
        assert_eq!(display_local("localhost:10.0"), Ok(None));
        for errado in ["", ":", "0", ":abc", ":0x"] {
            assert_eq!(display_local(errado), Err(()), "{errado:?}");
        }
    }

    #[test]
    fn tela_x_so_passa_com_alguem_atendendo_no_socket() {
        let sockets = pasta_de_teste("tela");
        let _x5 = UnixListener::bind(sockets.join("X5")).unwrap();

        assert_eq!(tela_x(Some(":5"), &sockets), Ok(()));
        assert_eq!(tela_x(Some("localhost:10.0"), &sockets), Ok(()));
        let sem_ninguem = tela_x(Some(":6"), &sockets).unwrap_err();
        assert!(sem_ninguem.contains(r#"DISPLAY=":6""#), "{sem_ninguem}");
        assert!(tela_x(None, &sockets).unwrap_err().contains("vazio"));
        assert!(tela_x(Some(""), &sockets).unwrap_err().contains("vazio"));
        assert!(
            tela_x(Some(":abc"), &sockets)
                .unwrap_err()
                .contains("não é um display")
        );

        std::fs::remove_dir_all(&sockets).unwrap();
    }

    /// O XWayland do Hyprland atende só pelo `X0_`; quando o `Xvfb -displayfd` das ferramentas de
    /// teste toma o `:0`, ele apaga o `X0`, e nenhum cliente X novo conecta (28/10, 19:02; o
    /// conserto do autor foi o link físico). O aviso diz o comando.
    #[test]
    fn tela_x_sem_o_x_mas_com_o_x_sublinhado_diz_o_conserto() {
        let sockets = pasta_de_teste("tela-sublinhado");
        let _x7 = UnixListener::bind(sockets.join("X7_")).unwrap();
        let x7 = sockets.join("X7");
        let x7_ = sockets.join("X7_");

        let aviso = tela_x(Some(":7"), &sockets).unwrap_err();
        let conserto = format!("ln {} {}", x7_.display(), x7.display());
        assert!(aviso.contains(&conserto), "{aviso}");
        assert!(aviso.contains("sumiu"), "{aviso}");

        std::fs::hard_link(&x7_, &x7).unwrap();
        assert_eq!(tela_x(Some(":7"), &sockets), Ok(()), "o conserto funciona");

        std::fs::remove_dir_all(&sockets).unwrap();
    }
}
