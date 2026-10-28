//! Como o painel escreve tempo, ritmo e campos: funções puras, sem terminal.

use std::time::Duration;

/// Duração para coluna estreita: `3s`, `12m`, `5h`, `2d` (arredonda para baixo).
pub fn duracao_curta(segundos: u64) -> String {
    match segundos {
        0..60 => format!("{segundos}s"),
        60..3600 => format!("{}m", segundos / 60),
        3600..86400 => format!("{}h", segundos / 3600),
        _ => format!("{}d", segundos / 86400),
    }
}

/// O relógio da partida: `02:14`, ou `1:02:14` a partir de uma hora.
pub fn relogio(decorrido: Duration) -> String {
    let total = decorrido.as_secs();
    let (horas, minutos, segundos) = (total / 3600, total / 60 % 60, total % 60);
    if horas > 0 {
        format!("{horas}:{minutos:02}:{segundos:02}")
    } else {
        format!("{minutos:02}:{segundos:02}")
    }
}

const BLOCOS: [char; 8] = ['▁', '▂', '▃', '▄', '▅', '▆', '▇', '█'];

/// A faísca do ritmo: um bloco por janela, de `▁` (zero) a `█` (o máximo da série).
pub fn faisca(contagens: &[u32]) -> String {
    let maximo = u64::from(contagens.iter().copied().max().unwrap_or(0).max(1));
    contagens
        .iter()
        .map(|&contagem| {
            // Arredonda para cima: qualquer fato já sobe do `▁`.
            let degrau = (u64::from(contagem) * 7).div_ceil(maximo);
            BLOCOS[degrau as usize]
        })
        .collect()
}

/// Os campos de um fato numa linha: `chave valor · chave valor`. Campo sem chave (a linha solta
/// inteira, num `texto` ou `erro`) vai como está; valor de mais de 12 caracteres (hash, digital)
/// vira os 7 primeiros e `…`.
pub fn resumo(campos: &[(String, String)]) -> String {
    campos
        .iter()
        .map(|(chave, valor)| {
            if chave.is_empty() {
                valor.clone()
            } else {
                format!("{chave} {}", encurta(valor))
            }
        })
        .collect::<Vec<_>>()
        .join(" · ")
}

fn encurta(valor: &str) -> String {
    if valor.chars().count() <= 12 {
        valor.to_string()
    } else {
        format!("{}…", valor.chars().take(7).collect::<String>())
    }
}

#[cfg(test)]
mod testes {
    use super::*;

    #[test]
    fn duracao_curta_usa_a_maior_unidade_inteira() {
        let casos = [
            (0, "0s"),
            (59, "59s"),
            (60, "1m"),
            (3599, "59m"),
            (3600, "1h"),
            (86399, "23h"),
            (86400, "1d"),
        ];
        for (segundos, esperado) in casos {
            assert_eq!(duracao_curta(segundos), esperado, "{segundos}");
        }
    }

    #[test]
    fn relogio_mostra_minutos_e_segundos_e_horas_quando_ha() {
        assert_eq!(relogio(Duration::from_secs(0)), "00:00");
        assert_eq!(relogio(Duration::from_millis(134_900)), "02:14");
        assert_eq!(relogio(Duration::from_secs(3734)), "1:02:14");
    }

    #[test]
    fn faisca_escala_pelo_maximo_da_serie() {
        assert_eq!(faisca(&[0, 1, 2, 4]), "▁▃▅█");
        assert_eq!(faisca(&[0, 0, 0]), "▁▁▁");
        assert_eq!(faisca(&[]), "");
    }

    #[test]
    fn resumo_junta_os_campos_e_encurta_o_valor_longo() {
        let campos = |pares: &[(&str, &str)]| -> Vec<(String, String)> {
            pares
                .iter()
                .map(|(chave, valor)| (chave.to_string(), valor.to_string()))
                .collect()
        };
        assert_eq!(
            resumo(&campos(&[
                ("soldado", "2"),
                ("texto", "2º"),
                ("marcado", "sim")
            ])),
            "soldado 2 · texto 2º · marcado sim"
        );
        assert_eq!(
            resumo(&campos(&[
                ("", "solto"),
                ("digital", "708171b0098615fe5378")
            ])),
            "solto · digital 708171b…"
        );
        assert_eq!(
            resumo(&campos(&[("fonte", "vera_mo_bd.t")])),
            "fonte vera_mo_bd.t",
            "12 caracteres ficam"
        );
    }
}
