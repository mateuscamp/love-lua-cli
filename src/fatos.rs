//! O que o jogo escreve enquanto roda, para o painel mostrar.
//!
//! Os quatro jogos escrevem fatos de uma linha, `<JOGO> <TIPO> chave=valor ...` (o JOGO é o nome em
//! maiúsculas), com o stdout em modo de linha (`io.stdout:setvbuf("line")` no topo do `conf.lua` de
//! cada um), então cada fato chega ao vivo pelo cano. O formato é dos jogos; aqui só se lê. O que
//! não é fato (um traceback, um aviso do LÖVE) vira o tipo `texto` ou, do stderr, `erro`.

use std::time::Duration;

/// De qual cano a linha veio.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Origem {
    Saida,
    Erro,
}

/// Uma linha do jogo, lida.
#[derive(Debug, PartialEq)]
pub enum Linha {
    Fato {
        tipo: String,
        campos: Vec<(String, String)>,
    },
    Texto(String),
}

/// O tipo das linhas soltas do stdout e do stderr; em minúsculas, para não colidir com os tipos
/// dos jogos, que são em maiúsculas.
pub const TEXTO: &str = "texto";
pub const ERRO: &str = "erro";

/// Lê uma linha do jogo: é fato se começa pelo `prefixo` (o nome do jogo em maiúsculas) seguido de
/// um tipo em maiúsculas; o resto é texto solto. Palavra sem `=` vira campo sem chave.
pub fn le_linha(prefixo: &str, linha: &str) -> Linha {
    let linha = linha.trim_end_matches(['\n', '\r']);
    let mut palavras = linha.split_whitespace();
    let (Some(jogo), Some(tipo)) = (palavras.next(), palavras.next()) else {
        return Linha::Texto(linha.to_string());
    };
    if jogo != prefixo || !eh_tipo(tipo) {
        return Linha::Texto(linha.to_string());
    }
    let campos = palavras
        .map(|palavra| match palavra.split_once('=') {
            Some((chave, valor)) => (chave.to_string(), valor.to_string()),
            None => (String::new(), palavra.to_string()),
        })
        .collect();
    Linha::Fato {
        tipo: tipo.to_string(),
        campos,
    }
}

/// `MAPA`, `FIM_ANDAR`: maiúscula no começo, depois maiúsculas, dígitos e `_`.
fn eh_tipo(palavra: &str) -> bool {
    palavra.starts_with(|letra: char| letra.is_ascii_uppercase())
        && palavra
            .chars()
            .all(|letra| letra.is_ascii_uppercase() || letra.is_ascii_digit() || letra == '_')
}

/// Um tipo de fato da partida: quantas vezes veio, quando veio por último e com que campos.
#[derive(Debug, Clone, PartialEq)]
pub struct Tipo {
    pub nome: String,
    pub vezes: u64,
    /// Desde que o jogo subiu.
    pub ultimo_em: Duration,
    pub campos: Vec<(String, String)>,
}

/// O placar da partida: um `Tipo` por tipo de fato, na ordem em que cada um apareceu, para as
/// linhas do painel não trocarem de lugar.
pub struct Placar {
    prefixo: String,
    tipos: Vec<Tipo>,
    /// Do `CONFIG`: chave, valor e origem, a última de cada chave.
    config: Vec<(String, String, String)>,
    total: u64,
    /// Fatos por segundo desde que o jogo subiu, para a faísca do título.
    por_segundo: Vec<u32>,
    primeiro_erro: Option<String>,
}

impl Placar {
    /// O placar vazio do jogo chamado `nome` (o prefixo dos fatos é ele em maiúsculas).
    pub fn novo(nome: &str) -> Placar {
        Placar {
            prefixo: nome.to_uppercase(),
            tipos: Vec::new(),
            config: Vec::new(),
            total: 0,
            por_segundo: Vec::new(),
            primeiro_erro: None,
        }
    }

    /// Anota uma linha que chegou `quando` (desde que o jogo subiu). Linha em branco não conta.
    pub fn anota(&mut self, origem: Origem, linha: &str, quando: Duration) {
        if linha.trim().is_empty() {
            return;
        }
        if origem == Origem::Erro && self.primeiro_erro.is_none() {
            self.primeiro_erro = Some(linha.trim_end().to_string());
        }
        match le_linha(&self.prefixo, linha) {
            Linha::Fato { tipo, campos } => {
                self.conta_fato(quando);
                if tipo == "CONFIG" {
                    self.anota_config(&campos);
                }
                self.atualiza(&tipo, campos, quando);
            }
            Linha::Texto(texto) => {
                let tipo = if origem == Origem::Erro { ERRO } else { TEXTO };
                self.atualiza(tipo, vec![(String::new(), texto)], quando);
            }
        }
    }

    fn conta_fato(&mut self, quando: Duration) {
        self.total += 1;
        let segundo = quando.as_secs() as usize;
        if self.por_segundo.len() <= segundo {
            self.por_segundo.resize(segundo + 1, 0);
        }
        self.por_segundo[segundo] += 1;
    }

    /// O `CONFIG` dos quatro jogos diz `chave=`, `valor=` e `origem=`; fica o último de cada chave.
    fn anota_config(&mut self, campos: &[(String, String)]) {
        let campo = |nome: &str| {
            campos
                .iter()
                .find(|(chave, _)| chave == nome)
                .map(|(_, valor)| valor.clone())
        };
        let (Some(chave), Some(valor), Some(origem)) =
            (campo("chave"), campo("valor"), campo("origem"))
        else {
            return;
        };
        match self.config.iter_mut().find(|(ja, _, _)| *ja == chave) {
            Some(entrada) => (entrada.1, entrada.2) = (valor, origem),
            None => self.config.push((chave, valor, origem)),
        }
    }

    fn atualiza(&mut self, nome: &str, campos: Vec<(String, String)>, quando: Duration) {
        match self.tipos.iter_mut().find(|tipo| tipo.nome == nome) {
            Some(tipo) => {
                tipo.vezes += 1;
                tipo.ultimo_em = quando;
                tipo.campos = campos;
            }
            None => self.tipos.push(Tipo {
                nome: nome.to_string(),
                vezes: 1,
                ultimo_em: quando,
                campos,
            }),
        }
    }

    pub fn tipos(&self) -> &[Tipo] {
        &self.tipos
    }

    /// Quantos fatos vieram (as linhas soltas ficam de fora).
    pub fn total(&self) -> u64 {
        self.total
    }

    /// A primeira linha do stderr: num jogo que morre no boot, é ela que diz por quê.
    pub fn primeiro_erro(&self) -> Option<&str> {
        self.primeiro_erro.as_deref()
    }

    /// As chaves do `CONFIG` cuja origem não é `padrao`, com o valor, na ordem em que vieram.
    pub fn fora_do_padrao(&self) -> Vec<(&str, &str)> {
        self.config
            .iter()
            .filter(|(_, _, origem)| origem != "padrao")
            .map(|(chave, valor, _)| (chave.as_str(), valor.as_str()))
            .collect()
    }

    /// Fatos por segundo nas últimas `janelas` até o segundo de `agora`, o mais antigo primeiro;
    /// antes de o jogo subir, zero.
    pub fn ritmo(&self, agora: Duration, janelas: usize) -> Vec<u32> {
        let fim = agora.as_secs() as i64 + 1;
        (fim - janelas as i64..fim)
            .map(|segundo| {
                usize::try_from(segundo)
                    .ok()
                    .and_then(|segundo| self.por_segundo.get(segundo).copied())
                    .unwrap_or(0)
            })
            .collect()
    }
}

#[cfg(test)]
mod testes {
    use super::*;

    fn campos(pares: &[(&str, &str)]) -> Vec<(String, String)> {
        pares
            .iter()
            .map(|(chave, valor)| (chave.to_string(), valor.to_string()))
            .collect()
    }

    fn em(segundos: f64) -> Duration {
        Duration::from_secs_f64(segundos)
    }

    #[test]
    fn le_linha_separa_fato_do_jogo_de_texto_solto() {
        assert_eq!(
            le_linha(
                "CAMPCRAWLER",
                "CAMPCRAWLER MAPA numero=1 linhas=9 esperado=9"
            ),
            Linha::Fato {
                tipo: "MAPA".to_string(),
                campos: campos(&[("numero", "1"), ("linhas", "9"), ("esperado", "9")]),
            }
        );
        assert_eq!(
            le_linha("CAMPCRAWLER", "CAMPCRAWLER FIM_ANDAR solto estado=won\r"),
            Linha::Fato {
                tipo: "FIM_ANDAR".to_string(),
                campos: campos(&[("", "solto"), ("estado", "won")]),
            }
        );
        for texto in [
            "BOMBERBOOM INICIO motor=love",
            "Error: Could not initialize SDL video subsystem (x11 not available)",
            "CAMPCRAWLER",
            "CAMPCRAWLER minusculo a=1",
            "\t[C]: in function 'xpcall'",
        ] {
            assert_eq!(
                le_linha("CAMPCRAWLER", texto),
                Linha::Texto(texto.to_string()),
                "{texto:?}"
            );
        }
    }

    #[test]
    fn placar_conta_cada_tipo_na_ordem_em_que_apareceu() {
        let mut placar = Placar::novo("CampCrawler");
        placar.anota(Origem::Saida, "CAMPCRAWLER PARTIDA semente=1", em(0.1));
        placar.anota(Origem::Saida, "CAMPCRAWLER VEZ soldado=1 texto=1º", em(0.5));
        placar.anota(Origem::Saida, "", em(0.6));
        placar.anota(Origem::Saida, "aviso solto do LÖVE", em(0.7));
        placar.anota(Origem::Saida, "CAMPCRAWLER VEZ soldado=2 texto=2º", em(1.2));
        placar.anota(Origem::Erro, "Error: boot.lua:479", em(3.9));
        placar.anota(Origem::Erro, "stack traceback:", em(3.9));

        let tipos: Vec<(&str, u64)> = placar
            .tipos()
            .iter()
            .map(|tipo| (tipo.nome.as_str(), tipo.vezes))
            .collect();
        assert_eq!(tipos, [("PARTIDA", 1), ("VEZ", 2), (TEXTO, 1), (ERRO, 2)]);
        let vez = &placar.tipos()[1];
        assert_eq!(vez.campos, campos(&[("soldado", "2"), ("texto", "2º")]));
        assert_eq!(vez.ultimo_em, em(1.2));
        assert_eq!(
            placar.tipos()[3].campos,
            campos(&[("", "stack traceback:")])
        );
        assert_eq!(placar.total(), 3, "só os fatos");
        assert_eq!(placar.primeiro_erro(), Some("Error: boot.lua:479"));
    }

    #[test]
    fn placar_guarda_a_config_fora_do_padrao() {
        let mut placar = Placar::novo("BomberBoom");
        let linhas = [
            "BOMBERBOOM CONFIG chave=seed valor=17 origem=plataforma",
            "BOMBERBOOM CONFIG chave=autoplay valor=0 origem=padrao",
            "BOMBERBOOM CONFIG chave=autoplay valor=1 origem=argumento",
            "BOMBERBOOM CONFIG chave=mode valor=- origem=padrao",
        ];
        for linha in linhas {
            placar.anota(Origem::Saida, linha, em(0.0));
        }
        assert_eq!(placar.fora_do_padrao(), [("seed", "17"), ("autoplay", "1")]);
        assert_eq!(placar.tipos()[0].vezes, 4);
    }

    #[test]
    fn ritmo_conta_os_fatos_por_segundo_ate_agora() {
        let mut placar = Placar::novo("Boomlitude");
        for segundos in [0.1, 0.5, 1.2, 3.9] {
            placar.anota(Origem::Saida, "BOOMLITUDE LANCE aceito=sim", em(segundos));
        }
        placar.anota(Origem::Saida, "texto solto não conta", em(2.5));
        assert_eq!(placar.ritmo(em(4.0), 5), [2, 1, 0, 1, 0]);
        assert_eq!(placar.ritmo(em(4.0), 3), [0, 1, 0]);
        assert_eq!(placar.ritmo(em(1.5), 4), [0, 0, 2, 1]);
    }
}
