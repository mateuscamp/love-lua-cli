//! O lançador comum é o mesmo em todo jogo (autor, 31/10; a regra mora no README). O painel lê a
//! cópia de cada jogo e marca a que destoa das outras: só avisa, não impede, como a marca da obra.
//!
//! Em 31/10 o `tools/love` era igual nos quatro jogos, mas o `tools/lancador/sistema.lua` do
//! BomberBoom já tinha uma correção (o `mktemp` no lugar do `os.tmpname`) que os outros não.

use std::path::Path;

/// Os arquivos do lançador comum numa pasta de jogo, pelo caminho relativo, em ordem: o
/// `tools/love` e as peças de `tools/lancador/` (um nível só, como nos jogos de 31/10). O
/// `tools/love_projeto.lua` é de cada projeto e fica de fora. Arquivo que não se lê também fica de
/// fora, e a cópia sem ele destoa das que o têm.
pub type Copia = Vec<(String, Vec<u8>)>;

/// Lê a cópia do lançador comum que mora em `pasta`.
pub fn le_copia(pasta: &Path) -> Copia {
    let mut copia: Copia = Vec::new();
    le_arquivo(pasta, "tools/love", &mut copia);
    let Ok(pecas) = std::fs::read_dir(pasta.join("tools/lancador")) else {
        return copia;
    };
    let mut nomes: Vec<String> = pecas
        .filter_map(|peca| peca.ok()?.file_name().into_string().ok())
        .collect();
    nomes.sort();
    for nome in nomes {
        le_arquivo(pasta, &format!("tools/lancador/{nome}"), &mut copia);
    }
    copia
}

/// Põe em `copia` o conteúdo de `pasta/relativo`, se ele se lê (uma subpasta não se lê).
fn le_arquivo(pasta: &Path, relativo: &str, copia: &mut Copia) {
    if let Ok(conteudo) = std::fs::read(pasta.join(relativo)) {
        copia.push((relativo.to_string(), conteudo));
    }
}

/// Quais cópias destoam: as diferentes da cópia da maioria. Sem maioria (empate no topo), destoa
/// toda cópia, porque cada uma difere de alguma outra; com todas iguais, ou uma só, nenhuma.
pub fn destoam(copias: &[Copia]) -> Vec<bool> {
    let iguais: Vec<usize> = copias
        .iter()
        .map(|copia| copias.iter().filter(|outra| *outra == copia).count())
        .collect();
    let maior = iguais.iter().copied().max().unwrap_or(0);
    // Um grupo só no topo: as `maior` cópias com a contagem máxima são todas a mesma.
    let tem_maioria = iguais.iter().filter(|&&vezes| vezes == maior).count() == maior;
    iguais
        .iter()
        .map(|&vezes| !tem_maioria || vezes != maior)
        .collect()
}

#[cfg(test)]
mod testes {
    use super::*;
    use crate::test_support::{jogo_falso, pasta_de_teste};

    /// Uma cópia só com o `tools/love`, de `conteudo`.
    fn copia(conteudo: &str) -> Copia {
        vec![("tools/love".to_string(), conteudo.as_bytes().to_vec())]
    }

    #[test]
    fn destoa_a_copia_fora_da_maioria() {
        let (a, b) = (copia("a"), copia("b"));
        assert_eq!(destoam(&[]), Vec::<bool>::new());
        assert_eq!(
            destoam(std::slice::from_ref(&a)),
            [false],
            "uma só não destoa"
        );
        assert_eq!(destoam(&[a.clone(), a.clone(), a.clone()]), [false; 3]);
        assert_eq!(
            destoam(&[a.clone(), b.clone(), a.clone()]),
            [false, true, false]
        );
        assert_eq!(
            destoam(&[a.clone(), b.clone(), b.clone(), a.clone()]),
            [true; 4],
            "dois contra dois: sem maioria, todas"
        );
        assert_eq!(destoam(&[a, b]), [true; 2]);
    }

    /// A cópia é o `tools/love` e as peças de `tools/lancador/`; o `tools/love_projeto.lua`, que é
    /// de cada projeto, não conta.
    #[test]
    fn le_copia_pega_o_lancador_e_as_pecas_sem_o_do_projeto() {
        let raiz = pasta_de_teste("le-copia");
        for (pasta, projeto, sistema) in [
            ("um", "nome = 'um'", "local M = {}"),
            ("dois", "nome = 'dois'", "local M = {}"),
            ("tres", "nome = 'tres'", "local M = { tmp = 'mktemp' }"),
        ] {
            jogo_falso(&raiz, pasta, "exec love .\n");
            let ferramentas = raiz.join(pasta).join("tools");
            std::fs::write(ferramentas.join("love_projeto.lua"), projeto).unwrap();
            std::fs::create_dir_all(ferramentas.join("lancador")).unwrap();
            std::fs::write(ferramentas.join("lancador/sistema.lua"), sistema).unwrap();
            std::fs::write(ferramentas.join("lancador/troca.lua"), "-- troca").unwrap();
        }
        let copias: Vec<Copia> = ["um", "dois", "tres"]
            .iter()
            .map(|pasta| le_copia(&raiz.join(pasta)))
            .collect();
        let caminhos: Vec<&str> = copias[0].iter().map(|(nome, _)| nome.as_str()).collect();
        assert_eq!(
            caminhos,
            [
                "tools/love",
                "tools/lancador/sistema.lua",
                "tools/lancador/troca.lua"
            ]
        );
        assert_eq!(copias[0], copias[1], "o love_projeto.lua não conta");
        assert_ne!(copias[0], copias[2], "a peça diferente conta");
        std::fs::remove_dir_all(&raiz).unwrap();
    }
}
